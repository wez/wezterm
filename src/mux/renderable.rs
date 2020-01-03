use downcast_rs::{impl_downcast, Downcast};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use term::{Line, StableRowIndex, Terminal, TerminalState, VisibleRowIndex};
use termwiz::hyperlink::Hyperlink;

/// Describes the location of the cursor
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct StableCursorPosition {
    pub x: usize,
    pub y: StableRowIndex,
    pub shape: termwiz::surface::CursorShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderableDimensions {
    /// The viewport width
    pub cols: usize,
    /// How many rows fit in the viewport
    pub viewport_rows: usize,
    /// The total number of lines in the scrollback, including the viewport
    pub scrollback_rows: usize,

    /// The top of the physical, non-scrollback, screen expressed
    /// as a stable index.  It is envisioned that this will be used
    /// to compute row/cols for mouse events and to produce a range
    /// for the `get_lines` call when the scroll position is at the
    /// bottom of the screen.
    pub physical_top: StableRowIndex,
    pub scrollback_top: StableRowIndex,
}

/// Renderable allows passing something that isn't an actual term::Terminal
/// instance into the renderer, which opens up remoting of the terminal
/// surfaces via a multiplexer.
pub trait Renderable: Downcast {
    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    fn get_cursor_position(&self) -> StableCursorPosition;

    /// Returns the set of visible lines that are dirty.
    /// The return value is a Vec<(line_idx, line, selrange)>, where
    /// line_idx is relative to the top of the viewport.
    /// The selrange value is the column range representing the selected
    /// columns on this line.
    fn get_dirty_lines(&self) -> Vec<(usize, Cow<Line>, Range<usize>)>;

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
    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Cow<Line>>);

    fn has_dirty_lines(&self) -> bool;

    fn make_all_lines_dirty(&mut self);

    /// Clear the dirty flag for all dirty lines
    fn clean_dirty_lines(&mut self);

    /// Returns the currently highlighted hyperlink
    fn current_highlight(&self) -> Option<Arc<Hyperlink>>;

    /// Returns render related dimensions
    fn get_dimensions(&self) -> RenderableDimensions;

    fn set_viewport_position(&mut self, position: VisibleRowIndex);
}
impl_downcast!(Renderable);

impl Renderable for Terminal {
    fn get_cursor_position(&self) -> StableCursorPosition {
        let pos = self.cursor_pos();

        StableCursorPosition {
            x: pos.x,
            y: self.screen().visible_row_to_stable_row(pos.y),
            shape: pos.shape,
        }
    }

    fn get_dirty_lines(&self) -> Vec<(usize, Cow<Line>, Range<usize>)> {
        TerminalState::get_dirty_lines(self)
            .into_iter()
            .map(|(idx, line, range)| (idx, Cow::Borrowed(line), range))
            .collect()
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Cow<Line>>) {
        let screen = self.screen();
        let phys_range = screen.stable_range(&lines);
        (
            screen.phys_to_stable_row_index(phys_range.start),
            screen
                .lines
                .iter()
                .skip(phys_range.start)
                .take(phys_range.end - phys_range.start)
                .map(|line| Cow::Borrowed(line))
                .collect(),
        )
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

    fn get_dimensions(&self) -> RenderableDimensions {
        let screen = self.screen();
        RenderableDimensions {
            cols: screen.physical_cols,
            viewport_rows: screen.physical_rows,
            scrollback_rows: screen.lines.len(),
            physical_top: screen.visible_row_to_stable_row(0),
            scrollback_top: screen.phys_to_stable_row_index(0),
        }
    }

    fn has_dirty_lines(&self) -> bool {
        TerminalState::has_dirty_lines(self)
    }

    fn set_viewport_position(&mut self, position: VisibleRowIndex) {
        self.set_scroll_viewport(position);
    }
}
