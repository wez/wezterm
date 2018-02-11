use std::str;

use super::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Line {
    pub cells: Vec<Cell>,
    dirty: bool,
    has_hyperlink: bool,
}

/// A CellCluster is another representation of a Line.
/// A Vec<CellCluster> is produced by walking through the Cells in
/// a line and collecting succesive Cells with the same attributes
/// together into a CellCluster instance.  Additional metadata to
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
        }
    }

    /// Recompose line into the corresponding utf8 string.
    /// In the future, we'll want to decompose into clusters of Cells that share
    /// the same render attributes
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for c in self.cells.iter() {
            s.push_str(str::from_utf8(c.bytes()).unwrap_or("?"));
        }
        s
    }

    /// Compute the list of CellClusters for this line
    pub fn cluster(&self) -> Vec<CellCluster> {
        let mut last_cluster = None;
        let mut clusters = Vec::new();

        for (cell_idx, c) in self.cells.iter().enumerate() {
            let cell_str = str::from_utf8(c.bytes()).unwrap_or("?");

            last_cluster = match last_cluster.take() {
                None => {
                    // Start new cluster
                    Some(CellCluster::new(c.attrs.clone(), cell_str, cell_idx))
                }
                Some(mut last) => {
                    if last.attrs != c.attrs {
                        // Flush pending cluster and start a new one
                        clusters.push(last);
                        Some(CellCluster::new(c.attrs.clone(), cell_str, cell_idx))
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

        for (_, sub) in unicode_segmentation::UnicodeSegmentation::grapheme_indices(s, true) {
            cells.push(Cell::new(sub, attrs))
        }

        Line {
            cells,
            dirty: true,
            has_hyperlink: false,
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
