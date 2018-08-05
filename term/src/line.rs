use std::ops::Range;
use std::str;
use termwiz::hyperlink::Rule;

use super::*;

#[derive(Debug, Clone, Eq, PartialEq)]
enum ImplicitHyperlinks {
    DontKnow,
    HasNone,
    HasSome,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Line {
    cells: Vec<Cell>,
    dirty: bool,
    has_hyperlink: bool,
    has_implicit_hyperlinks: ImplicitHyperlinks,
}

/// A `CellCluster` is another representation of a Line.
/// A `Vec<CellCluster>` is produced by walking through the Cells in
/// a line and collecting succesive Cells with the same attributes
/// together into a `CellCluster` instance.  Additional metadata to
/// aid in font rendering is also collected.
#[derive(Debug, Clone)]
pub struct CellCluster {
    pub attrs: CellAttributes,
    pub text: String,
    pub byte_to_cell_idx: Vec<usize>,
}

impl CellCluster {
    /// Start off a new cluster with some initial data
    fn new(attrs: CellAttributes, text: &str, cell_idx: usize) -> CellCluster {
        let mut idx = Vec::new();
        for _ in 0..text.len() {
            idx.push(cell_idx);
        }
        CellCluster {
            attrs,
            text: text.into(),
            byte_to_cell_idx: idx,
        }
    }

    /// Add to this cluster
    fn add(&mut self, text: &str, cell_idx: usize) {
        for _ in 0..text.len() {
            self.byte_to_cell_idx.push(cell_idx);
        }
        self.text.push_str(text);
    }
}

impl Line {
    /// Create a new line with the specified number of columns.
    /// Each cell has the default attributes.
    pub fn new(cols: usize) -> Line {
        let mut cells = Vec::with_capacity(cols);
        cells.resize(cols, Default::default());
        Line {
            cells,
            dirty: true,
            has_hyperlink: false,
            has_implicit_hyperlinks: ImplicitHyperlinks::HasNone,
        }
    }

    pub fn reset(&mut self, width: usize) {
        let blank = Cell::default();
        self.cells.resize(width, blank);
        for mut cell in &mut self.cells {
            *cell = Cell::default();
        }
    }

    fn invalidate_grapheme_at_or_before(&mut self, idx: usize) {
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
    }

    /// If we're about to modify a cell obscured by a double-width
    /// character ahead of that cell, we need to nerf that sequence
    /// of cells to avoid partial rendering concerns.
    /// Similarly, when we assign a cell, we need to blank out those
    /// occluded successor cells.
    pub fn set_cell(&mut self, idx: usize, cell: Cell) -> &Cell {
        let width = cell.width();

        // if the line isn't wide enough, pad it out with the default attributes
        if idx + width >= self.cells.len() {
            self.cells.resize(idx + width, Cell::default());
        }

        self.invalidate_grapheme_at_or_before(idx);

        // For double-wide or wider chars, ensure that the cells that
        // are overlapped by this one are blanked out.
        for i in 1..=width.saturating_sub(1) {
            self.cells[idx + i] = Cell::new(' ', cell.attrs().clone());
        }

        self.cells[idx] = cell;
        &self.cells[idx]
    }

    pub fn insert_cell(&mut self, x: usize, cell: Cell) {
        self.invalidate_implicit_links();

        // If we're inserting a wide cell, we should also insert the overlapped cells.
        // We insert them first so that the grapheme winds up left-most.
        let width = cell.width();
        for _ in 1..=width.saturating_sub(1) {
            self.cells.insert(x, Cell::new(' ', cell.attrs().clone()));
        }

        self.cells.insert(x, cell);
    }

    pub fn erase_cell(&mut self, x: usize) {
        self.invalidate_implicit_links();
        self.invalidate_grapheme_at_or_before(x);
        self.cells.remove(x);
        self.cells.push(Cell::default());
    }

    pub fn fill_range(&mut self, cols: impl Iterator<Item = usize>, cell: &Cell) {
        let max_col = self.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            self.set_cell(x, cell.clone());
        }
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
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

    /// Recompose line into the corresponding utf8 string.
    /// In the future, we'll want to decompose into clusters of Cells that share
    /// the same render attributes
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for (_, cell) in self.visible_cells() {
            s.push_str(cell.str());
        }
        s
    }

    /// Returns a substring from the line.
    pub fn columns_as_str(&self, range: Range<usize>) -> String {
        let mut s = String::new();
        for (n, c) in self.visible_cells() {
            if n < range.start {
                continue;
            }
            if n >= range.end {
                break;
            }
            s.push_str(c.str());
        }
        s
    }

    /// Compute the list of CellClusters for this line
    pub fn cluster(&self) -> Vec<CellCluster> {
        let mut last_cluster = None;
        let mut clusters = Vec::new();

        for (cell_idx, c) in self.visible_cells() {
            let cell_str = c.str();

            last_cluster = match last_cluster.take() {
                None => {
                    // Start new cluster
                    Some(CellCluster::new(c.attrs().clone(), cell_str, cell_idx))
                }
                Some(mut last) => {
                    if last.attrs != *c.attrs() {
                        // Flush pending cluster and start a new one
                        clusters.push(last);
                        Some(CellCluster::new(c.attrs().clone(), cell_str, cell_idx))
                    } else {
                        // Add to current cluster
                        last.add(cell_str, cell_idx);
                        Some(last)
                    }
                }
            };
        }

        if let Some(cluster) = last_cluster {
            // Don't forget to include any pending cluster on the final step!
            clusters.push(cluster);
        }

        clusters
    }

    pub fn from_text(s: &str, attrs: &CellAttributes) -> Line {
        let mut cells = Vec::new();

        for sub in unicode_segmentation::UnicodeSegmentation::graphemes(s, true) {
            let cell = Cell::new_grapheme(sub, attrs.clone());
            let width = cell.width();
            cells.push(cell);
            for _ in 1..width {
                cells.push(Cell::new(' ', attrs.clone()));
            }
        }

        Line {
            cells,
            dirty: true,
            has_hyperlink: false,
            has_implicit_hyperlinks: ImplicitHyperlinks::DontKnow,
        }
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn invalidate_implicit_links(&mut self) {
        // Clear any cells that have implicit hyperlinks
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
        // We'll need to recompute them after the line has been mutated
        self.has_implicit_hyperlinks = ImplicitHyperlinks::DontKnow;
    }

    pub fn find_hyperlinks(&mut self, rules: &[Rule]) {
        if self.has_implicit_hyperlinks != ImplicitHyperlinks::DontKnow {
            return;
        }
        self.has_implicit_hyperlinks = ImplicitHyperlinks::HasNone;

        let line = self.as_str();

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
                    self.has_implicit_hyperlinks = ImplicitHyperlinks::HasSome;
                    self.has_hyperlink = true;
                }
            }
        }
    }

    #[inline]
    pub fn set_clean(&mut self) {
        self.dirty = false;
    }

    #[inline]
    pub fn has_hyperlink(&self) -> bool {
        self.has_hyperlink
    }

    #[inline]
    pub fn set_has_hyperlink(&mut self, has: bool) {
        self.has_hyperlink = has;
    }
}

impl<'a> From<&'a str> for Line {
    fn from(s: &str) -> Line {
        Line::from_text(s, &CellAttributes::default())
    }
}
