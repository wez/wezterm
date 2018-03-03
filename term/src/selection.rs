use super::ScrollbackOrVisibleRowIndex;
use std::ops::Range;

/// The x,y coordinates of either the start or end of a selection region
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct SelectionCoordinate {
    pub x: usize,
    pub y: ScrollbackOrVisibleRowIndex,
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
    pub fn rows(&self) -> Range<ScrollbackOrVisibleRowIndex> {
        debug_assert!(
            self.start.y <= self.end.y,
            "you forgot to normalize a SelectionRange"
        );
        self.start.y..self.end.y + 1
    }

    /// Yields a range representing the selected columns for the specified row.
    /// Not that the range may include usize::max_value() for some rows; this
    /// indicates that the selection extends to the end of that row.
    /// Since this struct has no knowledge of line length, it cannot be
    /// more precise than that.
    /// Must be called on a normalized range!
    pub fn cols_for_row(&self, row: ScrollbackOrVisibleRowIndex) -> Range<usize> {
        debug_assert!(
            self.start.y <= self.end.y,
            "you forgot to normalize a SelectionRange"
        );
        if row < self.start.y || row > self.end.y {
            0..0
        } else if self.start.y == self.end.y {
            // A single line selection
            if self.start.x <= self.end.x {
                self.start.x..self.end.x.saturating_add(1)
            } else {
                self.end.x..self.start.x.saturating_add(1)
            }
        } else if row == self.end.y {
            // last line of multi-line
            0..self.end.x.saturating_add(1)
        } else if row == self.start.y {
            // first line of multi-line
            self.start.x..usize::max_value()
        } else {
            // some "middle" line of multi-line
            0..usize::max_value()
        }
    }
}
