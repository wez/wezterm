use config::configuration;
use luahelper::impl_lua_conversion;
use rangeset::RangeSet;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use wezterm_term::{Line, StableRowIndex, Terminal};

/// Describes the location of the cursor
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct StableCursorPosition {
    pub x: usize,
    pub y: StableRowIndex,
    pub shape: termwiz::surface::CursorShape,
    pub visibility: termwiz::surface::CursorVisibility,
}
impl_lua_conversion!(StableCursorPosition);

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
impl_lua_conversion!(RenderableDimensions);

/// Implements Pane::get_cursor_position for Terminal
pub fn terminal_get_cursor_position(term: &mut Terminal) -> StableCursorPosition {
    let pos = term.cursor_pos();

    StableCursorPosition {
        x: pos.x,
        y: term.screen().visible_row_to_stable_row(pos.y),
        shape: pos.shape,
        visibility: pos.visibility,
    }
}

/// Implements Pane::get_dirty_lines for Terminal
pub fn terminal_get_dirty_lines(
    term: &mut Terminal,
    lines: Range<StableRowIndex>,
) -> RangeSet<StableRowIndex> {
    let screen = term.screen();
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

/// Implements Pane::get_lines for Terminal
pub fn terminal_get_lines(
    term: &mut Terminal,
    lines: Range<StableRowIndex>,
) -> (StableRowIndex, Vec<Line>) {
    let screen = term.screen_mut();
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

/// Implements Pane::get_dimensions for Terminal
pub fn terminal_get_dimensions(term: &mut Terminal) -> RenderableDimensions {
    let screen = term.screen();
    RenderableDimensions {
        cols: screen.physical_cols,
        viewport_rows: screen.physical_rows,
        scrollback_rows: screen.lines.len(),
        physical_top: screen.visible_row_to_stable_row(0),
        scrollback_top: screen.phys_to_stable_row_index(0),
    }
}
