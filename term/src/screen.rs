#![allow(clippy::range_plus_one)]
use super::*;
use crate::config::BidiMode;
use log::debug;
use std::collections::VecDeque;
use std::sync::Arc;
use termwiz::input::KeyboardEncoding;
use termwiz::surface::SequenceNo;

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
    lines: VecDeque<Line>,

    /// Whenever we scroll a line off the top of the scrollback, we
    /// increment this.  We use this offset to translate between
    /// PhysRowIndex and StableRowIndex.
    stable_row_index_offset: usize,

    /// config so we can access Maximum number of lines of scrollback
    config: Arc<dyn TerminalConfiguration>,

    /// Whether scrollback is allowed; this is another way of saying
    /// that we're the primary rather than the alternate screen.
    allow_scrollback: bool,

    pub(crate) keyboard_stack: Vec<KeyboardEncoding>,

    /// Physical, visible height of the screen (not including scrollback)
    pub physical_rows: usize,
    /// Physical, visible width of the screen
    pub physical_cols: usize,
    pub dpi: u32,
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
        size: TerminalSize,
        config: &Arc<dyn TerminalConfiguration>,
        allow_scrollback: bool,
        seqno: SequenceNo,
        bidi_mode: BidiMode,
    ) -> Screen {
        let physical_rows = size.rows.max(1);
        let physical_cols = size.cols.max(1);

        let mut lines =
            VecDeque::with_capacity(physical_rows + scrollback_size(config, allow_scrollback));
        for _ in 0..physical_rows {
            let mut line = Line::new(seqno);
            bidi_mode.apply_to_line(&mut line, seqno);
            lines.push_back(line);
        }

        Screen {
            lines,
            config: Arc::clone(config),
            allow_scrollback,
            physical_rows,
            physical_cols,
            stable_row_index_offset: 0,
            dpi: size.dpi,
            keyboard_stack: vec![],
        }
    }

    pub fn full_reset(&mut self) {
        self.keyboard_stack.clear();
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
        seqno: SequenceNo,
    ) -> (usize, PhysRowIndex) {
        let mut rewrapped = VecDeque::new();
        let mut logical_line: Option<Line> = None;
        let mut logical_cursor_x: Option<usize> = None;
        let mut adjusted_cursor = (cursor_x, cursor_y);

        for (phys_idx, mut line) in self.lines.drain(..).enumerate() {
            line.update_last_change_seqno(seqno);
            let was_wrapped = line.last_cell_was_wrapped();

            if was_wrapped {
                line.set_last_cell_was_wrapped(false, seqno);
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
                        logical_cursor_x = Some(cursor_x + prior.len());
                    }
                    prior.append_line(line, seqno);
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

                // Special case: if the cursor lands in column zero, we'll
                // lose track of its logical association with the wrapped
                // line and it won't resize with the line correctly.
                // Put it back on the prior line. The cursor is now
                // technically outside of the viewport width.
                if adjusted_cursor.0 == 0 && adjusted_cursor.1 > 0 {
                    if physical_cols < self.physical_cols {
                        // getting smaller: preserve its original position
                        // on the prior line
                        adjusted_cursor.0 = cursor_x;
                    } else {
                        // getting larger; we were most likely in column 1
                        // or somewhere close. Jump to the end of the
                        // prior line.
                        adjusted_cursor.0 = physical_cols;
                    }
                    adjusted_cursor.1 -= 1;
                }
            }

            if line.len() <= physical_cols {
                rewrapped.push_back(line);
            } else {
                for line in line.wrap(physical_cols, seqno) {
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
        size: TerminalSize,
        cursor: CursorPosition,
        seqno: SequenceNo,
        is_conpty: bool,
    ) -> CursorPosition {
        let physical_rows = size.rows.max(1);
        let physical_cols = size.cols.max(1);

        if physical_rows == self.physical_rows
            && physical_cols == self.physical_cols
            && size.dpi == self.dpi
        {
            return cursor;
        }
        log::debug!(
            "resize screen to {physical_cols}x{physical_rows} dpi={}",
            size.dpi
        );
        self.dpi = size.dpi;

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
                self.rewrap_lines(physical_cols, physical_rows, cursor.x, cursor_phys, seqno)
            } else {
                for line in &mut self.lines {
                    if physical_cols < self.physical_cols {
                        // Do a simple prune of the lines instead
                        line.resize(physical_cols, seqno);
                    } else {
                        // otherwise: invalidate them
                        line.update_last_change_seqno(seqno);
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
            // FIXME: borrow bidi mode from line
            self.lines.push_back(Line::new(seqno));
        }

        let new_cursor_y;

        // true if a resize operation should consider rows that have
        // made it to scrollback as being immutable.
        // When immutable, the resize operation will pad out the screen height
        // with additional blank rows and due to implementation details means
        // that the user will need to scroll back the scrollbar post-resize
        // than they would otherwise.
        //
        // When mutable, resizing the window taller won't add extra rows;
        // instead the resize will tend to have "bottom gravity" meaning that
        // making the window taller will reveal more history than in the other
        // mode.
        //
        // mutable is generally speaking a nicer experience.
        //
        // On Windows, the PTY layer doesn't play well with a mutable scrollback,
        // frequently moving the cursor up to high and erasing portions of the
        // screen.
        //
        // This behavior only happens with the windows pty layer; it doesn't
        // manifest when using eg: ssh directly to a remote unix system.
        let resize_preserves_scrollback = is_conpty;

        if resize_preserves_scrollback {
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
                // FIXME: borrow bidi mode from line
                self.lines.push_back(Line::new(seqno));
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
            seqno,
        }
    }

    /// Get mutable reference to a line, relative to start of scrollback.
    #[inline]
    pub fn line_mut(&mut self, idx: PhysRowIndex) -> &mut Line {
        &mut self.lines[idx]
    }

    /// Returns the number of occupied rows of scrollback
    pub fn scrollback_rows(&self) -> usize {
        self.lines.len()
    }

    /// Sets a line dirty.  The line is relative to the visible origin.
    #[inline]
    pub fn dirty_line(&mut self, idx: VisibleRowIndex, seqno: SequenceNo) {
        let line_idx = self.phys_row(idx);
        if line_idx < self.lines.len() {
            self.lines[line_idx].update_last_change_seqno(seqno);
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

    pub fn insert_cell(
        &mut self,
        x: usize,
        y: VisibleRowIndex,
        right_margin: usize,
        seqno: SequenceNo,
    ) {
        let phys_cols = self.physical_cols;

        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.update_last_change_seqno(seqno);
        line.insert_cell(x, Cell::default(), right_margin, seqno);
        if line.len() > phys_cols {
            // Don't allow the line width to grow beyond
            // the physical width
            line.resize(phys_cols, seqno);
        }
    }

    pub fn erase_cell(
        &mut self,
        x: usize,
        y: VisibleRowIndex,
        right_margin: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
    ) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.erase_cell_with_margin(x, right_margin, seqno, blank_attr);
    }

    /// Set a cell.  the x and y coordinates are relative to the visible screeen
    /// origin.  0,0 is the top left.
    pub fn set_cell(&mut self, x: usize, y: VisibleRowIndex, cell: &Cell, seqno: SequenceNo) {
        let line_idx = self.phys_row(y);
        //debug!("set_cell x={} y={} phys={} {:?}", x, y, line_idx, cell);

        let line = self.line_mut(line_idx);
        line.set_cell(x, cell.clone(), seqno);
    }

    pub fn set_cell_grapheme(
        &mut self,
        x: usize,
        y: VisibleRowIndex,
        text: &str,
        width: usize,
        attr: CellAttributes,
        seqno: SequenceNo,
    ) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        line.set_cell_grapheme(x, text, width, attr, seqno);
    }

    pub fn cell_mut(&mut self, x: usize, y: VisibleRowIndex) -> Option<&mut Cell> {
        let line_idx = self.phys_row(y);
        let line = self.lines.get_mut(line_idx)?;
        line.cells_mut().get_mut(x)
    }

    pub fn get_cell(&mut self, x: usize, y: VisibleRowIndex) -> Option<&Cell> {
        let line_idx = self.phys_row(y);
        let line = self.lines.get_mut(line_idx)?;
        line.cells_mut().get(x)
    }

    pub fn clear_line(
        &mut self,
        y: VisibleRowIndex,
        cols: Range<usize>,
        attr: &CellAttributes,
        seqno: SequenceNo,
        bidi_mode: BidiMode,
    ) {
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        if cols.start == 0 {
            bidi_mode.apply_to_line(line, seqno);
        }
        line.fill_range(cols, &Cell::blank_with_attrs(attr.clone()), seqno);
    }

    /// Ensure that row is within the range of the physical portion of
    /// the screen; 0 .. physical_rows by clamping it to the nearest
    /// boundary.
    #[inline]
    fn clamp_visible_row(&self, row: VisibleRowIndex) -> VisibleRowIndex {
        (row.max(0) as usize).min(self.physical_rows) as VisibleRowIndex
    }

    /// Translate a VisibleRowIndex into a PhysRowIndex.  The resultant index
    /// will be invalidated by inserting or removing rows!
    #[inline]
    pub fn phys_row(&self, row: VisibleRowIndex) -> PhysRowIndex {
        let row = self.clamp_visible_row(row);
        self.lines
            .len()
            .saturating_sub(self.physical_rows)
            .saturating_add(row as PhysRowIndex)
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
        seqno: SequenceNo,
        blank_attr: CellAttributes,
        bidi_mode: BidiMode,
    ) {
        log::debug!(
            "scroll_up_within_margins region:{:?} margins:{:?} rows={}",
            scroll_region,
            left_and_right_margins,
            num_rows
        );

        if left_and_right_margins.start == 0 && left_and_right_margins.end == self.physical_cols {
            return self.scroll_up(scroll_region, num_rows, seqno, blank_attr, bidi_mode);
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
                        .cells_mut()
                        .iter()
                        .skip(left_and_right_margins.start)
                        .take(left_and_right_margins.end - left_and_right_margins.start)
                        .cloned()
                        .collect::<Vec<_>>()
                };

                // and place them into the dest
                let dest_row = self.line_mut(dest_row);
                dest_row.update_last_change_seqno(seqno);
                let dest_range =
                    left_and_right_margins.start..left_and_right_margins.start + cells.len();
                if dest_row.len() < dest_range.end {
                    dest_row.resize(dest_range.end, seqno);
                }

                let tail_range = dest_range.end..left_and_right_margins.end;

                for (src_cell, dest_cell) in
                    cells.into_iter().zip(&mut dest_row.cells_mut()[dest_range])
                {
                    *dest_cell = src_cell.clone();
                }

                dest_row.fill_range(
                    tail_range,
                    &Cell::blank_with_attrs(blank_attr.clone()),
                    seqno,
                );
            }
        }

        // and blank out rows at the bottom
        for n in phys_scroll.start + rows_to_copy..phys_scroll.end {
            let dest_row = self.line_mut(n);
            dest_row.update_last_change_seqno(seqno);
            for cell in dest_row
                .cells_mut()
                .iter_mut()
                .skip(left_and_right_margins.start)
                .take(left_and_right_margins.end - left_and_right_margins.start)
            {
                *cell = Cell::blank_with_attrs(blank_attr.clone());
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
    pub fn scroll_up(
        &mut self,
        scroll_region: &Range<VisibleRowIndex>,
        num_rows: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
        bidi_mode: BidiMode,
    ) {
        let phys_scroll = self.phys_range(scroll_region);
        let num_rows = num_rows.min(phys_scroll.end - phys_scroll.start);
        let scrollback_ok = scroll_region.start == 0 && self.allow_scrollback;
        let insert_at_end = scroll_region.end as usize == self.physical_rows;

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
        if !scrollback_ok {
            for y in phys_scroll.clone() {
                self.line_mut(y).update_last_change_seqno(seqno);
            }
        }

        // if we're going to remove lines due to lack of scrollback capacity,
        // remember how many so that we can adjust our insertion point later.
        let lines_removed = if !scrollback_ok {
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

        if scroll_region.start == 0 {
            for y in self.phys_range(&(0..num_rows as VisibleRowIndex)) {
                self.line_mut(y).compress_for_scrollback();
            }
        }

        let remove_idx = if scroll_region.start == 0 {
            0
        } else {
            phys_scroll.start
        };

        let default_blank = CellAttributes::blank();
        // To avoid thrashing the heap, prefer to move lines that were
        // scrolled off the top and re-use them at the bottom.
        let to_move = lines_removed.min(num_rows);
        let (to_remove, to_add) = {
            for _ in 0..to_move {
                let mut line = self.lines.remove(remove_idx).unwrap();
                let line = if default_blank == blank_attr {
                    Line::new(seqno)
                } else {
                    // Make the line like a new one of the appropriate width
                    line.resize_and_clear(self.physical_cols, seqno, blank_attr.clone());
                    line.update_last_change_seqno(seqno);
                    line
                };
                if insert_at_end {
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

        if remove_idx == 0 && scrollback_ok {
            self.stable_row_index_offset += lines_removed;
        }

        for _ in 0..to_add {
            let mut line = if default_blank == blank_attr {
                Line::new(seqno)
            } else {
                Line::with_width_and_cell(
                    self.physical_cols,
                    Cell::blank_with_attrs(blank_attr.clone()),
                    seqno,
                )
            };
            bidi_mode.apply_to_line(&mut line, seqno);
            if insert_at_end {
                self.lines.push_back(line);
            } else {
                self.lines.insert(phys_scroll.end, line);
            }
        }

        // If we have invalidated the StableRowIndex, mark all subsequent lines as dirty
        if to_remove > 0 || (to_add > 0 && !insert_at_end) {
            for y in self.phys_range(&(scroll_region.end..self.physical_rows as VisibleRowIndex)) {
                self.line_mut(y).update_last_change_seqno(seqno);
            }
        }
    }

    pub fn erase_scrollback(&mut self) {
        let len = self.lines.len();
        let to_clear = len - self.physical_rows;
        for _ in 0..to_clear {
            self.lines.pop_front();
            if self.allow_scrollback {
                self.stable_row_index_offset += 1;
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
    /// scroll the region down by num_rows.  Any rows that would be scrolled
    /// beyond the bottom get removed from the screen.
    /// In other words, we remove (bottom-num_rows..bottom) and then insert
    /// num_rows at scroll_top.
    pub fn scroll_down(
        &mut self,
        scroll_region: &Range<VisibleRowIndex>,
        num_rows: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
        bidi_mode: BidiMode,
    ) {
        debug!("scroll_down {:?} {}", scroll_region, num_rows);
        let phys_scroll = self.phys_range(scroll_region);
        let num_rows = num_rows.min(phys_scroll.end - phys_scroll.start);

        let middle = phys_scroll.end - num_rows;

        // dirty the rows in the region
        for y in phys_scroll.start..middle {
            self.line_mut(y).update_last_change_seqno(seqno);
        }

        for _ in 0..num_rows {
            self.lines.remove(middle);
        }

        let default_blank = CellAttributes::blank();

        for _ in 0..num_rows {
            let mut line = if blank_attr == default_blank {
                Line::new(seqno)
            } else {
                Line::with_width_and_cell(
                    self.physical_cols,
                    Cell::blank_with_attrs(blank_attr.clone()),
                    seqno,
                )
            };
            bidi_mode.apply_to_line(&mut line, seqno);
            self.lines.insert(phys_scroll.start, line);
        }
    }

    pub fn scroll_down_within_margins(
        &mut self,
        scroll_region: &Range<VisibleRowIndex>,
        left_and_right_margins: &Range<usize>,
        num_rows: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
        bidi_mode: BidiMode,
    ) {
        if left_and_right_margins.start == 0 && left_and_right_margins.end == self.physical_cols {
            return self.scroll_down(scroll_region, num_rows, seqno, blank_attr, bidi_mode);
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
                        .cells_mut()
                        .iter()
                        .skip(left_and_right_margins.start)
                        .take(left_and_right_margins.end - left_and_right_margins.start)
                        .cloned()
                        .collect::<Vec<_>>()
                };

                // and place them into the dest
                let dest_row = self.line_mut(dest_row);
                dest_row.update_last_change_seqno(seqno);
                let dest_range =
                    left_and_right_margins.start..left_and_right_margins.start + cells.len();
                if dest_row.len() < dest_range.end {
                    dest_row.resize(dest_range.end, seqno);
                }
                let tail_range = dest_range.end..left_and_right_margins.end;

                for (src_cell, dest_cell) in
                    cells.into_iter().zip(&mut dest_row.cells_mut()[dest_range])
                {
                    *dest_cell = src_cell.clone();
                }

                dest_row.fill_range(
                    tail_range,
                    &Cell::blank_with_attrs(blank_attr.clone()),
                    seqno,
                );
            }
        }

        // and blank out rows at the top
        for n in phys_scroll.start..phys_scroll.start + num_rows {
            let dest_row = self.line_mut(n);
            dest_row.update_last_change_seqno(seqno);
            for cell in dest_row
                .cells_mut()
                .iter_mut()
                .skip(left_and_right_margins.start)
                .take(left_and_right_margins.end - left_and_right_margins.start)
            {
                *cell = Cell::blank_with_attrs(blank_attr.clone());
            }
        }
    }

    pub fn lines_in_phys_range(&self, phys_range: Range<PhysRowIndex>) -> Vec<Line> {
        self.lines
            .iter()
            .skip(phys_range.start)
            .take(phys_range.end - phys_range.start)
            .cloned()
            .collect()
    }

    pub fn get_changed_stable_rows(
        &self,
        stable_lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> Vec<StableRowIndex> {
        let phys = self.stable_range(&stable_lines);
        let mut set = vec![];
        for (idx, line) in self
            .lines
            .iter()
            .enumerate()
            .skip(phys.start)
            .take(phys.end - phys.start)
        {
            if line.changed_since(seqno) {
                set.push(self.phys_to_stable_row_index(idx))
            }
        }
        set
    }

    pub fn with_phys_lines<F>(&self, phys_range: Range<PhysRowIndex>, mut func: F)
    where
        F: FnMut(&[&Line]),
    {
        let (first, second) = self.lines.as_slices();
        let first_range = 0..first.len();
        let second_range = first.len()..first.len() + second.len();
        let first_range = phys_intersection(&first_range, &phys_range);
        let second_range = phys_intersection(&second_range, &phys_range);

        let mut lines: Vec<&Line> = Vec::with_capacity(phys_range.end - phys_range.start);
        for line in &first[first_range] {
            lines.push(line);
        }
        for line in &second[second_range] {
            lines.push(line);
        }
        func(&lines)
    }

    pub fn with_phys_lines_mut<F>(&mut self, phys_range: Range<PhysRowIndex>, mut func: F)
    where
        F: FnMut(&mut [&mut Line]),
    {
        let (first, second) = self.lines.as_mut_slices();
        let first_len = first.len();
        let first_range = 0..first.len();
        let second_range = first.len()..first.len() + second.len();
        let first_range = phys_intersection(&first_range, &phys_range);
        let second_range = phys_intersection(&second_range, &phys_range);

        let mut lines: Vec<&mut Line> = Vec::with_capacity(phys_range.end - phys_range.start);
        for line in &mut first[first_range] {
            lines.push(line);
        }
        for line in &mut second[second_range.start.saturating_sub(first_len)
            ..second_range.end.saturating_sub(first_len)]
        {
            lines.push(line);
        }
        func(&mut lines)
    }

    pub fn for_each_phys_line<F>(&self, mut f: F)
    where
        F: FnMut(usize, &Line),
    {
        for (idx, line) in self.lines.iter().enumerate() {
            f(idx, line);
        }
    }

    pub fn for_each_phys_line_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, &mut Line),
    {
        for (idx, line) in self.lines.iter_mut().enumerate() {
            f(idx, line);
        }
    }

    pub fn for_each_logical_line_in_stable_range_mut<F>(
        &mut self,
        stable_range: Range<StableRowIndex>,
        mut f: F,
    ) where
        F: FnMut(Range<StableRowIndex>, &mut [&mut Line]) -> bool,
    {
        let mut phys_range = self.stable_range(&stable_range);

        // Avoid pathological cases where we have eg: a really long logical line
        // (such as 1.5MB of json) that we previously wrapped.  We don't want to
        // un-wrap, scan, and re-wrap that thing.
        // This is an imperfect length constraint to partially manage the cost.
        const MAX_LOGICAL_LINE_LEN: usize = 1024;

        // Look backwards to find the start of the first logical line
        let mut back_len = 0;
        while phys_range.start > 0 {
            let prior = &mut self.lines[phys_range.start - 1];
            if !prior.last_cell_was_wrapped() {
                break;
            }
            if prior.len() + back_len > MAX_LOGICAL_LINE_LEN {
                break;
            }
            back_len += prior.len();
            phys_range.start -= 1
        }

        let mut phys_row = phys_range.start;
        while phys_row < phys_range.end {
            // Look forwards until we find the end of this logical line
            let mut total_len = 0;
            let mut end_inclusive = phys_row;

            // First pass to measure number of lines
            for idx in phys_row.. {
                if let Some(line) = self.lines.get(idx) {
                    if total_len > 0 && total_len + line.len() > MAX_LOGICAL_LINE_LEN {
                        break;
                    }
                    end_inclusive = idx;
                    total_len += line.len();
                    if !line.last_cell_was_wrapped() {
                        break;
                    }
                } else if idx == phys_row {
                    // No more rows exist
                    return;
                } else {
                    break;
                }
            }

            let phys_range = phys_row..end_inclusive + 1;

            let logical_stable_range = self.phys_to_stable_row_index(phys_row)
                ..self.phys_to_stable_row_index(end_inclusive + 1);

            phys_row = end_inclusive + 1;

            if logical_stable_range.end < stable_range.start {
                continue;
            }
            if logical_stable_range.start > stable_range.end {
                break;
            }

            let mut continue_iteration = false;
            self.with_phys_lines_mut(phys_range, |lines| {
                continue_iteration = f(logical_stable_range.clone(), lines);
            });

            if !continue_iteration {
                break;
            }
        }
    }

    pub fn for_each_logical_line_in_stable_range<F>(
        &self,
        stable_range: Range<StableRowIndex>,
        mut f: F,
    ) where
        F: FnMut(Range<StableRowIndex>, &[&Line]) -> bool,
    {
        let mut phys_range = self.stable_range(&stable_range);

        // Avoid pathological cases where we have eg: a really long logical line
        // (such as 1.5MB of json) that we previously wrapped.  We don't want to
        // un-wrap, scan, and re-wrap that thing.
        // This is an imperfect length constraint to partially manage the cost.
        const MAX_LOGICAL_LINE_LEN: usize = 1024;

        // Look backwards to find the start of the first logical line
        let mut back_len = 0;
        while phys_range.start > 0 {
            let prior = &self.lines[phys_range.start - 1];
            if !prior.last_cell_was_wrapped() {
                break;
            }
            if prior.len() + back_len > MAX_LOGICAL_LINE_LEN {
                break;
            }
            back_len += prior.len();
            phys_range.start -= 1
        }

        let mut phys_row = phys_range.start;
        let mut line_vec: Vec<&Line> = vec![];
        while phys_row < phys_range.end {
            // Look forwards until we find the end of this logical line
            let mut total_len = 0;
            let mut end_inclusive = phys_row;
            line_vec.clear();

            for idx in phys_row.. {
                if let Some(line) = self.lines.get(idx) {
                    if total_len > 0 && total_len + line.len() > MAX_LOGICAL_LINE_LEN {
                        break;
                    }
                    end_inclusive = idx;
                    total_len += line.len();
                    line_vec.push(line);
                    if !line.last_cell_was_wrapped() {
                        break;
                    }
                } else if idx == phys_row {
                    // No more rows exist
                    return;
                } else {
                    break;
                }
            }

            let logical_stable_range = self.phys_to_stable_row_index(phys_row)
                ..self.phys_to_stable_row_index(end_inclusive + 1);

            phys_row = end_inclusive + 1;

            if logical_stable_range.end < stable_range.start {
                continue;
            }
            if logical_stable_range.start > stable_range.end {
                break;
            }

            let continue_iteration = f(logical_stable_range, &line_vec);

            if !continue_iteration {
                break;
            }
        }
    }
}

fn phys_intersection(r1: &Range<PhysRowIndex>, r2: &Range<PhysRowIndex>) -> Range<PhysRowIndex> {
    let start = r1.start.max(r2.start);
    let end = r1.end.min(r2.end);
    if end > start {
        start..end
    } else {
        0..0
    }
}
