// The range_plus_one lint can't see when the LHS is not compatible with
// and inclusive range
#![cfg_attr(feature = "cargo-clippy", allow(clippy::range_plus_one))]
use std::ops::Range;
use term::StableRowIndex;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Selection {
    /// Remembers the starting coordinate of the selection prior to
    /// dragging.
    pub start: Option<SelectionCoordinate>,
    /// Holds the not-normalized selection range.
    pub range: Option<SelectionRange>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.range = None;
        self.start = None;
    }

    pub fn begin(&mut self, start: SelectionCoordinate) {
        self.range = None;
        self.start = Some(start);
    }
}

/// The x,y coordinates of either the start or end of a selection region
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct SelectionCoordinate {
    pub x: usize,
    pub y: StableRowIndex,
}

/// Represents the selected text range.
/// The end coordinates are inclusive.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct SelectionRange {
    pub start: SelectionCoordinate,
    pub end: SelectionCoordinate,
}

impl SelectionRange {
    /// Create a new range that starts at the specified location
    pub fn start(start: SelectionCoordinate) -> Self {
        let end = start;
        Self { start, end }
    }

    /// Returns an extended selection that it ends at the specified location
    pub fn extend(&self, end: SelectionCoordinate) -> Self {
        Self {
            start: self.start,
            end,
        }
    }

    /// Return a normalized selection such that the starting y coord
    /// is <= the ending y coord.
    pub fn normalize(&self) -> Self {
        if self.start.y <= self.end.y {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    /// Yields a range representing the row indices.
    /// Make sure that you invoke this on a normalized range!
    pub fn rows(&self) -> Range<StableRowIndex> {
        let norm = self.normalize();
        norm.start.y..norm.end.y + 1
    }

    /// Yields a range representing the selected columns for the specified row.
    /// Not that the range may include usize::max_value() for some rows; this
    /// indicates that the selection extends to the end of that row.
    /// Since this struct has no knowledge of line length, it cannot be
    /// more precise than that.
    /// Must be called on a normalized range!
    pub fn cols_for_row(&self, row: StableRowIndex) -> Range<usize> {
        let norm = self.normalize();
        if row < norm.start.y || row > norm.end.y {
            0..0
        } else if norm.start.y == norm.end.y {
            // A single line selection
            if norm.start.x <= norm.end.x {
                norm.start.x..norm.end.x.saturating_add(1)
            } else {
                norm.end.x..norm.start.x.saturating_add(1)
            }
        } else if row == norm.end.y {
            // last line of multi-line
            0..norm.end.x.saturating_add(1)
        } else if row == norm.start.y {
            // first line of multi-line
            norm.start.x..usize::max_value()
        } else {
            // some "middle" line of multi-line
            0..usize::max_value()
        }
    }
}
