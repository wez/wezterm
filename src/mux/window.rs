use crate::mux::{Tab, TabId};
use std::rc::Rc;
use std::sync::Arc;
use wezterm_term::Clipboard;

static WIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type WindowId = usize;

pub struct Window {
    id: WindowId,
    tabs: Vec<Rc<Tab>>,
    active: usize,
    clipboard: Option<Arc<dyn Clipboard>>,
    invalidated: bool,
}

impl Window {
    pub fn new() -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![],
            active: 0,
            clipboard: None,
            invalidated: false,
        }
    }

    pub fn set_clipboard(&mut self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.replace(Arc::clone(clipboard));
    }

    pub fn window_id(&self) -> WindowId {
        self.id
    }

    fn check_that_tab_isnt_already_in_window(&self, tab: &Rc<Tab>) {
        for t in &self.tabs {
            assert_ne!(t.tab_id(), tab.tab_id(), "tab already added to this window");
        }
    }

    fn assign_clipboard_to_tab(&self, tab: &Rc<Tab>) {
        if let Some(clip) = self.clipboard.as_ref() {
            if let Some(pane) = tab.get_active_pane() {
                pane.set_clipboard(clip);
            }
        }
    }

    pub fn insert(&mut self, index: usize, tab: &Rc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.assign_clipboard_to_tab(tab);
        self.tabs.insert(index, Rc::clone(tab));
        self.invalidated = true;
    }

    pub fn push(&mut self, tab: &Rc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.assign_clipboard_to_tab(tab);
        self.tabs.push(Rc::clone(tab));
        self.invalidated = true;
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_by_idx(&self, idx: usize) -> Option<&Rc<Tab>> {
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

    pub fn remove_by_idx(&mut self, idx: usize) -> Rc<Tab> {
        self.invalidated = true;
        self.tabs.remove(idx)
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

    pub fn check_and_reset_invalidated(&mut self) -> bool {
        let res = self.invalidated;
        self.invalidated = false;
        res
    }

    pub fn get_active(&self) -> Option<&Rc<Tab>> {
        self.get_by_idx(self.active)
    }

    #[inline]
    pub fn get_active_idx(&self) -> usize {
        self.active
    }

    pub fn set_active(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.invalidated = true;
        self.active = idx;
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Tab>> {
        self.tabs.iter()
    }

    pub fn prune_dead_tabs(&mut self, live_tab_ids: &[TabId]) {
        let dead: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                if let Some(pane) = tab.get_active_pane() {
                    if pane.is_dead() {
                        return Some(tab.tab_id());
                    } else {
                        None
                    }
                } else {
                    Some(tab.tab_id())
                }
            })
            .collect();
        for tab_id in dead {
            self.remove_by_id(tab_id);
        }

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
