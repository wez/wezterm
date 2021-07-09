#![cfg_attr(feature = "cargo-clippy", allow(clippy::range_plus_one))]
use super::*;
use log::debug;
use std::collections::VecDeque;
use std::sync::Arc;

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

    /// Whenever we scroll a line off the top of the scrollback, we
    /// increment this.  We use this offset to translate between
    /// PhysRowIndex and StableRowIndex.
    stable_row_index_offset: usize,

    /// config so we can access Maximum number of lines of scrollback
    config: Arc<dyn TerminalConfiguration>,

    /// Whether scrollback is allowed; this is another way of saying
    /// that we're the primary rather than the alternate screen.
    allow_scrollback: bool,

    /// Physical, visible height of the screen (not including scrollback)
    pub physical_rows: usize,
    /// Physical, visible width of the screen
    pub physical_cols: usize,
}

fn scrollback_size(config: &Arc<dyn TerminalConfiguration>, allow_scrollback: bool) -> usize {
    if allow_scrollback {
        config.scrollback_size()
    } else {
        0
    }
}

impl Screen {
    /// Create a new Screen with the specified dimensions.
    /// The Cells in the viewable portion of the screen are set to the
    /// default cell attributes.
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        config: &Arc<dyn TerminalConfiguration>,
        allow_scrollback: bool,
    ) -> Screen {
        let physical_rows = physical_rows.max(1);
        let physical_cols = physical_cols.max(1);

        let mut lines =
            VecDeque::with_capacity(physical_rows + scrollback_size(config, allow_scrollback));
        for _ in 0..physical_rows {
            lines.push_back(Line::with_width(0));
        }

        Screen {
            lines,
            config: Arc::clone(config),
            allow_scrollback,
            physical_rows,
            physical_cols,
            stable_row_index_offset: 0,
        }
    }

    fn scrollback_size(&self) -> usize {
        scrollback_size(&self.config, self.allow_scrollback)
    }

    fn rewrap_lines(
        &mut self,
        physical_cols: usize,
        physical_rows: usize,
        cursor_x: usize,
        cursor_y: PhysRowIndex,
    ) -> (usize, PhysRowIndex) {
        let mut rewrapped = VecDeque::new();
        let mut logical_line: Option<Line> = None;
        let mut logical_cursor_x: Option<usize> = None;
        let mut adjusted_cursor = (cursor_y, cursor_y);

        for (phys_idx, mut line) in self.lines.drain(..).enumerate() {
            line.invalidate_implicit_hyperlinks();
            line.set_dirty();
            let was_wrapped = line.last_cell_was_wrapped();

            if was_wrapped {
                line.set_last_cell_was_wrapped(false);
            }

            let line = match logical_line.take() {
                None => {
                    if phys_idx == cursor_y {
                        logical_cursor_x = Some(cursor_x);
                    }
                    line
                }
                Some(mut prior) => {
                    if phys_idx == cursor_y {
                        logical_cursor_x = Some(cursor_x + prior.cells().len());
                    }
                    prior.append_line(line);
                    prior
                }
            };

            if was_wrapped {
                logical_line.replace(line);
                continue;
            }

            if let Some(x) = logical_cursor_x.take() {
                let num_lines = x / physical_cols;
                let last_x = x - (num_lines * physical_cols);
                adjusted_cursor = (last_x, rewrapped.len() + num_lines);
            }

            if line.cells().len() <= physical_cols {
                rewrapped.push_back(line);
            } else {
                for line in line.wrap(physical_cols) {
                    rewrapped.push_back(line);
                }
            }
        }
        self.lines = rewrapped;

        // If we resized narrower and generated additional lines,
        // we may need to scroll the lines to make room.  However,
        // if the bottom line(s) are whitespace, we'll prune those
        // out first in the rewrap case so that we don't lose any
        // real information off the top of the scrollback
        let capacity = physical_rows + self.scrollback_size();
        while self.lines.len() > capacity
            && self.lines.back().map(Line::is_whitespace).unwrap_or(false)
        {
            self.lines.pop_back();
        }

        adjusted_cursor
    }

    /// Resize the physical, viewable portion of the screen
    pub fn resize(
        &mut self,
        physical_rows: usize,
        physical_cols: usize,
        cursor: CursorPosition,
    ) -> CursorPosition {
        let physical_rows = physical_rows.max(1);
        let physical_cols = physical_cols.max(1);
        if physical_rows == self.physical_rows && physical_cols == self.physical_cols {
            return cursor;
        }
        log::debug!("resize screen to {}x{}", physical_cols, physical_rows);

        // pre-prune blank lines that range from the cursor position to the end of the display;
        // this avoids growing the scrollback size when rapidly switching between normal and
        // maximized states.
        let cursor_phys = self.phys_row(cursor.y);
        for _ in cursor_phys + 1..self.lines.len() {
            if self.lines.back().map(Line::is_whitespace).unwrap_or(false) {
                self.lines.pop_back();
            }
        }

        let (cursor_x, cursor_y) = if physical_cols != self.physical_cols {
            // Check to see if we need to rewrap lines that were
            // wrapped due to reaching the right hand side of the terminal.
            // For each one that we find, we need to join it with its
            // successor and then re-split it.
            // We only do this for the primary, and not for the alternate
            // screen (hence the check for allow_scrollback), to avoid
            // conflicting screen updates with full screen apps.
            if self.allow_scrollback {
                self.rewrap_lines(physical_cols, physical_rows, cursor.x, cursor_phys)
            } else {
                for line in &mut self.lines {
                    if physical_cols < self.physical_cols {
                        // Do a simple prune of the lines instead
                        line.resize(physical_cols);
                    } else {
                        // otherwise: invalidate them
                        line.set_dirty();
                    }
                }
                (cursor.x, cursor_phys)
            }
        } else {
            (cursor.x, cursor_phys)
        };

        let capacity = physical_rows + self.scrollback_size();
        let current_capacity = self.lines.capacity();
        if capacity > current_capacity {
            self.lines.reserve(capacity - current_capacity);
        }

        // If we resized wider and the rewrap resulted in fewer
        // lines than the viewport size, or we resized taller,
        // pad us back out to the viewport size
        while self.lines.len() < physical_rows {
            self.lines.push_back(Line::with_width(0));
        }

        let new_cursor_y;

        if self.config.resize_preserves_scrollback() {
            new_cursor_y = cursor
                .y
                .saturating_add(cursor_y as i64)
                .saturating_sub(cursor_phys as i64)
                .max(0);

            // We need to ensure that the bottom of the screen has sufficient lines;
            // we use simple subtraction of physical_rows from the bottom of the lines
            // array to define the visible region.  Our resize operation may have
            // temporarily violated that, which can result in the cursor unintentionally
            // moving up into the scrollback and damaging the output
            let required_num_rows_after_cursor =
                physical_rows.saturating_sub(new_cursor_y as usize);
            let actual_num_rows_after_cursor = self.lines.len().saturating_sub(cursor_y);
            for _ in actual_num_rows_after_cursor..required_num_rows_after_cursor {
                self.lines.push_back(Line::with_width(0));
            }
        } else {
            // Compute the new cursor location; this is logically the inverse
            // of the phys_row() function, but given the revised cursor_y
            // (the rewrap adjusted physical row of the cursor).  This
            // computes its new VisibleRowIndex given the new viewport size.
            new_cursor_y = cursor_y as VisibleRowIndex
                - (self.lines.len() as VisibleRowIndex - physical_rows as VisibleRowIndex);
        }

        self.physical_rows = physical_rows;
        self.physical_cols = physical_cols;
        CursorPosition {
            x: cursor_x,
            y: new_cursor_y,
            shape: cursor.shape,
            visibility: cursor.visibility,
        }
    }

    /// Get mutable reference to a line, relative to start of scrollback.
    #[inline]
    pub fn line_mut(&mut self, idx: PhysRowIndex) -> &mut Line {
        &mut self.lines[idx]
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

    pub fn insert_cell(&mut self, x: usize, y: VisibleRowIndex, right_margin: usize) {
        let phys_cols = self.physical_cols;

        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.insert_cell(x, Cell::default(), right_margin);
        if line.cells().len() > phys_cols {
            // Don't allow the line width to grow beyond
            // the physical width
            line.resize(phys_cols);
        }
    }

    pub fn erase_cell(&mut self, x: usize, y: VisibleRowIndex, right_margin: usize) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.erase_cell_with_margin(x, right_margin);
    }

    /// Set a cell.  the x and y coordinates are relative to the visible screeen
    /// origin.  0,0 is the top left.
    pub fn set_cell(&mut self, x: usize, y: VisibleRowIndex, cell: &Cell) -> &Cell {
        let line_idx = self.phys_row(y);
        //debug!("set_cell x={} y={} phys={} {:?}", x, y, line_idx, cell);

        let line = self.line_mut(line_idx);
        line.set_cell(x, cell.clone())
    }

    pub fn clear_line(&mut self, y: VisibleRowIndex, cols: Range<usize>, attr: &CellAttributes) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.fill_range(cols, &Cell::new(' ', attr.clone()));
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

    /// Converts a StableRowIndex range to the current effective
    /// physical row index range.  If the StableRowIndex goes off the top
    /// of the scrollback, we'll return the top n rows, but if it goes off
    /// the bottom we'll return the bottom n rows.
    pub fn stable_range(&self, range: &Range<StableRowIndex>) -> Range<PhysRowIndex> {
        let range_len = (range.end - range.start) as usize;

        let first = match self.stable_row_to_phys(range.start) {
            Some(first) => first,
            None => {
                return 0..range_len.min(self.lines.len());
            }
        };

        let last = match self.stable_row_to_phys(range.end.saturating_sub(1)) {
            Some(last) => last,
            None => {
                let last = self.lines.len() - 1;
                return last.saturating_sub(range_len)..last + 1;
            }
        };

        first..last + 1
    }

    /// Translate a range of VisibleRowIndex to a range of PhysRowIndex.
    /// The resultant range will be invalidated by inserting or removing rows!
    #[inline]
    pub fn phys_range(&self, range: &Range<VisibleRowIndex>) -> Range<PhysRowIndex> {
        self.phys_row(range.start)..self.phys_row(range.end)
    }

    #[inline]
    pub fn phys_to_stable_row_index(&self, phys: PhysRowIndex) -> StableRowIndex {
        (phys + self.stable_row_index_offset) as StableRowIndex
    }

    #[inline]
    pub fn stable_row_to_phys(&self, stable: StableRowIndex) -> Option<PhysRowIndex> {
        let idx = stable - self.stable_row_index_offset as isize;
        if idx < 0 || idx >= self.lines.len() as isize {
            // Index is no longer valid
            None
        } else {
            Some(idx as PhysRowIndex)
        }
    }

    #[inline]
    pub fn visible_row_to_stable_row(&self, vis: VisibleRowIndex) -> StableRowIndex {
        self.phys_to_stable_row_index(self.phys_row(vis))
    }

    /// Scroll the scroll_region up by num_rows, respecting left and right margins.
    /// Text outside the left and right margins is left untouched.
    /// Any rows that would be scrolled beyond the top get removed from the screen.
    /// Blank rows are added at the bottom.
    /// If left and right margins are set smaller than the screen width, scrolled rows
    /// will not be placed into scrollback, because they are not complete rows.
    pub fn scroll_up_within_margins(
        &mut self,
        scroll_region: &Range<VisibleRowIndex>,
        left_and_right_margins: &Range<usize>,
        num_rows: usize,
    ) {
        log::debug!(
            "scroll_up_within_margins region:{:?} margins:{:?} rows={}",
            scroll_region,
            left_and_right_margins,
            num_rows
        );

        if left_and_right_margins.start == 0 && left_and_right_margins.end == self.physical_cols {
            return self.scroll_up(scroll_region, num_rows);
        }

        // Need to do the slower, more complex left and right bounded scroll
        let phys_scroll = self.phys_range(scroll_region);

        // The scroll is really a copy + a clear operation
        let region_height = phys_scroll.end - phys_scroll.start;
        let num_rows = num_rows.min(region_height);
        let rows_to_copy = region_height - num_rows;

        if rows_to_copy > 0 {
            for dest_row in phys_scroll.start..phys_scroll.start + rows_to_copy {
                let src_row = dest_row + num_rows;

                // Copy the source cells first
                let cells = {
                    self.lines[src_row]
                        .cells()
                        .iter()
                        .skip(left_and_right_margins.start)
                        .take(left_and_right_margins.end - left_and_right_margins.start)
                        .cloned()
                        .collect::<Vec<_>>()
                };

                // and place them into the dest
                let dest_row = self.line_mut(dest_row);
                dest_row.set_dirty();
                dest_row.invalidate_implicit_hyperlinks();
                let dest_range =
                    left_and_right_margins.start..left_and_right_margins.start + cells.len();
                if dest_row.cells().len() < dest_range.end {
                    dest_row.resize(dest_range.end);
                }

                let tail_range = dest_range.end..left_and_right_margins.end;

                for (src_cell, dest_cell) in
                    cells.into_iter().zip(&mut dest_row.cells_mut()[dest_range])
                {
                    *dest_cell = src_cell.clone();
                }

                dest_row.fill_range(tail_range, &Cell::default());
            }
        }

        // and blank out rows at the bottom
        for n in phys_scroll.start + rows_to_copy..phys_scroll.end {
            let dest_row = self.line_mut(n);
            dest_row.set_dirty();
            dest_row.invalidate_implicit_hyperlinks();
            for cell in dest_row
                .cells_mut()
                .iter_mut()
                .skip(left_and_right_margins.start)
                .take(left_and_right_margins.end - left_and_right_margins.start)
            {
                *cell = Cell::default();
            }
        }
    }

    /// ```text
    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    /// ```
    ///
    /// scroll the region up by num_rows.  Any rows that would be scrolled
    /// beyond the top get removed from the screen.
    /// In other words, we remove (top..top+num_rows) and then insert num_rows
    /// at bottom.
    /// If the top of the region is the top of the visible display, rather than
    /// removing the lines we let them go into the scrollback.
    pub fn scroll_up(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        let phys_scroll = self.phys_range(scroll_region);
        let num_rows = num_rows.min(phys_scroll.end - phys_scroll.start);

        debug!(
            "scroll_up {:?} num_rows={} phys_scroll={:?}",
            scroll_region, num_rows, phys_scroll
        );
        // Invalidate the lines that will move before they move so that
        // the indices of the lines are stable (we may remove lines below)
        // We only need invalidate if the StableRowIndex of the row would be
        // changed by the scroll operation.  For normal newline at the bottom
        // of the screen based scrolling, the StableRowIndex does not change,
        // so we use the scroll region bounds to gate the invalidation.
        if scroll_region.start != 0 || scroll_region.end as usize != self.physical_rows {
            for y in phys_scroll.clone() {
                self.line_mut(y).set_dirty();
            }
        }

        // if we're going to remove lines due to lack of scrollback capacity,
        // remember how many so that we can adjust our insertion point later.
        let lines_removed = if scroll_region.start > 0 {
            // No scrollback available for these;
            // Remove the scrolled lines
            num_rows
        } else {
            let max_allowed = self.physical_rows + self.scrollback_size();
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
                line.resize_and_clear(0);
                line.set_dirty();
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

        if remove_idx == 0 {
            self.stable_row_index_offset += lines_removed;
        }

        if scroll_region.end as usize == self.physical_rows {
            // It's cheaper to push() than it is insert() at the end
            for _ in 0..to_add {
                self.lines.push_back(Line::with_width(0));
            }
        } else {
            for _ in 0..to_add {
                self.lines.insert(phys_scroll.end, Line::with_width(0));
            }
        }
    }

    pub fn erase_scrollback(&mut self) {
        let len = self.lines.len();
        let to_clear = len - self.physical_rows;
        for _ in 0..to_clear {
            self.lines.pop_front();
            self.stable_row_index_offset += 1;
        }
    }

    /// ```text
    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    /// ```
    ///
    /// scroll the region down by num_rows.  Any rows that would be scrolled
    /// beyond the bottom get removed from the screen.
    /// In other words, we remove (bottom-num_rows..bottom) and then insert
    /// num_rows at scroll_top.
    pub fn scroll_down(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        debug!("scroll_down {:?} {}", scroll_region, num_rows);
        let phys_scroll = self.phys_range(scroll_region);
        let num_rows = num_rows.min(phys_scroll.end - phys_scroll.start);

        let middle = phys_scroll.end - num_rows;

        // dirty the rows in the region
        for y in phys_scroll.start..middle {
            self.line_mut(y).set_dirty();
        }

        for _ in 0..num_rows {
            self.lines.remove(middle);
        }

        for _ in 0..num_rows {
            self.lines.insert(phys_scroll.start, Line::with_width(0));
        }
    }

    pub fn scroll_down_within_margins(
        &mut self,
        scroll_region: &Range<VisibleRowIndex>,
        left_and_right_margins: &Range<usize>,
        num_rows: usize,
    ) {
        if left_and_right_margins.start == 0 && left_and_right_margins.end == self.physical_cols {
            return self.scroll_down(scroll_region, num_rows);
        }

        // Need to do the slower, more complex left and right bounded scroll
        let phys_scroll = self.phys_range(scroll_region);

        // The scroll is really a copy + a clear operation
        let region_height = phys_scroll.end - phys_scroll.start;
        let num_rows = num_rows.min(region_height);
        let rows_to_copy = region_height - num_rows;

        if rows_to_copy > 0 {
            for src_row in (phys_scroll.start..phys_scroll.start + rows_to_copy).rev() {
                let dest_row = src_row + num_rows;

                // Copy the source cells first
                let cells = {
                    self.lines[src_row]
                        .cells()
                        .iter()
                        .skip(left_and_right_margins.start)
                        .take(left_and_right_margins.end - left_and_right_margins.start)
                        .cloned()
                        .collect::<Vec<_>>()
                };

                // and place them into the dest
                let dest_row = self.line_mut(dest_row);
                dest_row.set_dirty();
                dest_row.invalidate_implicit_hyperlinks();
                let dest_range =
                    left_and_right_margins.start..left_and_right_margins.start + cells.len();
                if dest_row.cells().len() < dest_range.end {
                    dest_row.resize(dest_range.end);
                }
                let tail_range = dest_range.end..left_and_right_margins.end;

                for (src_cell, dest_cell) in
                    cells.into_iter().zip(&mut dest_row.cells_mut()[dest_range])
                {
                    *dest_cell = src_cell.clone();
                }

                dest_row.fill_range(tail_range, &Cell::default());
            }
        }

        // and blank out rows at the top
        for n in phys_scroll.start..phys_scroll.start + num_rows {
            let dest_row = self.line_mut(n);
            dest_row.set_dirty();
            dest_row.invalidate_implicit_hyperlinks();
            for cell in dest_row
                .cells_mut()
                .iter_mut()
                .skip(left_and_right_margins.start)
                .take(left_and_right_margins.end - left_and_right_margins.start)
            {
                *cell = Cell::default();
            }
        }
    }
}
