use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::Mux;
use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};
use portable_pty::PtySize;
use serde::{Deserialize, Serialize};
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, StableRowIndex};

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
    pane: RefCell<Option<PaneNode>>,
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

/// A tab contains a tree of PaneNode's.
#[derive(Clone)]
enum PaneNode {
    /// This node is filled with a single Pane
    Single(Rc<dyn Pane>),
    /// This node is split horizontally in two.
    HorizontalSplit {
        left: Box<PaneNode>,
        left_width: usize,
        right: Box<PaneNode>,
    },
    /// This node is split vertically in two.
    VerticalSplit {
        top: Box<PaneNode>,
        top_height: usize,
        bottom: Box<PaneNode>,
    },
}

impl PaneNode {
    /// Returns true if this node or any of its children are
    /// alive.  Stops evaluating as soon as it identifies that
    /// something is alive.
    pub fn is_alive(&self) -> bool {
        match self {
            PaneNode::Single(p) => !p.is_dead(),
            PaneNode::HorizontalSplit { left, right, .. } => left.is_alive() || right.is_alive(),
            PaneNode::VerticalSplit { top, bottom, .. } => top.is_alive() || bottom.is_alive(),
        }
    }

    /// Returns a ref to the PaneNode::Single that contains a pane
    /// given its topological index.
    /// The if topological index is invalid, returns None.
    fn node_by_index_mut(
        &mut self,
        wanted_index: usize,
        current_index: &mut usize,
    ) -> Option<&mut Self> {
        match self {
            PaneNode::Single(_) => {
                if wanted_index == *current_index {
                    Some(self)
                } else {
                    *current_index += 1;
                    None
                }
            }
            PaneNode::HorizontalSplit { left, right, .. } => {
                if let Some(found) = left.node_by_index_mut(wanted_index, current_index) {
                    Some(found)
                } else {
                    right.node_by_index_mut(wanted_index, current_index)
                }
            }
            PaneNode::VerticalSplit { top, bottom, .. } => {
                if let Some(found) = top.node_by_index_mut(wanted_index, current_index) {
                    Some(found)
                } else {
                    bottom.node_by_index_mut(wanted_index, current_index)
                }
            }
        }
    }

    /// Recursively Walk to compute the positioning information
    fn walk(
        &self,
        active_index: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        panes: &mut Vec<PositionedPane>,
    ) {
        match self {
            PaneNode::Single(p) => {
                let index = panes.len();
                panes.push(PositionedPane {
                    index,
                    is_active: index == active_index,
                    left: x,
                    top: y,
                    width,
                    height,
                    pane: Rc::clone(p),
                });
            }
            PaneNode::HorizontalSplit {
                left,
                left_width,
                right,
            } => {
                left.walk(active_index, x, y, *left_width, height, panes);
                right.walk(
                    active_index,
                    x + *left_width,
                    y,
                    width.saturating_sub(*left_width),
                    height,
                    panes,
                );
            }
            PaneNode::VerticalSplit {
                top,
                top_height,
                bottom,
            } => {
                top.walk(active_index, x, y, width, *top_height, panes);
                bottom.walk(
                    active_index,
                    x,
                    y + *top_height,
                    width,
                    height.saturating_sub(*top_height),
                    panes,
                );
            }
        }
    }
}

impl Tab {
    pub fn new(size: &PtySize) -> Self {
        Self {
            id: TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            pane: RefCell::new(None),
            size: RefCell::new(*size),
            active: RefCell::new(0),
        }
    }

    /// Walks the pane tree to produce the topologically ordered flattened
    /// list of PositionedPane instances along with their positioning information.
    pub fn iter_panes(&self) -> Vec<PositionedPane> {
        let mut panes = vec![];
        let size = *self.size.borrow();

        if let Some(pane) = self.pane.borrow().as_ref() {
            pane.walk(
                *self.active.borrow(),
                0,
                0,
                size.cols as _,
                size.rows as _,
                &mut panes,
            );
        }

        panes
    }

    pub fn tab_id(&self) -> TabId {
        self.id
    }

    pub fn is_dead(&self) -> bool {
        if let Some(pane) = self.pane.borrow().as_ref() {
            !pane.is_alive()
        } else {
            true
        }
    }

    pub fn get_active_pane(&self) -> Option<Rc<dyn Pane>> {
        self.iter_panes()
            .iter()
            .nth(*self.active.borrow())
            .map(|p| Rc::clone(&p.pane))
    }

    /// Assigns the root pane.
    /// This is suitable when creating a new tab and then assigning
    /// the initial pane
    pub fn assign_pane(&self, pane: &Rc<dyn Pane>) {
        self.pane
            .borrow_mut()
            .replace(PaneNode::Single(Rc::clone(pane)));
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
    ) -> Option<PtySize> {
        let cell_dims = self.cell_dimensions();

        self.iter_panes().iter().nth(pane_index).map(|pos| {
            let (width, height) = match direction {
                SplitDirection::Horizontal => (pos.width / 2, pos.height),
                SplitDirection::Vertical => (pos.width, pos.height / 2),
            };
            PtySize {
                rows: height as _,
                cols: width as _,
                pixel_width: cell_dims.pixel_width * width as u16,
                pixel_height: cell_dims.pixel_height * height as u16,
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
        let new_size = self
            .compute_split_size(pane_index, direction)
            .ok_or_else(|| anyhow::anyhow!("invalid pane_index {}; cannot split!", pane_index))?;

        pane.resize(new_size.clone())?;
        let new_pane = Box::new(PaneNode::Single(pane));

        if let Some(root_node) = self.pane.borrow_mut().as_mut() {
            let mut active = 0;
            let node = root_node
                .node_by_index_mut(pane_index, &mut active)
                .ok_or_else(|| {
                    anyhow::anyhow!("invalid pane_index {}; cannot split!", pane_index)
                })?;

            let prior_node = match node {
                PaneNode::Single(orig_pane) => {
                    orig_pane.resize(new_size)?;
                    Box::new(PaneNode::Single(Rc::clone(orig_pane)))
                }
                _ => unreachable!("impossible PaneNode variant returned from node_by_index_mut"),
            };

            match direction {
                SplitDirection::Horizontal => {
                    *node = PaneNode::HorizontalSplit {
                        left: prior_node,
                        right: new_pane,
                        left_width: new_size.cols as _,
                    }
                }

                SplitDirection::Vertical => {
                    *node = PaneNode::VerticalSplit {
                        top: prior_node,
                        bottom: new_pane,
                        top_height: new_size.rows as _,
                    }
                }
            }

            let new_index = pane_index + 1;

            *self.active.borrow_mut() = new_index;

            Ok(new_index)
        } else {
            anyhow::bail!("no panes have been assigned; cannot split!");
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
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
            PtySize {
                rows: 24,
                cols: 40,
                pixel_width: 400,
                pixel_height: 600
            }
        );

        let vert_size = tab.compute_split_size(0, SplitDirection::Vertical).unwrap();
        assert_eq!(
            vert_size,
            PtySize {
                rows: 12,
                cols: 80,
                pixel_width: 800,
                pixel_height: 300
            }
        );

        let new_index = tab
            .split_and_insert(0, SplitDirection::Horizontal, FakePane::new(2, horz_size))
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
        assert_eq!(40, panes[1].left);
        assert_eq!(0, panes[1].top);
        assert_eq!(40, panes[1].width);
        assert_eq!(24, panes[1].height);
        assert_eq!(2, panes[1].pane.pane_id());

        let vert_size = tab.compute_split_size(0, SplitDirection::Vertical).unwrap();
        let new_index = tab
            .split_and_insert(0, SplitDirection::Vertical, FakePane::new(3, vert_size))
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
        assert_eq!(12, panes[1].top);
        assert_eq!(40, panes[1].width);
        assert_eq!(12, panes[1].height);
        assert_eq!(3, panes[1].pane.pane_id());

        assert_eq!(2, panes[2].index);
        assert_eq!(false, panes[2].is_active);
        assert_eq!(40, panes[2].left);
        assert_eq!(0, panes[2].top);
        assert_eq!(40, panes[2].width);
        assert_eq!(24, panes[2].height);
        assert_eq!(2, panes[2].pane.pane_id());
    }
}
