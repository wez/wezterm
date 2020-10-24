use crate::domain::ClientInner;
use crate::pane::clienttab::ClientPane;
use anyhow::anyhow;
use codec::*;
use config::{configuration, ConfigHandle};
use lru::LruCache;
use mux::renderable::{Renderable, RenderableDimensions, StableCursorPosition};
use mux::tab::TabId;
use mux::Mux;
use promise::BrokenPromise;
use rangeset::*;
use ratelim::RateLimiter;
use std::cell::RefCell;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::cell::{Cell, CellAttributes, Underline};
use termwiz::color::AnsiColor;
use url::Url;
use wezterm_term::{KeyCode, KeyModifiers};
use wezterm_term::{Line, StableRowIndex};

const MAX_POLL_INTERVAL: Duration = Duration::from_secs(30);
const BASE_POLL_INTERVAL: Duration = Duration::from_millis(20);

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
    // We have a local copy but it is stale and will need to be
    // fetched again
    Stale(Line),
}

impl LineEntry {
    fn kind(&self) -> (&'static str, Option<Instant>) {
        match self {
            Self::Line(_) => ("Line", None),
            Self::Dirty(_) => ("Dirty", None),
            Self::Fetching(since) => ("Fetching", Some(*since)),
            Self::DirtyAndFetching(_, since) => ("DirtyAndFetching", Some(*since)),
            Self::Stale(_) => ("Stale", None),
        }
    }
}

pub struct RenderableInner {
    client: Arc<ClientInner>,
    remote_pane_id: TabId,
    local_pane_id: TabId,
    last_poll: Instant,
    pub dead: bool,
    poll_in_progress: AtomicBool,
    poll_interval: Duration,

    cursor_position: StableCursorPosition,
    pub dimensions: RenderableDimensions,

    lines: LruCache<StableRowIndex, LineEntry>,
    pub title: String,
    pub working_dir: Option<Url>,

    fetch_limiter: RateLimiter,

    last_send_time: Instant,
    last_recv_time: Instant,
    last_late_dirty: Instant,
    last_input_rtt: u64,

    pub input_serial: InputSerial,
}

pub struct RenderableState {
    pub inner: RefCell<RenderableInner>,
}

impl RenderableInner {
    pub fn new(
        client: &Arc<ClientInner>,
        remote_pane_id: TabId,
        local_pane_id: TabId,
        dimensions: RenderableDimensions,
        title: &str,
        fetch_limiter: RateLimiter,
    ) -> Self {
        let now = Instant::now();

        Self {
            client: Arc::clone(client),
            remote_pane_id,
            local_pane_id,
            last_poll: now,
            dead: false,
            poll_in_progress: AtomicBool::new(false),
            poll_interval: BASE_POLL_INTERVAL,
            cursor_position: StableCursorPosition::default(),
            dimensions,
            lines: LruCache::new(configuration().scrollback_lines),
            title: title.to_string(),
            working_dir: None,
            fetch_limiter,
            last_send_time: now,
            last_recv_time: now,
            last_late_dirty: now,
            last_input_rtt: 0,
            input_serial: InputSerial::empty(),
        }
    }

    /// Returns true if we think we should display the laggy connection
    /// indicator.  If we're past our poll interval and more recently
    /// tried to send something than receive something, the UI is worth
    /// showing.
    pub fn is_tardy(&self) -> bool {
        let elapsed = self.last_recv_time.elapsed();
        if elapsed > self.poll_interval.max(Duration::from_secs(3)) {
            self.last_send_time > self.last_recv_time
        } else {
            false
        }
    }

    /// Predictive echo can be noisy when the link is working well,
    /// so we only employ it when it looks like the latency is high.
    /// We pick 100ms as the threshold for this.
    fn should_predict(&self) -> bool {
        self.last_input_rtt >= 100
    }

    /// Compute a "prediction" and apply it to the line data that we
    /// have available, marking it as dirty so that it gets rendered.
    /// The prediction is basically just local echo.
    /// Open questions:
    /// how do we tell if the intent is to suppress local echo during eg:
    ///  * password prompt?  One option is to look back and see if the line
    ///                      looks like a password prompt.
    ///  * normal mode in vim: letter presses are typically movement or
    ///                        other editor commands
    /// There are bound to be a number of other edge cases that we should
    /// handle.
    fn apply_prediction(&mut self, c: KeyCode, line: &mut Line) {
        let text = line.as_str();
        if text.contains("sword") {
            // This line might be a password prompt.  Don't force
            // on local echo here, as we don't want to reveal content
            // from their password
            return;
        }

        match c {
            KeyCode::Enter => {
                self.cursor_position.x = 0;
                self.cursor_position.y += 1;
            }
            KeyCode::UpArrow => {
                self.cursor_position.y = self.cursor_position.y.saturating_sub(1);
            }
            KeyCode::DownArrow => {
                self.cursor_position.y += 1;
            }
            KeyCode::RightArrow => {
                self.cursor_position.x += 1;
            }
            KeyCode::LeftArrow => {
                self.cursor_position.x = self.cursor_position.x.saturating_sub(1);
            }
            KeyCode::Delete => {
                line.erase_cell(self.cursor_position.x);
            }
            KeyCode::Backspace => {
                if self.cursor_position.x > 0 {
                    line.erase_cell(self.cursor_position.x - 1);
                    self.cursor_position.x -= 1;
                }
            }
            KeyCode::Char(c) => {
                let cell = Cell::new(
                    c,
                    CellAttributes::default()
                        .set_underline(Underline::Double)
                        .clone(),
                );

                let cell = line.set_cell(self.cursor_position.x, cell);
                // Adjust the cursor to reflect the width of this new cell
                self.cursor_position.x += cell.width();
            }
            _ => {}
        }
    }

    /// Based on a keypress, apply a "prediction" of what the terminal
    /// content will look like once we receive the response from the
    /// remote system.  The prediction helps to reduce perceived latency
    /// when a user is typing at any reasonable velocity.
    pub fn predict_from_key_event(&mut self, key: KeyCode, mods: KeyModifiers) {
        if !self.should_predict() {
            return;
        }

        let c = match key {
            KeyCode::LeftArrow
            | KeyCode::RightArrow
            | KeyCode::UpArrow
            | KeyCode::DownArrow
            | KeyCode::Delete
            | KeyCode::Backspace
            | KeyCode::Enter
            | KeyCode::Char(_) => key,
            _ => return,
        };
        if mods != KeyModifiers::NONE && mods != KeyModifiers::SHIFT {
            return;
        }

        let row = self.cursor_position.y;
        match self.lines.pop(&row) {
            Some(LineEntry::Stale(mut line))
            | Some(LineEntry::Line(mut line))
            | Some(LineEntry::Dirty(mut line)) => {
                self.apply_prediction(c, &mut line);
                self.lines.put(row, LineEntry::Dirty(line));
            }
            Some(LineEntry::DirtyAndFetching(mut line, instant)) => {
                self.apply_prediction(c, &mut line);
                self.lines
                    .put(row, LineEntry::DirtyAndFetching(line, instant));
            }
            Some(entry) => {
                self.lines.put(row, entry);
            }
            None => {}
        }
    }

    fn apply_paste_prediction(&mut self, row: usize, text: &str, line: &mut Line) {
        let attrs = CellAttributes::default()
            .set_underline(Underline::Double)
            .clone();

        let text_line = Line::from_text(text, &attrs);

        if row == 0 {
            for cell in text_line.cells() {
                line.set_cell(self.cursor_position.x, cell.clone());
                self.cursor_position.x += cell.width();
            }
        } else {
            // The pasted line replaces the data for the existing line
            line.resize_and_clear(0);
            line.append_line(text_line);
            self.cursor_position.x = line.cells().len();
        }
    }

    pub fn predict_from_paste(&mut self, text: &str) {
        if !self.should_predict() {
            return;
        }

        let text = textwrap::fill(text, self.dimensions.cols);
        let lines: Vec<&str> = text.split("\n").collect();

        for (idx, paste_line) in lines.iter().enumerate() {
            let row = self.cursor_position.y + idx as StableRowIndex;

            match self.lines.pop(&row) {
                Some(LineEntry::Stale(mut line))
                | Some(LineEntry::Line(mut line))
                | Some(LineEntry::Dirty(mut line)) => {
                    self.apply_paste_prediction(idx, paste_line, &mut line);
                    self.lines.put(row, LineEntry::Dirty(line));
                }
                Some(LineEntry::DirtyAndFetching(mut line, instant)) => {
                    self.apply_paste_prediction(idx, paste_line, &mut line);
                    self.lines
                        .put(row, LineEntry::DirtyAndFetching(line, instant));
                }
                Some(entry) => {
                    self.lines.put(row, entry);
                }
                None => {}
            }
        }
        self.cursor_position.y += lines.len().saturating_sub(1) as StableRowIndex;
    }

    pub fn update_last_send(&mut self) {
        self.last_send_time = Instant::now();
        self.poll_interval = BASE_POLL_INTERVAL;
    }

    pub fn apply_changes_to_surface(&mut self, delta: GetPaneRenderChangesResponse) {
        let now = Instant::now();
        self.poll_interval = BASE_POLL_INTERVAL;
        self.last_recv_time = now;

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

        // Keep track of the approximate round trip time by recording how
        // long it took for this response to come back
        if let Some(serial) = delta.input_serial {
            self.last_input_rtt = serial.elapsed_millis();
        }

        // When it comes to updating the cursor position, if the update was tagged
        // with keyboard input, we'll only take the position if the update comes from
        // the most recent key event.  This helps to prevent the cursor wiggling if the
        // user is typing more than one character per roundtrip interval--the wiggle
        // manifests because we may have already predicted a local cursor move forwards
        // by one character, and we may receive the response to the prior update after
        // we have rendered that, and then shortly receive the most recent response.
        // The result of that is that the cursor moves right one, left one and then
        // finally right one in quick succession.
        // If the delta was not from an input event then we trust it; this is most
        // like due to a unilateral movement by the application on the other end.
        if delta.input_serial.is_none()
            || delta.input_serial.unwrap_or(InputSerial::empty()) >= self.input_serial
        {
            self.cursor_position = delta.cursor_position;
        }
        self.dimensions = delta.dimensions;
        self.title = delta.title;
        self.working_dir = delta.working_dir.map(Into::into);

        let config = configuration();
        for (stable_row, line) in delta.bonus_lines.lines() {
            self.put_line(stable_row, line, &config, None);
            dirty.remove(stable_row);
        }

        if !dirty.is_empty() {
            Mux::get()
                .unwrap()
                .notify(mux::MuxNotification::PaneOutput(self.local_pane_id));
        }

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
                    self.make_stale(stable_row);
                    continue;
                }
                to_fetch.add(stable_row);
                let entry = match prior {
                    Some(LineEntry::Fetching(_)) | None => LineEntry::Fetching(now),
                    Some(LineEntry::DirtyAndFetching(old, ..))
                    | Some(LineEntry::Stale(old))
                    | Some(LineEntry::Dirty(old))
                    | Some(LineEntry::Line(old)) => LineEntry::DirtyAndFetching(old, now),
                };
                log::trace!(
                    "row {} {:?} -> {:?} due to dirty and IN viewport",
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
                        self.make_stale(stable_row);
                    }
                }
            }
        }
    }

    pub fn make_all_stale(&mut self) {
        let mut lines = LruCache::unbounded();
        while let Some((stable_row, entry)) = self.lines.pop_lru() {
            let entry = match entry {
                LineEntry::Dirty(old) | LineEntry::Stale(old) | LineEntry::Line(old) => {
                    LineEntry::Stale(old)
                }
                entry => entry,
            };
            lines.put(stable_row, entry);
        }
        self.lines = lines;
    }

    fn make_stale(&mut self, stable_row: StableRowIndex) {
        match self.lines.pop(&stable_row) {
            Some(LineEntry::Dirty(old))
            | Some(LineEntry::Stale(old))
            | Some(LineEntry::Line(old))
            | Some(LineEntry::DirtyAndFetching(old, _)) => {
                self.lines.put(stable_row, LineEntry::Stale(old));
            }
            Some(LineEntry::Fetching(_)) | None => {}
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
                Some(e) => {
                    // It changed since we started: leave it alone!
                    log::trace!(
                        "row {} {:?} changed since fetch started at {:?}, so leave it be",
                        stable_row,
                        e.kind(),
                        fetch_start
                    );
                    self.lines.put(stable_row, e);
                    return;
                }
                None => return,
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

        let local_pane_id = self.local_pane_id;
        log::trace!(
            "will fetch lines {:?} for remote tab id {} at {:?}",
            to_fetch,
            self.remote_pane_id,
            now,
        );

        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;

        promise::spawn::spawn(async move {
            let result = client
                .client
                .get_lines(GetLines {
                    pane_id: remote_pane_id,
                    lines: to_fetch.clone().into(),
                })
                .await;
            Self::apply_lines(local_pane_id, result, to_fetch, now)
        })
        .detach();
    }

    fn apply_lines(
        local_pane_id: TabId,
        result: anyhow::Result<GetLinesResponse>,
        to_fetch: RangeSet<StableRowIndex>,
        now: Instant,
    ) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let pane = mux
            .get_pane(local_pane_id)
            .ok_or_else(|| anyhow!("no such tab {}", local_pane_id))?;
        if let Some(client_tab) = pane.downcast_ref::<ClientPane>() {
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
                                Some(LineEntry::DirtyAndFetching(line, then)) if then == now => {
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
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        if self.poll_in_progress.load(Ordering::SeqCst) {
            // We have a poll in progress
            return Ok(());
        }

        if self.last_poll.elapsed() < self.poll_interval {
            return Ok(());
        }

        let interval = self.poll_interval;
        let interval = (interval + interval).min(MAX_POLL_INTERVAL);
        self.poll_interval = interval;

        self.last_poll = Instant::now();
        self.poll_in_progress.store(true, Ordering::SeqCst);
        let remote_pane_id = self.remote_pane_id;
        let local_pane_id = self.local_pane_id;
        let client = Arc::clone(&self.client);
        promise::spawn::spawn(async move {
            let alive = match client
                .client
                .get_tab_render_changes(GetPaneRenderChanges {
                    pane_id: remote_pane_id,
                })
                .await
            {
                Ok(resp) => resp.is_alive,
                // if we got a timeout on a reconnectable, don't
                // consider the tab to be dead; that helps to
                // avoid having a tab get shuffled around
                Err(_) => client.client.is_reconnectable,
            };

            let mux = Mux::get().unwrap();
            let tab = mux
                .get_pane(local_pane_id)
                .ok_or_else(|| anyhow!("no such tab {}", local_pane_id))?;
            if let Some(client_tab) = tab.downcast_ref::<ClientPane>() {
                let renderable = client_tab.renderable.borrow_mut();
                let mut inner = renderable.inner.borrow_mut();

                inner.dead = !alive;
                inner.last_recv_time = Instant::now();
                inner.poll_in_progress.store(false, Ordering::SeqCst);
            }
            Ok::<(), anyhow::Error>(())
        })
        .detach();
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
                Some(LineEntry::Stale(line)) => {
                    result.push(line.clone());
                    to_fetch.add(idx);
                    LineEntry::DirtyAndFetching(line, now)
                }
                None => {
                    result.push(Line::with_width(inner.dimensions.cols));
                    to_fetch.add(idx);
                    LineEntry::Fetching(now)
                }
            };

            if idx == inner.dimensions.physical_top {
                if inner.is_tardy() {
                    let status = format!(
                        "wezterm: {:.0?}⏳since last response",
                        inner.last_recv_time.elapsed()
                    );
                    // Right align it in the tab
                    let col = inner
                        .dimensions
                        .cols
                        .saturating_sub(wezterm_term::unicode_column_width(&status));

                    let mut attr = CellAttributes::default();
                    attr.foreground = AnsiColor::White.into();
                    attr.background = AnsiColor::Blue.into();

                    result
                        .last_mut()
                        .unwrap()
                        .overlay_text_with_attribute(col, &status, attr);
                }
            }

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
                None | Some(LineEntry::Dirty(_)) | Some(LineEntry::DirtyAndFetching(..)) => {
                    result.add(r);
                }
                _ => {}
            }
        }

        // If we're behind receiving an update, invalidate the top row so
        // that the indicator will update in a more timely fashion
        if inner.is_tardy() {
            // ... but take care to avoid always reporting it as dirty, so
            // that we don't end up busy looping just to repaint it
            if inner.last_late_dirty.elapsed() >= Duration::from_secs(1) {
                result.add(inner.dimensions.physical_top);
                inner.last_late_dirty = Instant::now();
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
