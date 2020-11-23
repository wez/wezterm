use crate::cell::{Cell, CellAttributes};

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
    /// Compute the list of CellClusters from a set of visible cells.
    /// The input is typically the result of calling `Line::visible_cells()`.
    pub fn make_cluster<'a>(iter: impl Iterator<Item = (usize, &'a Cell)>) -> Vec<CellCluster> {
        let mut last_cluster = None;
        let mut clusters = Vec::new();

        for (cell_idx, c) in iter {
            let cell_str = c.str();
            let normalized_attr = c.attrs().clone().set_wrapped(false).clone();

            last_cluster = match last_cluster.take() {
                None => {
                    // Start new cluster
                    Some(CellCluster::new(c.attrs().clone(), cell_str, cell_idx))
                }
                Some(mut last) => {
                    if last.attrs != normalized_attr {
                        // Flush pending cluster and start a new one
                        clusters.push(last);
                        Some(CellCluster::new(normalized_attr, cell_str, cell_idx))
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
