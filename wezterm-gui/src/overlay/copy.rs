use crate::selection::{SelectionCoordinate, SelectionRange};
use crate::termwindow::{TermWindow, TermWindowNotif};
use config::keyassignment::{
    CopyModeAssignment, KeyAssignment, KeyTable, KeyTableEntry, ScrollbackEraseMode,
};
use mux::domain::DomainId;
use mux::pane::{Pane, PaneId};
use mux::renderable::*;
use portable_pty::PtySize;
use rangeset::RangeSet;
use std::cell::{RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::surface::{CursorVisibility, SequenceNo};
use unicode_segmentation::*;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    unicode_column_width, Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex,
};
use window::{KeyCode as WKeyCode, Modifiers, WindowOps};

pub struct CopyOverlay {
    delegate: Rc<dyn Pane>,
    render: RefCell<CopyRenderable>,
}

struct CopyRenderable {
    cursor: StableCursorPosition,
    delegate: Rc<dyn Pane>,
    start: Option<SelectionCoordinate>,
    viewport: Option<StableRowIndex>,
    /// We use this to cancel ourselves later
    window: ::window::Window,
}

struct Dimensions {
    vertical_gap: isize,
    dims: RenderableDimensions,
    top: StableRowIndex,
}

impl CopyOverlay {
    pub fn with_pane(term_window: &TermWindow, pane: &Rc<dyn Pane>) -> Rc<dyn Pane> {
        let mut cursor = pane.get_cursor_position();
        cursor.shape = termwiz::surface::CursorShape::SteadyBlock;
        cursor.visibility = CursorVisibility::Visible;

        let window = term_window.window.clone().unwrap();
        let render = CopyRenderable {
            cursor,
            window,
            delegate: Rc::clone(pane),
            start: None,
            viewport: term_window.get_viewport(pane.pane_id()),
        };
        Rc::new(CopyOverlay {
            delegate: Rc::clone(pane),
            render: RefCell::new(render),
        })
    }

    pub fn viewport_changed(&self, viewport: Option<StableRowIndex>) {
        let mut r = self.render.borrow_mut();
        r.viewport = viewport;
    }
}

impl CopyRenderable {
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
        if let Some(start) = self.start {
            let start = SelectionCoordinate {
                x: start.x,
                y: start.y,
            };

            let end = SelectionCoordinate {
                x: self.cursor.x,
                y: self.cursor.y,
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
        self.window
            .notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                let mut selection = term_window.selection(pane_id);
                selection.origin = Some(start);
                selection.range = Some(range);
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
        self.set_viewport(None);
        TermWindow::schedule_cancel_overlay_for_pane(self.window.clone(), self.delegate.pane_id());
    }

    fn page_up(&mut self) {
        let dims = self.dimensions();
        self.cursor.y -= dims.dims.viewport_rows as isize;
        self.select_to_cursor_pos();
    }

    fn page_down(&mut self) {
        let dims = self.dimensions();
        self.cursor.y += dims.dims.viewport_rows as isize;
        self.select_to_cursor_pos();
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
            for (x, cell) in line.cells().iter().enumerate().rev() {
                if cell.str() != " " {
                    self.cursor.x = x;
                    break;
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
            for (x, cell) in line.cells().iter().enumerate() {
                if cell.str() != " " {
                    self.cursor.x = x;
                    break;
                }
            }
        }
        self.select_to_cursor_pos();
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
                self.cursor.x = line.cells().len().saturating_sub(1);
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
            let width = line.cells().len();
            let s = line.columns_as_str(self.cursor.x..width + 1);
            let mut words = s.split_word_bounds();

            if let Some(word) = words.next() {
                self.cursor.x += unicode_column_width(word, None);
                if !is_whitespace_word(word) {
                    // We were part-way through a word, so look
                    // at the next word
                    if let Some(word) = words.next() {
                        if is_whitespace_word(word) {
                            self.cursor.x += unicode_column_width(word, None);
                            // If we advance off the RHS, move to the start of the word on the
                            // next line, if any!
                            if self.cursor.x >= width {
                                let dims = self.delegate.get_dimensions();
                                let max_row = dims.scrollback_top + dims.scrollback_rows as isize;
                                if self.cursor.y + 1 < max_row {
                                    self.cursor.y += 1;
                                    return self.move_to_start_of_line_content();
                                }
                            }
                        }
                    }
                } else {
                    // We were in whitespace and advancing
                    // has put us at the start of the next word
                }
            }
        }
        self.select_to_cursor_pos();
    }

    fn toggle_selection_by_cell(&mut self) {
        if self.start.take().is_none() {
            let coord = SelectionCoordinate {
                x: self.cursor.x,
                y: self.cursor.y,
            };
            self.start.replace(coord);
            self.select_to_cursor_pos();
        }
    }
}

impl Pane for CopyOverlay {
    fn pane_id(&self) -> PaneId {
        self.delegate.pane_id()
    }

    fn get_title(&self) -> String {
        format!("Copy mode: {}", self.delegate.get_title())
    }

    fn send_paste(&self, _text: &str) -> anyhow::Result<()> {
        anyhow::bail!("ignoring paste while copying");
    }

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(None)
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.delegate.writer()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.delegate.resize(size)
    }

    fn key_up(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
        Ok(())
    }

    fn perform_assignment(&self, assignment: &KeyAssignment) -> bool {
        use CopyModeAssignment::*;
        match assignment {
            KeyAssignment::CopyMode(assignment) => {
                let mut render = self.render.borrow_mut();
                match assignment {
                    MoveToViewportBottom => render.move_to_viewport_bottom(),
                    MoveToViewportTop => render.move_to_viewport_top(),
                    MoveToViewportMiddle => render.move_to_viewport_middle(),
                    MoveToScrollbackTop => render.move_to_top(),
                    MoveToScrollbackBottom => render.move_to_bottom(),
                    ToggleSelectionByCell => render.toggle_selection_by_cell(),
                    MoveToStartOfLineContent => render.move_to_start_of_line_content(),
                    MoveToEndOfLineContent => render.move_to_end_of_line_content(),
                    MoveToStartOfLine => render.move_to_start_of_line(),
                    MoveToStartOfNextLine => render.move_to_start_of_next_line(),
                    MoveBackwardWord => render.move_backward_one_word(),
                    MoveForwardWord => render.move_forward_one_word(),
                    MoveRight => render.move_right_single_cell(),
                    MoveLeft => render.move_left_single_cell(),
                    MoveUp => render.move_up_single_row(),
                    MoveDown => render.move_down_single_row(),
                    PageUp => render.page_up(),
                    PageDown => render.page_down(),
                    Close => render.close(),
                }
                true
            }
            _ => false,
        }
    }

    fn key_down(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
        Ok(())
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

    fn get_current_working_dir(&self) -> Option<Url> {
        self.delegate.get_current_working_dir()
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        self.render.borrow().cursor
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

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        self.delegate.get_lines(lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        self.delegate.get_dimensions()
    }
}

fn is_whitespace_word(word: &str) -> bool {
    if let Some(c) = word.chars().next() {
        c.is_whitespace()
    } else {
        false
    }
}

pub fn key_table() -> KeyTable {
    let mut table = KeyTable::default();
    for (key, mods, action) in [
        (
            WKeyCode::Char('c'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::Close),
        ),
        (
            WKeyCode::Char('g'),
            Modifiers::CTRL,
            KeyAssignment::CopyMode(CopyModeAssignment::Close),
        ),
        (
            WKeyCode::Char('q'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::Close),
        ),
        (
            WKeyCode::Char('\x1b'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::Close),
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
            WKeyCode::Char('\n'),
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
            KeyAssignment::CopyMode(CopyModeAssignment::ToggleSelectionByCell),
        ),
        (
            WKeyCode::Char('v'),
            Modifiers::NONE,
            KeyAssignment::CopyMode(CopyModeAssignment::ToggleSelectionByCell),
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
    ] {
        table.insert((key, mods), KeyTableEntry { action });
    }
    table
}
