use crate::config::{configuration, ConfigHandle};
use crate::frontend::executor;
use crate::mux::domain::DomainId;
use crate::mux::renderable::{Renderable, RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::mux::Mux;
use crate::ratelim::RateLimiter;
use crate::server::client::Client;
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use anyhow::anyhow;
use anyhow::bail;
use filedescriptor::Pipe;
use log::info;
use lru::LruCache;
use portable_pty::PtySize;
use promise::{BrokenPromise, Future};
use rangeset::*;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use term::color::ColorPalette;
use term::{
    Clipboard, KeyCode, KeyModifiers, Line, MouseButton, MouseEvent, MouseEventKind,
    StableRowIndex, TerminalHost,
};
use termwiz::input::KeyEvent;

struct MouseState {
    future: Option<Future<()>>,
    queue: VecDeque<MouseEvent>,
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
            let state = Arc::clone(state);

            mouse.future = Some(
                mouse
                    .client
                    .mouse_event(SendMouseEvent {
                        tab_id: mouse.remote_tab_id,
                        event,
                    })
                    .then(move |_| {
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
    pub fn new(
        client: &Arc<ClientInner>,
        remote_tab_id: TabId,
        size: PtySize,
        title: &str,
    ) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };

        let mouse = Arc::new(Mutex::new(MouseState {
            remote_tab_id,
            client: client.client.clone(),
            future: None,
            queue: VecDeque::new(),
        }));

        let fetch_limiter =
            RateLimiter::new(|config| config.ratelimit_mux_line_prefetches_per_second);

        let render = RenderableState {
            inner: RefCell::new(RenderableInner {
                client: Arc::clone(client),
                remote_tab_id,
                local_tab_id,
                last_poll: Instant::now(),
                dead: false,
                poll_future: None,
                poll_interval: BASE_POLL_INTERVAL,
                cursor_position: StableCursorPosition::default(),
                dirty_rows: RangeSet::new(),
                fetch_pending: RangeSet::new(),
                dimensions: RenderableDimensions {
                    cols: size.cols as _,
                    viewport_rows: size.rows as _,
                    scrollback_rows: size.rows as _,
                    physical_top: 0,
                    scrollback_top: 0,
                },
                lines: LruCache::unbounded(),
                title: title.to_string(),
                fetch_limiter,
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
        let inner = renderable.inner.borrow();
        inner.title.clone()
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
    local_tab_id: TabId,
    last_poll: Instant,
    dead: bool,
    poll_future: Option<Future<UnitResponse>>,
    poll_interval: Duration,

    dirty_rows: RangeSet<StableRowIndex>,
    fetch_pending: RangeSet<StableRowIndex>,
    cursor_position: StableCursorPosition,
    dimensions: RenderableDimensions,

    lines: LruCache<StableRowIndex, Line>,
    title: String,

    fetch_limiter: RateLimiter,
}

struct RenderableState {
    inner: RefCell<RenderableInner>,
}

const MAX_POLL_INTERVAL: Duration = Duration::from_secs(30);
const BASE_POLL_INTERVAL: Duration = Duration::from_millis(20);

impl RenderableInner {
    fn apply_changes_to_surface(&mut self, delta: GetTabRenderChangesResponse) {
        self.poll_interval = BASE_POLL_INTERVAL;

        let mut to_fetch = RangeSet::new();
        for r in delta.dirty_lines {
            self.dirty_rows.add_range(r.clone());
            for idx in r {
                // If a line is in the (probable) viewport region,
                // then we'll likely want to fetch it.
                // If it is outside that region, remove it from our cache
                // so that we'll fetch it on demand later.
                if idx >= delta.dimensions.physical_top {
                    to_fetch.add(idx);
                } else {
                    self.lines.pop(&idx);
                }
            }
        }

        if delta.cursor_position != self.cursor_position {
            self.dirty_rows.add(self.cursor_position.y);
            // But note that the server may have sent this in bonus_lines;
            // we'll address that below
            self.dirty_rows.add(delta.cursor_position.y);
            to_fetch.add(delta.cursor_position.y);
        }
        self.cursor_position = delta.cursor_position;
        self.dimensions = delta.dimensions;
        self.title = delta.title;

        let config = configuration();
        for (stable_row, line) in delta.bonus_lines.lines() {
            to_fetch.remove(stable_row);
            self.put_line(stable_row, line, &config);
        }

        let is_high_priority = false;
        self.fetch_lines(to_fetch, is_high_priority);
    }

    fn put_line(&mut self, stable_row: StableRowIndex, mut line: Line, config: &ConfigHandle) {
        line.scan_and_create_hyperlinks(&config.hyperlink_rules);
        self.fetch_pending.remove(stable_row);
        if let Some(existing) = self.lines.get(&stable_row) {
            if *existing == line {
                return;
            }
        }
        self.dirty_rows.add(stable_row);
        self.lines.put(stable_row, line);
    }

    /// Request a set of lines.
    /// The is_high_priority flag bypasses any throttling checks and should
    /// be used when we definitely require those lines.
    /// Set is_high_priority to false for speculative fetches.
    fn fetch_lines(&mut self, mut to_fetch: RangeSet<StableRowIndex>, is_high_priority: bool) {
        to_fetch.remove_set(&self.fetch_pending);
        if to_fetch.is_empty() {
            return;
        }

        if !is_high_priority && !self.fetch_limiter.non_blocking_admittance_check(1) {
            // Throttled
            return;
        }

        self.fetch_pending.add_set(&to_fetch);

        let local_tab_id = self.local_tab_id;
        log::trace!(
            "will fetch lines {:?} for remote tab id {}, is_high_priority={}",
            to_fetch,
            self.remote_tab_id,
            is_high_priority
        );
        self.client
            .client
            .get_lines(GetLines {
                tab_id: self.remote_tab_id,
                lines: to_fetch.clone().into(),
            })
            .then(move |result| {
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(local_tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", local_tab_id))?;
                    if let Some(client_tab) = tab.downcast_ref::<ClientTab>() {
                        let renderable = client_tab.renderable.borrow_mut();
                        let mut inner = renderable.inner.borrow_mut();
                        inner.fetch_pending.remove_set(&to_fetch);

                        match result {
                            Ok(result) => {
                                let config = configuration();
                                let lines = result.lines.lines();
                                log::trace!("got {} lines", lines.len());

                                for (stable_row, line) in lines.into_iter() {
                                    inner.put_line(stable_row, line, &config);
                                }
                            }
                            Err(err) => {
                                log::error!("get_lines failed: {}", err);
                            }
                        }
                    }
                    Ok(())
                });
                Ok(())
            });
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
                },
            ));
        }
        Ok(())
    }
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> StableCursorPosition {
        self.inner.borrow().cursor_position
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let mut inner = self.inner.borrow_mut();
        let mut result = vec![];
        let mut to_fetch = RangeSet::new();
        for idx in lines.clone() {
            match inner.lines.get(&idx) {
                Some(line) => {
                    result.push(line.clone());
                    // mark clean
                    inner.dirty_rows.remove(idx);
                }
                None => {
                    to_fetch.add(idx);
                    result.push(Line::with_width(inner.dimensions.cols));
                }
            }
        }

        let is_high_priority = true;
        inner.fetch_lines(to_fetch, is_high_priority);
        (lines.start, result)
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
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

        let mut result = RangeSet::new();
        for r in inner.dirty_rows.intersection_with_range(lines).iter() {
            for line in r.clone() {
                result.add(line);
            }
        }

        if !result.is_empty() {
            log::trace!("get_dirty_lines: {:?}", result);
        }

        result
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        self.inner.borrow().dimensions
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
