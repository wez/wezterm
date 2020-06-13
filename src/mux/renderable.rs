use crate::config::configuration;
use downcast_rs::{impl_downcast, Downcast};
use rangeset::RangeSet;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use term::{Line, StableRowIndex, Terminal};

/// Describes the location of the cursor
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct StableCursorPosition {
    pub x: usize,
    pub y: StableRowIndex,
    pub shape: termwiz::surface::CursorShape,
    pub visibility: termwiz::surface::CursorVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
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
    /// The top of the scrollback (the earliest row we remember)
    /// expressed as a stable index.
    pub scrollback_top: StableRowIndex,
}

/// Renderable allows passing something that isn't an actual term::Terminal
/// instance into the renderer, which opens up remoting of the terminal
/// surfaces via a multiplexer.
pub trait Renderable: Downcast {
    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    fn get_cursor_position(&self) -> StableCursorPosition;

    /// Given a range of lines, return the subset of those lines that
    /// have their dirty flag set to true.
    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex>;

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
    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>);

    /// Returns render related dimensions
    fn get_dimensions(&self) -> RenderableDimensions;
}
impl_downcast!(Renderable);

impl Renderable for Terminal {
    fn get_cursor_position(&self) -> StableCursorPosition {
        let pos = self.cursor_pos();

        StableCursorPosition {
            x: pos.x,
            y: self.screen().visible_row_to_stable_row(pos.y),
            shape: pos.shape,
            visibility: pos.visibility,
        }
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        let screen = self.screen();
        let phys = screen.stable_range(&lines);
        let mut set = RangeSet::new();
        for (idx, line) in screen
            .lines
            .iter()
            .enumerate()
            .skip(phys.start)
            .take(phys.end - phys.start)
        {
            if line.is_dirty() {
                set.add(screen.phys_to_stable_row_index(idx))
            }
        }
        set
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let screen = self.screen_mut();
        let phys_range = screen.stable_range(&lines);
        let config = configuration();
        (
            screen.phys_to_stable_row_index(phys_range.start),
            screen
                .lines
                .iter_mut()
                .skip(phys_range.start)
                .take(phys_range.end - phys_range.start)
                .map(|line| {
                    line.scan_and_create_hyperlinks(&config.hyperlink_rules);
                    let cloned = line.clone();
                    line.clear_dirty();
                    cloned
                })
                .collect(),
        )
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
}
