use crate::frontend::gui::selection::{SelectionCoordinate, SelectionRange};
use crate::frontend::gui::termwindow::TermWindow;
use crate::mux::domain::DomainId;
use crate::mux::renderable::*;
use crate::mux::tab::{Pane, PaneId};
use portable_pty::PtySize;
use rangeset::RangeSet;
use std::cell::{RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use unicode_segmentation::*;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    unicode_column_width, Clipboard, KeyCode, KeyModifiers, Line, MouseEvent, StableRowIndex,
};
use window::WindowOps;

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
        let mut cursor = pane.renderer().get_cursor_position();
        cursor.shape = termwiz::surface::CursorShape::SteadyBlock;

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
        let dims = self.delegate.renderer().get_dimensions();
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
        self.window.apply(move |term_window, window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                let mut selection = term_window.selection(pane_id);
                selection.start = Some(start);
                selection.range = Some(range);
                window.invalidate();
            }
            Ok(())
        });
        self.adjust_viewport_for_cursor_position();
    }

    fn dimensions(&self) -> Dimensions {
        const VERTICAL_GAP: isize = 5;
        let dims = self.delegate.renderer().get_dimensions();
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
        let dims = self.delegate.renderer().get_dimensions();
        let pane_id = self.delegate.pane_id();
        self.window.apply(move |term_window, _window| {
            if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                term_window.set_viewport(pane_id, row, dims);
            }
            Ok(())
        });
    }

    fn close(&self) {
        self.set_viewport(None);
        TermWindow::schedule_cancel_overlay(self.window.clone(), self.delegate.pane_id());
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
        let (top, lines) = self.get_lines(y..y + 1);
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
        let (top, lines) = self.get_lines(y..y + 1);
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

        let (top, lines) = self.get_lines(y..y + 1);
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
                let width = unicode_column_width(word);

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
        let (top, lines) = self.get_lines(y..y + 1);
        if let Some(line) = lines.get(0) {
            self.cursor.y = top;
            let width = line.cells().len();
            let s = line.columns_as_str(self.cursor.x..width + 1);
            let mut words = s.split_word_bounds();

            if let Some(word) = words.next() {
                self.cursor.x += unicode_column_width(word);
                if !is_whitespace_word(word) {
                    // We were part-way through a word, so look
                    // at the next word
                    if let Some(word) = words.next() {
                        if is_whitespace_word(word) {
                            self.cursor.x += unicode_column_width(word);
                            // If we advance off the RHS, move to the start of the word on the
                            // next line, if any!
                            if self.cursor.x >= width {
                                let dims = self.delegate.renderer().get_dimensions();
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

    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.render.borrow_mut()
    }

    fn get_title(&self) -> String {
        format!("Copy mode: {}", self.delegate.get_title())
    }

    fn send_paste(&self, _text: &str) -> anyhow::Result<()> {
        anyhow::bail!("ignoring paste while copying");
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        panic!("do not call reader on CopyOverlay bar tab instance");
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.delegate.writer()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        self.delegate.resize(size)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        match (key, mods) {
            (KeyCode::Char('c'), KeyModifiers::CTRL)
            | (KeyCode::Char('g'), KeyModifiers::CTRL)
            | (KeyCode::Char('q'), KeyModifiers::NONE)
            | (KeyCode::Escape, KeyModifiers::NONE) => self.render.borrow().close(),
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::LeftArrow, KeyModifiers::NONE) => {
                self.render.borrow_mut().move_left_single_cell();
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::DownArrow, KeyModifiers::NONE) => {
                self.render.borrow_mut().move_down_single_row();
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::UpArrow, KeyModifiers::NONE) => {
                self.render.borrow_mut().move_up_single_row();
            }
            (KeyCode::Char('l'), KeyModifiers::NONE)
            | (KeyCode::RightArrow, KeyModifiers::NONE) => {
                self.render.borrow_mut().move_right_single_cell();
            }

            (KeyCode::RightArrow, KeyModifiers::ALT) |
            (KeyCode::Char('f'), KeyModifiers::ALT)|
            (KeyCode::Tab, KeyModifiers::NONE) |
            (KeyCode::Char('w'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_forward_one_word();
            }

            (KeyCode::LeftArrow, KeyModifiers::ALT) |
            (KeyCode::Char('b'), KeyModifiers::ALT) |
            (KeyCode::Tab, KeyModifiers::SHIFT) |
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_backward_one_word();
            }
            (KeyCode::Char('0'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_start_of_line();
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_start_of_next_line();
            }
            (KeyCode::Char('$'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('$'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_end_of_line_content();
            }
            (KeyCode::Char('m'), KeyModifiers::ALT) |
            (KeyCode::Char('^'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('^'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_start_of_line_content();
            }
            (KeyCode::Char(' '), KeyModifiers::NONE) | (KeyCode::Char('v'), KeyModifiers::NONE) => {
                self.render.borrow_mut().toggle_selection_by_cell();
            }
            (KeyCode::Char('G'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('G'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_bottom();
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_top();
            }
            (KeyCode::Char('H'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('H'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_viewport_top();
            }
            (KeyCode::Char('M'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('M'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_viewport_middle();
            }
            (KeyCode::Char('L'), KeyModifiers::SHIFT) | // FIXME: normalize the shift away!
            (KeyCode::Char('L'), KeyModifiers::NONE) => {
                self.render.borrow_mut().move_to_viewport_bottom();
            }
            (KeyCode::PageUp, KeyModifiers::NONE) | (KeyCode::Char('b'), KeyModifiers::CTRL) => self.render.borrow_mut().page_up(),
            (KeyCode::PageDown, KeyModifiers::NONE) | (KeyCode::Char('f'), KeyModifiers::CTRL) => self.render.borrow_mut().page_down(),
            _ => {}
        }
        Ok(())
    }

    fn mouse_event(&self, _event: MouseEvent) -> anyhow::Result<()> {
        anyhow::bail!("ignoring mouse while copying");
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

    fn erase_scrollback(&self) {
        self.delegate.erase_scrollback()
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

impl Renderable for CopyRenderable {
    fn get_cursor_position(&self) -> StableCursorPosition {
        self.cursor
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        self.delegate.renderer().get_dirty_lines(lines)
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        self.delegate.renderer().get_lines(lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        self.delegate.renderer().get_dimensions()
    }
}

fn is_whitespace_word(word: &str) -> bool {
    if let Some(c) = word.chars().next() {
        c.is_whitespace()
    } else {
        false
    }
}
