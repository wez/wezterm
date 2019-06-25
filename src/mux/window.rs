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
    pub fn new() -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![],
            active: 0,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.id
    }

    pub fn push(&mut self, tab: &Rc<dyn Tab>) {
        for t in &self.tabs {
            assert_ne!(t.tab_id(), tab.tab_id(), "tab already added to this window");
        }
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

    pub fn remove_by_id(&mut self, id: TabId) -> bool {
        if let Some(idx) = self.idx_by_id(id) {
            self.tabs.remove(idx);
            let len = self.tabs.len();
            if len > 0 && self.active == idx && idx >= len {
                self.set_active(len - 1);
            }
            true
        } else {
            false
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

    pub fn prune_dead_tabs(&mut self, live_tab_ids: &[TabId]) {
        let dead: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                if live_tab_ids
                    .iter()
                    .find(|&&id| id == tab.tab_id())
                    .is_none()
                {
                    Some(tab.tab_id())
                } else {
                    None
                }
            })
            .collect();
        for tab_id in dead {
            self.remove_by_id(tab_id);
        }
    }
}
