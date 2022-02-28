use crate::domain::DomainId;
use crate::renderable::*;
use crate::Mux;
use async_trait::async_trait;
use config::keyassignment::ScrollbackEraseMode;
use downcast_rs::{impl_downcast, Downcast};
use portable_pty::PtySize;
use rangeset::RangeSet;
use serde::{Deserialize, Serialize};
use std::cell::RefMut;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use termwiz::hyperlink::Rule;
use termwiz::input::KeyboardEncoding;
use termwiz::surface::{Line, SequenceNo, SEQ_ZERO};
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    Clipboard, DownloadHandler, KeyCode, KeyModifiers, MouseEvent, SemanticZone, StableRowIndex,
    TerminalConfiguration,
};

static PANE_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type PaneId = usize;

pub fn alloc_pane_id() -> PaneId {
    PANE_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SearchResult {
    pub start_y: StableRowIndex,
    /// The cell index into the line of the start of the match
    pub start_x: usize,
    pub end_y: StableRowIndex,
    /// The cell index into the line of the end of the match
    pub end_x: usize,
    /// An identifier that can be used to group results that have
    /// the same textual content
    pub match_id: usize,
}

pub use config::keyassignment::Pattern;

/// Why a close request is being made
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloseReason {
    /// The containing window is being closed
    Window,
    /// The containing tab is being close
    Tab,
    /// Just this tab is being closed
    Pane,
}

const PASTE_CHUNK_SIZE: usize = 1024;

struct Paste {
    pane_id: PaneId,
    text: String,
    offset: usize,
}

fn paste_next_chunk(paste: &Arc<Mutex<Paste>>) {
    let mut locked = paste.lock().unwrap();
    let mux = Mux::get().unwrap();
    let pane = mux.get_pane(locked.pane_id).unwrap();

    let remain = locked.text.len() - locked.offset;
    let mut chunk = remain.min(PASTE_CHUNK_SIZE);

    // Make sure we chunk at a char boundary, otherwise the
    // slice operation below will panic
    while !locked.text.is_char_boundary(locked.offset + chunk) && chunk < remain {
        chunk += 1;
    }
    let text_slice = &locked.text[locked.offset..locked.offset + chunk];
    pane.send_paste(text_slice).unwrap();

    if chunk < remain {
        // There is more to send
        locked.offset += chunk;
        schedule_next_paste(paste);
    }
}

fn schedule_next_paste(paste: &Arc<Mutex<Paste>>) {
    let paste = Arc::clone(paste);
    promise::spawn::spawn(async move {
        paste_next_chunk(&paste);
    })
    .detach();
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicalLine {
    pub physical_lines: Vec<Line>,
    pub logical: Line,
    pub first_row: StableRowIndex,
}

impl LogicalLine {
    pub fn contains_y(&self, y: StableRowIndex) -> bool {
        y >= self.first_row && y < self.first_row + self.physical_lines.len() as StableRowIndex
    }

    pub fn xy_to_logical_x(&self, x: usize, y: StableRowIndex) -> usize {
        let mut offset = 0;
        for (idx, line) in self.physical_lines.iter().enumerate() {
            let phys_y = self.first_row + idx as StableRowIndex;
            if y < phys_y {
                // Eg: trying to drag off the top of the viewport.
                // Their y coordinate precedes our first line, so
                // the only logical x we can return is 0
                return 0;
            }
            if phys_y == y {
                return offset + x;
            }
            offset += line.cells().len();
        }
        // Allow selecting off the end of the line
        offset + x
    }

    pub fn logical_x_to_physical_coord(&self, x: usize) -> (StableRowIndex, usize) {
        let mut y = self.first_row;
        let mut idx = 0;
        for line in &self.physical_lines {
            let x_off = x - idx;
            let line_len = line.cells().len();
            if x_off < line_len {
                return (y, x_off);
            }
            y += 1;
            idx += line_len;
        }
        (
            y - 1,
            x - idx + self.physical_lines.last().unwrap().cells().len(),
        )
    }

    pub fn apply_hyperlink_rules(&mut self, rules: &[Rule]) {
        self.logical.invalidate_implicit_hyperlinks(SEQ_ZERO);
        self.logical.scan_and_create_hyperlinks(rules);
        if !self.logical.has_hyperlink() {
            return;
        }

        // Re-compute the physical lines
        let mut line = self.logical.clone();
        let num_phys = self.physical_lines.len();
        for (idx, phys) in self.physical_lines.iter_mut().enumerate() {
            let len = phys.cells().len();
            let remainder = line.split_off(len, SEQ_ZERO);
            *phys = line;
            line = remainder;
            let wrapped = idx == num_phys - 1;
            phys.set_last_cell_was_wrapped(wrapped, SEQ_ZERO);
        }
    }
}

/// A Pane represents a view on a terminal
#[async_trait(?Send)]
pub trait Pane: Downcast {
    fn pane_id(&self) -> PaneId;

    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    fn get_cursor_position(&self) -> StableCursorPosition;

    fn set_cursor_position(&self, _x: u64, _y: u64) {}

    fn get_current_seqno(&self) -> SequenceNo;

    /// Given a range of lines, return the subset of those lines that
    /// have changed since the supplied sequence no.
    fn get_changed_since(
        &self,
        lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> RangeSet<StableRowIndex>;

    /// Returns a set of lines from the scrollback or visible portion of
    /// the display.  The lines are indexed using StableRowIndex, which
    /// can be invalidated if the scrollback is busy, or when switching
    /// to the alternate screen.
    /// To deal with this, this function will adjust the input so that
    /// a range that has been scrolled off the top will return the top
    /// n rows of the scrollback (where n is the size of the input range),
    /// or the bottom n rows of the scrollback when switching to the alt
    /// screen and the index would go off the bottom.
    /// Because of this, we also return the adjusted StableRowIndex for
    /// the first row in the range.
    ///
    /// For each line, if it was dirty in the backing data, then the dirty
    /// flag will be cleared in the backing data.  The returned line will
    /// have its dirty bit set appropriately.
    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>);

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        let (mut first, mut phys) = self.get_lines(lines);

        // Avoid pathological cases where we have eg: a really long logical line
        // (such as 1.5MB of json) that we previously wrapped.  We don't want to
        // un-wrap, scan, and re-wrap that thing.
        // This is an imperfect length constraint to partially manage the cost.
        const MAX_LOGICAL_LINE_LEN: usize = 1024;
        let mut back_len = 0;

        // Look backwards to find the start of the first logical line
        while first > 0 {
            let (prior, back) = self.get_lines(first - 1..first);
            if prior == first {
                break;
            }
            if !back[0].last_cell_was_wrapped() {
                break;
            }
            if back[0].cells().len() + back_len > MAX_LOGICAL_LINE_LEN {
                break;
            }
            back_len += back[0].cells().len();
            first = prior;
            for (idx, line) in back.into_iter().enumerate() {
                phys.insert(idx, line);
            }
        }

        // Look forwards to find the end of the last logical line
        while let Some(last) = phys.last() {
            if !last.last_cell_was_wrapped() {
                break;
            }
            if last.cells().len() > MAX_LOGICAL_LINE_LEN {
                break;
            }

            let next_row = first + phys.len() as StableRowIndex;
            let (last_row, mut ahead) = self.get_lines(next_row..next_row + 1);
            if last_row != next_row {
                break;
            }
            phys.append(&mut ahead);
        }

        // Now process this stuff into logical lines
        let mut lines = vec![];
        for (idx, line) in phys.into_iter().enumerate() {
            match lines.last_mut() {
                None => {
                    let logical = line.clone();
                    lines.push(LogicalLine {
                        physical_lines: vec![line],
                        logical,
                        first_row: first + idx as StableRowIndex,
                    });
                }
                Some(prior) => {
                    if prior.logical.last_cell_was_wrapped()
                        && prior.logical.cells().len() <= MAX_LOGICAL_LINE_LEN
                    {
                        prior.logical.set_last_cell_was_wrapped(false, SEQ_ZERO);
                        prior.logical.append_line(line.clone(), SEQ_ZERO);
                        prior.physical_lines.push(line);
                    } else {
                        let logical = line.clone();
                        lines.push(LogicalLine {
                            physical_lines: vec![line],
                            logical,
                            first_row: first + idx as StableRowIndex,
                        });
                    }
                }
            }
        }
        lines
    }

    fn get_lines_with_hyperlinks_applied(
        &self,
        lines: Range<StableRowIndex>,
        rules: &[Rule],
    ) -> (StableRowIndex, Vec<Line>) {
        let requested_first = lines.start;
        let num_lines = (lines.end - lines.start) as usize;
        let logical = self.get_logical_lines(lines);

        let mut first = None;
        let mut phys_lines = vec![];
        'outer: for mut log_line in logical {
            log_line.apply_hyperlink_rules(rules);
            for (idx, phys) in log_line.physical_lines.into_iter().enumerate() {
                if log_line.first_row + idx as StableRowIndex >= requested_first {
                    if first.is_none() {
                        first.replace(log_line.first_row + idx as StableRowIndex);
                    }
                    phys_lines.push(phys);
                    if phys_lines.len() == num_lines {
                        break 'outer;
                    }
                }
            }
        }

        if first.is_none() {
            assert_eq!(phys_lines.len(), 0);
        }

        (first.unwrap_or(0), phys_lines)
    }

    /// Returns render related dimensions
    fn get_dimensions(&self) -> RenderableDimensions;

    fn get_title(&self) -> String;
    fn send_paste(&self, text: &str) -> anyhow::Result<()>;
    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>>;
    fn writer(&self) -> RefMut<dyn std::io::Write>;
    fn resize(&self, size: PtySize) -> anyhow::Result<()>;
    /// Called as a hint that the pane is being resized as part of
    /// a zoom-to-fill-all-the-tab-space operation.
    fn set_zoomed(&self, _zoomed: bool) {}
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()>;
    fn key_up(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()>;
    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()>;
    fn perform_actions(&self, _actions: Vec<termwiz::escape::Action>) {}
    fn is_dead(&self) -> bool;
    fn kill(&self) {}
    fn palette(&self) -> ColorPalette;
    fn domain_id(&self) -> DomainId;

    fn get_keyboard_encoding(&self) -> KeyboardEncoding {
        KeyboardEncoding::Xterm
    }

    fn copy_user_vars(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    fn erase_scrollback(&self, _erase_mode: ScrollbackEraseMode) {}

    /// Called to advise on whether this tab has focus
    fn focus_changed(&self, _focused: bool) {}
    fn has_unseen_output(&self) -> bool {
        false
    }

    /// Certain panes are OK to be closed with impunity (no prompts)
    fn can_close_without_prompting(&self, _reason: CloseReason) -> bool {
        false
    }

    /// Performs a search.
    /// If the result is empty then there are no matches.
    /// Otherwise, the result shall contain all possible matches.
    async fn search(&self, _pattern: Pattern) -> anyhow::Result<Vec<SearchResult>> {
        Ok(vec![])
    }

    /// Retrieve the set of semantic zones
    fn get_semantic_zones(&self) -> anyhow::Result<Vec<SemanticZone>> {
        Ok(vec![])
    }

    /// Returns true if the terminal has grabbed the mouse and wants to
    /// give the embedded application a chance to process events.
    /// In practice this controls whether the gui will perform local
    /// handling of clicks.
    fn is_mouse_grabbed(&self) -> bool;
    fn is_alt_screen_active(&self) -> bool;

    fn set_clipboard(&self, _clipboard: &Arc<dyn Clipboard>) {}
    fn set_download_handler(&self, _handler: &Arc<dyn DownloadHandler>) {}
    fn set_config(&self, _config: Arc<dyn TerminalConfiguration>) {}
    fn get_config(&self) -> Option<Arc<dyn TerminalConfiguration>> {
        None
    }

    fn get_current_working_dir(&self) -> Option<Url>;
    fn get_foreground_process_name(&self) -> Option<String> {
        None
    }

    fn trickle_paste(&self, text: String) -> anyhow::Result<()> {
        if text.len() <= PASTE_CHUNK_SIZE {
            // Send it all now
            self.send_paste(&text)?;
        } else {
            // It's pretty heavy, so we trickle it into the pty
            let paste = Arc::new(Mutex::new(Paste {
                pane_id: self.pane_id(),
                text,
                offset: 0,
            }));
            paste_next_chunk(&paste);
        }
        Ok(())
    }
}
impl_downcast!(Pane);

#[cfg(test)]
mod test {
    use super::*;
    use k9::snapshot;

    struct FakePane {
        lines: Vec<Line>,
    }

    impl Pane for FakePane {
        fn pane_id(&self) -> PaneId {
            unimplemented!()
        }
        fn get_cursor_position(&self) -> StableCursorPosition {
            unimplemented!()
        }

        fn get_current_seqno(&self) -> SequenceNo {
            unimplemented!()
        }

        fn get_changed_since(
            &self,
            _: Range<StableRowIndex>,
            _: SequenceNo,
        ) -> RangeSet<StableRowIndex> {
            unimplemented!()
        }
        fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
            let first = lines.start;
            (
                first,
                self.lines
                    .iter()
                    .skip(lines.start as usize)
                    .take((lines.end - lines.start) as usize)
                    .cloned()
                    .collect(),
            )
        }
        fn get_dimensions(&self) -> RenderableDimensions {
            unimplemented!()
        }

        fn get_title(&self) -> String {
            unimplemented!()
        }
        fn send_paste(&self, _: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
            Ok(None)
        }
        fn writer(&self) -> RefMut<dyn std::io::Write> {
            unimplemented!()
        }
        fn resize(&self, _: PtySize) -> anyhow::Result<()> {
            unimplemented!()
        }

        fn mouse_event(&self, _: MouseEvent) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn is_dead(&self) -> bool {
            unimplemented!()
        }
        fn palette(&self) -> ColorPalette {
            unimplemented!()
        }
        fn domain_id(&self) -> DomainId {
            unimplemented!()
        }

        fn is_mouse_grabbed(&self) -> bool {
            false
        }
        fn is_alt_screen_active(&self) -> bool {
            false
        }
        fn get_current_working_dir(&self) -> Option<Url> {
            None
        }
        fn key_down(&self, _: KeyCode, _: KeyModifiers) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn key_up(&self, _: KeyCode, _: KeyModifiers) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    #[test]
    fn logical_lines() {
        let text = "Hello there this is a long line.\nlogical line two\nanother long line here\nlogical line four\nlogical line five\ncap it off with another long line";
        let mut physical_lines = vec![];
        let width = 20;
        for logical in text.split('\n') {
            let chunks = logical
                .chars()
                .collect::<Vec<char>>()
                .chunks(width)
                .map(|c| c.into_iter().collect::<String>())
                .collect::<Vec<String>>();
            let n_chunks = chunks.len();
            for (idx, chunk) in chunks.into_iter().enumerate() {
                let mut line = Line::from_text(&chunk, &Default::default(), SEQ_ZERO);
                if idx < n_chunks - 1 {
                    line.set_last_cell_was_wrapped(true, SEQ_ZERO);
                }
                physical_lines.push(line);
            }
        }

        fn text_from_lines(lines: &[Line]) -> Vec<String> {
            lines.iter().map(|l| l.as_str()).collect::<Vec<_>>()
        }

        let line_text = text_from_lines(&physical_lines);
        snapshot!(
            line_text,
            r#"
[
    "Hello there this is ",
    "a long line.",
    "logical line two",
    "another long line he",
    "re",
    "logical line four",
    "logical line five",
    "cap it off with anot",
    "her long line",
]
"#
        );

        let pane = FakePane {
            lines: physical_lines,
        };

        fn summarize_logical_lines(lines: &[LogicalLine]) -> Vec<(StableRowIndex, String)> {
            lines
                .iter()
                .map(|l| (l.first_row, l.logical.as_str()))
                .collect::<Vec<_>>()
        }

        let logical = pane.get_logical_lines(0..30);
        snapshot!(
            summarize_logical_lines(&logical),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
    (
        5,
        "logical line four",
    ),
    (
        6,
        "logical line five",
    ),
    (
        7,
        "cap it off with another long line",
    ),
]
"#
        );

        // Now try with offset bounds
        let offset = pane.get_logical_lines(1..3);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
]
"#
        );

        let offset = pane.get_logical_lines(1..4);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
]
"#
        );

        let offset = pane.get_logical_lines(1..5);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
]
"#
        );

        let offset = pane.get_logical_lines(1..6);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
    (
        5,
        "logical line four",
    ),
]
"#
        );

        let offset = pane.get_logical_lines(1..7);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
    (
        5,
        "logical line four",
    ),
    (
        6,
        "logical line five",
    ),
]
"#
        );

        let offset = pane.get_logical_lines(1..8);
        snapshot!(
            summarize_logical_lines(&offset),
            r#"
[
    (
        0,
        "Hello there this is a long line.",
    ),
    (
        2,
        "logical line two",
    ),
    (
        3,
        "another long line here",
    ),
    (
        5,
        "logical line four",
    ),
    (
        6,
        "logical line five",
    ),
    (
        7,
        "cap it off with another long line",
    ),
]
"#
        );

        let line = &offset[0];
        let coords = (0..line.logical.cells().len())
            .map(|idx| line.logical_x_to_physical_coord(idx))
            .collect::<Vec<_>>();
        snapshot!(
            coords,
            "
[
    (
        0,
        0,
    ),
    (
        0,
        1,
    ),
    (
        0,
        2,
    ),
    (
        0,
        3,
    ),
    (
        0,
        4,
    ),
    (
        0,
        5,
    ),
    (
        0,
        6,
    ),
    (
        0,
        7,
    ),
    (
        0,
        8,
    ),
    (
        0,
        9,
    ),
    (
        0,
        10,
    ),
    (
        0,
        11,
    ),
    (
        0,
        12,
    ),
    (
        0,
        13,
    ),
    (
        0,
        14,
    ),
    (
        0,
        15,
    ),
    (
        0,
        16,
    ),
    (
        0,
        17,
    ),
    (
        0,
        18,
    ),
    (
        0,
        19,
    ),
    (
        1,
        0,
    ),
    (
        1,
        1,
    ),
    (
        1,
        2,
    ),
    (
        1,
        3,
    ),
    (
        1,
        4,
    ),
    (
        1,
        5,
    ),
    (
        1,
        6,
    ),
    (
        1,
        7,
    ),
    (
        1,
        8,
    ),
    (
        1,
        9,
    ),
    (
        1,
        10,
    ),
    (
        1,
        11,
    ),
]
"
        );
    }

    #[test]
    fn double_click() {
        let attr = Default::default();
        let logical = LogicalLine {
            physical_lines: vec![
                Line::from_text("hello", &attr, SEQ_ZERO),
                Line::from_text("yo", &attr, SEQ_ZERO),
            ],
            logical: Line::from_text("helloyo", &attr, SEQ_ZERO),
            first_row: 0,
        };

        assert_eq!(logical.xy_to_logical_x(2, -1), 0);
        assert_eq!(logical.xy_to_logical_x(20, 1), 25);

        let start_idx = logical.xy_to_logical_x(2, 1);

        fn is_double_click_word(s: &str) -> bool {
            match s.chars().count() {
                1 => !" \t\n{[}]()\"'`".contains(s),
                0 => false,
                _ => true,
            }
        }
        use termwiz::surface::line::DoubleClickRange;

        assert_eq!(start_idx, 7);
        match logical
            .logical
            .compute_double_click_range(start_idx, is_double_click_word)
        {
            DoubleClickRange::Range(click_range) => {
                assert_eq!(click_range, 7..7);
            }
            _ => unreachable!(),
        }
    }
}
