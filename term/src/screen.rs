use super::*;
use std::collections::VecDeque;

/// Holds the model of a screen.  This can either be the primary screen
/// which includes lines of scrollback text, or the alternate screen
/// which holds no scrollback.  The intent is to have one instance of
/// Screen for each of these things.
#[derive(Debug, Clone)]
pub struct Screen {
    /// Holds the line data that comprises the screen contents.
    /// This is allocated with capacity for the entire scrollback.
    /// The last N lines are the visible lines, with those prior being
    /// the lines that have scrolled off the top of the screen.
    /// Index 0 is the topmost line of the screen/scrollback (depending
    /// on the current window size) and will be the first line to be
    /// popped off the front of the screen when a new line is added that
    /// would otherwise have exceeded the line capacity
    pub lines: VecDeque<Line>,

    /// Maximum number of lines of scrollback
    pub scrollback_size: usize,

    /// Physical, visible height of the screen (not including scrollback)
    pub physical_rows: usize,
    /// Physical, visible width of the screen
    pub physical_cols: usize,
}

impl Screen {
    /// Create a new Screen with the specified dimensions.
    /// The Cells in the viewable portion of the screen are set to the
    /// default cell attributes.
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Screen {
        let mut lines = VecDeque::with_capacity(physical_rows + scrollback_size);
        for _ in 0..physical_rows {
            lines.push_back(Line::new(physical_cols));
        }

        Screen {
            lines,
            scrollback_size,
            physical_rows,
            physical_cols,
        }
    }

    /// Resize the physical, viewable portion of the screen
    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        let capacity = physical_rows + self.scrollback_size;
        let current_capacity = self.lines.capacity();
        if capacity > current_capacity {
            self.lines.reserve(capacity - current_capacity);
        }

        if physical_rows > self.physical_rows {
            // Enlarging the viewable portion?  Add more lines at the bottom
            for _ in self.physical_rows..physical_rows {
                self.lines.push_back(Line::new(physical_cols));
            }
        }
        self.physical_rows = physical_rows;
        self.physical_cols = physical_cols;
    }

    /// Get mutable reference to a line, relative to start of scrollback.
    /// Sets the line dirty.
    #[inline]
    pub fn line_mut(&mut self, idx: PhysRowIndex) -> &mut Line {
        let line = &mut self.lines[idx];
        line.set_dirty();
        line
    }

    /// Sets a line dirty.  The line is relative to the visible origin.
    #[inline]
    pub fn dirty_line(&mut self, idx: VisibleRowIndex) {
        let line_idx = self.phys_row(idx);
        if line_idx < self.lines.len() {
            self.lines[line_idx].set_dirty();
        }
    }

    /// Returns a copy of the visible lines in the screen (no scrollback)
    #[cfg(test)]
    pub fn visible_lines(&self) -> Vec<Line> {
        let line_idx = self.lines.len() - self.physical_rows;
        let mut lines = Vec::new();
        for line in self.lines.iter().skip(line_idx) {
            if lines.len() >= self.physical_rows {
                break;
            }
            lines.push(line.clone());
        }
        lines
    }

    /// Returns a copy of the lines in the screen (including scrollback)
    #[cfg(test)]
    pub fn all_lines(&self) -> Vec<Line> {
        self.lines.iter().map(|l| l.clone()).collect()
    }

    pub fn insert_cell(&mut self, x: usize, y: VisibleRowIndex) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.invalidate_implicit_links();
        line.cells.insert(x, Cell::default());
    }

    pub fn erase_cell(&mut self, x: usize, y: VisibleRowIndex) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.invalidate_implicit_links();
        line.cells.remove(x);
        line.cells.push(Cell::default());
    }

    /// Set a cell.  the x and y coordinates are relative to the visible screeen
    /// origin.  0,0 is the top left.
    pub fn set_cell(
        &mut self,
        x: usize,
        y: VisibleRowIndex,
        c: char,
        attr: &CellAttributes,
    ) -> &Cell {
        let line_idx = self.phys_row(y);
        debug!(
            "set_cell {} x={} y={} phys={} {:?}",
            c, x, y, line_idx, attr
        );

        let line = self.line_mut(line_idx);
        line.invalidate_implicit_links();

        if attr.hyperlink.is_some() {
            line.set_has_hyperlink(true);
        }

        let width = line.cells.len();
        let cell = Cell::new(c, attr.clone());
        if x == width {
            line.cells.push(cell);
        } else if x > width {
            // if the line isn't wide enough, pad it out with the default attributes
            line.cells.resize(x, Cell::default());
            line.cells.push(cell);
        } else {
            line.cells[x] = cell;
        }
        &line.cells[x]
    }

    pub fn clear_line(
        &mut self,
        y: VisibleRowIndex,
        cols: std::ops::Range<usize>,
        attr: &CellAttributes,
    ) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        let max_col = line.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            line.cells[x] = Cell::new(' ', attr.clone());
        }
    }

    /// Translate a VisibleRowIndex into a PhysRowIndex.  The resultant index
    /// will be invalidated by inserting or removing rows!
    #[inline]
    pub fn phys_row(&self, row: VisibleRowIndex) -> PhysRowIndex {
        assert!(row >= 0, "phys_row called with negative row {}", row);
        (self.lines.len() - self.physical_rows) + row as usize
    }

    /// Given a possibly negative row number, return the corresponding physical
    /// row.  This is similar to phys_row() but allows indexing backwards into
    /// the scrollback.
    #[inline]
    pub fn scrollback_or_visible_row(&self, row: ScrollbackOrVisibleRowIndex) -> PhysRowIndex {
        ((self.lines.len() - self.physical_rows) as ScrollbackOrVisibleRowIndex + row).max(0)
            as usize
    }

    #[inline]
    pub fn scrollback_or_visible_range(
        &self,
        range: &Range<ScrollbackOrVisibleRowIndex>,
    ) -> Range<PhysRowIndex> {
        self.scrollback_or_visible_row(range.start)..self.scrollback_or_visible_row(range.end)
    }

    /// Translate a range of VisibleRowIndex to a range of PhysRowIndex.
    /// The resultant range will be invalidated by inserting or removing rows!
    #[inline]
    pub fn phys_range(&self, range: &Range<VisibleRowIndex>) -> Range<PhysRowIndex> {
        self.phys_row(range.start)..self.phys_row(range.end)
    }

    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    ///
    /// scroll the region up by num_rows.  Any rows that would be scrolled
    /// beyond the top get removed from the screen.
    /// In other words, we remove (top..top+num_rows) and then insert num_rows
    /// at bottom.
    /// If the top of the region is the top of the visible display, rather than
    /// removing the lines we let them go into the scrollback.
    pub fn scroll_up(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        debug!("scroll_up {:?} {}", scroll_region, num_rows);
        let phys_scroll = self.phys_range(scroll_region);
        debug_assert!(num_rows <= phys_scroll.end - phys_scroll.start);

        // Invalidate the lines that will move before they move so that
        // the indices of the lines are stable (we may remove lines below)
        for y in phys_scroll.clone() {
            self.line_mut(y).set_dirty();
        }

        // if we're going to remove lines due to lack of scrollback capacity,
        // remember how many so that we can adjust our insertion point later.
        let lines_removed = if scroll_region.start > 0 {
            // No scrollback available for these;
            // Remove the scrolled lines
            num_rows
        } else {
            let max_allowed = self.physical_rows + self.scrollback_size;
            if self.lines.len() + num_rows >= max_allowed {
                (self.lines.len() + num_rows) - max_allowed
            } else {
                0
            }
        };

        let remove_idx = if scroll_region.start == 0 {
            0
        } else {
            phys_scroll.start
        };

        // To avoid thrashing the heap, prefer to move lines that were
        // scrolled off the top and re-use them at the bottom.
        let to_move = lines_removed.min(num_rows);
        let (to_remove, to_add) = {
            for _ in 0..to_move {
                let mut line = self.lines.remove(remove_idx).unwrap();
                // Make the line like a new one of the appropriate width
                line.reset(self.physical_cols);
                if scroll_region.end as usize == self.physical_rows {
                    self.lines.push_back(line);
                } else {
                    self.lines.insert(phys_scroll.end - 1, line);
                }
            }
            // We may still have some lines to add at the bottom, so
            // return revised counts for remove/add
            (lines_removed - to_move, num_rows - to_move)
        };

        // Perform the removal
        for _ in 0..to_remove {
            self.lines.remove(remove_idx);
        }

        if scroll_region.end as usize == self.physical_rows {
            // It's cheaper to push() than it is insert() at the end
            for _ in 0..to_add {
                self.lines.push_back(Line::new(self.physical_cols));
            }
        } else {
            for _ in 0..to_add {
                self.lines
                    .insert(phys_scroll.end - 1, Line::new(self.physical_cols));
            }
        }
    }

    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    ///
    /// scroll the region down by num_rows.  Any rows that would be scrolled
    /// beyond the bottom get removed from the screen.
    /// In other words, we remove (bottom-num_rows..bottom) and then insert
    /// num_rows at scroll_top.
    pub fn scroll_down(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        debug!("scroll_down {:?} {}", scroll_region, num_rows);
        let phys_scroll = self.phys_range(scroll_region);
        assert!(num_rows <= phys_scroll.end - phys_scroll.start);

        let middle = phys_scroll.end - num_rows;

        // dirty the rows in the region
        for y in phys_scroll.start..middle {
            self.line_mut(y).set_dirty();
        }

        for _ in 0..num_rows {
            self.lines.remove(middle);
        }

        for _ in 0..num_rows {
            self.lines
                .insert(phys_scroll.start, Line::new(self.physical_cols));
        }
    }
}
