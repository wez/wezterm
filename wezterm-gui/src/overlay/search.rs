use crate::selection::{SelectionCoordinate, SelectionRange};
use crate::termwindow::TermWindow;
use config::keyassignment::ScrollbackEraseMode;
use mux::domain::DomainId;
use mux::pane::{Pane, PaneId, Pattern, SearchResult};
use mux::renderable::*;
use portable_pty::PtySize;
use rangeset::RangeSet;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::AnsiColor;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex};
use window::WindowOps;

pub struct SearchOverlay {
    renderer: RefCell<SearchRenderable>,
    delegate: Rc<dyn Pane>,
}

#[derive(Debug)]
struct MatchResult {
    range: Range<usize>,
    result_index: usize,
}

struct SearchRenderable {
    delegate: Rc<dyn Pane>,
    /// The text that the user entered
    pattern: Pattern,
    /// The most recently queried set of matches
    results: Vec<SearchResult>,
    by_line: HashMap<StableRowIndex, Vec<MatchResult>>,

    viewport: Option<StableRowIndex>,
    last_bar_pos: Option<StableRowIndex>,

    dirty_results: RangeSet<StableRowIndex>,
    result_pos: Option<usize>,
    width: usize,
    height: usize,

    /// We use this to cancel ourselves later
    window: ::window::Window,
}

impl SearchOverlay {
    pub fn with_pane(
        term_window: &TermWindow,
        pane: &Rc<dyn Pane>,
        pattern: Pattern,
    ) -> Rc<dyn Pane> {
        let viewport = term_window.get_viewport(pane.pane_id());
        let dims = pane.get_dimensions();

        let window = term_window.window.clone().unwrap();
        let mut renderer = SearchRenderable {
            delegate: Rc::clone(pane),
            pattern,
            results: vec![],
            by_line: HashMap::new(),
            dirty_results: RangeSet::default(),
            viewport,
            last_bar_pos: None,
            window,
            result_pos: None,
            width: dims.cols,
            height: dims.viewport_rows,
        };

        let search_row = renderer.compute_search_row();
        renderer.dirty_results.add(search_row);
        renderer.update_search();

        Rc::new(SearchOverlay {
            renderer: RefCell::new(renderer),
            delegate: Rc::clone(pane),
        })
    }

    pub fn viewport_changed(&self, viewport: Option<StableRowIndex>) {
        let mut render = self.renderer.borrow_mut();
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

impl Pane for SearchOverlay {
    fn pane_id(&self) -> PaneId {
        self.delegate.pane_id()
    }

    fn get_title(&self) -> String {
        self.delegate.get_title()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        // paste into the search bar
        let mut r = self.renderer.borrow_mut();
        r.pattern.push_str(text);
        r.update_search();
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        panic!("do not call reader on SearchOverlay bar tab instance");
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.delegate.writer()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.delegate.resize(size)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        match (key, mods) {
            (KeyCode::Escape, KeyModifiers::NONE) => self.renderer.borrow().close(),
            (KeyCode::UpArrow, KeyModifiers::NONE)
            | (KeyCode::Enter, KeyModifiers::NONE)
            | (KeyCode::Char('p'), KeyModifiers::CTRL) => {
                // Move to prior match
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos.as_ref() {
                    let prior = if *cur > 0 {
                        cur - 1
                    } else {
                        r.results.len() - 1
                    };
                    r.activate_match_number(prior);
                }
            }
            (KeyCode::PageUp, KeyModifiers::NONE) => {
                // Skip this page of matches and move up to the first match from
                // the prior page.
                let dims = self.delegate.get_dimensions();
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos {
                    let top = r.viewport.unwrap_or(dims.physical_top);
                    let prior = top - dims.viewport_rows as isize;
                    if let Some(pos) = r
                        .results
                        .iter()
                        .position(|res| res.start_y > prior && res.start_y < top)
                    {
                        r.activate_match_number(pos);
                    } else {
                        r.activate_match_number(cur.saturating_sub(1));
                    }
                }
            }
            (KeyCode::PageDown, KeyModifiers::NONE) => {
                // Skip this page of matches and move down to the first match from
                // the next page.
                let dims = self.delegate.get_dimensions();
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos {
                    let top = r.viewport.unwrap_or(dims.physical_top);
                    let bottom = top + dims.viewport_rows as isize;
                    if let Some(pos) = r.results.iter().position(|res| res.start_y >= bottom) {
                        r.activate_match_number(pos);
                    } else {
                        let len = r.results.len().saturating_sub(1);
                        r.activate_match_number(cur.min(len));
                    }
                }
            }
            (KeyCode::DownArrow, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CTRL) => {
                // Move to next match
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos.as_ref() {
                    let next = if *cur + 1 >= r.results.len() {
                        0
                    } else {
                        *cur + 1
                    };
                    r.activate_match_number(next);
                }
            }
            (KeyCode::Char('r'), KeyModifiers::CTRL) => {
                // CTRL-r cycles through pattern match types
                let mut r = self.renderer.borrow_mut();
                let pattern = match &r.pattern {
                    Pattern::CaseSensitiveString(s) => Pattern::CaseInSensitiveString(s.clone()),
                    Pattern::CaseInSensitiveString(s) => Pattern::Regex(s.clone()),
                    Pattern::Regex(s) => Pattern::CaseSensitiveString(s.clone()),
                };
                r.pattern = pattern;
                r.update_search();
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                // Type to add to the pattern
                let mut r = self.renderer.borrow_mut();
                r.pattern.push(c);
                r.update_search();
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                // Backspace to edit the pattern
                let mut r = self.renderer.borrow_mut();
                r.pattern.pop();
                r.update_search();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                // CTRL-u to clear the pattern
                let mut r = self.renderer.borrow_mut();
                r.pattern.clear();
                r.update_search();
            }
            _ => {}
        }
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()> {
        self.delegate.mouse_event(event)
    }

    fn advance_bytes(&self, buf: &[u8]) {
        self.delegate.advance_bytes(buf)
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

    fn get_current_working_dir(&self) -> Option<Url> {
        self.delegate.get_current_working_dir()
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        // move to the search box
        let renderer = self.renderer.borrow();
        StableCursorPosition {
            x: 8 + wezterm_term::unicode_column_width(&renderer.pattern),
            y: renderer.compute_search_row(),
            shape: termwiz::surface::CursorShape::SteadyBlock,
            visibility: termwiz::surface::CursorVisibility::Visible,
        }
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        let mut dirty = self.delegate.get_dirty_lines(lines.clone());
        dirty.add_set(&self.renderer.borrow().dirty_results);
        dirty.intersection_with_range(lines)
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let mut renderer = self.renderer.borrow_mut();
        renderer.check_for_resize();
        let dims = self.get_dimensions();

        let (top, mut lines) = self.delegate.get_lines(lines);

        // Process the lines; for the search row we want to render instead
        // the search UI.
        // For rows with search results, we want to highlight the matching ranges
        let search_row = renderer.compute_search_row();
        for (idx, line) in lines.iter_mut().enumerate() {
            let stable_idx = idx as StableRowIndex + top;
            renderer.dirty_results.remove(stable_idx);
            if stable_idx == search_row {
                // Replace with search UI
                let rev = CellAttributes::default().set_reverse(true).clone();
                line.fill_range(0..dims.cols, &Cell::new(' ', rev.clone()));
                let mode = &match renderer.pattern {
                    Pattern::CaseSensitiveString(_) => "case-sensitive",
                    Pattern::CaseInSensitiveString(_) => "ignore-case",
                    Pattern::Regex(_) => "regex",
                };
                line.overlay_text_with_attribute(
                    0,
                    &format!(
                        "Search: {} ({}/{} matches. {})",
                        *renderer.pattern,
                        renderer.result_pos.map(|x| x + 1).unwrap_or(0),
                        renderer.results.len(),
                        mode
                    ),
                    rev,
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
                                    .set_background(AnsiColor::Yellow)
                                    .set_foreground(AnsiColor::Black)
                                    .set_reverse(false);
                            } else {
                                cell.attrs_mut()
                                    .set_background(AnsiColor::Fuschia)
                                    .set_foreground(AnsiColor::Black)
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

impl SearchRenderable {
    fn compute_search_row(&self) -> StableRowIndex {
        let dims = self.delegate.get_dimensions();
        let top = self.viewport.unwrap_or_else(|| dims.physical_top);
        let bottom = (top + dims.viewport_rows as StableRowIndex).saturating_sub(1);
        bottom
    }

    fn close(&self) {
        TermWindow::schedule_cancel_overlay_for_pane(self.window.clone(), self.delegate.pane_id());
    }

    fn set_viewport(&self, row: Option<StableRowIndex>) {
        let dims = self.delegate.get_dimensions();
        let pane_id = self.delegate.pane_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                term_window.set_viewport(pane_id, row, dims);
            }
            Ok(())
        });
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

    fn recompute_results(&mut self) {
        for (result_index, res) in self.results.iter().enumerate() {
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

        let bar_pos = self.compute_search_row();
        self.dirty_results.add(bar_pos);

        if !self.pattern.is_empty() {
            let pane: Rc<dyn Pane> = self.delegate.clone();
            let window = self.window.clone();
            let pattern = self.pattern.clone();
            promise::spawn::spawn(async move {
                let mut results = pane.search(pattern).await?;
                results.sort();

                let pane_id = pane.pane_id();
                let mut results = Some(results);
                window.apply(move |term_window, _window| {
                    let term_window = term_window
                        .downcast_mut::<TermWindow>()
                        .expect("to be TermWindow");
                    let state = term_window.pane_state(pane_id);
                    if let Some(overlay) = state.overlay.as_ref() {
                        if let Some(search_overlay) = overlay.downcast_ref::<SearchOverlay>() {
                            let mut r = search_overlay.renderer.borrow_mut();
                            r.results = results.take().unwrap();
                            r.recompute_results();
                            let num_results = r.results.len();

                            if !r.results.is_empty() {
                                r.activate_match_number(num_results - 1);
                            } else {
                                r.set_viewport(None);
                                r.clear_selection();
                            }
                        }
                    }
                    Ok(())
                });
                anyhow::Result::<()>::Ok(())
            })
            .detach();
        } else {
            self.set_viewport(None);
            self.clear_selection();
        }
    }

    fn clear_selection(&mut self) {
        let pane_id = self.delegate.pane_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                let mut selection = term_window.selection(pane_id);
                selection.start.take();
                selection.range.take();
            }
            Ok(())
        });
    }

    fn activate_match_number(&mut self, n: usize) {
        self.result_pos.replace(n);
        let result = self.results[n].clone();

        let pane_id = self.delegate.pane_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                let mut selection = term_window.selection(pane_id);
                let start = SelectionCoordinate {
                    x: result.start_x,
                    y: result.start_y,
                };
                selection.start = Some(start);
                selection.range = Some(SelectionRange {
                    start,
                    end: SelectionCoordinate {
                        // inclusive range for selection, but the result
                        // range is exclusive
                        x: result.end_x.saturating_sub(1),
                        y: result.end_y,
                    },
                });
            }
            Ok(())
        });

        self.set_viewport(Some(result.start_y));
    }
}
