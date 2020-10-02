use crate::config::keyassignment::PaneDirection;
use crate::mux::domain::DomainId;
use crate::mux::pane::*;
use crate::mux::{Mux, WindowId};
use crate::server::codec::{PaneEntry, PaneNode};
use bintree::PathBranch;
use portable_pty::PtySize;
use rangeset::range_intersection;
use serde::{Deserialize, Serialize};
use std::cell::{RefCell, RefMut};
use std::convert::TryInto;
use std::rc::Rc;

pub type Tree = bintree::Tree<Rc<dyn Pane>, SplitDirectionAndSize>;
pub type Cursor = bintree::Cursor<Rc<dyn Pane>, SplitDirectionAndSize>;

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type TabId = usize;

/// A Tab is a container of Panes
pub struct Tab {
    id: TabId,
    pane: RefCell<Option<Tree>>,
    size: RefCell<PtySize>,
    active: RefCell<usize>,
    zoomed: RefCell<Option<Rc<dyn Pane>>>,
}

#[derive(Clone)]
pub struct PositionedPane {
    /// The topological pane index that can be used to reference this pane
    pub index: usize,
    /// true if this is the active pane at the time the position was computed
    pub is_active: bool,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub left: usize,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub top: usize,
    /// The width of this pane in cells
    pub width: usize,
    /// The height of this pane in cells
    pub height: usize,
    /// The pane instance
    pub pane: Rc<dyn Pane>,
}

impl std::fmt::Debug for PositionedPane {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("PositionedPane")
            .field("index", &self.index)
            .field("is_active", &self.is_active)
            .field("left", &self.left)
            .field("top", &self.top)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pane_id", &self.pane.pane_id())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// The size is of the (first, second) child of the split
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct SplitDirectionAndSize {
    pub direction: SplitDirection,
    pub first: PtySize,
    pub second: PtySize,
}

impl SplitDirectionAndSize {
    fn top_of_second(&self) -> usize {
        match self.direction {
            SplitDirection::Horizontal => 0,
            SplitDirection::Vertical => self.first.rows as usize + 1,
        }
    }

    fn left_of_second(&self) -> usize {
        match self.direction {
            SplitDirection::Horizontal => self.first.cols as usize + 1,
            SplitDirection::Vertical => 0,
        }
    }

    pub fn width(&self) -> u16 {
        if self.direction == SplitDirection::Horizontal {
            self.first.cols + self.second.cols + 1
        } else {
            self.first.cols
        }
    }

    pub fn height(&self) -> u16 {
        if self.direction == SplitDirection::Vertical {
            self.first.rows + self.second.rows + 1
        } else {
            self.first.rows
        }
    }

    pub fn size(&self) -> PtySize {
        let cell_width = self.first.pixel_width / self.first.cols;
        let cell_height = self.first.pixel_height / self.first.rows;

        let rows = self.height();
        let cols = self.width();

        PtySize {
            rows,
            cols,
            pixel_height: cell_height * rows,
            pixel_width: cell_width * cols,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PositionedSplit {
    /// The topological node index that can be used to reference this split
    pub index: usize,
    pub direction: SplitDirection,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this split, in cells.
    pub left: usize,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this split, in cells.
    pub top: usize,
    /// For Horizontal splits, how tall the split should be, for Vertical
    /// splits how wide it should be
    pub size: usize,
}

fn is_pane(pane: &Rc<dyn Pane>, other: &Option<&Rc<dyn Pane>>) -> bool {
    if let Some(other) = other {
        other.pane_id() == pane.pane_id()
    } else {
        false
    }
}

fn pane_tree(
    tree: &Tree,
    tab_id: TabId,
    window_id: WindowId,
    active: Option<&Rc<dyn Pane>>,
    zoomed: Option<&Rc<dyn Pane>>,
) -> PaneNode {
    match tree {
        Tree::Empty => PaneNode::Empty,
        Tree::Node { left, right, data } => PaneNode::Split {
            left: Box::new(pane_tree(&*left, tab_id, window_id, active, zoomed)),
            right: Box::new(pane_tree(&*right, tab_id, window_id, active, zoomed)),
            node: data.unwrap(),
        },
        Tree::Leaf(pane) => {
            let dims = pane.renderer().get_dimensions();
            let working_dir = pane.get_current_working_dir();

            PaneNode::Leaf(PaneEntry {
                window_id,
                tab_id,
                pane_id: pane.pane_id(),
                title: pane.get_title(),
                is_active_pane: is_pane(pane, &active),
                is_zoomed_pane: is_pane(pane, &zoomed),
                size: PtySize {
                    cols: dims.cols as u16,
                    rows: dims.viewport_rows as u16,
                    pixel_height: 0,
                    pixel_width: 0,
                },
                working_dir: working_dir.map(Into::into),
            })
        }
    }
}

fn build_from_pane_tree<F>(
    tree: bintree::Tree<PaneEntry, SplitDirectionAndSize>,
    active: &mut Option<Rc<dyn Pane>>,
    zoomed: &mut Option<Rc<dyn Pane>>,
    make_pane: &F,
) -> Tree
where
    F: Fn(PaneEntry) -> Rc<dyn Pane>,
{
    match tree {
        bintree::Tree::Empty => Tree::Empty,
        bintree::Tree::Node { left, right, data } => Tree::Node {
            left: Box::new(build_from_pane_tree(*left, active, zoomed, make_pane)),
            right: Box::new(build_from_pane_tree(*right, active, zoomed, make_pane)),
            data,
        },
        bintree::Tree::Leaf(entry) => {
            let is_zoomed_pane = entry.is_zoomed_pane;
            let is_active_pane = entry.is_active_pane;
            let pane = make_pane(entry);
            if is_zoomed_pane {
                zoomed.replace(Rc::clone(&pane));
            }
            if is_active_pane {
                active.replace(Rc::clone(&pane));
            }
            Tree::Leaf(pane)
        }
    }
}

/// Computes the minimum (x, y) size based on the panes in this portion
/// of the tree.
fn compute_min_size(tree: &mut Tree) -> (usize, usize) {
    match tree {
        Tree::Node { data: None, .. } | Tree::Empty => (1, 1),
        Tree::Node {
            left,
            right,
            data: Some(data),
        } => {
            let (left_x, left_y) = compute_min_size(&mut *left);
            let (right_x, right_y) = compute_min_size(&mut *right);
            match data.direction {
                SplitDirection::Vertical => (left_x.max(right_x), left_y + right_y + 1),
                SplitDirection::Horizontal => (left_x + right_x + 1, left_y.max(right_y)),
            }
        }
        Tree::Leaf(_) => (1, 1),
    }
}

fn adjust_x_size(tree: &mut Tree, mut x_adjust: isize, cell_dimensions: &PtySize) {
    let (min_x, _) = compute_min_size(tree);
    while x_adjust != 0 {
        match tree {
            Tree::Empty | Tree::Leaf(_) => return,
            Tree::Node { data: None, .. } => return,
            Tree::Node {
                left,
                right,
                data: Some(data),
            } => match data.direction {
                SplitDirection::Vertical => {
                    let new_cols = (data.first.cols as isize)
                        .saturating_add(x_adjust)
                        .max(min_x as isize);
                    x_adjust = new_cols.saturating_sub(data.first.cols as isize);

                    if x_adjust != 0 {
                        adjust_x_size(&mut *left, x_adjust, cell_dimensions);
                        data.first.cols = new_cols.try_into().unwrap();
                        data.first.pixel_width =
                            data.first.cols.saturating_mul(cell_dimensions.pixel_width);

                        adjust_x_size(&mut *right, x_adjust, cell_dimensions);
                        data.second.cols = data.first.cols;
                        data.second.pixel_width = data.first.pixel_width;
                    }
                    return;
                }
                SplitDirection::Horizontal if x_adjust > 0 => {
                    adjust_x_size(&mut *left, 1, cell_dimensions);
                    data.first.cols += 1;
                    data.first.pixel_width =
                        data.first.cols.saturating_mul(cell_dimensions.pixel_width);
                    x_adjust -= 1;

                    if x_adjust > 0 {
                        adjust_x_size(&mut *right, 1, cell_dimensions);
                        data.second.cols += 1;
                        data.second.pixel_width =
                            data.second.cols.saturating_mul(cell_dimensions.pixel_width);
                        x_adjust -= 1;
                    }
                }
                SplitDirection::Horizontal => {
                    // x_adjust is negative
                    if data.first.cols > 1 {
                        adjust_x_size(&mut *left, -1, cell_dimensions);
                        data.first.cols -= 1;
                        data.first.pixel_width =
                            data.first.cols.saturating_mul(cell_dimensions.pixel_width);
                        x_adjust += 1;
                    }
                    if x_adjust < 0 && data.second.cols > 1 {
                        adjust_x_size(&mut *right, -1, cell_dimensions);
                        data.second.cols -= 1;
                        data.second.pixel_width =
                            data.second.cols.saturating_mul(cell_dimensions.pixel_width);
                        x_adjust += 1;
                    }
                }
            },
        }
    }
}

fn adjust_y_size(tree: &mut Tree, mut y_adjust: isize, cell_dimensions: &PtySize) {
    let (_, min_y) = compute_min_size(tree);
    while y_adjust != 0 {
        match tree {
            Tree::Empty | Tree::Leaf(_) => return,
            Tree::Node { data: None, .. } => return,
            Tree::Node {
                left,
                right,
                data: Some(data),
            } => match data.direction {
                SplitDirection::Horizontal => {
                    let new_rows = (data.first.rows as isize)
                        .saturating_add(y_adjust)
                        .max(min_y as isize);
                    y_adjust = new_rows.saturating_sub(data.first.rows as isize);

                    if y_adjust != 0 {
                        adjust_y_size(&mut *left, y_adjust, cell_dimensions);
                        data.first.rows = new_rows.try_into().unwrap();
                        data.first.pixel_height =
                            data.first.rows.saturating_mul(cell_dimensions.pixel_height);

                        adjust_y_size(&mut *right, y_adjust, cell_dimensions);
                        data.second.rows = data.first.rows;
                        data.second.pixel_height = data.first.pixel_height;
                    }
                    return;
                }
                SplitDirection::Vertical if y_adjust > 0 => {
                    adjust_y_size(&mut *left, 1, cell_dimensions);
                    data.first.rows += 1;
                    data.first.pixel_height =
                        data.first.rows.saturating_mul(cell_dimensions.pixel_height);
                    y_adjust -= 1;
                    if y_adjust > 0 {
                        adjust_y_size(&mut *right, 1, cell_dimensions);
                        data.second.rows += 1;
                        data.second.pixel_height = data
                            .second
                            .rows
                            .saturating_mul(cell_dimensions.pixel_height);
                        y_adjust -= 1;
                    }
                }
                SplitDirection::Vertical => {
                    // y_adjust is negative
                    if data.first.rows > 1 {
                        adjust_y_size(&mut *left, -1, cell_dimensions);
                        data.first.rows -= 1;
                        data.first.pixel_height =
                            data.first.rows.saturating_mul(cell_dimensions.pixel_height);
                        y_adjust += 1;
                    }
                    if y_adjust < 0 && data.second.rows > 1 {
                        adjust_y_size(&mut *right, -1, cell_dimensions);
                        data.second.rows -= 1;
                        data.second.pixel_height = data
                            .second
                            .rows
                            .saturating_mul(cell_dimensions.pixel_height);
                        y_adjust += 1;
                    }
                }
            },
        }
    }
}

fn apply_sizes_from_splits(tree: &Tree, size: &PtySize) {
    match tree {
        Tree::Empty => return,
        Tree::Node { data: None, .. } => return,
        Tree::Node {
            left,
            right,
            data: Some(data),
        } => {
            apply_sizes_from_splits(&*left, &data.first);
            apply_sizes_from_splits(&*right, &data.second);
        }
        Tree::Leaf(pane) => {
            pane.resize(*size).ok();
        }
    }
}

fn cell_dimensions(size: &PtySize) -> PtySize {
    PtySize {
        rows: 1,
        cols: 1,
        pixel_width: size.pixel_width / size.cols,
        pixel_height: size.pixel_height / size.rows,
    }
}

impl Tab {
    pub fn new(size: &PtySize) -> Self {
        Self {
            id: TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            pane: RefCell::new(Some(Tree::new())),
            size: RefCell::new(*size),
            active: RefCell::new(0),
            zoomed: RefCell::new(None),
        }
    }

    /// Called by the multiplexer client when building a local tab to
    /// mirror a remote tab.  The supplied `root` is the information
    /// about our counterpart in the the remote server.
    /// This method builds a local tree based on the remote tree which
    /// then replaces the local tree structure.
    ///
    /// The `make_pane` function is provided by the caller, and its purpose
    /// is to lookup an existing Pane that corresponds to the provided
    /// PaneEntry, or to create a new Pane from that entry.
    /// make_pane is expected to add the pane to the mux if it creates
    /// a new pane, otherwise the pane won't poll/update in the GUI.
    pub fn sync_with_pane_tree<F>(&self, size: PtySize, root: PaneNode, make_pane: F)
    where
        F: Fn(PaneEntry) -> Rc<dyn Pane>,
    {
        let mut active = None;
        let mut zoomed = None;

        log::debug!("sync_with_pane_tree with size {:?}", size);

        let t = build_from_pane_tree(root.into_tree(), &mut active, &mut zoomed, &make_pane);
        let mut cursor = t.cursor();

        *self.active.borrow_mut() = 0;
        if let Some(active) = active {
            // Resolve the active pane to its index
            let mut index = 0;
            loop {
                if let Some(pane) = cursor.leaf_mut() {
                    if active.pane_id() == pane.pane_id() {
                        // Found it
                        *self.active.borrow_mut() = index;
                        break;
                    }
                    index += 1;
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(c) => {
                        // Didn't find it
                        cursor = c;
                        break;
                    }
                }
            }
        }
        self.pane.borrow_mut().replace(cursor.tree());
        *self.zoomed.borrow_mut() = zoomed;
        *self.size.borrow_mut() = size;

        self.resize(size);

        log::debug!(
            "sync tab: {:#?} zoomed: {} {:#?}",
            size,
            self.zoomed.borrow().is_some(),
            self.iter_panes()
        );
        assert!(self.pane.borrow().is_some());
    }

    pub fn codec_pane_tree(&self) -> PaneNode {
        let mux = Mux::get().unwrap();
        let tab_id = self.id;
        let window_id = match mux.window_containing_tab(tab_id) {
            Some(w) => w,
            None => {
                log::error!("no window contains tab {}", tab_id);
                return PaneNode::Empty;
            }
        };

        let zoomed = self.zoomed.borrow();
        let active = self.get_active_pane();
        if let Some(root) = self.pane.borrow().as_ref() {
            pane_tree(root, tab_id, window_id, active.as_ref(), zoomed.as_ref())
        } else {
            PaneNode::Empty
        }
    }

    /// Returns a count of how many panes are in this tab
    pub fn count_panes(&self) -> usize {
        let mut count = 0;
        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                count += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    return count;
                }
            }
        }
    }

    pub fn set_zoomed(&self, zoomed: bool) {
        if self.zoomed.borrow().is_some() == zoomed {
            // Current zoom state matches intended zoom state,
            // so we have nothing to do.
            return;
        }
        self.toggle_zoom();
    }

    pub fn toggle_zoom(&self) {
        let size = *self.size.borrow();
        if self.zoomed.borrow_mut().take().is_some() {
            // We were zoomed, but now we are not.
            // Re-apply the size to the panes
            if let Some(pane) = self.get_active_pane() {
                pane.set_zoomed(false);
            }

            let mut root = self.pane.borrow_mut();
            apply_sizes_from_splits(root.as_mut().unwrap(), &size);
        } else {
            // We weren't zoomed, but now we want to zoom.
            // Locate the active pane
            if let Some(pane) = self.get_active_pane() {
                pane.set_zoomed(true);
                pane.resize(size).ok();
                self.zoomed.borrow_mut().replace(pane);
            }
        }
    }

    /// Walks the pane tree to produce the topologically ordered flattened
    /// list of PositionedPane instances along with their positioning information.
    pub fn iter_panes(&self) -> Vec<PositionedPane> {
        let mut panes = vec![];

        if let Some(zoomed) = self.zoomed.borrow().as_ref() {
            let size = *self.size.borrow();
            panes.push(PositionedPane {
                index: 0,
                is_active: true,
                left: 0,
                top: 0,
                width: size.cols.into(),
                height: size.rows.into(),
                pane: Rc::clone(zoomed),
            });
            return panes;
        }

        let active_idx = *self.active.borrow();
        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();

        loop {
            if cursor.is_leaf() {
                let index = panes.len();
                let mut left = 0usize;
                let mut top = 0usize;
                let mut parent_size = None;
                for (branch, node) in cursor.path_to_root() {
                    if let Some(node) = node {
                        if parent_size.is_none() {
                            parent_size.replace(if branch == PathBranch::IsRight {
                                node.second
                            } else {
                                node.first
                            });
                        }
                        if branch == PathBranch::IsRight {
                            top += node.top_of_second();
                            left += node.left_of_second();
                        }
                    }
                }

                let pane = Rc::clone(cursor.leaf_mut().unwrap());
                let dims = parent_size.unwrap_or_else(|| *self.size.borrow());

                panes.push(PositionedPane {
                    index,
                    is_active: index == active_idx,
                    left,
                    top,
                    width: dims.cols as _,
                    height: dims.rows as _,
                    pane,
                });
            }

            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    break;
                }
            }
        }

        panes
    }

    pub fn iter_splits(&self) -> Vec<PositionedSplit> {
        let mut dividers = vec![];
        if self.zoomed.borrow().is_some() {
            return dividers;
        }

        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();
        let mut index = 0;

        loop {
            if !cursor.is_leaf() {
                let mut left = 0usize;
                let mut top = 0usize;
                for (branch, p) in cursor.path_to_root() {
                    if let Some(p) = p {
                        if branch == PathBranch::IsRight {
                            left += p.left_of_second();
                            top += p.top_of_second();
                        }
                    }
                }
                if let Ok(Some(node)) = cursor.node_mut() {
                    match node.direction {
                        SplitDirection::Horizontal => left += node.first.cols as usize,
                        SplitDirection::Vertical => top += node.first.rows as usize,
                    }

                    dividers.push(PositionedSplit {
                        index,
                        direction: node.direction,
                        left,
                        top,
                        size: if node.direction == SplitDirection::Horizontal {
                            node.height() as usize
                        } else {
                            node.width() as usize
                        },
                    })
                }
                index += 1;
            }

            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    break;
                }
            }
        }

        dividers
    }

    pub fn tab_id(&self) -> TabId {
        self.id
    }

    pub fn get_size(&self) -> PtySize {
        *self.size.borrow()
    }

    /// Apply the new size of the tab to the panes contained within.
    /// The delta between the current and the new size is computed,
    /// and is distributed between the splits.  For small resizes
    /// this algorithm biases towards adjusting the left/top nodes
    /// first.  For large resizes this tends to proportionally adjust
    /// the relative sizes of the elements in a split.
    pub fn resize(&self, size: PtySize) {
        if size.rows == 0 || size.cols == 0 {
            // Ignore "impossible" resize requests
            return;
        }

        // Un-zoom first, so that the layout can be reasoned about
        // more easily.
        let was_zoomed = self.zoomed.borrow().is_some();
        self.set_zoomed(false);

        {
            let mut root = self.pane.borrow_mut();
            let dims = cell_dimensions(&size);
            let (min_x, min_y) = compute_min_size(root.as_mut().unwrap());
            let current_size = *self.size.borrow();

            // Constrain the new size to the minimum possible dimensions
            let cols = size.cols.max(min_x as u16);
            let rows = size.rows.max(min_y as u16);
            let size = PtySize {
                rows,
                cols,
                pixel_width: cols * dims.pixel_width,
                pixel_height: rows * dims.pixel_height,
            };

            if size != current_size {
                // Update the split nodes with adjusted sizes
                adjust_x_size(
                    root.as_mut().unwrap(),
                    cols as isize - current_size.cols as isize,
                    &dims,
                );
                adjust_y_size(
                    root.as_mut().unwrap(),
                    rows as isize - current_size.rows as isize,
                    &dims,
                );

                *self.size.borrow_mut() = size;

                // And then resize the individual panes to match
                apply_sizes_from_splits(root.as_mut().unwrap(), &size);
            }
        }

        // And finally restore the zoom, if appropriate
        self.set_zoomed(was_zoomed);
    }

    fn apply_pane_size(&self, pane_size: PtySize, cursor: &mut Cursor) {
        let cell_width = pane_size.pixel_width / pane_size.cols;
        let cell_height = pane_size.pixel_height / pane_size.rows;
        if let Ok(Some(node)) = cursor.node_mut() {
            // Adjust the size of the node; we preserve the size of the first
            // child and adjust the second, so if we are split down the middle
            // and the window is made wider, the right column will grow in
            // size, leaving the left at its current width.
            if node.direction == SplitDirection::Horizontal {
                node.first.rows = pane_size.rows;
                node.second.rows = pane_size.rows;

                node.second.cols = pane_size.cols.saturating_sub(1 + node.first.cols);
            } else {
                node.first.cols = pane_size.cols;
                node.second.cols = pane_size.cols;

                node.second.rows = pane_size.rows.saturating_sub(1 + node.first.rows);
            }
            node.first.pixel_width = node.first.cols * cell_width;
            node.first.pixel_height = node.first.rows * cell_height;

            node.second.pixel_width = node.second.cols * cell_width;
            node.second.pixel_height = node.second.rows * cell_height;
        }
    }

    /// Called when running in the mux server after an individual pane
    /// has been resized.
    /// Because the split manipulation happened on the GUI we "lost"
    /// the information that would have allowed us to call resize_split_by()
    /// and instead need to back-infer the split size information.
    /// We rely on the client to have resized (or be in the process
    /// of resizing) affected panes consistently with its own Tab
    /// tree model.
    /// This method does a simple tree walk to the leaves to back-propagate
    /// the size of the panes up to their containing node split data.
    /// Without this step, disconnecting and reconnecting would cause
    /// the GUI to use stale size information for the window it spawns
    /// to attach this tab.
    pub fn rebuild_splits_sizes_from_contained_panes(&self) {
        if self.zoomed.borrow().is_some() {
            return;
        }

        fn compute_size(node: &mut Tree) -> Option<PtySize> {
            match node {
                Tree::Empty => None,
                Tree::Leaf(pane) => {
                    let dims = pane.renderer().get_dimensions();
                    let size = PtySize {
                        cols: dims.cols as u16,
                        rows: dims.viewport_rows as u16,
                        pixel_height: 0,
                        pixel_width: 0,
                    };
                    Some(size)
                }
                Tree::Node { left, right, data } => {
                    if let Some(data) = data {
                        if let Some(first) = compute_size(left) {
                            data.first = first;
                        }
                        if let Some(second) = compute_size(right) {
                            data.second = second;
                        }
                        Some(data.size())
                    } else {
                        None
                    }
                }
            }
        }

        let mut root = self.pane.borrow_mut();
        if let Some(root) = root.as_mut() {
            if let Some(size) = compute_size(root) {
                *self.size.borrow_mut() = size;
            }
        }
    }

    /// Given split_index, the topological index of a split returned by
    /// iter_splits() as PositionedSplit::index, revised the split position
    /// by the provided delta; positive values move the split to the right/bottom,
    /// and negative values to the left/top.
    /// The adjusted size is propogated downwards to contained children and
    /// their panes are resized accordingly.
    pub fn resize_split_by(&self, split_index: usize, delta: isize) {
        if self.zoomed.borrow().is_some() {
            return;
        }

        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();
        let mut index = 0;

        // Position cursor on the specified split
        loop {
            if !cursor.is_leaf() {
                if index == split_index {
                    // Found it
                    break;
                }
                index += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    // Didn't find it
                    root.replace(c.tree());
                    return;
                }
            }
        }

        // Now cursor is looking at the split
        self.adjust_node_at_cursor(&mut cursor, delta);
        self.cascade_size_from_cursor(root, cursor);
    }

    fn adjust_node_at_cursor(&self, cursor: &mut Cursor, delta: isize) {
        if let Ok(Some(node)) = cursor.node_mut() {
            match node.direction {
                SplitDirection::Horizontal => {
                    let width = node.width();

                    let mut cols = node.first.cols as isize;
                    cols = cols
                        .saturating_add(delta)
                        .max(1)
                        .min((width as isize).saturating_sub(2));
                    node.first.cols = cols as u16;

                    node.second.cols = width.saturating_sub(node.first.cols.saturating_add(1));
                }
                SplitDirection::Vertical => {
                    let height = node.height();

                    let mut rows = node.first.rows as isize;
                    rows = rows
                        .saturating_add(delta)
                        .max(1)
                        .min((height as isize).saturating_sub(2));
                    node.first.rows = rows as u16;

                    node.second.rows = height.saturating_sub(node.first.rows.saturating_add(1));
                }
            }
        }
    }

    fn cascade_size_from_cursor(&self, mut root: RefMut<Option<Tree>>, mut cursor: Cursor) {
        // Now we need to cascade this down to children
        match cursor.preorder_next() {
            Ok(c) => cursor = c,
            Err(c) => {
                root.replace(c.tree());
                return;
            }
        }
        let root_size = *self.size.borrow();

        loop {
            // Figure out the available size by looking at our immediate parent node.
            // If we are the root, look at the provided new size
            let pane_size = if let Some((branch, Some(parent))) = cursor.path_to_root().next() {
                if branch == PathBranch::IsRight {
                    parent.second
                } else {
                    parent.first
                }
            } else {
                root_size
            };

            if cursor.is_leaf() {
                // Apply our size to the tty
                cursor.leaf_mut().map(|pane| pane.resize(pane_size));
            } else {
                self.apply_pane_size(pane_size, &mut cursor);
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    break;
                }
            }
        }
    }

    /// Adjusts the size of the active pane in the specified direction
    /// by the specified amount.
    pub fn adjust_pane_size(&self, direction: PaneDirection, amount: usize) {
        if self.zoomed.borrow().is_some() {
            return;
        }
        let active_index = *self.active.borrow();
        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();
        let mut index = 0;

        // Position cursor on the active leaf
        loop {
            if cursor.is_leaf() {
                if index == active_index {
                    // Found it
                    break;
                }
                index += 1;
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(c) => {
                    // Didn't find it
                    root.replace(c.tree());
                    return;
                }
            }
        }

        // We are on the active leaf.
        // Now we go up until we find the parent node that is
        // aligned with the desired direction.
        let split_direction = match direction {
            PaneDirection::Left | PaneDirection::Right => SplitDirection::Horizontal,
            PaneDirection::Up | PaneDirection::Down => SplitDirection::Vertical,
        };
        let delta = match direction {
            PaneDirection::Down | PaneDirection::Right => amount as isize,
            PaneDirection::Up | PaneDirection::Left => -(amount as isize),
        };
        loop {
            match cursor.go_up() {
                Ok(mut c) => {
                    if let Ok(Some(node)) = c.node_mut() {
                        if node.direction == split_direction {
                            self.adjust_node_at_cursor(&mut c, delta);
                            self.cascade_size_from_cursor(root, c);
                            return;
                        }
                    }

                    cursor = c;
                }

                Err(c) => {
                    root.replace(c.tree());
                    return;
                }
            }
        }
    }

    /// Activate an adjacent pane in the specified direction.
    /// In cases where there are multiple adjacent panes in the
    /// intended direction, we take the pane that has the largest
    /// edge intersection.
    pub fn activate_pane_direction(&self, direction: PaneDirection) {
        if self.zoomed.borrow().is_some() {
            return;
        }
        let panes = self.iter_panes();

        let active = match panes.iter().find(|pane| pane.is_active) {
            Some(p) => p,
            None => {
                // No active pane somehow...
                self.set_active_idx(0);
                return;
            }
        };

        let mut best = None;

        /// Compute the edge intersection size between two touching panes
        fn compute_score(
            active_start: usize,
            active_size: usize,
            current_start: usize,
            current_size: usize,
        ) -> usize {
            range_intersection(
                &(active_start..active_start + active_size),
                &(current_start..current_start + current_size),
            )
            .unwrap_or(0..0)
            .count()
        }

        for pane in &panes {
            let score = match direction {
                PaneDirection::Right => {
                    if pane.left == active.left + active.width + 1 {
                        compute_score(active.top, active.height, pane.top, pane.height)
                    } else {
                        0
                    }
                }
                PaneDirection::Left => {
                    if pane.left + pane.width + 1 == active.left {
                        compute_score(active.top, active.height, pane.top, pane.height)
                    } else {
                        0
                    }
                }
                PaneDirection::Up => {
                    if pane.top + pane.height + 1 == active.top {
                        compute_score(active.left, active.width, pane.left, pane.width)
                    } else {
                        0
                    }
                }
                PaneDirection::Down => {
                    if active.top + active.height + 1 == pane.top {
                        compute_score(active.left, active.width, pane.left, pane.width)
                    } else {
                        0
                    }
                }
            };

            if score > 0 {
                let target = match best.take() {
                    Some((best_score, best_pane)) if best_score > score => (best_score, best_pane),
                    _ => (score, pane),
                };
                best.replace(target);
            }
        }

        if let Some((_, target)) = best.take() {
            self.set_active_idx(target.index);
        }
    }

    pub fn prune_dead_panes(&self) -> bool {
        self.remove_pane_if(|_, pane| pane.is_dead())
    }

    pub fn kill_pane(&self, pane_id: PaneId) -> bool {
        self.remove_pane_if(|_, pane| pane.pane_id() == pane_id)
    }

    pub fn kill_panes_in_domain(&self, domain: DomainId) -> bool {
        self.remove_pane_if(|_, pane| pane.domain_id() == domain)
    }

    fn remove_pane_if<F>(&self, f: F) -> bool
    where
        F: Fn(usize, &Rc<dyn Pane>) -> bool,
    {
        let mut dead_panes = vec![];

        {
            let root_size = *self.size.borrow();
            let mut active_idx = *self.active.borrow();
            let mut root = self.pane.borrow_mut();
            let mut cursor = root.take().unwrap().cursor();
            let mut pane_index = 0;
            let cell_dims = self.cell_dimensions();

            loop {
                // Figure out the available size by looking at our immediate parent node.
                // If we are the root, look at the tab size
                let pane_size = if let Some((branch, Some(parent))) = cursor.path_to_root().next() {
                    if branch == PathBranch::IsRight {
                        parent.second
                    } else {
                        parent.first
                    }
                } else {
                    root_size
                };

                if cursor.is_leaf() {
                    let pane = Rc::clone(cursor.leaf_mut().unwrap());
                    if f(pane_index, &pane) {
                        if pane_index == active_idx {
                            active_idx = pane_index.saturating_sub(1);
                        }
                        let parent;
                        match cursor.unsplit_leaf() {
                            Ok((c, dead, p)) => {
                                dead_panes.push(dead.pane_id());
                                parent = p.unwrap();
                                cursor = c;
                            }
                            Err(c) => {
                                // We might be the root, for example
                                if c.is_top() && c.is_leaf() {
                                    root.replace(Tree::Empty);
                                    dead_panes.push(pane.pane_id());
                                } else {
                                    root.replace(c.tree());
                                }
                                break;
                            }
                        };

                        // Now we need to increase the size of the current node
                        // and propagate the revised size to its children.
                        let size = PtySize {
                            rows: parent.height(),
                            cols: parent.width(),
                            pixel_width: cell_dims.pixel_width * parent.width(),
                            pixel_height: cell_dims.pixel_height * parent.height(),
                        };

                        if let Some(unsplit) = cursor.leaf_mut() {
                            unsplit.resize(size).ok();
                        } else {
                            self.apply_pane_size(size, &mut cursor);
                        }
                    } else if !dead_panes.is_empty() {
                        // Apply our revised size to the tty
                        pane.resize(pane_size).ok();
                    }

                    pane_index += 1;
                } else if !dead_panes.is_empty() {
                    self.apply_pane_size(pane_size, &mut cursor);
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(c) => {
                        root.replace(c.tree());
                        break;
                    }
                }
            }
            *self.active.borrow_mut() = active_idx;
        }

        if !dead_panes.is_empty() {
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                for pane_id in dead_panes.into_iter() {
                    mux.remove_pane(pane_id);
                }
            });
            true
        } else {
            false
        }
    }

    pub fn is_dead(&self) -> bool {
        let panes = self.iter_panes();
        let mut dead_count = 0;
        for pos in &panes {
            if pos.pane.is_dead() {
                dead_count += 1;
            }
        }
        dead_count == panes.len()
    }

    pub fn get_active_pane(&self) -> Option<Rc<dyn Pane>> {
        if let Some(zoomed) = self.zoomed.borrow().as_ref() {
            return Some(Rc::clone(zoomed));
        }

        self.iter_panes()
            .iter()
            .nth(*self.active.borrow())
            .map(|p| Rc::clone(&p.pane))
    }

    #[allow(unused)]
    pub fn get_active_idx(&self) -> usize {
        *self.active.borrow()
    }

    pub fn set_active_pane(&self, pane: &Rc<dyn Pane>) {
        if let Some(item) = self
            .iter_panes()
            .iter()
            .find(|p| p.pane.pane_id() == pane.pane_id())
        {
            *self.active.borrow_mut() = item.index;
        }
    }

    pub fn set_active_idx(&self, pane_index: usize) {
        *self.active.borrow_mut() = pane_index;
    }

    /// Assigns the root pane.
    /// This is suitable when creating a new tab and then assigning
    /// the initial pane
    pub fn assign_pane(&self, pane: &Rc<dyn Pane>) {
        match Tree::new().cursor().assign_top(Rc::clone(pane)) {
            Ok(c) => *self.pane.borrow_mut() = Some(c.tree()),
            Err(_) => panic!("tried to assign root pane to non-empty tree"),
        }
    }

    fn cell_dimensions(&self) -> PtySize {
        cell_dimensions(&*self.size.borrow())
    }

    /// Computes the size of the pane that would result if the specified
    /// pane was split in a particular direction.
    /// The intent is to call this prior to spawning the new pane so that
    /// you can create it with the correct size.
    /// May return None if the specified pane_index is invalid.
    pub fn compute_split_size(
        &self,
        pane_index: usize,
        direction: SplitDirection,
    ) -> Option<SplitDirectionAndSize> {
        let cell_dims = self.cell_dimensions();

        self.iter_panes().iter().nth(pane_index).map(|pos| {
            fn split_dimension(dim: usize) -> (usize, usize) {
                let halved = dim / 2;
                if halved * 2 == dim {
                    // Was an even size; we need to allow 1 cell to render
                    // the split UI, so make the newly created leaf slightly
                    // smaller
                    (halved, halved.saturating_sub(1))
                } else {
                    (halved, halved)
                }
            }

            let ((width1, width2), (height1, height2)) = match direction {
                SplitDirection::Horizontal => {
                    (split_dimension(pos.width), (pos.height, pos.height))
                }
                SplitDirection::Vertical => ((pos.width, pos.width), split_dimension(pos.height)),
            };

            SplitDirectionAndSize {
                direction,
                first: PtySize {
                    rows: height1 as _,
                    cols: width1 as _,
                    pixel_height: cell_dims.pixel_height * height1 as u16,
                    pixel_width: cell_dims.pixel_width * width1 as u16,
                },
                second: PtySize {
                    rows: height2 as _,
                    cols: width2 as _,
                    pixel_height: cell_dims.pixel_height * height2 as u16,
                    pixel_width: cell_dims.pixel_width * width2 as u16,
                },
            }
        })
    }

    /// Split the pane that has pane_index in the given direction and assign
    /// the right/bottom pane of the newly created split to the provided Pane
    /// instance.  Returns the resultant index of the newly inserted pane.
    /// Both the split and the inserted pane will be resized.
    pub fn split_and_insert(
        &self,
        pane_index: usize,
        direction: SplitDirection,
        pane: Rc<dyn Pane>,
    ) -> anyhow::Result<usize> {
        if self.zoomed.borrow().is_some() {
            anyhow::bail!("cannot split while zoomed");
        }

        {
            let split_info = self
                .compute_split_size(pane_index, direction)
                .ok_or_else(|| {
                    anyhow::anyhow!("invalid pane_index {}; cannot split!", pane_index)
                })?;
            let tab_size = *self.size.borrow();
            if split_info.first.rows == 0
                || split_info.first.cols == 0
                || split_info.second.rows == 0
                || split_info.second.cols == 0
                || split_info.top_of_second() as u16 + split_info.second.rows > tab_size.rows
                || split_info.left_of_second() as u16 + split_info.second.cols > tab_size.cols
            {
                log::error!(
                    "No splace for split!!! {:#?} height={} width={} top_of_second={} left_of_second={} tab_size={:?}",
                    split_info,
                    split_info.height(),
                    split_info.width(),
                    split_info.top_of_second(),
                    split_info.left_of_second(),
                    tab_size
                );
                anyhow::bail!("No space for split!");
            }

            let mut root = self.pane.borrow_mut();
            let mut cursor = root.take().unwrap().cursor();

            match cursor.go_to_nth_leaf(pane_index) {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    anyhow::bail!("invalid pane_index {}; cannot split!", pane_index);
                }
            };

            let existing_pane = Rc::clone(cursor.leaf_mut().unwrap());

            existing_pane.resize(split_info.first)?;
            pane.resize(split_info.second.clone())?;

            match cursor.split_leaf_and_insert_right(pane) {
                Ok(c) => cursor = c,
                Err(c) => {
                    root.replace(c.tree());
                    anyhow::bail!("invalid pane_index {}; cannot split!", pane_index);
                }
            };

            // cursor now points to the newly created split node;
            // we need to populate its split information
            match cursor.assign_node(Some(split_info)) {
                Err(c) | Ok(c) => root.replace(c.tree()),
            };

            *self.active.borrow_mut() = pane_index + 1;
        }

        log::debug!("split info after split: {:#?}", self.iter_splits());
        log::debug!("pane info after split: {:#?}", self.iter_panes());

        Ok(pane_index + 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::mux::renderable::Renderable;
    use url::Url;
    use wezterm_term::color::ColorPalette;
    use wezterm_term::{KeyCode, KeyModifiers, MouseEvent};

    struct FakePane {
        id: PaneId,
        size: RefCell<PtySize>,
    }

    impl FakePane {
        fn new(id: PaneId, size: PtySize) -> Rc<dyn Pane> {
            Rc::new(Self {
                id,
                size: RefCell::new(size),
            })
        }
    }

    impl Pane for FakePane {
        fn pane_id(&self) -> PaneId {
            self.id
        }
        fn renderer(&self) -> RefMut<dyn Renderable> {
            unimplemented!()
        }
        fn get_title(&self) -> String {
            unimplemented!()
        }
        fn send_paste(&self, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
            unimplemented!()
        }
        fn writer(&self) -> RefMut<dyn std::io::Write> {
            unimplemented!()
        }
        fn resize(&self, size: PtySize) -> anyhow::Result<()> {
            *self.size.borrow_mut() = size;
            Ok(())
        }

        fn key_down(&self, _key: KeyCode, _mods: KeyModifiers) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn mouse_event(&self, _event: MouseEvent) -> anyhow::Result<()> {
            unimplemented!()
        }
        fn advance_bytes(&self, _buf: &[u8]) {
            unimplemented!()
        }
        fn is_dead(&self) -> bool {
            false
        }
        fn palette(&self) -> ColorPalette {
            unimplemented!()
        }
        fn domain_id(&self) -> DomainId {
            1
        }
        fn is_mouse_grabbed(&self) -> bool {
            false
        }
        fn get_current_working_dir(&self) -> Option<Url> {
            None
        }
    }

    #[test]
    fn tab_splitting() {
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 800,
            pixel_height: 600,
        };

        let tab = Tab::new(&size);
        tab.assign_pane(&FakePane::new(1, size));

        let panes = tab.iter_panes();
        assert_eq!(1, panes.len());
        assert_eq!(0, panes[0].index);
        assert_eq!(true, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(80, panes[0].width);
        assert_eq!(24, panes[0].height);

        assert!(tab
            .compute_split_size(1, SplitDirection::Horizontal)
            .is_none());

        let horz_size = tab
            .compute_split_size(0, SplitDirection::Horizontal)
            .unwrap();
        assert_eq!(
            horz_size,
            SplitDirectionAndSize {
                direction: SplitDirection::Horizontal,
                first: PtySize {
                    rows: 24,
                    cols: 40,
                    pixel_width: 400,
                    pixel_height: 600
                },
                second: PtySize {
                    rows: 24,
                    cols: 39,
                    pixel_width: 390,
                    pixel_height: 600
                },
            }
        );

        let vert_size = tab.compute_split_size(0, SplitDirection::Vertical).unwrap();
        assert_eq!(
            vert_size,
            SplitDirectionAndSize {
                direction: SplitDirection::Vertical,
                first: PtySize {
                    rows: 12,
                    cols: 80,
                    pixel_width: 800,
                    pixel_height: 300
                },
                second: PtySize {
                    rows: 11,
                    cols: 80,
                    pixel_width: 800,
                    pixel_height: 275
                }
            }
        );

        let new_index = tab
            .split_and_insert(
                0,
                SplitDirection::Horizontal,
                FakePane::new(2, horz_size.second),
            )
            .unwrap();
        assert_eq!(new_index, 1);

        let panes = tab.iter_panes();
        assert_eq!(2, panes.len());

        assert_eq!(0, panes[0].index);
        assert_eq!(false, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(40, panes[0].width);
        assert_eq!(24, panes[0].height);
        assert_eq!(1, panes[0].pane.pane_id());

        assert_eq!(1, panes[1].index);
        assert_eq!(true, panes[1].is_active);
        assert_eq!(41, panes[1].left);
        assert_eq!(0, panes[1].top);
        assert_eq!(39, panes[1].width);
        assert_eq!(24, panes[1].height);
        assert_eq!(2, panes[1].pane.pane_id());

        let vert_size = tab.compute_split_size(0, SplitDirection::Vertical).unwrap();
        let new_index = tab
            .split_and_insert(
                0,
                SplitDirection::Vertical,
                FakePane::new(3, vert_size.second),
            )
            .unwrap();
        assert_eq!(new_index, 1);

        let panes = tab.iter_panes();
        assert_eq!(3, panes.len());

        assert_eq!(0, panes[0].index);
        assert_eq!(false, panes[0].is_active);
        assert_eq!(0, panes[0].left);
        assert_eq!(0, panes[0].top);
        assert_eq!(40, panes[0].width);
        assert_eq!(12, panes[0].height);
        assert_eq!(1, panes[0].pane.pane_id());

        assert_eq!(1, panes[1].index);
        assert_eq!(true, panes[1].is_active);
        assert_eq!(0, panes[1].left);
        assert_eq!(13, panes[1].top);
        assert_eq!(40, panes[1].width);
        assert_eq!(11, panes[1].height);
        assert_eq!(3, panes[1].pane.pane_id());

        assert_eq!(2, panes[2].index);
        assert_eq!(false, panes[2].is_active);
        assert_eq!(41, panes[2].left);
        assert_eq!(0, panes[2].top);
        assert_eq!(39, panes[2].width);
        assert_eq!(24, panes[2].height);
        assert_eq!(2, panes[2].pane.pane_id());
    }
}
