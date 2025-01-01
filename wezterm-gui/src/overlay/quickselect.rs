use crate::selection::{SelectionCoordinate, SelectionRange};
use crate::termwindow::{TermWindow, TermWindowNotif};
use config::keyassignment::{ClipboardCopyDestination, QuickSelectArguments, ScrollbackEraseMode};
use config::ConfigHandle;
use mux::domain::DomainId;
use mux::pane::{
    CachePolicy, ForEachPaneLogicalLine, LogicalLine, Pane, PaneId, Pattern, SearchResult,
    WithPaneLines,
};
use mux::renderable::*;
use parking_lot::{MappedMutexGuard, Mutex};
use rangeset::RangeSet;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::AnsiColor;
use termwiz::surface::{SequenceNo, SEQ_ZERO};
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex, TerminalSize,
};
use window::WindowOps;

const PATTERNS: [&str; 14] = [
    // markdown_url
    r"\[[^]]*\]\(([^)]+)\)",
    // url
    r"(?:https?://|git@|git://|ssh://|ftp://|file://)\S+",
    // diff_a
    r"--- a/(\S+)",
    // diff_b
    r"\+\+\+ b/(\S+)",
    // docker
    r"sha256:([0-9a-f]{64})",
    // path
    r"(?:[.\w\-@~]+)?(?:/+[.\w\-@]+)+",
    // color
    r"#[0-9a-fA-F]{6}",
    // uuid
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
pub fn compute_labels_for_alphabet(alphabet: &str, num_matches: usize) -> Vec<String> {
    compute_labels_for_alphabet_impl(alphabet, num_matches, true)
}

pub fn compute_labels_for_alphabet_with_preserved_case(
    alphabet: &str,
    num_matches: usize,
) -> Vec<String> {
    compute_labels_for_alphabet_impl(alphabet, num_matches, false)
}

fn compute_labels_for_alphabet_impl(
    alphabet: &str,
    num_matches: usize,
    make_lowercase: bool,
) -> Vec<String> {
    let alphabet = if make_lowercase {
        alphabet
            .chars()
            .map(|c| c.to_lowercase().to_string())
            .collect::<Vec<String>>()
    } else {
        alphabet
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<String>>()
    };
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
    fn more_matches_than_alphabet_can_represent() {
        assert_eq!(
            compute_labels_for_alphabet("asdfqwerzxcvjklmiuopghtybn", 792).len(),
            676
        );
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

    #[test]
    fn composed_capital() {
        assert_eq!(
            compute_labels_for_alphabet_with_preserved_case("AB", 4),
            vec!["AA", "AB", "BA", "BB"]
        );
    }

    #[test]
    fn composed_mixed() {
        assert_eq!(
            compute_labels_for_alphabet_with_preserved_case("aA", 4),
            vec!["aa", "aA", "Aa", "AA"]
        );
    }

    #[test]
    fn lowercase_alphabet_equal() {
        assert_eq!(
            compute_labels_for_alphabet_with_preserved_case("abc123", 12),
            compute_labels_for_alphabet("abc123", 12)
        );
    }
}

pub struct QuickSelectOverlay {
    renderer: Mutex<QuickSelectRenderable>,
    delegate: Arc<dyn Pane>,
}

#[derive(Debug)]
struct MatchResult {
    range: Range<usize>,
    label: String,
}

struct QuickSelectRenderable {
    delegate: Arc<dyn Pane>,
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
    args: QuickSelectArguments,
}

impl QuickSelectOverlay {
    pub fn with_pane(
        term_window: &TermWindow,
        pane: &Arc<dyn Pane>,
        args: &QuickSelectArguments,
    ) -> Arc<dyn Pane> {
        let viewport = term_window.get_viewport(pane.pane_id());
        let dims = pane.get_dimensions();

        let config = term_window.config.clone();

        let mut pattern = "(?m)(".to_string();
        let mut have_patterns = false;
        if !args.patterns.is_empty() {
            for p in &args.patterns {
                if have_patterns {
                    pattern.push('|');
                }
                pattern.push_str(p);
                have_patterns = true;
            }
        } else {
            // User-provided patterns take precedence over built-ins
            for p in &config.quick_select_patterns {
                if have_patterns {
                    pattern.push('|');
                }
                pattern.push_str(p);
                have_patterns = true;
            }
            if !config.disable_default_quick_select_patterns {
                for p in &PATTERNS {
                    if have_patterns {
                        pattern.push('|');
                    }
                    pattern.push_str(p);
                    have_patterns = true;
                }
            }
        }
        pattern.push(')');

        let pattern = Pattern::Regex(pattern);

        let window = term_window.window.clone().unwrap();
        let mut renderer = QuickSelectRenderable {
            delegate: Arc::clone(pane),
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
            args: args.clone(),
        };

        let search_row = renderer.compute_search_row();
        renderer.dirty_results.add(search_row);
        renderer.update_search(true);

        Arc::new(QuickSelectOverlay {
            renderer: Mutex::new(renderer),
            delegate: Arc::clone(pane),
        })
    }

    pub fn viewport_changed(&self, viewport: Option<StableRowIndex>) {
        let mut render = self.renderer.lock();
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

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(None)
    }

    fn writer(&self) -> MappedMutexGuard<dyn std::io::Write> {
        self.delegate.writer()
    }

    fn resize(&self, size: TerminalSize) -> anyhow::Result<()> {
        self.delegate.resize(size)
    }

    fn key_up(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        let mods = mods.remove_positional_mods();
        match (key, mods) {
            (KeyCode::Escape, KeyModifiers::NONE) => self.renderer.lock().close(),
            (KeyCode::UpArrow, KeyModifiers::NONE)
            | (KeyCode::Enter, KeyModifiers::NONE)
            | (KeyCode::Char('p'), KeyModifiers::CTRL) => {
                // Move to prior match
                let mut r = self.renderer.lock();
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
                let mut r = self.renderer.lock();
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
                let mut r = self.renderer.lock();
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
                let mut r = self.renderer.lock();
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
                let mut r = self.renderer.lock();
                r.selection.push(c);
                let lowered = r.selection.to_lowercase();
                let paste = lowered != r.selection;
                if let Some(result_index) = r.by_label.get(&lowered).cloned() {
                    r.select_and_copy_match_number(result_index, paste);
                    r.close();
                }
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                // Backspace to edit the selection
                let mut r = self.renderer.lock();
                r.selection.pop();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                // CTRL-u to clear the selection
                let mut r = self.renderer.lock();
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

    fn get_current_working_dir(&self, policy: CachePolicy) -> Option<Url> {
        self.delegate.get_current_working_dir(policy)
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        // move to the search box
        let renderer = self.renderer.lock();
        StableCursorPosition {
            x: 8 + wezterm_term::unicode_column_width(&renderer.selection, None),
            y: renderer.compute_search_row(),
            shape: termwiz::surface::CursorShape::SteadyBlock,
            visibility: termwiz::surface::CursorVisibility::Visible,
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
        let mut dirty = self.delegate.get_changed_since(lines.clone(), seqno);
        dirty.add_set(&self.renderer.lock().dirty_results);
        dirty.intersection_with_range(lines)
    }

    fn for_each_logical_line_in_stable_range_mut(
        &self,
        lines: Range<StableRowIndex>,
        for_line: &mut dyn ForEachPaneLogicalLine,
    ) {
        self.delegate
            .for_each_logical_line_in_stable_range_mut(lines, for_line);
    }

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        self.delegate.get_logical_lines(lines)
    }

    fn with_lines_mut(&self, lines: Range<StableRowIndex>, with_lines: &mut dyn WithPaneLines) {
        let mut renderer = self.renderer.lock();
        // Take care to access self.delegate methods here before we get into
        // calling into its own with_lines_mut to avoid a runtime
        // borrow erro!
        renderer.check_for_resize();
        let dims = self.get_dimensions();
        let search_row = renderer.compute_search_row();

        struct OverlayLines<'a> {
            with_lines: &'a mut dyn WithPaneLines,
            dims: RenderableDimensions,
            search_row: StableRowIndex,
            renderer: &'a mut QuickSelectRenderable,
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

                let colors = self.renderer.config.resolved_palette.clone();

                // Process the lines; for the search row we want to render instead
                // the search UI.
                // For rows with search results, we want to highlight the matching ranges

                for (idx, line) in lines.iter_mut().enumerate() {
                    let mut line: Line = line.clone();
                    let stable_idx = idx as StableRowIndex + first_row;
                    self.renderer.dirty_results.remove(stable_idx);
                    if stable_idx == self.search_row {
                        // Replace with search UI
                        let rev = CellAttributes::default().set_reverse(true).clone();
                        line.fill_range(0..self.dims.cols, &Cell::new(' ', rev.clone()), SEQ_ZERO);
                        line.overlay_text_with_attribute(
                            0,
                            &format!(
                                "Select: {}  (type highlighted prefix to {}, uppercase pastes, ESC to cancel)",
                                self.renderer.selection,
                                if self.renderer.args.label.is_empty() {
                                    "copy"
                                } else {
                                    &self.renderer.args.label
                                },
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
                                    cell.attrs_mut()
                                        .set_background(
                                            colors
                                                .quick_select_match_bg
                                                .unwrap_or(AnsiColor::Black.into()),
                                        )
                                        .set_foreground(
                                            colors
                                                .quick_select_match_fg
                                                .unwrap_or(AnsiColor::Green.into()),
                                        )
                                        .set_reverse(false);
                                }
                            }
                            for (idx, c) in m.label.chars().enumerate() {
                                let mut attr = line
                                    .get_cell(idx)
                                    .map(|cell| cell.attrs().clone())
                                    .unwrap_or_else(|| CellAttributes::default());
                                attr.set_background(
                                    colors
                                        .quick_select_label_bg
                                        .unwrap_or(AnsiColor::Black.into()),
                                )
                                .set_foreground(
                                    colors
                                        .quick_select_label_fg
                                        .unwrap_or(AnsiColor::Olive.into()),
                                )
                                .set_reverse(false);
                                line.set_cell(m.range.start + idx, Cell::new(c, attr), SEQ_ZERO);
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
        let mut renderer = self.renderer.lock();
        renderer.check_for_resize();
        let dims = self.get_dimensions();

        let (top, mut lines) = self.delegate.get_lines(lines);
        let colors = renderer.config.resolved_palette.clone();

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
                line.fill_range(0..dims.cols, &Cell::new(' ', rev.clone()), SEQ_ZERO);
                line.overlay_text_with_attribute(
                    0,
                    &format!(
                        "Select: {}  (type highlighted prefix to {}, uppercase pastes, ESC to cancel)",
                        renderer.selection,
                        if renderer.args.label.is_empty() {
                            "copy"
                        } else {
                            &renderer.args.label
                        },
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
                            cell.attrs_mut()
                                .set_background(
                                    colors
                                        .quick_select_match_bg
                                        .unwrap_or(AnsiColor::Black.into()),
                                )
                                .set_foreground(
                                    colors
                                        .quick_select_match_fg
                                        .unwrap_or(AnsiColor::Green.into()),
                                )
                                .set_reverse(false);
                        }
                    }
                    for (idx, c) in m.label.chars().enumerate() {
                        let mut attr = line
                            .get_cell(idx)
                            .map(|cell| cell.attrs().clone())
                            .unwrap_or_else(|| CellAttributes::default());
                        attr.set_background(
                            colors
                                .quick_select_label_bg
                                .unwrap_or(AnsiColor::Black.into()),
                        )
                        .set_foreground(
                            colors
                                .quick_select_label_fg
                                .unwrap_or(AnsiColor::Olive.into()),
                        )
                        .set_reverse(false);
                        line.set_cell(m.range.start + idx, Cell::new(c, attr), SEQ_ZERO);
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
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                term_window.set_viewport(pane_id, row, dims);
            })));
    }

    fn check_for_resize(&mut self) {
        let dims = self.delegate.get_dimensions();
        if dims.cols == self.width && dims.viewport_rows == self.height {
            return;
        }

        self.width = dims.cols;
        self.height = dims.viewport_rows;

        let pos = self.result_pos;
        self.update_search(false);
        self.result_pos = pos;
    }

    fn recompute_results(&mut self) {
        /// Produce the sorted seq of unique match_ids from the results
        fn compute_uniq_results(results: &[SearchResult]) -> Vec<usize> {
            let mut ids: Vec<usize> = results.iter().map(|sr| sr.match_id).collect();
            ids.sort();
            ids.dedup();
            ids
        }

        let uniq_results = compute_uniq_results(&self.results);

        // Label each unique result
        let labels = compute_labels_for_alphabet(
            if !self.args.alphabet.is_empty() {
                &self.args.alphabet
            } else {
                &self.config.quick_select_alphabet
            },
            uniq_results.len(),
        );
        self.by_label.clear();

        // Keep track of match_id -> label
        let mut assigned_labels: HashMap<usize, usize> = HashMap::new();

        // Work through the results in reverse order, so that we assign eg: `a` to the
        // bottom-right-most result first and so on
        for (result_index, res) in self.results.iter().enumerate().rev() {
            // Figure out which label to use based on the match_id
            let label_index = match assigned_labels.get(&res.match_id).copied() {
                Some(idx) => idx,
                None => {
                    let idx = assigned_labels.len();
                    assigned_labels.insert(res.match_id, idx);
                    idx
                }
            };
            let label = match labels.get(label_index) {
                Some(l) => l,
                None => {
                    // There are more result candidates than the alphabet
                    // can support, so we skip this one and keep looking:
                    // we may still have matches that have an assigned
                    // label, so we keep going rather than breaking
                    // out of the loop.
                    continue;
                }
            };

            self.by_label.entry(label.clone()).or_insert(result_index);
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
                    label: label.clone(),
                };

                let matches = self.by_line.entry(idx).or_insert_with(|| vec![]);
                matches.push(result);

                self.dirty_results.add(idx);
            }
        }
    }

    fn update_search(&mut self, is_initial_run: bool) {
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
            let pane: Arc<dyn Pane> = self.delegate.clone();
            let window = self.window.clone();
            let pattern = self.pattern.clone();
            let scope = self.args.scope_lines;
            let viewport = self.viewport;
            promise::spawn::spawn(async move {
                let dims = pane.get_dimensions();
                let scope = scope.unwrap_or(1000).max(dims.viewport_rows);
                let top = viewport.unwrap_or(dims.physical_top);
                let range = top.saturating_sub(scope as StableRowIndex)
                    ..top + (dims.viewport_rows + scope) as StableRowIndex;
                let limit = None;
                let mut results = pane.search(pattern, range, limit).await?;
                results.sort();

                let pane_id = pane.pane_id();
                let mut results = Some(results);
                window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                    let state = term_window.pane_state(pane_id);
                    if let Some(overlay) = state.overlay.as_ref() {
                        if let Some(search_overlay) =
                            overlay.pane.downcast_ref::<QuickSelectOverlay>()
                        {
                            let mut r = search_overlay.renderer.lock();
                            r.results = results.take().unwrap();
                            r.recompute_results();
                            let num_results = r.results.len();

                            if !r.results.is_empty() {
                                match &r.viewport {
                                    Some(y) if is_initial_run => {
                                        r.result_pos = r
                                            .results
                                            .iter()
                                            .position(|result| result.start_y >= *y);
                                    }
                                    _ => {
                                        r.activate_match_number(num_results - 1);
                                    }
                                }
                            } else {
                                if !is_initial_run {
                                    r.set_viewport(None);
                                }
                                r.clear_selection();
                            }
                        }
                    }
                })));
                anyhow::Result::<()>::Ok(())
            })
            .detach();
        } else {
            if !is_initial_run {
                self.set_viewport(None);
            }
            self.clear_selection();
        }
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

    fn select_and_copy_match_number(&mut self, n: usize, paste: bool) {
        let result = self.results[n].clone();

        let pane_id = self.delegate.pane_id();
        let action = self.args.action.clone();
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let mux = mux::Mux::get();
                if let Some(pane) = mux.get_pane(pane_id) {
                    {
                        let mut selection = term_window.selection(pane_id);
                        let start = SelectionCoordinate::x_y(result.start_x, result.start_y);
                        selection.origin = Some(start);
                        selection.range = Some(SelectionRange {
                            start,
                            // inclusive range for selection, but the result
                            // range is exclusive
                            end: SelectionCoordinate::x_y(
                                result.end_x.saturating_sub(1),
                                result.end_y,
                            ),
                        });
                        // Ensure that selection doesn't get invalidated when
                        // the overlay is closed
                        selection.seqno = pane.get_current_seqno();
                    }

                    let text = term_window.selection_text(&pane);
                    if !text.is_empty() {
                        if paste {
                            let _ = pane.send_paste(&text);
                        }
                        if let Some(action) = action {
                            let _ = term_window.perform_key_assignment(&pane, &action);
                        } else {
                            term_window.copy_to_clipboard(
                                ClipboardCopyDestination::ClipboardAndPrimarySelection,
                                text,
                            );
                        }
                    }
                }
            })));
    }

    fn activate_match_number(&mut self, n: usize) {
        self.result_pos.replace(n);
        let result = self.results[n].clone();
        self.set_viewport(Some(result.start_y));
    }
}
