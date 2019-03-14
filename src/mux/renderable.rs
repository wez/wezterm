use std::ops::Range;
use std::sync::Arc;
use term::{CursorPosition, Line, Terminal, TerminalState};
use termwiz::hyperlink::Hyperlink;

/// Renderable allows passing something that isn't an actual term::Terminal
/// instance into the renderer, which opens up remoting of the terminal
/// surfaces via a multiplexer.
pub trait Renderable {
    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    fn get_cursor_position(&self) -> CursorPosition;

    /// Returns the set of visible lines that are dirty.
    /// The return value is a Vec<(line_idx, line, selrange)>, where
    /// line_idx is relative to the top of the viewport.
    /// The selrange value is the column range representing the selected
    /// columns on this line.
    fn get_dirty_lines(&self) -> Vec<(usize, &Line, Range<usize>)>;

    fn has_dirty_lines(&self) -> bool;

    fn make_all_lines_dirty(&mut self);

    /// Clear the dirty flag for all dirty lines
    fn clean_dirty_lines(&mut self);

    /// Returns the currently highlighted hyperlink
    fn current_highlight(&self) -> Option<Arc<Hyperlink>>;

    /// Returns physical, non-scrollback (rows, cols) for the
    /// terminal screen
    fn physical_dimensions(&self) -> (usize, usize);
}

impl Renderable for Terminal {
    fn get_cursor_position(&self) -> CursorPosition {
        self.cursor_pos()
    }

    fn get_dirty_lines(&self) -> Vec<(usize, &Line, Range<usize>)> {
        TerminalState::get_dirty_lines(self)
    }

    fn clean_dirty_lines(&mut self) {
        TerminalState::clean_dirty_lines(self)
    }

    fn make_all_lines_dirty(&mut self) {
        TerminalState::make_all_lines_dirty(self)
    }

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        TerminalState::current_highlight(self)
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        let screen = self.screen();
        (screen.physical_rows, screen.physical_cols)
    }

    fn has_dirty_lines(&self) -> bool {
        TerminalState::has_dirty_lines(self)
    }
}
