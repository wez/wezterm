use crate::{Mux, MuxNotification, Tab, TabId};
use std::rc::Rc;
use std::sync::Arc;
use wezterm_term::Clipboard;

static WIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type WindowId = usize;

pub struct Window {
    id: WindowId,
    tabs: Vec<Rc<Tab>>,
    active: usize,
    last_active: Option<TabId>,
    clipboard: Option<Arc<dyn Clipboard>>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![],
            active: 0,
            last_active: None,
            clipboard: None,
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

    fn invalidate(&self) {
        let mux = Mux::get().unwrap();
        mux.notify(MuxNotification::WindowInvalidated(self.id));
    }

    pub fn insert(&mut self, index: usize, tab: &Rc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.assign_clipboard_to_tab(tab);
        self.tabs.insert(index, Rc::clone(tab));
        self.invalidate();
    }

    pub fn push(&mut self, tab: &Rc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.assign_clipboard_to_tab(tab);
        self.tabs.push(Rc::clone(tab));
        self.invalidate();
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

    pub fn can_close_without_prompting(&self) -> bool {
        for tab in &self.tabs {
            if !tab.can_close_without_prompting() {
                return false;
            }
        }
        true
    }

    pub fn idx_by_id(&self, id: TabId) -> Option<usize> {
        for (idx, t) in self.tabs.iter().enumerate() {
            if t.tab_id() == id {
                return Some(idx);
            }
        }
        None
    }

    fn fixup_active_tab_after_removal(&mut self, active: Option<Rc<Tab>>) {
        let len = self.tabs.len();
        if let Some(active) = active {
            for (idx, tab) in self.tabs.iter().enumerate() {
                if tab.tab_id() == active.tab_id() {
                    self.set_active_without_saving(idx);
                    return;
                }
            }
        }

        if len > 0 && self.active >= len {
            self.set_active_without_saving(len - 1);
        }
    }

    pub fn remove_by_idx(&mut self, idx: usize) -> Rc<Tab> {
        self.invalidate();
        let active = self.get_active().map(Rc::clone);
        let tab = self.tabs.remove(idx);
        self.fixup_active_tab_after_removal(active);
        tab
    }

    pub fn remove_by_id(&mut self, id: TabId) {
        let active = self.get_active().map(Rc::clone);
        if let Some(idx) = self.idx_by_id(id) {
            self.tabs.remove(idx);
        }
        self.fixup_active_tab_after_removal(active);
    }

    pub fn get_active(&self) -> Option<&Rc<Tab>> {
        self.get_by_idx(self.active)
    }

    #[inline]
    pub fn get_active_idx(&self) -> usize {
        self.active
    }

    pub fn save_last_active(&mut self) {
        self.last_active = self.get_by_idx(self.active).map(|tab| tab.tab_id());
    }

    #[inline]
    pub fn get_last_active_idx(&self) -> Option<usize> {
        if let Some(tab_id) = self.last_active {
            self.idx_by_id(tab_id)
        } else {
            None
        }
    }

    /// If `idx` is different from the current active tab,
    /// save the current tabid and then make `idx` the active
    /// tab position.
    pub fn save_and_then_set_active(&mut self, idx: usize) {
        if idx == self.get_active_idx() {
            return;
        }
        self.save_last_active();
        self.set_active_without_saving(idx);
    }

    /// Make `idx` the active tab position.
    /// The saved tab id is not changed.
    pub fn set_active_without_saving(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.active = idx;
        self.invalidate();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Tab>> {
        self.tabs.iter()
    }

    pub fn prune_dead_tabs(&mut self, live_tab_ids: &[TabId]) {
        let mut invalidated = false;
        let dead: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                if tab.prune_dead_panes() {
                    invalidated = true;
                }
                if tab.is_dead() {
                    Some(tab.tab_id())
                } else {
                    None
                }
            })
            .collect();

        for tab_id in dead {
            log::trace!("Window::prune_dead_tabs: tab_id {} is dead", tab_id);
            self.remove_by_id(tab_id);
            invalidated = true;
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
            log::trace!("Window::prune_dead_tabs: (live) tab_id {} is dead", tab_id);
            self.remove_by_id(tab_id);
        }

        if invalidated {
            self.invalidate();
        }
    }
}
