use crate::pane::LogicalLine;
use luahelper::impl_lua_conversion_dynamic;
use rangeset::RangeSet;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use termwiz::surface::SequenceNo;
use wezterm_dynamic::{FromDynamic, ToDynamic};
use wezterm_term::{Line, StableRowIndex, Terminal};

/// Describes the location of the cursor
#[derive(
    Debug, Default, Copy, Clone, Eq, PartialEq, Deserialize, Serialize, FromDynamic, ToDynamic,
)]
pub struct StableCursorPosition {
    pub x: usize,
    pub y: StableRowIndex,
    pub shape: termwiz::surface::CursorShape,
    pub visibility: termwiz::surface::CursorVisibility,
}
impl_lua_conversion_dynamic!(StableCursorPosition);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, FromDynamic, ToDynamic,
)]
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
    pub dpi: u32,
}
impl_lua_conversion_dynamic!(RenderableDimensions);

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
    seqno: SequenceNo,
) -> RangeSet<StableRowIndex> {
    let screen = term.screen();
    let lines = screen.get_changed_stable_rows(lines, seqno);
    let mut set = RangeSet::new();
    for line in lines {
        set.add(line);
    }
    set
}

pub fn terminal_get_logical_lines(
    term: &mut Terminal,
    lines: Range<StableRowIndex>,
) -> Vec<LogicalLine> {
    let screen = term.screen();
    let mut result = vec![];
    let reverse = term.get_reverse_video();
    screen.for_each_logical_line_in_stable_range(lines.clone(), |sr, lines| {
        let mut physical_lines: Vec<Line> = lines
            .iter()
            .map(|line| {
                let mut line = (*line).clone();
                let seqno = line.current_seqno();
                line.set_reverse(reverse, seqno);
                line
            })
            .collect();

        let mut logical = physical_lines[0].clone();
        for line in &mut physical_lines[1..] {
            let seqno = line.current_seqno();
            logical.set_last_cell_was_wrapped(false, seqno);
            logical.append_line((*line).clone(), seqno);
        }

        result.push(LogicalLine {
            physical_lines,
            logical,
            first_row: sr.start,
        });
    });
    result
}

/// Implements Pane::get_lines for Terminal
pub fn terminal_get_lines(
    term: &mut Terminal,
    lines: Range<StableRowIndex>,
) -> (StableRowIndex, Vec<Line>) {
    let reverse = term.get_reverse_video();
    let screen = term.screen_mut();
    let phys_range = screen.stable_range(&lines);

    let first = screen.phys_to_stable_row_index(phys_range.start);
    let mut lines = screen.lines_in_phys_range(phys_range);
    for line in &mut lines {
        let seqno = line.current_seqno();
        line.set_reverse(reverse, seqno);
    }

    (first, lines)
}

/// Implements Pane::get_dimensions for Terminal
pub fn terminal_get_dimensions(term: &mut Terminal) -> RenderableDimensions {
    let screen = term.screen();
    RenderableDimensions {
        cols: screen.physical_cols,
        viewport_rows: screen.physical_rows,
        scrollback_rows: screen.scrollback_rows(),
        physical_top: screen.visible_row_to_stable_row(0),
        scrollback_top: screen.phys_to_stable_row_index(0),
        dpi: screen.dpi,
    }
}
