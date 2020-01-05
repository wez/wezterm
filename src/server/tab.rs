use crate::config::configuration;
use crate::frontend::executor;
use crate::mux::domain::DomainId;
use crate::mux::renderable::{Renderable, RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::server::client::Client;
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use anyhow::bail;
use filedescriptor::Pipe;
use log::{error, info};
use portable_pty::PtySize;
use promise::{BrokenPromise, Future};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use term::color::ColorPalette;
use term::{
    Clipboard, KeyCode, KeyModifiers, Line, MouseButton, MouseEvent, MouseEventKind,
    StableRowIndex, TerminalHost,
};
use termwiz::hyperlink::Hyperlink;
use termwiz::input::KeyEvent;
use termwiz::surface::{SequenceNo, Surface};

struct MouseState {
    future: Option<Future<()>>,
    queue: VecDeque<MouseEvent>,
    something_changed: Arc<AtomicBool>,
    client: Client,
    remote_tab_id: TabId,
}

impl MouseState {
    fn append(&mut self, event: MouseEvent) {
        if let Some(last) = self.queue.back_mut() {
            if last.modifiers == event.modifiers {
                if last.kind == MouseEventKind::Move
                    && event.kind == MouseEventKind::Move
                    && last.button == event.button
                {
                    // Collapse any interim moves and just buffer up
                    // the last of them
                    *last = event;
                    return;
                }

                // Similarly, for repeated wheel scrolls, add up the deltas
                // rather than swamping the queue
                match (&last.button, &event.button) {
                    (MouseButton::WheelUp(a), MouseButton::WheelUp(b)) => {
                        last.button = MouseButton::WheelUp(a + b);
                        return;
                    }
                    (MouseButton::WheelDown(a), MouseButton::WheelDown(b)) => {
                        last.button = MouseButton::WheelDown(a + b);
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.queue.push_back(event);
        log::trace!("MouseEvent {}: queued", self.queue.len());
    }

    fn pop(&mut self) -> anyhow::Result<Option<MouseEvent>> {
        if self.can_send()? {
            Ok(self.queue.pop_front())
        } else {
            Ok(None)
        }
    }

    fn can_send(&mut self) -> anyhow::Result<bool> {
        if self.future.is_none() {
            Ok(true)
        } else {
            let ready = self.future.as_ref().map(Future::is_ready).unwrap_or(false);
            if ready {
                self.future.take().unwrap().wait()?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    fn next(state: &Arc<Mutex<Self>>) -> anyhow::Result<()> {
        let mut mouse = state.lock().unwrap();
        if let Some(event) = mouse.pop()? {
            let something_changed = Arc::clone(&mouse.something_changed);
            let state = Arc::clone(state);

            mouse.future = Some(
                mouse
                    .client
                    .mouse_event(SendMouseEvent {
                        tab_id: mouse.remote_tab_id,
                        event,
                    })
                    .then(move |resp| {
                        if let Ok(_) = resp.as_ref() {
                            something_changed.store(true, Ordering::SeqCst);
                        }
                        Future::with_executor(executor(), move || {
                            Self::next(&state)?;
                            Ok(())
                        });
                        Ok(())
                    }),
            );
        }
        Ok(())
    }
}

pub struct ClientTab {
    client: Arc<ClientInner>,
    local_tab_id: TabId,
    remote_tab_id: TabId,
    renderable: RefCell<RenderableState>,
    writer: RefCell<TabWriter>,
    reader: Pipe,
    mouse: Arc<Mutex<MouseState>>,
    clipboard: RefCell<Option<Arc<dyn Clipboard>>>,
    mouse_grabbed: RefCell<bool>,
}

impl ClientTab {
    pub fn new(client: &Arc<ClientInner>, remote_tab_id: TabId, size: PtySize) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };
        let something_changed = Arc::new(AtomicBool::new(true));
        let highlight = Arc::new(Mutex::new(None));

        let mouse = Arc::new(Mutex::new(MouseState {
            something_changed: Arc::clone(&something_changed),
            remote_tab_id,
            client: client.client.clone(),
            future: None,
            queue: VecDeque::new(),
        }));
        let render = RenderableState {
            inner: RefCell::new(RenderableInner {
                client: Arc::clone(client),
                remote_tab_id,
                last_poll: Instant::now(),
                dead: false,
                poll_future: None,
                poll_interval: BASE_POLL_INTERVAL,
                surface: Surface::new(size.cols as usize, size.rows as usize),
                remote_sequence: 0,
                local_sequence: 0,
                something_changed,
                highlight,
                cursor_position: StableCursorPosition::default(),
                dirty_rows: RangeSet::new(),
                dimensions: RenderableDimensions::default(),
            }),
        };

        let reader = Pipe::new().expect("Pipe::new failed");

        Self {
            client: Arc::clone(client),
            mouse,
            remote_tab_id,
            local_tab_id,
            renderable: RefCell::new(render),
            writer: RefCell::new(writer),
            reader,
            clipboard: RefCell::new(None),
            mouse_grabbed: RefCell::new(false),
        }
    }

    pub fn process_unilateral(&self, pdu: Pdu) -> anyhow::Result<()> {
        match pdu {
            Pdu::GetTabRenderChangesResponse(delta) => {
                log::trace!("new delta {}", delta.sequence_no);
                *self.mouse_grabbed.borrow_mut() = delta.mouse_grabbed;
                self.renderable
                    .borrow()
                    .inner
                    .borrow_mut()
                    .apply_changes_to_surface(delta);
            }
            Pdu::SetClipboard(SetClipboard { clipboard, .. }) => {
                match self.clipboard.borrow().as_ref() {
                    Some(clip) => {
                        clip.set_contents(clipboard)?;
                    }
                    None => {
                        log::error!("ClientTab: Ignoring SetClipboard request {:?}", clipboard);
                    }
                }
            }
            Pdu::OpenURL(OpenURL { url, .. }) => {
                // FIXME: ideally we'd have a provider that we can
                // capture (like the clipboard) so that we can propagate
                // the click back to the ultimate client, but for now
                // we just do a single stage
                match open::that(&url) {
                    Ok(_) => {}
                    Err(err) => error!("failed to open {}: {:?}", url, err),
                }
            }
            _ => bail!("unhandled unilateral pdu: {:?}", pdu),
        };
        Ok(())
    }

    pub fn remote_tab_id(&self) -> TabId {
        self.remote_tab_id
    }
}

impl Tab for ClientTab {
    fn tab_id(&self) -> TabId {
        self.local_tab_id
    }
    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.renderable.borrow_mut()
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.borrow_mut().replace(Arc::clone(clipboard));
    }

    fn get_title(&self) -> String {
        let renderable = self.renderable.borrow();
        let surface = &renderable.inner.borrow().surface;
        surface.title().to_string()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        self.client.client.send_paste(SendPaste {
            tab_id: self.remote_tab_id,
            data: text.to_owned(),
        });
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        info!("made reader for ClientTab");
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.renderable
            .borrow()
            .inner
            .borrow_mut()
            .surface
            .resize(size.cols as usize, size.rows as usize);
        self.client.client.resize(Resize {
            tab_id: self.remote_tab_id,
            size,
        });
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        self.client.client.key_down(SendKeyDown {
            tab_id: self.remote_tab_id,
            event: KeyEvent {
                key,
                modifiers: mods,
            },
        });
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, _host: &mut dyn TerminalHost) -> anyhow::Result<()> {
        self.mouse.lock().unwrap().append(event);
        MouseState::next(&self.mouse)?;
        Ok(())
    }

    fn advance_bytes(&self, _buf: &[u8], _host: &mut dyn TerminalHost) {
        panic!("ClientTab::advance_bytes not impl");
    }

    fn is_dead(&self) -> bool {
        self.renderable.borrow().inner.borrow().dead
    }

    fn palette(&self) -> ColorPalette {
        configuration()
            .colors
            .as_ref()
            .cloned()
            .map(Into::into)
            .unwrap_or_else(ColorPalette::default)
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        *self.mouse_grabbed.borrow()
    }
}

struct RenderableInner {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
    last_poll: Instant,
    dead: bool,
    poll_future: Option<Future<UnitResponse>>,
    surface: Surface,
    remote_sequence: SequenceNo,
    local_sequence: SequenceNo,
    poll_interval: Duration,
    something_changed: Arc<AtomicBool>,
    highlight: Arc<Mutex<Option<Arc<Hyperlink>>>>,

    dirty_rows: RangeSet<StableRowIndex>,
    cursor_position: StableCursorPosition,
    dimensions: RenderableDimensions,
}

struct RenderableState {
    inner: RefCell<RenderableInner>,
}

const MAX_POLL_INTERVAL: Duration = Duration::from_secs(30);
const BASE_POLL_INTERVAL: Duration = Duration::from_millis(20);

impl RenderableInner {
    fn apply_changes_to_surface(&mut self, delta: GetTabRenderChangesResponse) {
        if let Some(first) = delta.changes.first().as_ref() {
            log::trace!("{:?}", first);
        }
        self.poll_interval = BASE_POLL_INTERVAL;
        self.surface.add_changes(delta.changes);
        self.remote_sequence = delta.sequence_no;

        for r in delta.dirty_lines {
            self.dirty_rows.add_range(r.start..r.end);
        }

        self.cursor_position = delta.cursor_position;
        self.dimensions = delta.dimensions;
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let ready = self
            .poll_future
            .as_ref()
            .map(Future::is_ready)
            .unwrap_or(false);
        if ready {
            self.poll_future.take().unwrap().wait()?;
            let interval = self.poll_interval;
            let interval = (interval + interval).min(MAX_POLL_INTERVAL);
            self.poll_interval = interval;

            self.last_poll = Instant::now();
        } else if self.poll_future.is_some() {
            // We have a poll in progress
            return Ok(());
        }

        let last = self.last_poll;
        if last.elapsed() < self.poll_interval {
            return Ok(());
        }

        {
            self.last_poll = Instant::now();
            self.poll_future = Some(self.client.client.get_tab_render_changes(
                GetTabRenderChanges {
                    tab_id: self.remote_tab_id,
                    sequence_no: self.remote_sequence,
                },
            ));
        }
        Ok(())
    }
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> StableCursorPosition {
        let surface = &self.inner.borrow().surface;
        let (x, y) = surface.cursor_position();
        let shape = surface.cursor_shape();
        StableCursorPosition {
            x,
            y: y as StableRowIndex,
            shape,
        }
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        // FIXME: mark clean
        let inner = self.inner.borrow_mut();
        (
            lines.start,
            inner
                .surface
                .screen_lines()
                .into_iter()
                .skip(lines.start.try_into().unwrap())
                .take((lines.end - lines.start).try_into().unwrap())
                .map(|line| line.into_owned())
                .collect(),
        )
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> Vec<StableRowIndex> {
        let mut inner = self.inner.borrow_mut();
        if let Err(err) = inner.poll() {
            // We allow for BrokenPromise here for now; for a TLS backed
            // session it indicates that we'll retry.  For a local unix
            // domain session it is terminal... but we will detect that
            // terminal condition elsewhere
            if let Err(err) = err.downcast::<BrokenPromise>() {
                log::error!("remote tab poll failed: {}, marking as dead", err);
                inner.dead = true;
            }
        }

        let mut result = vec![];
        for r in inner.dirty_rows.intersection_with_range(lines).iter() {
            for line in r.clone() {
                result.push(line);
            }
        }

        result
    }

    /*
    fn get_dirty_lines(&self) -> Vec<(usize, Cow<Line>, Range<usize>)> {
        let mut inner = self.inner.borrow_mut();
        let seq = inner.surface.current_seqno();
        inner.surface.flush_changes_older_than(seq);
        let selection = *inner.selection_range.lock().unwrap();
        inner.something_changed.store(false, Ordering::SeqCst);
        inner.local_sequence = seq;
        inner
            .surface
            .screen_lines()
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let r = match selection {
                    None => 0..0,
                    Some(sel) => sel.normalize().cols_for_row(idx as i32),
                };
                (idx, Cow::Owned(line.into_owned()), r)
            })
            .collect()
    }
    */

    /*
    fn has_dirty_lines(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if let Err(err) = inner.poll() {
            // We allow for BrokenPromise here for now; for a TLS backed
            // session it indicates that we'll retry.  For a local unix
            // domain session it is terminal... but we will detect that
            // terminal condition elsewhere
            if let Err(err) = err.downcast::<BrokenPromise>() {
                log::error!("remote tab poll failed: {}, marking as dead", err);
                inner.dead = true;
            }
        }
        if inner.something_changed.load(Ordering::SeqCst) {
            return true;
        }
        inner.surface.has_changes(inner.local_sequence)
    }
    */

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        self.inner
            .borrow()
            .highlight
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        let (cols, viewport_rows) = self.inner.borrow().surface.dimensions();
        RenderableDimensions {
            viewport_rows,
            cols,
            scrollback_rows: viewport_rows,
            physical_top: 0,
            scrollback_top: 0,
        }
    }
}

struct TabWriter {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
}

impl std::io::Write for TabWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        self.client
            .client
            .write_to_tab(WriteToTab {
                tab_id: self.remote_tab_id,
                data: data.to_vec(),
            })
            .wait()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
