use crate::selection::{SelectionCoordinate, SelectionRange, SelectionX};
use crate::termwindow::keyevent::KeyTableArgs;
use crate::termwindow::{TermWindow, TermWindowNotif};
use config::keyassignment::{
    ClipboardCopyDestination, CopyModeAssignment, KeyAssignment, KeyTable, KeyTableEntry,
    ScrollbackEraseMode, SelectionMode,
};
use mux::domain::DomainId;
use mux::pane::{
    CachePolicy, ForEachPaneLogicalLine, LogicalLine, Pane, PaneId, Pattern, PatternType,
    PerformAssignmentResult, SearchResult, WithPaneLines,
};
use mux::renderable::*;
use mux::tab::TabId;
use ordered_float::NotNan;
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use rangeset::RangeSet;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::AnsiColor;
use termwiz::lineedit::{LineEditBuffer, Movement};
use termwiz::surface::{CursorVisibility, SequenceNo, SEQ_ZERO};
use unicode_segmentation::*;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    unicode_column_width, Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, SemanticType,
    StableRowIndex, TerminalSize,
};
use window::{KeyCode as WKeyCode, Modifiers, WindowOps};

lazy_static::lazy_static! {
    static ref SAVED_PATTERN: Mutex<HashMap<TabId, Pattern>> = Mutex::new(HashMap::new());
}

const SEARCH_CHUNK_SIZE: StableRowIndex = 1000;

pub struct CopyOverlay {
    delegate: Arc<dyn Pane>,
    render: Arc<Mutex<CopyRenderable>>,
    writer: Mutex<SearchOverlayPatternWriter>,
}

#[derive(Copy, Clone, Debug)]
struct PendingJump {
    forward: bool,
    prev_char: bool,
}

#[derive(Copy, Clone, Debug)]
struct Jump {
    forward: bool,
    prev_char: bool,
    target: char,
}

struct CopyRenderable {
    cursor: StableCursorPosition,
    delegate: Arc<dyn Pane>,
    start: Option<SelectionCoordinate>,
    selection_mode: SelectionMode,
    viewport: Option<StableRowIndex>,
    /// We use this to cancel ourselves later
    window: ::window::Window,

    /// The text that the user entered
    pattern_type: PatternType,
    search_line: LineEditBuffer,
    /// The most recently queried set of matches
    results: Vec<SearchResult>,
    by_line: HashMap<StableRowIndex, Vec<MatchResult>>,
    last_result_seqno: SequenceNo,
    last_bar_pos: Option<StableRowIndex>,
    dirty_results: RangeSet<StableRowIndex>,
    width: usize,
    height: usize,
    editing_search: bool,
    result_pos: Option<usize>,
    tab_id: TabId,
    /// Used to debounce queries while the user is typing
    typing_cookie: usize,
    searching: Option<Searching>,
    pending_jump: Option<PendingJump>,
    last_jump: Option<Jump>,
}

struct Searching {
    remain: StableRowIndex,
}

#[derive(Debug)]
struct MatchResult {
    range: Range<usize>,
    result_index: usize,
}

struct Dimensions {
    vertical_gap: isize,
    dims: RenderableDimensions,
    top: StableRowIndex,
}

#[derive(Debug)]
pub struct CopyModeParams {
    pub pattern: Pattern,
    pub editing_search: bool,
}

impl CopyOverlay {
    pub fn with_pane(
        term_window: &TermWindow,
        pane: &Arc<dyn Pane>,
        params: CopyModeParams,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        let mut cursor = pane.get_cursor_position();
        cursor.shape = termwiz::surface::CursorShape::SteadyBlock;
        cursor.visibility = CursorVisibility::Visible;

        let (_domain, _window, tab_id) = mux::Mux::get()
            .resolve_pane_id(pane.pane_id())
            .ok_or_else(|| anyhow::anyhow!("no tab contains the current pane"))?;

        let window = term_window
            .window
            .clone()
            .ok_or_else(|| anyhow::anyhow!("failed to clone window handle"))?;
        let dims = pane.get_dimensions();
        let pattern = if params.pattern.is_empty() {
            SAVED_PATTERN
                .lock()
                .get(&tab_id)
                .map(|p| p.clone())
                .unwrap_or(params.pattern)
        } else {
            params.pattern
        };
        let search_line = LineEditBuffer::new(&pattern, pattern.len());

        let mut render = CopyRenderable {
            cursor,
            window,
            delegate: Arc::clone(pane),
            start: None,
            viewport: term_window.get_viewport(pane.pane_id()),
            results: vec![],
            by_line: HashMap::new(),
            dirty_results: RangeSet::default(),
            width: dims.cols,
            height: dims.viewport_rows,
            last_result_seqno: SEQ_ZERO,
            last_bar_pos: None,
            tab_id,
            pattern_type: PatternType::from(&pattern),
            search_line,
            editing_search: params.editing_search,
            result_pos: None,
            selection_mode: SelectionMode::Cell,
            typing_cookie: 0,
            searching: None,
            pending_jump: None,
            last_jump: None,
        };

        let search_row = render.compute_search_row();
        render.dirty_results.add(search_row);
        render.update_search();

        let shared_render = Arc::new(Mutex::new(render));
        let writer = SearchOverlayPatternWriter {
            render: Arc::clone(&shared_render),
        };

        Ok(Arc::new(CopyOverlay {
            delegate: Arc::clone(pane),
            render: shared_render,
            writer: Mutex::new(writer),
        }))
    }

    pub fn get_params(&self) -> CopyModeParams {
        let render = self.render.lock();
        CopyModeParams {
            pattern: render.get_pattern(),
            editing_search: render.editing_search,
        }
    }

    pub fn apply_params(&self, params: CopyModeParams) {
        let mut render = self.render.lock();
        render.editing_search = params.editing_search;
        if render.get_pattern() != params.pattern {
            render.pattern_type = PatternType::from(&params.pattern);
            render
                .search_line
                .set_line_and_cursor(&params.pattern, params.pattern.len());
            render.schedule_update_search();
        }
        let search_row = render.compute_search_row();
        render.dirty_results.add(search_row);
    }

    pub fn viewport_changed(&self, viewport: Option<StableRowIndex>) {
        let mut render = self.render.lock();
        if render.viewport != viewport {
            if let Some(last) = render.last_bar_pos.take() {
                render.dirty_results.add(last);
            }
            if let Some(pos) = viewport.as_ref() {
                render.dirty_results.add(*pos);
            }
            render.viewport = viewport;
        }
    }
}

impl CopyRenderable {
    fn compute_search_row(&self) -> StableRowIndex {
        let dims = self.delegate.get_dimensions();
        let top = self.viewport.unwrap_or_else(|| dims.physical_top);
        let bottom = (top + dims.viewport_rows as StableRowIndex).saturating_sub(1);
        bottom
    }

    fn check_for_resize(&mut self) {
        let dims = self.delegate.get_dimensions();
        if dims.cols == self.width && dims.viewport_rows == self.height {
            return;
        }

        self.width = dims.cols;
        self.height = dims.viewport_rows;

        let pos = self.result_pos;
        self.update_search();
        self.result_pos = pos;
    }

    fn incrementally_recompute_results(&mut self, mut results: Vec<SearchResult>) {
        results.sort();
        results.reverse();
        for (result_index, res) in results.iter().enumerate() {
            let result_index = self.results.len() + result_index;
            for idx in res.start_y..=res.end_y {
                let range = if idx == res.start_y && idx == res.end_y {
                    // Range on same line
                    res.start_x..res.end_x
                } else if idx == res.end_y {
                    // final line of multi-line
                    0..res.end_x
                } else if idx == res.start_y {
                    // first line of multi-line
                    res.start_x..self.width
                } else {
                    // a middle line
                    0..self.width
                };

                let result = MatchResult {
                    range,
                    result_index,
                };

                let matches = self.by_line.entry(idx).or_insert_with(|| vec![]);
                matches.push(result);

                self.dirty_results.add(idx);
            }
        }
        self.results.append(&mut results);
    }

    fn schedule_update_search(&mut self) {
        self.typing_cookie += 1;
        let cookie = self.typing_cookie;

        let window = self.window.clone();
        let pane_id = self.delegate.pane_id();

        promise::spawn::spawn(async move {
            smol::Timer::after(Duration::from_millis(350)).await;
            window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let state = term_window.pane_state(pane_id);
                if let Some(overlay) = state.overlay.as_ref() {
                    if let Some(copy_overlay) = overlay.pane.downcast_ref::<CopyOverlay>() {
                        let mut r = copy_overlay.render.lock();
                        if cookie == r.typing_cookie {
                            r.update_search();
                        }
                    }
                }
            })));
            anyhow::Result::<()>::Ok(())
        })
        .detach();
    }

    fn update_search(&mut self) {
        for idx in self.by_line.keys() {
            self.dirty_results.add(*idx);
        }
        if let Some(idx) = self.last_bar_pos.as_ref() {
            self.dirty_results.add(*idx);
        }

        self.results.clear();
        self.by_line.clear();
        self.result_pos.take();

        SAVED_PATTERN.lock().insert(self.tab_id, self.get_pattern());

        let bar_pos = self.compute_search_row();
        self.dirty_results.add(bar_pos);
        self.last_result_seqno = self.delegate.get_current_seqno();

        let pattern = self.get_pattern();
        if !pattern.is_empty() {
            let pane: Arc<dyn Pane> = self.delegate.clone();
            let window = self.window.clone();
            let dims = pane.get_dimensions();

            let end = dims.scrollback_top + dims.scrollback_rows as StableRowIndex;
            let range = end
                .saturating_sub(SEARCH_CHUNK_SIZE)
                .max(dims.scrollback_top)..end;

            self.searching.replace(Searching {
                remain: range.start - dims.scrollback_top,
            });

            promise::spawn::spawn(async move {
                let limit = None;
                log::trace!("Searching for {pattern:?} in {range:?}");
                let results = pane.search(pattern.clone(), range.clone(), limit).await?;

                let pane_id = pane.pane_id();
                let mut results = Some(results);
                window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                    let state = term_window.pane_state(pane_id);
                    if let Some(overlay) = state.overlay.as_ref() {
                        if let Some(copy_overlay) = overlay.pane.downcast_ref::<CopyOverlay>() {
                            let mut r = copy_overlay.render.lock();
                            r.processed_search_chunk(pattern, results.take().unwrap(), range);
                        }
                    }
                })));

                anyhow::Result::<()>::Ok(())
            })
            .detach();
        } else {
            self.searching.take();
            self.clear_selection();
        }
        self.window.invalidate();
    }

    fn processed_search_chunk(
        &mut self,
        pattern: Pattern,
        results: Vec<SearchResult>,
        range: Range<StableRowIndex>,
    ) {
        self.window.invalidate();
        if pattern != self.get_pattern() {
            return;
        }
        let is_first = self.results.is_empty();
        self.incrementally_recompute_results(results);

        if is_first {
            if !self.results.is_empty() {
                self.activate_match_number(0);
            } else {
                self.set_viewport(None);
                self.clear_selection();
            }
        }

        let dims = self.delegate.get_dimensions();
        if range.start == dims.scrollback_top {
            self.searching.take();
            return;
        }

        // Search next chunk
        let pane: Arc<dyn Pane> = self.delegate.clone();
        let window = self.window.clone();
        let end = range.start;
        let range = end
            .saturating_sub(SEARCH_CHUNK_SIZE)
            .max(dims.scrollback_top)..end;

        self.searching.replace(Searching {
            remain: range.start - dims.scrollback_top,
        });

        promise::spawn::spawn(async move {
            let limit = None;
            log::trace!("Searching for {pattern:?} in {range:?}");
            let results = pane.search(pattern.clone(), range.clone(), limit).await?;

            let pane_id = pane.pane_id();
            let mut results = Some(results);
            window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let state = term_window.pane_state(pane_id);
                if let Some(overlay) = state.overlay.as_ref() {
                    if let Some(copy_overlay) = overlay.pane.downcast_ref::<CopyOverlay>() {
                        let mut r = copy_overlay.render.lock();
                        r.processed_search_chunk(pattern, results.take().unwrap(), range);
                    }
                }
            })));

            anyhow::Result::<()>::Ok(())
        })
        .detach();
    }

    fn clear_selection(&mut self) {
        let pane_id = self.delegate.pane_id();
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let mut selection = term_window.selection(pane_id);
                selection.origin.take();
                selection.range.take();
            })));
    }

    fn activate_match_number(&mut self, n: usize) {
        self.result_pos.replace(n);
        let result = self.results[n].clone();
        self.cursor.y = result.end_y;
        self.cursor.x = result.end_x.saturating_sub(1);

        let start = SelectionCoordinate::x_y(result.start_x, result.start_y);
        let end = SelectionCoordinate::x_y(result.end_x.saturating_sub(1), result.end_y);
        self.start.replace(start);
        self.adjust_selection(start, SelectionRange { start, end });
    }

    fn clamp_cursor_to_scrollback(&mut self) {
        let dims = self.delegate.get_dimensions();
        if self.cursor.x >= dims.cols {
            self.cursor.x = dims.cols - 1;
        }
        if self.cursor.y < dims.scrollback_top {
            self.cursor.y = dims.scrollback_top;
        }

        let max_row = dims.scrollback_top + dims.scrollback_rows as isize;
        if self.cursor.y >= max_row {
            self.cursor.y = max_row - 1;
        }
    }

    fn select_to_cursor_pos(&mut self) {
        self.clamp_cursor_to_scrollback();
        if let Some(sel_start) = self.start {
            let cursor = SelectionCoordinate::x_y(self.cursor.x, self.cursor.y);

            let (start, end) = match self.selection_mode {
                SelectionMode::Line => {
                    let cursor_is_above_start = self.cursor.y < sel_start.y;

                    let start = SelectionCoordinate::x_y(
                        if cursor_is_above_start {
                            usize::max_value()
                        } else {
                            0
                        },
                        sel_start.y,
                    );
                    let end = SelectionCoordinate::x_y(
                        if cursor_is_above_start {
                            0
                        } else {
                            usize::max_value()
                        },
                        self.cursor.y,
                    );
                    (start, end)
                }
                SelectionMode::SemanticZone => {
                    let zone_range = SelectionRange::zone_around(cursor, &*self.delegate);
                    let start_zone = SelectionRange::zone_around(sel_start, &*self.delegate);

                    let range = zone_range.extend_with(start_zone);

                    (range.start, range.end)
                }
                _ => {
                    let start = SelectionCoordinate {
                        x: sel_start.x,
                        y: sel_start.y,
                    };
                    let end = cursor;
                    (start, end)
                }
            };

            self.adjust_selection(start, SelectionRange { start, end });
        } else {
            self.adjust_viewport_for_cursor_position();
            self.window.invalidate();
        }
    }

    fn adjust_selection(&self, start: SelectionCoordinate, range: SelectionRange) {
        let pane_id = self.delegate.pane_id();
        let window = self.window.clone();
        let mode = self.selection_mode;
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let mut selection = term_window.selection(pane_id);
                selection.origin = Some(start);
                selection.range = Some(range);
                selection.rectangular = mode == SelectionMode::Block;
                window.invalidate();
            })));
        self.adjust_viewport_for_cursor_position();
    }

    fn dimensions(&self) -> Dimensions {
        const VERTICAL_GAP: isize = 5;
        let dims = self.delegate.get_dimensions();
        let vertical_gap = if dims.physical_top <= VERTICAL_GAP {
            1
        } else {
            VERTICAL_GAP
        };
        let top = self.viewport.unwrap_or_else(|| dims.physical_top);
        Dimensions {
            vertical_gap,
            top,
            dims,
        }
    }

    fn adjust_viewport_for_cursor_position(&self) {
        let dims = self.dimensions();

        if dims.top > self.cursor.y {
            // Cursor is off the top of the viewport; adjust
            self.set_viewport(Some(self.cursor.y.saturating_sub(dims.vertical_gap)));
            return;
        }

        let top_gap = self.cursor.y - dims.top;
        if top_gap < dims.vertical_gap {
            // Increase the gap so we can "look ahead"
            self.set_viewport(Some(self.cursor.y.saturating_sub(dims.vertical_gap)));
            return;
        }

        let bottom_gap = (dims.dims.viewport_rows as isize).saturating_sub(top_gap);
        if bottom_gap < dims.vertical_gap {
            self.set_viewport(Some(dims.top + dims.vertical_gap - bottom_gap));
        }
    }

    fn set_viewport(&self, row: Option<StableRowIndex>) {
        let dims = self.delegate.get_dimensions();
        let pane_id = self.delegate.pane_id();
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                term_window.set_viewport(pane_id, row, dims);
            })));
    }

    fn close(&self) {
        TermWindow::schedule_cancel_overlay_for_pane(self.window.clone(), self.delegate.pane_id());
    }

    fn move_by_page(&mut self, amount: f64) {
        let dims = self.dimensions();
        let rows = (dims.dims.viewport_rows as f64 * amount) as isize;
        self.cursor.y += rows;
        self.select_to_cursor_pos();
    }

    /// Move to next match
    fn next_match(&mut self) {
        if let Some(cur) = self.result_pos.as_ref() {
            let prior = if *cur > 0 {
                cur - 1
            } else {
                self.results.len() - 1
            };
            self.activate_match_number(prior);
        }
    }

    /// Move to prior match
    fn prior_match(&mut self) {
        if let Some(cur) = self.result_pos.as_ref() {
            let next = if *cur + 1 >= self.results.len() {
                0
            } else {
                *cur + 1
            };
            self.activate_match_number(next);
        }
    }

    /// Skip this page of matches and move down to the first match from
    /// the next page.
    fn next_match_page(&mut self) {
        let dims = self.delegate.get_dimensions();
        if let Some(cur) = self.result_pos {
            let top = self.viewport.unwrap_or(dims.physical_top);
            let prior = top - dims.viewport_rows as isize;
            if let Some(pos) = self
                .results
                .iter()
                .position(|res| res.start_y > prior && res.start_y < top)
            {
                self.activate_match_number(pos);
            } else {
                self.activate_match_number(cur.saturating_sub(1));
            }
        }
    }

    /// Skip this page of matches and move up to the first match from
    /// the prior page.
    fn prior_match_page(&mut self) {
        let dims = self.delegate.get_dimensions();
        if let Some(cur) = self.result_pos {
            let top = self.viewport.unwrap_or(dims.physical_top);
            let bottom = top + dims.viewport_rows as isize;
            if let Some(pos) = self.results.iter().position(|res| res.start_y >= bottom) {
                self.activate_match_number(pos);
            } else {
                let len = self.results.len().saturating_sub(1);
                self.activate_match_number(cur.min(len));
            }
        }
    }

    fn get_pattern(&self) -> Pattern {
        let pattern = self.search_line.get_line().to_string();
        match self.pattern_type {
            PatternType::CaseSensitiveString => Pattern::CaseSensitiveString(pattern),
            PatternType::CaseInSensitiveString => Pattern::CaseInSensitiveString(pattern),
            PatternType::Regex => Pattern::Regex(pattern),
        }
    }

    fn clear_pattern(&mut self) {
        self.search_line.clear();
        self.update_search();
    }

    fn edit_pattern(&mut self) {
        self.editing_search = true;
        self.update_key_table();
    }

    fn accept_pattern(&mut self) {
        self.editing_search = false;
        self.update_key_table();
    }

    fn update_key_table(&mut self) {
        let window = self.window.clone();
        let pane_id = self.delegate.pane_id();

        window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
            let mut state = term_window.pane_state(pane_id);
            if let Some(overlay) = state.overlay.as_mut() {
                if let Some(copy_overlay) = overlay.pane.downcast_ref::<CopyOverlay>() {
                    let editing_search = copy_overlay.render.lock().editing_search;

                    overlay.key_table_state.activate(KeyTableArgs {
                        name: if editing_search {
                            "search_mode"
                        } else {
                            "copy_mode"
                        },
                        timeout_milliseconds: None,
                        replace_current: true,
                        one_shot: false,
                        until_unknown: false,
                        prevent_fallback: false,
                    });
                }
            }
        })));
    }

    fn cycle_match_type(&mut self) {
        let pattern_type = match &self.pattern_type {
            PatternType::CaseSensitiveString => PatternType::CaseInSensitiveString,
            PatternType::CaseInSensitiveString => PatternType::Regex,
            PatternType::Regex => PatternType::CaseSensitiveString,
        };
        self.pattern_type = pattern_type;
        self.schedule_update_search();
    }

    fn move_to_viewport_middle(&mut self) {
        let dims = self.dimensions();
        self.cursor.y = dims.top + (dims.dims.viewport_rows as isize) / 2;
        self.select_to_cursor_pos();
    }

    fn move_to_viewport_top(&mut self) {
        let dims = self.dimensions();
        self.cursor.y = dims.top + dims.vertical_gap;
        self.select_to_cursor_pos();
    }

    fn move_to_viewport_bottom(&mut self) {
        let dims = self.dimensions();
        self.cursor.y = dims.top + (dims.dims.viewport_rows as isize) - dims.vertical_gap;
        self.select_to_cursor_pos();
    }

    fn move_left_single_cell(&mut self) {
        self.cursor.x = self.cursor.x.saturating_sub(1);
        self.select_to_cursor_pos();
    }

    fn move_right_single_cell(&mut self) {
        self.cursor.x += 1;
        self.select_to_cursor_pos();
    }

    fn move_up_single_row(&mut self) {
        self.cursor.y = self.cursor.y.saturating_sub(1);
        self.select_to_cursor_pos();
    }

    fn move_down_single_row(&mut self) {
        self.cursor.y += 1;
        self.select_to_cursor_pos();
    }
    fn move_to_start_of_line(&mut self) {
        self.cursor.x = 0;
        self.select_to_cursor_pos();
    }

    fn move_to_start_of_next_line(&mut self) {
        self.cursor.x = 0;
        self.cursor.y += 1;
        self.select_to_cursor_pos();
    }

    fn move_to_top(&mut self) {
        // This will get fixed up by clamp_cursor_to_scrollback
        self.cursor.y = 0;
        self.select_to_cursor_pos();
    }

    fn move_to_bottom(&mut self) {
        // This will get fixed up by clamp_cursor_to_scrollback
        self.cursor.y = isize::max_value();
        self.select_to_cursor_pos();
    }

    fn move_to_end_of_line_content(&mut self) {
        let y = self.cursor.y;
        let (top, lines) = self.delegate.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            self.cursor.x = 0;
            for cell in line.visible_cells() {
                if cell.str() != " " {
                    self.cursor.x = cell.cell_index();
                }
            }
        }
        self.select_to_cursor_pos();
    }

    fn move_to_start_of_line_content(&mut self) {
        let y = self.cursor.y;
        let (top, lines) = self.delegate.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            self.cursor.x = 0;
            for cell in line.visible_cells() {
                if cell.str() != " " {
                    self.cursor.x = cell.cell_index();
                    break;
                }
            }
        }
        self.select_to_cursor_pos();
    }

    fn move_to_selection_other_end(&mut self) {
        if let Some(old_start) = self.start {
            // Swap cursor & start of selection
            self.start
                .replace(SelectionCoordinate::x_y(self.cursor.x, self.cursor.y));
            self.cursor.x = match &old_start.x {
                SelectionX::Cell(x) => *x,
                SelectionX::BeforeZero => 0,
            };
            self.cursor.y = old_start.y;
            self.select_to_cursor_pos();
        }
    }

    fn move_to_selection_other_end_horiz(&mut self) {
        if self.selection_mode != SelectionMode::Block {
            return self.move_to_selection_other_end();
        }
        if let Some(old_start) = self.start {
            // Swap X coordinate of cursor & start of selection
            self.start
                .replace(SelectionCoordinate::x_y(self.cursor.x, old_start.y));
            self.cursor.x = match &old_start.x {
                SelectionX::Cell(x) => *x,
                SelectionX::BeforeZero => 0,
            };
            self.select_to_cursor_pos();
        }
    }

    fn move_backward_one_word(&mut self) {
        let y = if self.cursor.x == 0 && self.cursor.y > 0 {
            self.cursor.x = usize::max_value();
            self.cursor.y.saturating_sub(1)
        } else {
            self.cursor.y
        };

        let (top, lines) = self.delegate.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            if self.cursor.x == usize::max_value() {
                self.cursor.x = line.len().saturating_sub(1);
            }
            let s = line.columns_as_str(0..self.cursor.x.saturating_add(1));

            // "hello there you"
            //              |_
            //        |    _
            //  |    _
            //        |     _
            //  |     _

            let mut last_was_whitespace = false;

            for (idx, word) in s.split_word_bounds().rev().enumerate() {
                let width = unicode_column_width(word, None);

                if is_whitespace_word(word) {
                    self.cursor.x = self.cursor.x.saturating_sub(width);
                    last_was_whitespace = true;
                    continue;
                }
                last_was_whitespace = false;

                if idx == 0 && width == 1 {
                    // We were at the start of the initial word
                    self.cursor.x = self.cursor.x.saturating_sub(width);
                    continue;
                }

                self.cursor.x = self.cursor.x.saturating_sub(width.saturating_sub(1));
                break;
            }

            if last_was_whitespace && self.cursor.y > 0 {
                // The line begins with whitespace
                self.cursor.x = usize::max_value();
                self.cursor.y -= 1;
                return self.move_backward_one_word();
            }
        }
        self.select_to_cursor_pos();
    }

    fn move_forward_one_word(&mut self) {
        let y = self.cursor.y;
        let (top, lines) = self.delegate.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            let width = line.len();
            let s = line.columns_as_str(self.cursor.x..width + 1);
            let mut words = s.split_word_bounds();

            if let Some(word) = words.next() {
                self.cursor.x += unicode_column_width(word, None);
                if !is_whitespace_word(word) {
                    if let Some(word) = words.next() {
                        if is_whitespace_word(word) {
                            self.cursor.x += unicode_column_width(word, None);
                        }
                    }
                }
            }

            if self.cursor.x >= width {
                let dims = self.delegate.get_dimensions();
                let max_row = dims.scrollback_top + dims.scrollback_rows as isize;
                if self.cursor.y + 1 < max_row {
                    self.cursor.y += 1;
                    return self.move_to_start_of_line_content();
                }
            }
        }
        self.select_to_cursor_pos();
    }

    fn move_to_end_of_word(&mut self) {
        let y = self.cursor.y;
        let (top, lines) = self.delegate.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            let width = line.len();
            let s = line.columns_as_str(self.cursor.x..width + 1);
            let mut words = s.split_word_bounds();

            if self.cursor.x >= width - 1 {
                let dims = self.delegate.get_dimensions();
                let max_row = dims.scrollback_top + dims.scrollback_rows as isize;
                if self.cursor.y + 1 < max_row {
                    self.cursor.y += 1;
                    self.cursor.x = 0;
                    return self.move_to_end_of_word();
                }
            }

            if let Some(word) = words.next() {
                let mut word_end = self.cursor.x + unicode_column_width(word, None);
                if !is_whitespace_word(word) {
                    if self.cursor.x == word_end - 1 {
                        while let Some(next_word) = words.next() {
                            word_end += unicode_column_width(next_word, None);
                            if !is_whitespace_word(next_word) {
                                break;
                            }
                        }
                    }
                }
                while let Some(next_word) = words.next() {
                    if !is_whitespace_word(next_word) {
                        word_end += unicode_column_width(next_word, None);
                    } else {
                        break;
                    }
                }
                self.cursor.x = word_end - 1;
            }
        }
        self.select_to_cursor_pos();
    }

    fn move_by_zone(&mut self, mut delta: isize, zone_type: Option<SemanticType>) {
        if delta == 0 {
            return;
        }

        let zones = self
            .delegate
            .get_semantic_zones()
            .unwrap_or_else(|_| vec![]);
        let mut idx = match zones.binary_search_by(|zone| {
            if zone.start_y == self.cursor.y {
                zone.start_x.cmp(&self.cursor.x)
            } else if zone.start_y < self.cursor.y {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) | Err(idx) => idx,
        };

        let step = if delta > 0 { 1 } else { -1 };

        while delta != 0 {
            if step > 0 {
                idx = match idx.checked_add(1) {
                    Some(n) => n,
                    None => return,
                };
            } else {
                idx = match idx.checked_sub(1) {
                    Some(n) => n,
                    None => return,
                };
            }
            let zone = match zones.get(idx) {
                Some(z) => z,
                None => return,
            };
            if let Some(zone_type) = &zone_type {
                if zone.semantic_type != *zone_type {
                    continue;
                }
            }
            delta = delta.saturating_sub(step);

            self.cursor.x = zone.start_x;
            self.cursor.y = zone.start_y;
        }
        self.select_to_cursor_pos();
    }

    fn perform_jump(&mut self, jump: Jump, repeat: bool) {
        let y = self.cursor.y;
        let (_top, lines) = self.delegate.get_lines(y..y + 1);
        let target_str = jump.target.to_string();
        if let Some(line) = lines.get(0) {
            // Find the indices of cells with a matching target
            let mut candidates: Vec<usize> = line
                .visible_cells()
                .filter_map(|cell| {
                    if cell.str() == &target_str {
                        Some(cell.cell_index())
                    } else {
                        None
                    }
                })
                .collect();

            if !jump.forward {
                candidates.reverse();
            }

            // Adjust cursor cutoff so that we don't end up matching
            // the current cursor position for the prev_char cases
            let cursor_x = match (jump.prev_char && repeat, jump.forward) {
                (false, _) => self.cursor.x,
                (true, true) => self.cursor.x.saturating_add(1),
                (true, false) => self.cursor.x.saturating_sub(1),
            };

            // Find the target that matches the jump
            let target = candidates
                .iter()
                .find(|&&idx| {
                    if jump.forward {
                        idx > cursor_x
                    } else {
                        idx < cursor_x
                    }
                })
                .copied();

            if let Some(target) = target {
                // We'll select the target cell index, or the cell
                // before/after depending on the prev_char and direction
                let target = match (jump.prev_char, jump.forward) {
                    (false, true | false) => target,
                    (true, true) => target.saturating_sub(1),
                    (true, false) => target.saturating_add(1),
                };

                self.cursor.x = target;
                self.select_to_cursor_pos();
            }
        }
    }

    fn jump(&mut self, forward: bool, prev_char: bool) {
        self.pending_jump
            .replace(PendingJump { forward, prev_char });
    }

    fn jump_again(&mut self, reverse: bool) {
        if let Some(mut jump) = self.last_jump {
            if reverse {
                jump.forward = !jump.forward;
            }
            self.perform_jump(jump, true);
        }
    }

    fn set_selection_mode(&mut self, mode: &Option<SelectionMode>) {
        match mode {
            None => self.clear_selection_mode(),
            Some(mode) => {
                if self.start.is_none() {
                    let coord = SelectionCoordinate::x_y(self.cursor.x, self.cursor.y);
                    self.start.replace(coord);
                } else if self.selection_mode == *mode {
                    // We have a selection and we're trying to set the same mode
                    // again; consider this to be a toggle that clears the selection
                    self.clear_selection_mode();
                    return;
                }
                self.selection_mode = *mode;
                self.select_to_cursor_pos();
            }
        }
    }

    fn clear_selection_mode(&mut self) {
        self.start.take();
        self.clear_selection();
    }
}

impl Pane for CopyOverlay {
    fn pane_id(&self) -> PaneId {
        self.delegate.pane_id()
    }

    fn get_title(&self) -> String {
        format!("Copy mode: {}", self.delegate.get_title())
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        // paste into the search bar
        let mut r = self.render.lock();
        r.search_line.insert_text(text);
        r.schedule_update_search();
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(None)
    }

    fn writer(&self) -> MappedMutexGuard<dyn std::io::Write> {
        MutexGuard::map(self.writer.lock(), |writer| {
            let w: &mut dyn std::io::Write = writer;
            w
        })
    }

    fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
        self.delegate.resize(size)
    }

    fn key_up(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        let mut render = self.render.lock();
        let mods = mods.remove_positional_mods();
        if let Some(jump) = render.pending_jump.take() {
            match (key, mods) {
                (KeyCode::Char(c), KeyModifiers::NONE)
                | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    let jump = Jump {
                        forward: jump.forward,
                        prev_char: jump.prev_char,
                        target: c,
                    };
                    render.last_jump.replace(jump);
                    render.perform_jump(jump, false);
                }
                _ => {
                    self.delegate
                        .perform_actions(vec![termwiz::escape::Action::Control(
                            termwiz::escape::ControlCode::Bell,
                        )]);
                }
            }
            return Ok(());
        }

        if render.editing_search {
            match (key, mods) {
                (KeyCode::Char(c), KeyModifiers::NONE)
                | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    // Type to add to the pattern
                    render.search_line.insert_char(c);

                    render.schedule_update_search();
                }
                (KeyCode::Char('H'), KeyModifiers::CTRL)
                | (KeyCode::Backspace, KeyModifiers::NONE) => {
                    render
                        .search_line
                        .kill_text(Movement::BackwardChar(1), Movement::BackwardChar(1));

                    render.schedule_update_search();
                }
                (KeyCode::Delete, KeyModifiers::NONE) => {
                    render
                        .search_line
                        .kill_text(Movement::ForwardChar(1), Movement::None);

                    render.schedule_update_search();
                }
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Char('W'), KeyModifiers::CTRL) => {
                    render
                        .search_line
                        .kill_text(Movement::BackwardWord(1), Movement::BackwardWord(1));

                    render.schedule_update_search();
                }
                (KeyCode::Backspace, KeyModifiers::SUPER) => {
                    render
                        .search_line
                        .kill_text(Movement::StartOfLine, Movement::StartOfLine);

                    render.schedule_update_search();
                }
                (KeyCode::Char('K'), KeyModifiers::CTRL) => {
                    render
                        .search_line
                        .kill_text(Movement::EndOfLine, Movement::EndOfLine);

                    render.schedule_update_search();
                }
                (KeyCode::Char('B'), KeyModifiers::CTRL)
                | (KeyCode::ApplicationLeftArrow, KeyModifiers::NONE)
                | (KeyCode::LeftArrow, KeyModifiers::NONE) => {
                    render.search_line.exec_movement(Movement::BackwardChar(1));
                }
                (KeyCode::Char('F'), KeyModifiers::CTRL)
                | (KeyCode::ApplicationRightArrow, KeyModifiers::NONE)
                | (KeyCode::RightArrow, KeyModifiers::NONE) => {
                    render.search_line.exec_movement(Movement::ForwardChar(1));
                }
                (KeyCode::ApplicationLeftArrow, KeyModifiers::CTRL)
                | (KeyCode::LeftArrow, KeyModifiers::CTRL) => {
                    render.search_line.exec_movement(Movement::BackwardWord(1));
                }
                (KeyCode::ApplicationRightArrow, KeyModifiers::CTRL)
                | (KeyCode::RightArrow, KeyModifiers::CTRL) => {
                    render.search_line.exec_movement(Movement::ForwardWord(1));
                }
                (KeyCode::Char('A'), KeyModifiers::CTRL) | (KeyCode::Home, KeyModifiers::NONE) => {
                    render.search_line.exec_movement(Movement::StartOfLine);
                }
                (KeyCode::Char('E'), KeyModifiers::CTRL) | (KeyCode::End, KeyModifiers::NONE) => {
                    render.search_line.exec_movement(Movement::EndOfLine);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn perform_assignment(&self, assignment: &KeyAssignment) -> PerformAssignmentResult {
        use CopyModeAssignment::*;
        let mut render = self.render.lock();
        if render.pending_jump.is_some() {
            // Block key assignments until key_down is called
            // and resolves the next state
            return PerformAssignmentResult::BlockAssignmentAndRouteToKeyDown;
        }
        match assignment {
            KeyAssignment::CopyMode(assignment) => {
                match assignment {
                    MoveToViewportBottom => render.move_to_viewport_bottom(),
                    MoveToViewportTop => render.move_to_viewport_top(),
                    MoveToViewportMiddle => render.move_to_viewport_middle(),
                    MoveToScrollbackTop => render.move_to_top(),
                    MoveToScrollbackBottom => render.move_to_bottom(),
                    MoveToStartOfLineContent => render.move_to_start_of_line_content(),
                    MoveToEndOfLineContent => render.move_to_end_of_line_content(),
                    MoveToStartOfLine => render.move_to_start_of_line(),
                    MoveToStartOfNextLine => render.move_to_start_of_next_line(),
                    MoveToSelectionOtherEnd => render.move_to_selection_other_end(),
                    MoveToSelectionOtherEndHoriz => render.move_to_selection_other_end_horiz(),
                    MoveBackwardWord => render.move_backward_one_word(),
                    MoveForwardWord => render.move_forward_one_word(),
                    MoveForwardWordEnd => render.move_to_end_of_word(),
                    MoveRight => render.move_right_single_cell(),
                    MoveLeft => render.move_left_single_cell(),
                    MoveUp => render.move_up_single_row(),
                    MoveDown => render.move_down_single_row(),
                    MoveByPage(n) => render.move_by_page(**n),
                    PageUp => render.move_by_page(-1.0),
                    PageDown => render.move_by_page(1.0),
                    Close => render.close(),
                    PriorMatch => render.prior_match(),
                    NextMatch => render.next_match(),
                    PriorMatchPage => render.prior_match_page(),
                    NextMatchPage => render.next_match_page(),
                    CycleMatchType => render.cycle_match_type(),
                    ClearPattern => render.clear_pattern(),
                    EditPattern => render.edit_pattern(),
                    AcceptPattern => render.accept_pattern(),
                    SetSelectionMode(mode) => render.set_selection_mode(mode),
                    ClearSelectionMode => render.clear_selection_mode(),
                    MoveBackwardSemanticZone => render.move_by_zone(-1, None),
                    MoveForwardSemanticZone => render.move_by_zone(1, None),
                    MoveBackwardZoneOfType(zone_type) => render.move_by_zone(-1, Some(*zone_type)),
                    MoveForwardZoneOfType(zone_type) => render.move_by_zone(1, Some(*zone_type)),
                    JumpForward { prev_char } => render.jump(true, *prev_char),
                    JumpBackward { prev_char } => render.jump(false, *prev_char),
                    JumpAgain => render.jump_again(false),
                    JumpReverse => render.jump_again(true),
                }
                PerformAssignmentResult::Handled
            }
            _ => PerformAssignmentResult::Unhandled,
        }
    }

    fn mouse_event(&self, _event: MouseEvent) -> anyhow::Result<()> {
        anyhow::bail!("ignoring mouse while copying");
    }

    fn perform_actions(&self, actions: Vec<termwiz::escape::Action>) {
        self.delegate.perform_actions(actions)
    }

    fn is_dead(&self) -> bool {
        self.delegate.is_dead()
    }

    fn palette(&self) -> ColorPalette {
        self.delegate.palette()
    }

    fn domain_id(&self) -> DomainId {
        self.delegate.domain_id()
    }

    fn erase_scrollback(&self, erase_mode: ScrollbackEraseMode) {
        self.delegate.erase_scrollback(erase_mode)
    }

    fn is_mouse_grabbed(&self) -> bool {
        // Force grabbing off while we're searching
        false
    }

    fn is_alt_screen_active(&self) -> bool {
        false
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.delegate.set_clipboard(clipboard)
    }

    fn get_current_working_dir(&self, policy: CachePolicy) -> Option<Url> {
        self.delegate.get_current_working_dir(policy)
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        let renderer = self.render.lock();
        if renderer.editing_search {
            // place in the search box
            // Padding between the start of the editable line and the left side of the terminal
            const SEARCH_CURSOR_PADDING: usize = 8;
            let cursor = unicode_column_width(
                &renderer.search_line.get_line()[0..renderer.search_line.get_cursor()],
                None,
            );
            StableCursorPosition {
                x: SEARCH_CURSOR_PADDING + cursor,
                y: renderer.compute_search_row(),
                shape: termwiz::surface::CursorShape::SteadyBlock,
                visibility: termwiz::surface::CursorVisibility::Visible,
            }
        } else {
            renderer.cursor
        }
    }

    fn get_current_seqno(&self) -> SequenceNo {
        self.delegate.get_current_seqno()
    }

    fn get_changed_since(
        &self,
        lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> RangeSet<StableRowIndex> {
        self.delegate.get_changed_since(lines, seqno)
    }

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        self.delegate.get_logical_lines(lines)
    }

    fn for_each_logical_line_in_stable_range_mut(
        &self,
        lines: Range<StableRowIndex>,
        for_line: &mut dyn ForEachPaneLogicalLine,
    ) {
        self.delegate
            .for_each_logical_line_in_stable_range_mut(lines, for_line);
    }

    fn with_lines_mut(&self, lines: Range<StableRowIndex>, with_lines: &mut dyn WithPaneLines) {
        // Take care to access self.delegate methods here before we get into
        // calling into its own with_lines_mut to avoid a runtime
        // lock erro!
        let mut renderer = self.render.lock();
        if self.delegate.get_current_seqno() > renderer.last_result_seqno {
            renderer.update_search();
        }
        renderer.check_for_resize();
        let dims = self.get_dimensions();
        let search_row = renderer.compute_search_row();

        struct OverlayLines<'a> {
            with_lines: &'a mut dyn WithPaneLines,
            dims: RenderableDimensions,
            search_row: StableRowIndex,
            renderer: &'a mut CopyRenderable,
        }

        self.delegate.with_lines_mut(
            lines,
            &mut OverlayLines {
                with_lines,
                dims,
                search_row,
                renderer: &mut *renderer,
            },
        );

        impl<'a> WithPaneLines for OverlayLines<'a> {
            fn with_lines_mut(&mut self, first_row: StableRowIndex, lines: &mut [&mut Line]) {
                let mut overlay_lines = vec![];
                let config = config::configuration();
                let colors = &config.resolved_palette;

                for (idx, line) in lines.iter_mut().enumerate() {
                    let mut line: Line = line.clone();

                    let stable_idx = idx as StableRowIndex + first_row;
                    self.renderer.dirty_results.remove(stable_idx);
                    let pattern = self.renderer.get_pattern();
                    if stable_idx == self.search_row
                        && (self.renderer.editing_search || !pattern.is_empty())
                    {
                        // Replace with search UI
                        let rev = CellAttributes::default().set_reverse(true).clone();
                        line.fill_range(0..self.dims.cols, &Cell::new(' ', rev.clone()), SEQ_ZERO);
                        let mode = &match pattern {
                            Pattern::CaseSensitiveString(_) => "case-sensitive",
                            Pattern::CaseInSensitiveString(_) => "ignore-case",
                            Pattern::Regex(_) => "regex",
                        };

                        let remain = match &self.renderer.searching {
                            Some(Searching { remain, .. }) => {
                                format!(" searching {remain} lines")
                            }
                            None => String::new(),
                        };

                        line.overlay_text_with_attribute(
                            0,
                            &format!(
                                "Search: {} ({}/{} matches. {}{remain})",
                                *pattern,
                                self.renderer.result_pos.map(|x| x + 1).unwrap_or(0),
                                self.renderer.results.len(),
                                mode
                            ),
                            rev,
                            SEQ_ZERO,
                        );
                        self.renderer.last_bar_pos = Some(self.search_row);
                        line.clear_appdata();
                    } else if let Some(matches) = self.renderer.by_line.get(&stable_idx) {
                        for m in matches {
                            // highlight
                            for cell_idx in m.range.clone() {
                                if let Some(cell) =
                                    line.cells_mut_for_attr_changes_only().get_mut(cell_idx)
                                {
                                    if Some(m.result_index) == self.renderer.result_pos {
                                        cell.attrs_mut()
                                            .set_background(
                                                colors
                                                    .copy_mode_active_highlight_bg
                                                    .unwrap_or(AnsiColor::Yellow.into()),
                                            )
                                            .set_foreground(
                                                colors
                                                    .copy_mode_active_highlight_fg
                                                    .unwrap_or(AnsiColor::Black.into()),
                                            )
                                            .set_reverse(false);
                                    } else {
                                        cell.attrs_mut()
                                            .set_background(
                                                colors
                                                    .copy_mode_inactive_highlight_bg
                                                    .unwrap_or(AnsiColor::Fuchsia.into()),
                                            )
                                            .set_foreground(
                                                colors
                                                    .copy_mode_inactive_highlight_fg
                                                    .unwrap_or(AnsiColor::Black.into()),
                                            )
                                            .set_reverse(false);
                                    }
                                }
                            }
                        }
                        line.clear_appdata();
                    }
                    overlay_lines.push(line);
                }

                let mut overlay_refs: Vec<&mut Line> = overlay_lines.iter_mut().collect();
                self.with_lines.with_lines_mut(first_row, &mut overlay_refs);
            }
        }
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let mut renderer = self.render.lock();
        if self.delegate.get_current_seqno() > renderer.last_result_seqno {
            renderer.update_search();
        }

        renderer.check_for_resize();
        let dims = self.get_dimensions();

        let (top, mut lines) = self.delegate.get_lines(lines);

        let config = config::configuration();
        let colors = &config.resolved_palette;

        // Process the lines; for the search row we want to render instead
        // the search UI.
        // For rows with search results, we want to highlight the matching ranges
        let search_row = renderer.compute_search_row();
        for (idx, line) in lines.iter_mut().enumerate() {
            let stable_idx = idx as StableRowIndex + top;
            renderer.dirty_results.remove(stable_idx);
            let pattern = renderer.get_pattern();
            if stable_idx == search_row && (renderer.editing_search || !pattern.is_empty()) {
                // Replace with search UI
                let rev = CellAttributes::default().set_reverse(true).clone();
                line.fill_range(0..dims.cols, &Cell::new(' ', rev.clone()), SEQ_ZERO);
                let mode = &match pattern {
                    Pattern::CaseSensitiveString(_) => "case-sensitive",
                    Pattern::CaseInSensitiveString(_) => "ignore-case",
                    Pattern::Regex(_) => "regex",
                };
                line.overlay_text_with_attribute(
                    0,
                    &format!(
                        "Search: {} ({}/{} matches. {})",
                        *pattern,
                        renderer.result_pos.map(|x| x + 1).unwrap_or(0),
                        renderer.results.len(),
                        mode
                    ),
                    rev,
                    SEQ_ZERO,
                );
                renderer.last_bar_pos = Some(search_row);
            } else if let Some(matches) = renderer.by_line.get(&stable_idx) {
                for m in matches {
                    // highlight
                    for cell_idx in m.range.clone() {
                        if let Some(cell) = line.cells_mut_for_attr_changes_only().get_mut(cell_idx)
                        {
                            if Some(m.result_index) == renderer.result_pos {
                                cell.attrs_mut()
                                    .set_background(
                                        colors
                                            .copy_mode_active_highlight_bg
                                            .unwrap_or(AnsiColor::Yellow.into()),
                                    )
                                    .set_foreground(
                                        colors
                                            .copy_mode_active_highlight_fg
                                            .unwrap_or(AnsiColor::Black.into()),
                                    )
                                    .set_reverse(false);
                            } else {
                                cell.attrs_mut()
                                    .set_background(
                                        colors
                                            .copy_mode_inactive_highlight_bg
                                            .unwrap_or(AnsiColor::Fuchsia.into()),
                                    )
                                    .set_foreground(
                                        colors
                                            .copy_mode_inactive_highlight_fg
                                            .unwrap_or(AnsiColor::Black.into()),
                                    )
                                    .set_reverse(false);
                            }
                        }
                    }
                }
            }
        }

        (top, lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        self.delegate.get_dimensions()
    }
}

pub struct SearchOverlayPatternWriter {
    render: Arc<Mutex<CopyRenderable>>,
}

impl std::io::Write for SearchOverlayPatternWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut render = self.render.lock();
        let s = std::str::from_utf8(buf).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("invalid UTF-8: {err:#}"))
        })?;
        render.search_line.insert_text(s);
        render.schedule_update_search();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn is_whitespace_word(word: &str) -> bool {
    if let Some(c) = word.chars().next() {
        c.is_whitespace()
    } else {
        false
    }
}

pub fn search_key_table() -> KeyTable {
    let mut table = KeyTable::default();
    for (key, mods, action) in [
        (
            WKeyCode::Char('\x1b'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::Close),
        ),
        (
            WKeyCode::UpArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::PriorMatch),
        ),
        (
            WKeyCode::Char('\r'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::PriorMatch),
        ),
        (
            WKeyCode::Char('p'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::PriorMatch),
        ),
        (
            WKeyCode::PageUp,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::PriorMatchPage),
        ),
        (
            WKeyCode::PageDown,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::NextMatchPage),
        ),
        (
            WKeyCode::Char('n'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::NextMatch),
        ),
        (
            WKeyCode::DownArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::NextMatch),
        ),
        (
            WKeyCode::Char('r'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::CycleMatchType),
        ),
        (
            WKeyCode::Char('u'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::ClearPattern),
        ),
    ] {
        table.insert((key, mods), KeyTableEntry { action });
    }
    table
}

fn scroll_to_bottom_and_close() -> KeyAssignment {
    KeyAssignment::Multiple(vec![
        KeyAssignment::ScrollToBottom,
        KeyAssignment::CopyMode(CopyModeAssignment::Close),
    ])
}

pub fn copy_key_table() -> KeyTable {
    let mut table = KeyTable::default();
    for (key, mods, action) in [
        (
            WKeyCode::Char('c'),
            Modifiers::CTRL,
            scroll_to_bottom_and_close(),
        ),
        (
            WKeyCode::Char('g'),
            Modifiers::CTRL,
            scroll_to_bottom_and_close(),
        ),
        (
            WKeyCode::Char('q'),
            Modifiers::NONE,
            scroll_to_bottom_and_close(),
        ),
        (
            WKeyCode::Char('\x1b'),
            Modifiers::NONE,
            scroll_to_bottom_and_close(),
        ),
        (
            WKeyCode::Char('h'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveLeft),
        ),
        (
            WKeyCode::LeftArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveLeft),
        ),
        (
            WKeyCode::Char('j'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveDown),
        ),
        (
            WKeyCode::DownArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveDown),
        ),
        (
            WKeyCode::Char('k'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveUp),
        ),
        (
            WKeyCode::UpArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveUp),
        ),
        (
            WKeyCode::Char('l'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveRight),
        ),
        (
            WKeyCode::RightArrow,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveRight),
        ),
        (
            WKeyCode::RightArrow,
            Modifiers::ALT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveForwardWord),
        ),
        (
            WKeyCode::Char('f'),
            Modifiers::ALT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveForwardWord),
        ),
        (
            WKeyCode::Char('\t'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveForwardWord),
        ),
        (
            WKeyCode::Char('w'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveForwardWord),
        ),
        (
            WKeyCode::Char('e'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveForwardWordEnd),
        ),
        (
            WKeyCode::LeftArrow,
            Modifiers::ALT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveBackwardWord),
        ),
        (
            WKeyCode::Char('b'),
            Modifiers::ALT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveBackwardWord),
        ),
        (
            WKeyCode::Char('\t'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveBackwardWord),
        ),
        (
            WKeyCode::Char('b'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveBackwardWord),
        ),
        (
            WKeyCode::Char('0'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfLine),
        ),
        (
            WKeyCode::Char('\r'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfNextLine),
        ),
        (
            WKeyCode::Char('$'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToEndOfLineContent),
        ),
        (
            WKeyCode::Char('$'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToEndOfLineContent),
        ),
        (
            WKeyCode::Char('m'),
            Modifiers::ALT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfLineContent),
        ),
        (
            WKeyCode::Char('^'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfLineContent),
        ),
        (
            WKeyCode::Char('^'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfLineContent),
        ),
        (
            WKeyCode::Char(' '),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::SetSelectionMode(Some(
                SelectionMode::Cell,
            ))),
        ),
        (
            WKeyCode::Char('v'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::SetSelectionMode(Some(
                SelectionMode::Cell,
            ))),
        ),
        (
            WKeyCode::Char('V'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::SetSelectionMode(Some(
                SelectionMode::Line,
            ))),
        ),
        (
            WKeyCode::Char('V'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::SetSelectionMode(Some(
                SelectionMode::Line,
            ))),
        ),
        (
            WKeyCode::Char('v'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::SetSelectionMode(Some(
                SelectionMode::Block,
            ))),
        ),
        (
            WKeyCode::Char('G'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToScrollbackBottom),
        ),
        (
            WKeyCode::Char('G'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToScrollbackBottom),
        ),
        (
            WKeyCode::Char('g'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToScrollbackTop),
        ),
        (
            WKeyCode::Char('H'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportTop),
        ),
        (
            WKeyCode::Char('H'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportTop),
        ),
        (
            WKeyCode::Char('M'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportMiddle),
        ),
        (
            WKeyCode::Char('M'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportMiddle),
        ),
        (
            WKeyCode::Char('L'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportBottom),
        ),
        (
            WKeyCode::Char('L'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToViewportBottom),
        ),
        (
            WKeyCode::PageUp,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::PageUp),
        ),
        (
            WKeyCode::PageDown,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::PageDown),
        ),
        (
            WKeyCode::Char('b'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::PageUp),
        ),
        (
            WKeyCode::Char('f'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::PageDown),
        ),
        (
            WKeyCode::Char('u'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveByPage(NotNan::new(-0.5).unwrap())),
        ),
        (
            WKeyCode::Char('d'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveByPage(NotNan::new(0.5).unwrap())),
        ),
        (
            WKeyCode::Char('o'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToSelectionOtherEnd),
        ),
        (
            WKeyCode::Char('O'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToSelectionOtherEndHoriz),
        ),
        (
            WKeyCode::Char('O'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToSelectionOtherEndHoriz),
        ),
        (
            WKeyCode::Char('y'),
            Modifiers::NONE,
            KeyAssignment::Multiple(vec![
                KeyAssignment::CopyTo(ClipboardCopyDestination::ClipboardAndPrimarySelection),
                scroll_to_bottom_and_close(),
            ]),
        ),
        (
            WKeyCode::Char(';'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpAgain),
        ),
        (
            WKeyCode::Char(','),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpReverse),
        ),
        (
            WKeyCode::Char('F'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpBackward { prev_char: false }),
        ),
        (
            WKeyCode::Char('F'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpBackward { prev_char: false }),
        ),
        (
            WKeyCode::Char('T'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpBackward { prev_char: true }),
        ),
        (
            WKeyCode::Char('T'),
            Modifiers::SHIFT,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpBackward { prev_char: true }),
        ),
        (
            WKeyCode::Char('f'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpForward { prev_char: false }),
        ),
        (
            WKeyCode::Char('t'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::JumpForward { prev_char: true }),
        ),
        (
            WKeyCode::Home,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToStartOfLine),
        ),
        (
            WKeyCode::End,
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::MoveToEndOfLineContent),
        ),
    ] {
        table.insert((key, mods), KeyTableEntry { action });
    }
    table
}
