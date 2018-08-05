use cell::{Cell, CellAttributes};
use surface::Change;

#[derive(Debug, Clone)]
pub struct Line {
    cells: Vec<Cell>,
}

impl Line {
    pub fn with_width(width: usize) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize(width, Cell::default());
        Self { cells }
    }

    pub fn resize(&mut self, width: usize) {
        self.cells.resize(width, Cell::default());
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
