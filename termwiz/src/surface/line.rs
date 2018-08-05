use cell::{Cell, CellAttributes};
use hyperlink::Rule;
use range::in_range;
use std::rc::Rc;
use surface::Change;

bitflags! {
    struct LineBits : u8 {
        const NONE = 0;
        /// The contents of the Line have changed and cached or
        /// derived data will need to be reassessed.
        const DIRTY = 1<<0;
        /// The line contains 1+ cells with hyperlinks set
        const HAS_HYPERLINK = 1<<1;
        /// true if we have scanned for implicit hyperlinks
        const SCANNED_IMPLICIT_HYPERLINKS = 1<<2;
        /// true if we found implicit hyperlinks in the last scan
        const HAS_IMPLICIT_HYPERLINKS = 1<<3;
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    bits: LineBits,
    cells: Vec<Cell>,
}

impl Line {
    pub fn with_width(width: usize) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize(width, Cell::default());
        let bits = LineBits::DIRTY;
        Self { bits, cells }
    }

    pub fn resize(&mut self, width: usize) {
        self.cells.resize(width, Cell::default());
        self.bits |= LineBits::DIRTY;
    }

    /// Check whether the dirty bit is set.
    /// If it is set, then something about the line has changed since
    /// the dirty bit was last cleared.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        (self.bits & LineBits::DIRTY) == LineBits::DIRTY
    }

    /// Clear the dirty bit.
    #[inline]
    pub fn clear_dirty(&mut self) {
        self.bits &= !LineBits::DIRTY;
    }

    /// If we have any cells with an implicit hyperlink, remove the hyperlink
    /// from the cell attributes but leave the remainder of the attributes alone.
    pub fn invalidate_implicit_hyperlinks(&mut self) {
        if (self.bits & (LineBits::SCANNED_IMPLICIT_HYPERLINKS | LineBits::HAS_IMPLICIT_HYPERLINKS))
            == LineBits::NONE
        {
            return;
        }

        self.bits &= !LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        if (self.bits & LineBits::HAS_IMPLICIT_HYPERLINKS) == LineBits::NONE {
            return;
        }

        for mut cell in &mut self.cells {
            let replace = match cell.attrs().hyperlink {
                Some(ref link) if link.is_implicit() => Some(Cell::new_grapheme(
                    cell.str(),
                    cell.attrs().clone().set_hyperlink(None).clone(),
                )),
                _ => None,
            };
            if let Some(replace) = replace {
                *cell = replace;
            }
        }

        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;
        self.bits |= LineBits::DIRTY;
    }

    /// Scan through the line and look for sequences that match the provided
    /// rules.  Matching sequences are considered to be implicit hyperlinks
    /// and will have a hyperlink attribute associated with them.
    /// This function will only make changes if the line has been invalidated
    /// since the last time this function was called.
    /// This function does not remember the values of the `rules` slice, so it
    /// is the responsibility of the caller to call `invalidate_implicit_hyperlinks`
    /// if it wishes to call this function with different `rules`.
    pub fn scan_and_create_hyperlinks(&mut self, rules: &[Rule]) {
        if (self.bits & LineBits::SCANNED_IMPLICIT_HYPERLINKS)
            == LineBits::SCANNED_IMPLICIT_HYPERLINKS
        {
            // Has not changed since last time we scanned
            return;
        }

        let line = self.as_str();
        self.bits |= LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;

        for m in Rule::match_hyperlinks(&line, rules) {
            // The capture range is measured in bytes but we need to translate
            // that to the char index of the column.
            for (cell_idx, (byte_idx, _char)) in line.char_indices().enumerate() {
                if self.cells[cell_idx].attrs().hyperlink.is_some() {
                    // Don't replace existing links
                    continue;
                }
                if in_range(byte_idx, &m.range) {
                    let attrs = self.cells[cell_idx]
                        .attrs()
                        .clone()
                        .set_hyperlink(Some(Rc::clone(&m.link)))
                        .clone();
                    let cell = Cell::new_grapheme(self.cells[cell_idx].str(), attrs);
                    self.cells[cell_idx] = cell;
                    self.bits |= LineBits::HAS_IMPLICIT_HYPERLINKS;
                }
            }
        }
    }

    /// Recompose line into the corresponding utf8 string.
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for (_, cell) in self.visible_cells() {
            s.push_str(cell.str());
        }
        s
    }

    /// If we're about to modify a cell obscured by a double-width
    /// character ahead of that cell, we need to nerf that sequence
    /// of cells to avoid partial rendering concerns.
    /// Similarly, when we assign a cell, we need to blank out those
    /// occluded successor cells.
    /// Note that an invalid index will be silently ignored; attempting
    /// to assign to an out of bounds index will not extend the cell array,
    /// and it will not flag an error.
    pub fn set_cell(&mut self, idx: usize, cell: Cell) {
        self.invalidate_implicit_hyperlinks();
        self.bits |= LineBits::DIRTY;
        // Assumption: that the width of a grapheme is never > 2.
        // This constrains the amount of look-back that we need to do here.
        if idx > 0 {
            let prior = idx - 1;
            let width = self.cells[prior].width();
            if width > 1 {
                let attrs = self.cells[prior].attrs().clone();
                for nerf in prior..prior + width {
                    self.cells[nerf] = Cell::new(' ', attrs.clone());
                }
            }
        }

        // For double-wide or wider chars, ensure that the cells that
        // are overlapped by this one are blanked out.
        let width = cell.width();
        for i in 1..=width.saturating_sub(1) {
            self.cells
                .get_mut(idx + i)
                .map(|target| *target = Cell::new(' ', cell.attrs().clone()));
        }

        self.cells.get_mut(idx).map(|target| *target = cell);
    }

    pub fn fill_range(&mut self, cols: impl Iterator<Item = usize>, cell: &Cell) {
        let max_col = self.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            // FIXME: we can skip the look-back for second and subsequent iterations
            self.set_cell(x, cell.clone());
        }
    }

    /// Iterates the visible cells, respecting the width of the cell.
    /// For instance, a double-width cell overlaps the following (blank)
    /// cell, so that blank cell is omitted from the iterator results.
    /// The iterator yields (column_index, Cell).  Column index is the
    /// index into Self::cells, and due to the possibility of skipping
    /// the characters that follow wide characters, the column index may
    /// skip some positions.  It is returned as a convenience to the consumer
    /// as using .enumerate() on this iterator wouldn't be as useful.
    pub fn visible_cells(&self) -> impl Iterator<Item = (usize, &Cell)> {
        let mut skip_width = 0;
        self.cells.iter().enumerate().filter(move |(_idx, cell)| {
            if skip_width > 0 {
                skip_width -= 1;
                false
            } else {
                skip_width = cell.width().saturating_sub(1);
                true
            }
        })
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Given a starting attribute value, produce a series of Change
    /// entries to recreate the current line
    pub fn changes(&self, start_attr: &CellAttributes) -> Vec<Change> {
        let mut result = Vec::new();
        let mut attr = start_attr.clone();
        let mut text_run = String::new();

        for (_, cell) in self.visible_cells() {
            if *cell.attrs() == attr {
                text_run.push_str(cell.str());
            } else {
                // flush out the current text run
                if text_run.len() > 0 {
                    result.push(Change::Text(text_run.clone()));
                    text_run.clear();
                }

                attr = cell.attrs().clone();
                result.push(Change::AllAttributes(attr.clone()));
                text_run.push_str(cell.str());
            }
        }

        // flush out any remaining text run
        if text_run.len() > 0 {
            // if this is just spaces then it is likely cheaper
            // to emit ClearToEndOfLine instead.
            if attr == CellAttributes::default()
                .set_background(attr.background)
                .clone()
            {
                let left = text_run.trim_right_matches(' ').to_string();
                let num_trailing_spaces = text_run.len() - left.len();

                if num_trailing_spaces > 0 {
                    if left.len() > 0 {
                        result.push(Change::Text(left.to_string()));
                    } else if result.len() == 1 {
                        // if the only queued result prior to clearing
                        // to the end of the line is an attribute change,
                        // we can prune it out and return just the line
                        // clearing operation
                        match result[0] {
                            Change::AllAttributes(_) => result.clear(),
                            _ => {}
                        }
                    }

                    // Since this function is only called in the full repaint
                    // case, and we always emit a clear screen with the default
                    // background color, we don't need to emit an instruction
                    // to clear the remainder of the line unless it has a different
                    // background color.
                    if attr.background != Default::default() {
                        result.push(Change::ClearToEndOfLine(attr.background));
                    }
                } else {
                    result.push(Change::Text(text_run.clone()));
                }
            } else {
                result.push(Change::Text(text_run.clone()));
            }
        }

        result
    }
}
