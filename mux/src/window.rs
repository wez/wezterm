use crate::pane::CloseReason;
use crate::{Mux, MuxNotification, Tab, TabId};
use config::GuiPosition;
use std::sync::Arc;

static WIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type WindowId = usize;

pub struct Window {
    id: WindowId,
    tabs: Vec<Arc<Tab>>,
    active_tab_idx: usize,
    last_active_tab_id: Option<TabId>,
    workspace: String,
    title: String,
    initial_position: Option<GuiPosition>,
}

impl Window {
    /// Create a new Window.
    pub fn new(workspace: Option<String>, initial_position: Option<GuiPosition>) -> Self {
        Self {
            id: WIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
            tabs: vec![],
            active_tab_idx: 0,
            last_active_tab_id: None,
            title: String::new(),
            workspace: workspace.unwrap_or_else(|| Mux::get().active_workspace()),
            initial_position,
        }
    }

    /// Return initial position of the window.
    pub fn get_initial_position(&self) -> &Option<GuiPosition> {
        &self.initial_position
    }

    /// Return current workspace.
    pub fn get_workspace(&self) -> &str {
        &self.workspace
    }

    /// Set window title, notifying listeners if it changed.
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

    /// Return current title.
    pub fn get_title(&self) -> &str {
        &self.title
    }

    /// Set window workspace, notifying listeners if it changed.
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

    /// Panic if given tab is already present in this window.
    fn assert_tab_isnt_already_in_window(&self, tab: &Arc<Tab>) {
        for t in &self.tabs {
            assert_ne!(t.tab_id(), tab.tab_id(), "tab already added to this window");
        }
    }

    /// Notify that this window's tab layout has changed (add/remove/reorder),
    /// so listeners can refresh any cached view of it.
    fn invalidate(&self) {
        let mux = Mux::get();
        mux.notify(MuxNotification::WindowInvalidated(self.id));
    }

    /// Insert `tab` in window, at given index.
    pub fn insert_tab_at_idx(&mut self, index: usize, tab: &Arc<Tab>) {
        self.assert_tab_isnt_already_in_window(tab);
        self.tabs.insert(index, Arc::clone(tab));
        self.invalidate();
    }

    /// Insert `tab` in window, after other tabs.
    pub fn push_tab(&mut self, tab: &Arc<Tab>) {
        self.assert_tab_isnt_already_in_window(tab);
        self.tabs.push(Arc::clone(tab));
        self.invalidate();
    }

    /// Return true if this window contains no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Return number of tabs in this window.
    pub fn count_tabs(&self) -> usize {
        self.tabs.len()
    }

    /// Return tab at given index, if any.
    pub fn get_tab_at_idx(&self, idx: usize) -> Option<&Arc<Tab>> {
        self.tabs.get(idx)
    }

    /// Return true if every tab can be closed without prompting the user.
    pub fn can_close_without_prompting(&self) -> bool {
        for tab in &self.tabs {
            if !tab.can_close_without_prompting(CloseReason::Window) {
                return false;
            }
        }
        true
    }

    /// Return index of tab with given id, if it is found in this window.
    pub fn get_tab_idx_for_id(&self, id: TabId) -> Option<usize> {
        for (idx, t) in self.tabs.iter().enumerate() {
            if t.tab_id() == id {
                return Some(idx);
            }
        }
        None
    }

    /// Re-establish a valid active tab after a tab removal.
    fn fixup_active_tab_after_removal(&mut self, active: Option<Arc<Tab>>) {
        let len = self.tabs.len();
        if let Some(active) = active {
            for (idx, tab) in self.tabs.iter().enumerate() {
                if tab.tab_id() == active.tab_id() {
                    self.set_active_tab_idx_without_saving(idx);
                    return;
                }
            }
        }

        if len > 0 && self.active_tab_idx >= len {
            self.set_active_tab_idx_without_saving(len - 1);
        } else {
            self.invalidate();
        }
    }

    /// Remove tab at given index and return it.
    pub fn remove_tab_idx(&mut self, idx: usize) -> Arc<Tab> {
        self.invalidate();
        let active = self.get_active_tab().map(Arc::clone);
        self.do_remove_tab_idx(idx, active)
    }

    /// Remove tab with given id, if present.
    pub fn remove_tab_id(&mut self, id: TabId) {
        let active = self.get_active_tab().map(Arc::clone);
        if let Some(idx) = self.get_tab_idx_for_id(id) {
            self.do_remove_tab_idx(idx, active);
        }
    }

    /// Remove tab at given index, switching to previous active tab if needed.
    fn do_remove_tab_idx(&mut self, idx: usize, active: Option<Arc<Tab>>) -> Arc<Tab> {
        if let (Some(active), Some(removing)) = (&active, self.tabs.get(idx)) {
            if active.tab_id() == removing.tab_id()
                && config::configuration().switch_to_last_active_tab_when_closing_tab
            {
                // If we are removing active tab, switch back to previously active tab
                if let Some(last_active) = self.get_last_active_tab_idx() {
                    self.set_active_tab_idx_without_saving(last_active);
                }
            }
        }
        let tab = self.tabs.remove(idx);
        self.fixup_active_tab_after_removal(active);
        tab
    }

    /// Return currently active tab.
    pub fn get_active_tab(&self) -> Option<&Arc<Tab>> {
        // FIXME: `self.active_tab_idx` is supposed to be always valid, so this should really always
        // return the Tab.. and warn/panic(?) if active tab index isn't valid somehow (logic error!)
        self.get_tab_at_idx(self.active_tab_idx)
    }

    /// Return index of currently active tab.
    #[inline]
    pub fn get_active_tab_idx(&self) -> usize {
        self.active_tab_idx
    }

    /// Remember current tab as the "last active" tab.
    pub fn remember_current_as_last_active_tab(&mut self) {
        self.last_active_tab_id = self
            .get_tab_at_idx(self.active_tab_idx)
            .map(|tab| tab.tab_id());
    }

    /// Return index of previously active tab, if any.
    #[inline]
    pub fn get_last_active_tab_idx(&self) -> Option<usize> {
        if let Some(tab_id) = self.last_active_tab_id {
            self.get_tab_idx_for_id(tab_id)
        } else {
            None
        }
    }

    /// Remember current tab as the "last active" tab, then make tab at given index active.
    /// No-op when given index is already the active tab.
    pub fn remember_and_set_active_tab_idx(&mut self, idx: usize) {
        if idx == self.get_active_tab_idx() {
            return;
        }
        self.remember_current_as_last_active_tab();
        self.set_active_tab_idx_without_saving(idx);
    }

    /// Make given index the active tab without remembering current tab as the "last active" tab.
    pub fn set_active_tab_idx_without_saving(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        if self.active_tab_idx != idx {
            if let Some(tab) = self.tabs.get(self.active_tab_idx) {
                if let Some(pane) = tab.get_active_pane() {
                    pane.focus_changed(false);
                }
            }
        }
        self.active_tab_idx = idx;
        self.invalidate();
    }

    /// Iterate over tabs in this window.
    pub fn iter_tabs(&self) -> impl Iterator<Item = &Arc<Tab>> {
        self.tabs.iter()
    }

    /// Remove dead tabs, and any tabs not present in given `live_tab_ids`.
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
            self.remove_tab_id(tab_id);
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
            self.remove_tab_id(tab_id);
        }

        if invalidated {
            self.invalidate();
        }
    }
}
