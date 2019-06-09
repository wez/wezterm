use crate::mux::{Tab, TabId};
use std::rc::Rc;

static WIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type WindowId = usize;

pub struct Window {
    id: WindowId,
    tabs: Vec<Rc<dyn Tab>>,
    active: usize,
}

impl Window {
    pub fn new(tab: &Rc<dyn Tab>) -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![Rc::clone(tab)],
            active: 0,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.id
    }

    pub fn push(&mut self, tab: &Rc<dyn Tab>) {
        self.tabs.push(Rc::clone(tab))
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_by_idx(&self, idx: usize) -> Option<&Rc<dyn Tab>> {
        self.tabs.get(idx)
    }

    pub fn idx_by_id(&self, id: TabId) -> Option<usize> {
        for (idx, t) in self.tabs.iter().enumerate() {
            if t.tab_id() == id {
                return Some(idx);
            }
        }
        None
    }

    pub fn remove_by_id(&mut self, id: TabId) {
        if let Some(idx) = self.idx_by_id(id) {
            self.tabs.remove(idx);
            let len = self.tabs.len();
            if len > 0 && self.active == idx && idx >= len {
                self.set_active(len - 1);
            }
        }
    }

    pub fn get_active(&self) -> Option<&Rc<dyn Tab>> {
        self.get_by_idx(self.active)
    }

    #[inline]
    pub fn get_active_idx(&self) -> usize {
        self.active
    }

    pub fn set_active(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.active = idx;
        self.get_by_idx(idx)
            .unwrap()
            .renderer()
            .make_all_lines_dirty();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<dyn Tab>> {
        self.tabs.iter()
    }
}
