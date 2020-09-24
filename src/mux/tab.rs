use crate::keyassignment::PaneDirection;
use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::Mux;
use async_trait::async_trait;
use bintree::PathBranch;
use downcast_rs::{impl_downcast, Downcast};
use portable_pty::PtySize;
use rangeset::range_intersection;
use serde::{Deserialize, Serialize};
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, StableRowIndex};

pub type Tree = bintree::Tree<Rc<dyn Pane>, SplitDirectionAndSize>;
pub type Cursor = bintree::Cursor<Rc<dyn Pane>, SplitDirectionAndSize>;

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type TabId = usize;

static PANE_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type PaneId = usize;

pub fn alloc_pane_id() -> PaneId {
    PANE_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

const PASTE_CHUNK_SIZE: usize = 1024;

struct Paste {
    pane_id: PaneId,
    text: String,
    offset: usize,
}

fn schedule_next_paste(paste: &Arc<Mutex<Paste>>) {
    let paste = Arc::clone(paste);
    promise::spawn::spawn(async move {
        let mut locked = paste.lock().unwrap();
        let mux = Mux::get().unwrap();
        let pane = mux.get_pane(locked.pane_id).unwrap();

        let remain = locked.text.len() - locked.offset;
        let chunk = remain.min(PASTE_CHUNK_SIZE);
        let text_slice = &locked.text[locked.offset..locked.offset + chunk];
        pane.send_paste(text_slice).unwrap();

        if chunk < remain {
            // There is more to send
            locked.offset += chunk;
            schedule_next_paste(&paste);
        }
    });
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Pattern {
    CaseSensitiveString(String),
    CaseInSensitiveString(String),
    Regex(String),
}

impl std::ops::Deref for Pattern {
    type Target = String;
    fn deref(&self) -> &String {
        match self {
            Pattern::CaseSensitiveString(s) => s,
            Pattern::CaseInSensitiveString(s) => s,
            Pattern::Regex(s) => s,
        }
    }
}

impl std::ops::DerefMut for Pattern {
    fn deref_mut(&mut self) -> &mut String {
        match self {
            Pattern::CaseSensitiveString(s) => s,
            Pattern::CaseInSensitiveString(s) => s,
            Pattern::Regex(s) => s,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SearchResult {
    pub start_y: StableRowIndex,
    pub end_y: StableRowIndex,
    /// The cell index into the line of the start of the match
    pub start_x: usize,
    /// The cell index into the line of the end of the match
    pub end_x: usize,
}

/// A Tab is a container of Panes
/// At this time only a single pane is supported
pub struct Tab {
    id: TabId,
    pane: RefCell<Option<Tree>>,
    size: RefCell<PtySize>,
    active: RefCell<usize>,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// The size is of the (first, second) child of the split
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

impl SplitDirectionAndSize {
    pub fn width(&self) -> u16 {
        if self.direction == SplitDirection::Horizontal {
            self.first.cols + self.second.cols + 1
        } else {
            if self.first.cols != self.second.cols {
                log::error!("{:#?}", self);
            }
            assert_eq!(self.first.cols, self.second.cols);
            self.first.cols
        }
    }
    pub fn height(&self) -> u16 {
        if self.direction == SplitDirection::Vertical {
            self.first.rows + self.second.rows + 1
        } else {
            assert_eq!(self.first.rows, self.second.rows);
            self.first.rows
        }
    }
}

impl Tab {
    pub fn new(size: &PtySize) -> Self {
        Self {
            id: TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            pane: RefCell::new(Some(Tree::new())),
            size: RefCell::new(*size),
            active: RefCell::new(0),
        }
    }

    /// Walks the pane tree to produce the topologically ordered flattened
    /// list of PositionedPane instances along with their positioning information.
    pub fn iter_panes(&self) -> Vec<PositionedPane> {
        let mut panes = vec![];
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
    /// This works by adjusting the size of the second half of each split.
    pub fn resize(&self, size: PtySize) {
        let mut root = self.pane.borrow_mut();
        let mut cursor = root.take().unwrap().cursor();

        *self.size.borrow_mut() = size;

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
                size
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

    /// Given split_index, the topological index of a split returned by
    /// iter_splits() as PositionedSplit::index, revised the split position
    /// by the provided delta; positive values move the split to the right/bottom,
    /// and negative values to the left/top.
    /// The adjusted size is propogated downwards to contained children and
    /// their panes are resized accordingly.
    pub fn resize_split_by(&self, split_index: usize, delta: isize) {
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
        loop {
            let is_second = cursor.is_right();
            match cursor.go_up() {
                Ok(mut c) => {
                    if let Ok(Some(node)) = c.node_mut() {
                        if node.direction == split_direction {
                            let delta = match (is_second, direction) {
                                (false, PaneDirection::Up)
                                | (false, PaneDirection::Left)
                                | (true, PaneDirection::Down)
                                | (true, PaneDirection::Right) => amount as isize,
                                (false, PaneDirection::Down)
                                | (false, PaneDirection::Right)
                                | (true, PaneDirection::Up)
                                | (true, PaneDirection::Left) => -(amount as isize),
                            };
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
                    let pane = Rc::clone(cursor.leaf_mut().unwrap());
                    if pane.is_dead() {
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
                                root.replace(c.tree());
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
        self.iter_panes()
            .iter()
            .nth(*self.active.borrow())
            .map(|p| Rc::clone(&p.pane))
    }

    pub fn get_active_idx(&self) -> usize {
        *self.active.borrow()
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
        let size = *self.size.borrow();
        PtySize {
            rows: 1,
            cols: 1,
            pixel_width: size.pixel_width / size.cols,
            pixel_height: size.pixel_height / size.rows,
        }
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

/// A Pane represents a view on a terminal
#[async_trait(?Send)]
pub trait Pane: Downcast {
    fn pane_id(&self) -> PaneId;
    fn renderer(&self) -> RefMut<dyn Renderable>;
    fn get_title(&self) -> String;
    fn send_paste(&self, text: &str) -> anyhow::Result<()>;
    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>>;
    fn writer(&self) -> RefMut<dyn std::io::Write>;
    fn resize(&self, size: PtySize) -> anyhow::Result<()>;
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()>;
    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()>;
    fn advance_bytes(&self, buf: &[u8]);
    fn is_dead(&self) -> bool;
    fn palette(&self) -> ColorPalette;
    fn domain_id(&self) -> DomainId;

    fn erase_scrollback(&self) {}

    /// Called to advise on whether this tab has focus
    fn focus_changed(&self, _focused: bool) {}

    /// Performs a search.
    /// If the result is empty then there are no matches.
    /// Otherwise, the result shall contain all possible matches.
    async fn search(&self, _pattern: Pattern) -> anyhow::Result<Vec<SearchResult>> {
        Ok(vec![])
    }

    /// Returns true if the terminal has grabbed the mouse and wants to
    /// give the embedded application a chance to process events.
    /// In practice this controls whether the gui will perform local
    /// handling of clicks.
    fn is_mouse_grabbed(&self) -> bool;

    fn set_clipboard(&self, _clipboard: &Arc<dyn Clipboard>) {}

    fn get_current_working_dir(&self) -> Option<Url>;

    fn trickle_paste(&self, text: String) -> anyhow::Result<()> {
        if text.len() <= PASTE_CHUNK_SIZE {
            // Send it all now
            self.send_paste(&text)?;
        } else {
            // It's pretty heavy, so we trickle it into the pty
            self.send_paste(&text[0..PASTE_CHUNK_SIZE])?;

            let paste = Arc::new(Mutex::new(Paste {
                pane_id: self.pane_id(),
                text,
                offset: PASTE_CHUNK_SIZE,
            }));
            schedule_next_paste(&paste);
        }
        Ok(())
    }
}
impl_downcast!(Pane);

#[cfg(test)]
mod test {
    use super::*;

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
                left: 0,
                top: 0,
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
                left: 0,
                top: 0,
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
