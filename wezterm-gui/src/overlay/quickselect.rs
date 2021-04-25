use crate::selection::{SelectionCoordinate, SelectionRange};
use crate::termwindow::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ScrollbackEraseMode};
use config::ConfigHandle;
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

const PATTERNS: [&str; 14] = [
    // markdown_url
    r"\[[^]]*\]\(([^)]+)\)",
    // url
    r"((https?://|git@|git://|ssh://|ftp://|file:///)[^ ]+)",
    // diff_a
    r"--- a/([^ ]+)",
    // diff_b
    r"\+\+\+ b/([^ ]+)",
    // docker
    r"sha256:([0-9a-f]{64})",
    // path
    r"(([.\w\-@~]+)?(/[.\w\-@]+)+)",
    // color
    r"#[0-9a-fA-F]{6}",
    // uid
    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
    // ipfs
    r"Qm[0-9a-zA-Z]{44}",
    // sha
    r"[0-9a-f]{7,40}",
    // ip
    r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}",
    // ipv6
    r"[A-f0-9:]+:+[A-f0-9:]+[%\w\d]+",
    // address
    r"0x[0-9a-fA-F]+",
    // number
    r"[0-9]{4,}",
];

/// This function computes a set of labels for a given alphabet.
/// It is derived from https://github.com/fcsonline/tmux-thumbs/blob/master/src/alphabets.rs
/// which is Copyright (c) 2019 Ferran Basora and provided under the MIT license
fn compute_labels_for_alphabet(alphabet: &str, num_matches: usize) -> Vec<String> {
    let alphabet = alphabet
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<String>>();
    // Prefer to use single character matches to represent everything
    let mut primary = alphabet.clone();
    let mut secondary = vec![];

    loop {
        if primary.len() + secondary.len() >= num_matches {
            break;
        }

        // We have more matches than can be represented by alphabet,
        // so steal one of the single character options from the end
        // of the alphabet and use it to generate a two character
        // label
        let prefix = match primary.pop() {
            Some(p) => p,
            None => break,
        };

        // Generate a two character label for each of the alphabet
        // characters.  This ignores later alphabet characters;
        // since we popped our prefix from the end of alphabet,
        // length limiting this iteration ensures that we don't
        // end up with a duplicate letters in the result.
        let prefixed: Vec<String> = alphabet
            .iter()
            .take(num_matches - primary.len() - secondary.len())
            .map(|s| format!("{}{}", prefix, s))
            .collect();

        secondary.splice(0..0, prefixed);
    }

    let len = secondary.len();

    primary
        .drain(0..)
        .take(num_matches - len)
        .chain(secondary.drain(0..))
        .collect()
}

#[cfg(test)]
mod alphabet_test {
    use super::*;

    #[test]
    fn simple_alphabet() {
        assert_eq!(compute_labels_for_alphabet("abcd", 3), vec!["a", "b", "c"]);
    }

    #[test]
    fn composed_single() {
        assert_eq!(
            compute_labels_for_alphabet("abcd", 6),
            vec!["a", "b", "c", "da", "db", "dc"]
        );
    }

    #[test]
    fn composed_multiple() {
        assert_eq!(
            compute_labels_for_alphabet("abcd", 8),
            vec!["a", "b", "ca", "cb", "da", "db", "dc", "dd"]
        );
    }

    #[test]
    fn composed_max() {
        // The number of chars in the alphabet limits the potential matches to fewer
        // than the number of matches that we requested
        assert_eq!(
            compute_labels_for_alphabet("ab", 5),
            vec!["aa", "ab", "ba", "bb"]
        );
    }
}

pub struct QuickSelectOverlay {
    renderer: RefCell<QuickSelectRenderable>,
    delegate: Rc<dyn Pane>,
}

#[derive(Debug)]
struct MatchResult {
    range: Range<usize>,
    result_index: usize,
    label: String,
}

struct QuickSelectRenderable {
    delegate: Rc<dyn Pane>,
    /// The text that the user entered
    pattern: Pattern,
    /// The most recently queried set of matches
    results: Vec<SearchResult>,
    by_line: HashMap<StableRowIndex, Vec<MatchResult>>,
    by_label: HashMap<String, usize>,
    selection: String,

    viewport: Option<StableRowIndex>,
    last_bar_pos: Option<StableRowIndex>,

    dirty_results: RangeSet<StableRowIndex>,
    result_pos: Option<usize>,
    width: usize,
    height: usize,

    /// We use this to cancel ourselves later
    window: ::window::Window,

    config: ConfigHandle,
}

impl QuickSelectOverlay {
    pub fn with_pane(term_window: &TermWindow, pane: &Rc<dyn Pane>) -> Rc<dyn Pane> {
        let viewport = term_window.get_viewport(pane.pane_id());
        let dims = pane.get_dimensions();

        let config = term_window.config.clone();

        let mut pattern = "(".to_string();
        if !config.disable_default_quick_select_patterns {
            for p in &PATTERNS {
                if pattern.len() > 1 {
                    pattern.push('|');
                }
                pattern.push_str(p);
            }
        }
        for p in &config.quick_select_patterns {
            if pattern.len() > 1 {
                pattern.push('|');
            }
            pattern.push_str(p);
        }
        pattern.push(')');

        let pattern = Pattern::Regex(pattern);

        let window = term_window.window.clone().unwrap();
        let mut renderer = QuickSelectRenderable {
            delegate: Rc::clone(pane),
            pattern,
            selection: "".to_string(),
            results: vec![],
            by_line: HashMap::new(),
            by_label: HashMap::new(),
            dirty_results: RangeSet::default(),
            viewport,
            last_bar_pos: None,
            window,
            result_pos: None,
            width: dims.cols,
            height: dims.viewport_rows,
            config,
        };

        let search_row = renderer.compute_search_row();
        renderer.dirty_results.add(search_row);
        renderer.update_search();

        Rc::new(QuickSelectOverlay {
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

impl Pane for QuickSelectOverlay {
    fn pane_id(&self) -> PaneId {
        self.delegate.pane_id()
    }

    fn get_title(&self) -> String {
        self.delegate.get_title()
    }

    fn send_paste(&self, _text: &str) -> anyhow::Result<()> {
        // Ignore
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        panic!("do not call reader on QuickSelectOverlay bar tab instance");
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
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                // Type to add to the selection
                let mut r = self.renderer.borrow_mut();
                r.selection.push(c);
                if let Some(result_index) = r.by_label.get(&r.selection).cloned() {
                    r.select_and_copy_match_number(result_index);
                    r.close();
                }
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                // Backspace to edit the selection
                let mut r = self.renderer.borrow_mut();
                r.selection.pop();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                // CTRL-u to clear the selection
                let mut r = self.renderer.borrow_mut();
                r.selection.clear();
            }
            _ => {}
        }
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()> {
        self.delegate.mouse_event(event)
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

    fn get_current_working_dir(&self) -> Option<Url> {
        self.delegate.get_current_working_dir()
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        // move to the search box
        let renderer = self.renderer.borrow();
        StableCursorPosition {
            x: 8 + wezterm_term::unicode_column_width(&renderer.selection),
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
                line.overlay_text_with_attribute(
                    0,
                    &format!(
                        "Select: {}  (type highlighted prefix to copy, ESC to cancel)",
                        renderer.selection,
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
                            cell.attrs_mut()
                                .set_background(AnsiColor::Yellow)
                                .set_foreground(AnsiColor::Black)
                                .set_reverse(false);
                        }
                    }
                    for (idx, c) in m.label.chars().enumerate() {
                        let mut attr = line.cells()[idx].attrs().clone();
                        attr.set_background(AnsiColor::Fuschia)
                            .set_foreground(AnsiColor::Black)
                            .set_reverse(false);
                        line.set_cell(m.range.start + idx, Cell::new(c, attr));
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

impl QuickSelectRenderable {
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
        let num_results = self.results.len();
        let labels = compute_labels_for_alphabet(&self.config.quick_select_alphabet, num_results);
        self.by_label.clear();

        for ((result_index, res), label) in self
            .results
            .iter()
            .enumerate()
            .rev()
            .take(labels.len())
            .zip(labels.into_iter())
        {
            self.by_label.insert(label.clone(), result_index);
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
                    label: label.clone(),
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
                        if let Some(search_overlay) = overlay.downcast_ref::<QuickSelectOverlay>() {
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

    fn select_and_copy_match_number(&mut self, n: usize) {
        let result = self.results[n].clone();

        let pane_id = self.delegate.pane_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                {
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

                let mux = mux::Mux::get().unwrap();
                if let Some(pane) = mux.get_pane(pane_id) {
                    let text = term_window.selection_text(&pane);
                    if !text.is_empty() {
                        term_window.copy_to_clipboard(
                            ClipboardCopyDestination::ClipboardAndPrimarySelection,
                            text,
                        );
                    }
                }
            }
            Ok(())
        });
    }

    fn activate_match_number(&mut self, n: usize) {
        self.result_pos.replace(n);
        let result = self.results[n].clone();
        self.set_viewport(Some(result.start_y));
    }
}
