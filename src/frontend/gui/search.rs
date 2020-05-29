use crate::frontend::gui::termwindow::TermWindow;
use crate::mux::domain::DomainId;
use crate::mux::renderable::*;
use crate::mux::tab::{Pattern, SearchResult};
use crate::mux::tab::{Tab, TabId};
use portable_pty::PtySize;
use rangeset::RangeSet;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex, TerminalHost};
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::AnsiColor;
use url::Url;
use window::WindowOps;

pub struct SearchOverlay {
    renderer: RefCell<SearchRenderable>,
    delegate: Rc<dyn Tab>,
}

#[derive(Debug)]
struct MatchResult {
    range: Range<usize>,
    result_index: usize,
}

struct SearchRenderable {
    delegate: Rc<dyn Tab>,
    /// The text that the user entered
    pattern: String,
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
    pub fn with_tab(term_window: &TermWindow, tab: &Rc<dyn Tab>) -> Rc<dyn Tab> {
        let viewport = term_window.get_viewport(tab.tab_id());
        let dims = tab.renderer().get_dimensions();

        let window = term_window.window.clone().unwrap();
        let mut renderer = SearchRenderable {
            delegate: Rc::clone(tab),
            pattern: String::new(),
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

        Rc::new(SearchOverlay {
            renderer: RefCell::new(renderer),
            delegate: Rc::clone(tab),
        })
    }

    pub fn viewport_changed(&self, viewport: Option<StableRowIndex>) {
        let mut render = self.renderer.borrow_mut();
        if let Some(last) = render.last_bar_pos.take() {
            render.dirty_results.add(last);
        }
        if let Some(pos) = viewport.as_ref() {
            render.dirty_results.add(*pos);
        }
        render.viewport = viewport;
    }
}

impl Tab for SearchOverlay {
    fn tab_id(&self) -> TabId {
        self.delegate.tab_id()
    }

    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.renderer.borrow_mut()
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
            (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('p'), KeyModifiers::CTRL) => {
                // Move to prior match
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos.as_ref() {
                    let prior = if *cur > 0 {
                        cur - 1
                    } else {
                        r.results.len() - 1
                    };
                    r.result_pos.replace(prior);
                    r.set_viewport(Some(r.results[prior].start_y));
                }
            }
            (KeyCode::Char('n'), KeyModifiers::CTRL) => {
                // Move to next match
                let mut r = self.renderer.borrow_mut();
                if let Some(cur) = r.result_pos.as_ref() {
                    let next = if *cur + 1 >= r.results.len() {
                        0
                    } else {
                        *cur + 1
                    };
                    r.result_pos.replace(next);
                    r.set_viewport(Some(r.results[next].start_y));
                }
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
            _ => {}
        }
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> anyhow::Result<()> {
        self.delegate.mouse_event(event, host)
    }

    fn advance_bytes(&self, buf: &[u8], host: &mut dyn TerminalHost) {
        self.delegate.advance_bytes(buf, host)
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

    fn erase_scrollback(&self) {
        self.delegate.erase_scrollback()
    }

    fn search(&self, _pattern: &Pattern) -> Vec<SearchResult> {
        // You can't search the search bar
        vec![]
    }

    fn is_mouse_grabbed(&self) -> bool {
        // Force grabbing off while we're searching
        false
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.delegate.set_clipboard(clipboard)
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.delegate.get_current_working_dir()
    }
}

impl SearchRenderable {
    fn compute_search_row(&self) -> StableRowIndex {
        let dims = self.delegate.renderer().get_dimensions();
        let top = self.viewport.unwrap_or_else(|| dims.physical_top);
        let bottom = (top + dims.viewport_rows as StableRowIndex).saturating_sub(1);
        bottom
    }

    fn close(&self) {
        TermWindow::schedule_cancel_overlay(self.window.clone(), self.delegate.tab_id());
    }

    fn set_viewport(&self, row: Option<StableRowIndex>) {
        let dims = self.delegate.renderer().get_dimensions();
        let tab_id = self.delegate.tab_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                term_window.set_viewport(tab_id, row, dims);
            }
            Ok(())
        });
    }

    fn check_for_resize(&mut self) {
        let dims = self.delegate.renderer().get_dimensions();
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
            self.results = self.delegate.search(&Pattern::String(self.pattern.clone()));
            self.results.sort();

            self.recompute_results();
        }
        if let Some(last) = self.results.last() {
            self.result_pos.replace(self.results.len() - 1);
            self.set_viewport(Some(last.start_y));
        } else {
            self.set_viewport(None);
        }
    }
}

impl Renderable for SearchRenderable {
    fn get_cursor_position(&self) -> StableCursorPosition {
        // move to the search box
        StableCursorPosition {
            x: 8 + self.pattern.len(), // FIXME: ucwidth
            y: self.compute_search_row(),
            shape: termwiz::surface::CursorShape::SteadyBlock,
        }
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        let mut dirty = self.delegate.renderer().get_dirty_lines(lines.clone());
        dirty.add_set(&self.dirty_results);
        dirty.intersection_with_range(lines)
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        self.check_for_resize();
        let dims = self.get_dimensions();

        let (top, mut lines) = self.delegate.renderer().get_lines(lines);

        // Process the lines; for the search row we want to render instead
        // the search UI.
        // For rows with search results, we want to highlight the matching ranges
        let search_row = self.compute_search_row();
        for (idx, line) in lines.iter_mut().enumerate() {
            let stable_idx = idx as StableRowIndex + top;
            self.dirty_results.remove(stable_idx);
            if stable_idx == search_row {
                // Replace with search UI
                let rev = CellAttributes::default().set_reverse(true).clone();
                line.fill_range(0..dims.cols, &Cell::new(' ', rev.clone()));
                line.overlay_text_with_attribute(
                    0,
                    &format!(
                        "Search: {} ({}/{} matches)",
                        self.pattern,
                        self.result_pos.map(|x| x + 1).unwrap_or(0),
                        self.results.len()
                    ),
                    rev,
                );
                self.last_bar_pos = Some(search_row);
            } else if let Some(matches) = self.by_line.get(&stable_idx) {
                for m in matches {
                    // highlight
                    for cell_idx in m.range.clone() {
                        if let Some(cell) = line.cells_mut_for_attr_changes_only().get_mut(cell_idx)
                        {
                            if Some(m.result_index) == self.result_pos {
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
        self.delegate.renderer().get_dimensions()
    }
}
