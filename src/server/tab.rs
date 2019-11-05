use crate::clipboard::SystemClipboard;
use crate::frontend::executor;
use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::server::client::Client;
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use failure::{bail, Fallible};
use filedescriptor::Pipe;
use log::error;
use portable_pty::PtySize;
use promise::{BrokenPromise, Future};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use term::color::ColorPalette;
use term::selection::SelectionRange;
use term::{Clipboard, CursorPosition, Line};
use term::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TerminalHost};
use termwiz::hyperlink::Hyperlink;
use termwiz::input::KeyEvent;
use termwiz::surface::{Change, SequenceNo, Surface};

struct MouseState {
    future: Option<Future<()>>,
    queue: VecDeque<MouseEvent>,
    selection_range: Arc<Mutex<Option<SelectionRange>>>,
    something_changed: Arc<AtomicBool>,
    highlight: Arc<Mutex<Option<Arc<Hyperlink>>>>,
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

    fn pop(&mut self) -> Fallible<Option<MouseEvent>> {
        if self.can_send()? {
            Ok(self.queue.pop_front())
        } else {
            Ok(None)
        }
    }

    fn can_send(&mut self) -> Fallible<bool> {
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

    fn next(state: &Arc<Mutex<Self>>) -> Fallible<()> {
        let mut mouse = state.lock().unwrap();
        if let Some(event) = mouse.pop()? {
            let selection_range = Arc::clone(&mouse.selection_range);
            let something_changed = Arc::clone(&mouse.something_changed);
            let highlight = Arc::clone(&mouse.highlight);
            let state = Arc::clone(state);

            mouse.future = Some(
                mouse
                    .client
                    .mouse_event(SendMouseEvent {
                        tab_id: mouse.remote_tab_id,
                        event,
                    })
                    .then(move |resp| {
                        if let Ok(r) = resp.as_ref() {
                            *selection_range.lock().unwrap() = r.selection_range;
                            *highlight.lock().unwrap() = r.highlight.clone();
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
    clipboard: Arc<dyn Clipboard>,
}

impl ClientTab {
    pub fn new(
        client: &Arc<ClientInner>,
        remote_tab_id: TabId,
        size: PtySize,
        palette: ColorPalette,
    ) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };
        let selection_range = Arc::new(Mutex::new(None));
        let something_changed = Arc::new(AtomicBool::new(true));
        let highlight = Arc::new(Mutex::new(None));

        let mouse = Arc::new(Mutex::new(MouseState {
            selection_range: Arc::clone(&selection_range),
            something_changed: Arc::clone(&something_changed),
            highlight: Arc::clone(&highlight),
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
                selection_range,
                something_changed,
                highlight,
                palette,
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
            // FIXME: ideally we'd pass down an instance of Clipboard
            // rather than creating a new SystemClipboard here.
            // That will be important if we end up with multiple chained
            // domains in the future.
            clipboard: Arc::new(SystemClipboard::new()),
            reader,
        }
    }

    pub fn process_unilateral(&self, pdu: Pdu) -> Fallible<()> {
        match pdu {
            Pdu::GetTabRenderChangesResponse(delta) => {
                log::trace!("new delta {}", delta.sequence_no);
                self.renderable
                    .borrow()
                    .inner
                    .borrow_mut()
                    .apply_changes_to_surface(delta.sequence_no, delta.changes);
            }
            Pdu::SetClipboard(SetClipboard { clipboard, .. }) => {
                self.clipboard.set_contents(clipboard)?;
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

    fn get_title(&self) -> String {
        let renderable = self.renderable.borrow();
        let surface = &renderable.inner.borrow().surface;
        surface.title().to_string()
    }

    fn send_paste(&self, text: &str) -> Fallible<()> {
        self.client.client.send_paste(SendPaste {
            tab_id: self.remote_tab_id,
            data: text.to_owned(),
        });
        Ok(())
    }

    fn reader(&self) -> Fallible<Box<dyn std::io::Read + Send>> {
        error!("made reader for ClientTab");
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> Fallible<()> {
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

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Fallible<()> {
        self.client.client.key_down(SendKeyDown {
            tab_id: self.remote_tab_id,
            event: KeyEvent {
                key,
                modifiers: mods,
            },
        });
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, _host: &mut dyn TerminalHost) -> Fallible<()> {
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
        self.renderable.borrow().inner.borrow().palette.clone()
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        *self
            .renderable
            .borrow()
            .inner
            .borrow()
            .selection_range
            .lock()
            .unwrap()
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
    selection_range: Arc<Mutex<Option<SelectionRange>>>,
    something_changed: Arc<AtomicBool>,
    highlight: Arc<Mutex<Option<Arc<Hyperlink>>>>,
    palette: ColorPalette,
}

struct RenderableState {
    inner: RefCell<RenderableInner>,
}

const MAX_POLL_INTERVAL: Duration = Duration::from_secs(30);
const BASE_POLL_INTERVAL: Duration = Duration::from_millis(20);

impl RenderableInner {
    fn apply_changes_to_surface(&mut self, remote_seq: SequenceNo, changes: Vec<Change>) {
        if let Some(first) = changes.first().as_ref() {
            log::trace!("{:?}", first);
        }
        self.poll_interval = BASE_POLL_INTERVAL;
        self.surface.add_changes(changes);
        self.remote_sequence = remote_seq;
    }

    fn poll(&mut self) -> Fallible<()> {
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
    fn get_cursor_position(&self) -> CursorPosition {
        let (x, y) = self.inner.borrow().surface.cursor_position();
        CursorPosition { x, y: y as i64 }
    }

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

    fn make_all_lines_dirty(&mut self) {
        self.inner
            .borrow()
            .something_changed
            .store(true, Ordering::SeqCst);
    }

    fn clean_dirty_lines(&mut self) {}

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        self.inner
            .borrow()
            .highlight
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        let (cols, rows) = self.inner.borrow().surface.dimensions();
        (rows, cols)
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
