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
        let render = self.renderable.borrow();
        let mut inner = render.inner.borrow_mut();
        // Invalidate any cached rows on a resize
        inner.lines.clear();
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

#[derive(Debug)]
enum LineEntry {
    // Up to date wrt. server and has been rendered at least once
    Line(Line),
    // Up to date wrt. server but needs to be rendered
    Dirty(Line),
    // Currently being downloaded from the server
    Fetching(Instant),
    // We have a version of the line locally and are treating it
    // as needing rendering because we are also in the process of
    // downloading a newer version from the server
    DirtyAndFetching(Line, Instant),
}

impl LineEntry {
    fn kind(&self) -> &'static str {
        match self {
            Self::Line(_) => "Line",
            Self::Dirty(_) => "Dirty",
            Self::Fetching(_) => "Fetching",
            Self::DirtyAndFetching(..) => "DirtyAndFetching",
        }
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

    cursor_position: StableCursorPosition,
    dimensions: RenderableDimensions,

    lines: LruCache<StableRowIndex, LineEntry>,
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

        let mut dirty = RangeSet::new();
        for r in delta.dirty_lines {
            dirty.add_range(r.clone());
        }
        if delta.cursor_position != self.cursor_position {
            dirty.add(self.cursor_position.y);
            // But note that the server may have sent this in bonus_lines;
            // we'll address that below
            dirty.add(delta.cursor_position.y);
        }

        self.cursor_position = delta.cursor_position;
        self.dimensions = delta.dimensions;
        self.title = delta.title;

        let config = configuration();
        for (stable_row, line) in delta.bonus_lines.lines() {
            self.put_line(stable_row, line, &config, None);
            dirty.remove(stable_row);
        }

        let now = Instant::now();
        let mut to_fetch = RangeSet::new();
        for r in dirty.iter() {
            for stable_row in r.clone() {
                // If a line is in the (probable) viewport region,
                // then we'll likely want to fetch it.
                // If it is outside that region, remove it from our cache
                // so that we'll fetch it on demand later.
                let fetchable = stable_row >= delta.dimensions.physical_top;
                let prior = self.lines.pop(&stable_row);
                let prior_kind = prior.as_ref().map(|e| e.kind());
                if !fetchable {
                    if prior.is_some() {
                        log::trace!(
                            "row {} {:?} -> unset due to dirty and out of viewport",
                            stable_row,
                            prior_kind,
                        );
                    }
                    continue;
                }
                to_fetch.add(stable_row);
                let entry = match prior {
                    Some(LineEntry::Fetching(_)) | None => LineEntry::Fetching(now),
                    Some(LineEntry::DirtyAndFetching(old, ..))
                    | Some(LineEntry::Dirty(old))
                    | Some(LineEntry::Line(old)) => LineEntry::DirtyAndFetching(old, now),
                };
                log::trace!(
                    "row {} {:?} -> {} due to dirty and IN viewport",
                    stable_row,
                    prior_kind,
                    entry.kind()
                );
                self.lines.put(stable_row, entry);
            }
        }
        if !to_fetch.is_empty() {
            if self.fetch_limiter.non_blocking_admittance_check(1) {
                self.schedule_fetch_lines(to_fetch, now);
            } else {
                log::trace!("exceeded throttle, drop {:?}", to_fetch);
                for r in to_fetch.iter() {
                    for stable_row in r.clone() {
                        self.lines.pop(&stable_row);
                    }
                }
            }
        }
    }

    fn put_line(
        &mut self,
        stable_row: StableRowIndex,
        mut line: Line,
        config: &ConfigHandle,
        fetch_start: Option<Instant>,
    ) {
        line.scan_and_create_hyperlinks(&config.hyperlink_rules);

        let entry = if let Some(fetch_start) = fetch_start {
            // If we're completing a fetch, only replace entries that were
            // set to fetching as part of our fetch.  If they are now longer
            // tagged that way, then someone came along after us and changed
            // the state, so we should leave it alone

            match self.lines.pop(&stable_row) {
                Some(LineEntry::DirtyAndFetching(_, then)) | Some(LineEntry::Fetching(then))
                    if fetch_start == then =>
                {
                    log::trace!("row {} fetch done -> Dirty", stable_row,);
                    LineEntry::Dirty(line)
                }
                e => {
                    // It changed since we started: leave it alone!
                    log::trace!(
                        "row {} {:?} changed since fetch started at {:?}, so leave it be",
                        stable_row,
                        e.map(|e| e.kind()),
                        fetch_start
                    );
                    return;
                }
            }
        } else {
            if let Some(LineEntry::Line(prior)) = self.lines.pop(&stable_row) {
                if prior == line {
                    LineEntry::Line(line)
                } else {
                    LineEntry::Dirty(line)
                }
            } else {
                LineEntry::Dirty(line)
            }
        };
        self.lines.put(stable_row, entry);
    }

    fn schedule_fetch_lines(&mut self, to_fetch: RangeSet<StableRowIndex>, now: Instant) {
        if to_fetch.is_empty() {
            return;
        }

        let local_tab_id = self.local_tab_id;
        log::trace!(
            "will fetch lines {:?} for remote tab id {} at {:?}",
            to_fetch,
            self.remote_tab_id,
            now,
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

                        match result {
                            Ok(result) => {
                                let config = configuration();
                                let lines = result.lines.lines();

                                log::trace!("fetch complete for {:?} at {:?}", to_fetch, now);
                                for (stable_row, line) in lines.into_iter() {
                                    inner.put_line(stable_row, line, &config, Some(now));
                                }
                            }
                            Err(err) => {
                                log::error!("get_lines failed: {}", err);
                                for r in to_fetch.iter() {
                                    for stable_row in r.clone() {
                                        let entry = match inner.lines.pop(&stable_row) {
                                            Some(LineEntry::Fetching(then)) if then == now => {
                                                // leave it popped
                                                continue;
                                            }
                                            Some(LineEntry::DirtyAndFetching(line, then))
                                                if then == now =>
                                            {
                                                // revert to just dirty
                                                LineEntry::Dirty(line)
                                            }
                                            Some(entry) => entry,
                                            None => continue,
                                        };
                                        inner.lines.put(stable_row, entry);
                                    }
                                }
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
        let now = Instant::now();

        for idx in lines.clone() {
            let entry = match inner.lines.pop(&idx) {
                Some(LineEntry::Line(line)) => {
                    result.push(line.clone());
                    LineEntry::Line(line)
                }
                Some(LineEntry::Dirty(line)) => {
                    result.push(line.clone());
                    // Clear the dirty status as part of this retrieval
                    LineEntry::Line(line)
                }
                Some(LineEntry::DirtyAndFetching(line, then)) => {
                    result.push(line.clone());
                    LineEntry::DirtyAndFetching(line, then)
                }
                Some(LineEntry::Fetching(then)) => {
                    result.push(Line::with_width(inner.dimensions.cols));
                    LineEntry::Fetching(then)
                }
                None => {
                    result.push(Line::with_width(inner.dimensions.cols));
                    to_fetch.add(idx);
                    LineEntry::Fetching(now)
                }
            };
            inner.lines.put(idx, entry);
        }

        inner.schedule_fetch_lines(to_fetch, now);
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
        for r in lines {
            match inner.lines.get(&r) {
                Some(LineEntry::Dirty(_)) | Some(LineEntry::DirtyAndFetching(..)) => {
                    result.add(r);
                }
                _ => {}
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
