use crate::pane::CloseReason;
use crate::{Mux, MuxNotification, Tab, TabId};
use config::GuiPosition;
use std::sync::Arc;

static WIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type WindowId = usize;

pub struct Window {
    id: WindowId,
    tabs: Vec<Arc<Tab>>,
    active: usize,
    last_active: Option<TabId>,
    workspace: String,
    title: String,
    initial_position: Option<GuiPosition>,
}

impl Window {
    pub fn new(workspace: Option<String>, initial_position: Option<GuiPosition>) -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![],
            active: 0,
            last_active: None,
            title: String::new(),
            workspace: workspace.unwrap_or_else(|| Mux::get().active_workspace()),
            initial_position,
        }
    }

    pub fn get_initial_position(&self) -> &Option<GuiPosition> {
        &self.initial_position
    }

    pub fn get_workspace(&self) -> &str {
        &self.workspace
    }

    pub fn set_title(&mut self, title: &str) {
        if self.title != title {
            self.title = title.to_string();
            Mux::try_get().map(|mux| {
                mux.notify(MuxNotification::WindowTitleChanged {
                    window_id: self.id,
                    title: title.to_string(),
                })
            });
        }
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    pub fn set_workspace(&mut self, workspace: &str) {
        if workspace == self.workspace {
            return;
        }
        self.workspace = workspace.to_string();
        Mux::get().notify(MuxNotification::WindowWorkspaceChanged(self.id));
    }

    pub fn window_id(&self) -> WindowId {
        self.id
    }

    fn check_that_tab_isnt_already_in_window(&self, tab: &Arc<Tab>) {
        for t in &self.tabs {
            assert_ne!(t.tab_id(), tab.tab_id(), "tab already added to this window");
        }
    }

    fn invalidate(&self) {
        let mux = Mux::get();
        mux.notify(MuxNotification::WindowInvalidated(self.id));
    }

    pub fn insert(&mut self, index: usize, tab: &Arc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.tabs.insert(index, Arc::clone(tab));
        self.invalidate();
    }

    pub fn push(&mut self, tab: &Arc<Tab>) {
        self.check_that_tab_isnt_already_in_window(tab);
        self.tabs.push(Arc::clone(tab));
        self.invalidate();
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_by_idx(&self, idx: usize) -> Option<&Arc<Tab>> {
        self.tabs.get(idx)
    }

    pub fn can_close_without_prompting(&self) -> bool {
        for tab in &self.tabs {
            if !tab.can_close_without_prompting(CloseReason::Window) {
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

    fn fixup_active_tab_after_removal(&mut self, active: Option<Arc<Tab>>) {
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
        } else {
            self.invalidate();
        }
    }

    pub fn remove_by_idx(&mut self, idx: usize) -> Arc<Tab> {
        self.invalidate();
        let active = self.get_active().map(Arc::clone);
        self.do_remove_idx(idx, active)
    }

    pub fn remove_by_id(&mut self, id: TabId) {
        let active = self.get_active().map(Arc::clone);
        if let Some(idx) = self.idx_by_id(id) {
            self.do_remove_idx(idx, active);
        }
    }

    fn do_remove_idx(&mut self, idx: usize, active: Option<Arc<Tab>>) -> Arc<Tab> {
        if let (Some(active), Some(removing)) = (&active, self.tabs.get(idx)) {
            if active.tab_id() == removing.tab_id()
                && config::configuration().switch_to_last_active_tab_when_closing_tab
            {
                // If we are removing the active tab, switch back to
                // the previously active tab
                if let Some(last_active) = self.get_last_active_idx() {
                    self.set_active_without_saving(last_active);
                }
            }
        }
        let tab = self.tabs.remove(idx);
        self.fixup_active_tab_after_removal(active);
        tab
    }

    pub fn get_active(&self) -> Option<&Arc<Tab>> {
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
        if self.active != idx {
            if let Some(tab) = self.tabs.get(self.active) {
                if let Some(pane) = tab.get_active_pane() {
                    pane.focus_changed(false);
                }
            }
        }
        self.active = idx;
        self.invalidate();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Tab>> {
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
