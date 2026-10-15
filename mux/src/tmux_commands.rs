use crate::domain::{DomainId, WriterWrapper};
use crate::localpane::LocalPane;
use crate::pane::{alloc_pane_id, PaneId};
use crate::renderable::StableCursorPosition;
use crate::tab::{
    PaneEntry, PaneNode, SplitDirection, SplitDirectionAndSize, SplitRequest, SplitSize, Tab, TabId,
};
use crate::tmux::{AttachState, TmuxDomain, TmuxDomainState, TmuxRemotePane, TmuxTab};
use crate::tmux_pty::{TmuxChild, TmuxPty};
use crate::{Mux, MuxNotification, Pane};
use anyhow::{anyhow, Context};
use parking_lot::{Condvar, Mutex};
use portable_pty::{MasterPty, PtySize};
use std::collections::HashSet;
use std::fmt::{Debug, Write};
use std::io::Write as _;
use std::sync::Arc;
use termwiz::escape::csi::{Cursor, CSI};
use termwiz::escape::{Action, OneBased};
use termwiz::tmux_cc::*;
use wezterm_term::TerminalSize;

pub(crate) trait TmuxCommand: Send + Debug {
    fn get_command(&self, domain_id: DomainId) -> String;
    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaneItem {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    pane_id: TmuxPaneId,
    _pane_index: u64,
    cursor_x: u64,
    cursor_y: u64,
    pane_width: u64,
    pane_height: u64,
    pane_left: u64,
    pane_top: u64,
    pane_active: bool,
}

#[derive(Debug)]
struct WindowItem {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    window_width: u64,
    window_height: u64,
    window_active: bool,
    window_name: String,
    layout: Vec<WindowLayout>,
    layout_csum: String,
    history_limit: isize,
}

impl TmuxDomainState {
    /// check if a PaneItem received from ListAllPanes has been attached
    pub fn check_pane_attached(&self, window_id: TmuxWindowId, pane_id: TmuxPaneId) -> bool {
        let gui_tabs = self.gui_tabs.lock();
        let Some(local_tab) = gui_tabs.get(&window_id) else {
            return false;
        };

        return local_tab.panes.get(&pane_id).is_some();
    }

    pub fn check_window_attached(&self, window_id: TmuxWindowId) -> bool {
        let gui_tabs = self.gui_tabs.lock();
        return gui_tabs.get(&window_id).is_some();
    }

    /// after we create a tab for a remote pane, save its ID into the
    /// TmuxPane-TmuxPane tree, so we can ref it later.
    fn add_attached_pane(
        &self,
        window_id: TmuxWindowId,
        pane_id: TmuxPaneId,
    ) -> anyhow::Result<()> {
        let mut gui_tabs = self.gui_tabs.lock();

        let panes = match gui_tabs.get_mut(&window_id) {
            Some(tab) => &mut tab.panes,
            None => anyhow::bail!("The window {window_id} is not attached"),
        };

        match panes.get(&pane_id) {
            Some(_) => {
                anyhow::bail!("Tmux pane already attached");
            }
            None => {
                panes.insert(pane_id);
                return Ok(());
            }
        }
    }

    fn add_attached_window(&self, target: &WindowItem, tab_id: &TabId) -> anyhow::Result<()> {
        let mut gui_tabs = self.gui_tabs.lock();
        if !gui_tabs.contains_key(&target.window_id) {
            gui_tabs.insert(
                target.window_id,
                TmuxTab {
                    tab_id: *tab_id,
                    tmux_window_id: target.window_id,
                    layout_csum: target.layout_csum.clone(),
                    panes: HashSet::new(),
                },
            );
        }

        Ok(())
    }

    fn remove_detached_pane(
        &self,
        window_id: TmuxWindowId,
        new_set: &HashSet<TmuxPaneId>,
    ) -> anyhow::Result<()> {
        let mut gui_tabs = self.gui_tabs.lock();

        let (tab_id, panes) = match gui_tabs.get_mut(&window_id) {
            Some(tab) => (tab.tab_id, &mut tab.panes),
            None => anyhow::bail!("The window {window_id} is not attached"),
        };

        let to_remove: Vec<_> = panes.difference(new_set).cloned().collect();

        let mux = Mux::get();
        for p in to_remove {
            let pane_map = self.remote_panes.lock();
            let Some(pane) = pane_map.get(&p) else {
                continue;
            };
            let local_pane_id = pane.lock().local_pane_id;
            mux.remove_pane(local_pane_id);
            panes.remove(&p);
        }

        if panes.is_empty() {
            mux.remove_tab(tab_id);
            gui_tabs.remove(&window_id);
        }

        Ok(())
    }

    pub fn remove_detached_window(&self, window_id: TmuxWindowId) -> anyhow::Result<()> {
        let mut gui_tabs = self.gui_tabs.lock();
        let tab = match gui_tabs.get(&window_id) {
            Some(x) => x,
            None => {
                anyhow::bail!("Cannot find the window {window_id}")
            }
        };

        let mux = Mux::get();
        mux.remove_tab(tab.tab_id);
        gui_tabs.remove(&window_id);

        Ok(())
    }

    fn set_pane_cursor_position(&self, pane: &Arc<dyn Pane>, x: usize, y: usize) {
        pane.perform_actions(vec![Action::CSI(CSI::Cursor(
            Cursor::CharacterAndLinePosition {
                col: OneBased::from_zero_based(x as u32),
                line: OneBased::from_zero_based(y as u32),
            },
        ))]);
    }

    fn create_pane(&self, pane: &PaneItem) -> anyhow::Result<Arc<dyn Pane>> {
        let local_pane_id = alloc_pane_id();
        let active_lock = Arc::new((Mutex::new(false), Condvar::new()));
        let (output_read, output_write) = filedescriptor::socketpair()?;
        let ref_pane = Arc::new(Mutex::new(TmuxRemotePane {
            local_pane_id,
            output_write,
            active_lock: active_lock.clone(),
            session_id: 0,
            window_id: pane.window_id,
            pane_id: pane.pane_id,
            cursor_x: pane.cursor_x,
            cursor_y: pane.cursor_y,
            pane_width: pane.pane_width,
            pane_height: pane.pane_height,
            pane_left: pane.pane_left,
            pane_top: pane.pane_top,
        }));

        {
            let mut pane_map = self.remote_panes.lock();
            pane_map.insert(pane.pane_id, ref_pane.clone());
        }

        let pane_pty = TmuxPty {
            domain_id: self.domain_id,
            reader: output_read,
            cmd_queue: self.cmd_queue.clone(),
            master_pane: ref_pane,
        };

        let writer = WriterWrapper::new(pane_pty.take_writer()?);

        let size = TerminalSize {
            rows: pane.pane_height as usize,
            cols: pane.pane_width as usize,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        };

        let child = TmuxChild {
            active_lock: active_lock.clone(),
        };

        let terminal = wezterm_term::Terminal::new(
            size,
            std::sync::Arc::new(config::TermConfig::new()),
            "WezTerm",
            config::wezterm_version(),
            Box::new(writer.clone()),
        );

        Ok(Arc::new(LocalPane::new(
            local_pane_id,
            terminal,
            Box::new(child),
            Box::new(pane_pty),
            Box::new(writer),
            self.domain_id,
            "tmux pane".to_string(),
        )))
    }

    pub fn split_pane(
        &self,
        tab_id: TabId,
        pane_id: PaneId,
        remote_id: TmuxPaneId,
        split_request: SplitRequest,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        let mux = Mux::get();
        let tab = match mux.get_tab(tab_id) {
            Some(t) => t,
            None => anyhow::bail!("Invalid tab id {}", tab_id),
        };

        let pane_index = match tab
            .iter_panes_ignoring_zoom()
            .iter()
            .find(|p| p.pane.pane_id() == pane_id)
        {
            Some(p) => p.index,
            None => anyhow::bail!("invalid pane id {}", pane_id),
        };

        let split_size = match tab.compute_split_size(pane_index, split_request) {
            Some(s) => s,
            None => anyhow::bail!("invalid pane index {}", pane_index),
        };

        let window_id = match self.gui_tabs.lock().iter().find(|t| t.1.tab_id == tab_id) {
            Some((_, tab)) => tab.tmux_window_id,
            None => anyhow::bail!("No tab {}", tab_id),
        };

        let p = PaneItem {
            session_id: 0,
            window_id: window_id,
            pane_id: remote_id,
            _pane_index: 0,
            cursor_x: 0,
            cursor_y: 0,
            pane_width: split_size.second.cols as u64,
            pane_height: split_size.second.rows as u64,
            pane_left: 0,
            pane_top: 0,
            pane_active: false,
        };

        // `%layout-change` may already have materialized this pane. Returning
        // it keeps a GUI-initiated split from inserting a second copy.
        if self.check_pane_attached(window_id, remote_id) {
            let local_pane_id = self
                .remote_panes
                .lock()
                .get(&remote_id)
                .map(|pane| pane.lock().local_pane_id);
            if let Some(local_pane_id) = local_pane_id {
                if let Some(pane) = mux.get_pane(local_pane_id) {
                    return Ok(pane);
                }
            }
        }

        let pane = self.create_pane(&p).context("failed to create pane")?;
        tab.split_and_insert(pane_index, split_request, Arc::clone(&pane))?;

        self.add_attached_pane(window_id, remote_id)?;

        let _ = mux.add_pane(&pane);

        return Ok(pane);
    }

    fn flush_backlog(&self, pane_id: TmuxPaneId) {
        let Some(text) = self.backlog.lock().remove(&pane_id) else {
            return;
        };
        let pane_map = self.remote_panes.lock();
        let Some(ref_pane) = pane_map.get(&pane_id) else {
            return;
        };
        let mut ref_pane = ref_pane.lock();
        if let Err(err) = ref_pane.output_write.write_all(&text) {
            log::error!("Failed to write tmux data to output: {:#}", err);
        }
    }

    fn tmux_id_for_local(&self, local_pane_id: Option<PaneId>) -> Option<TmuxPaneId> {
        let local_pane_id = local_pane_id?;
        self.remote_panes.lock().iter().find_map(|(_, pane)| {
            let pane = pane.lock();
            if pane.local_pane_id == local_pane_id {
                Some(pane.pane_id)
            } else {
                None
            }
        })
    }

    /// Rebuild the local tab from a tmux layout spec when the set of panes changed.
    ///
    /// Returns true when the tab tree was rebuilt. GUI-initiated splits are left
    /// to `split_pane` so this does not insert a second copy while one is pending.
    pub(crate) fn reflect_layout_change(
        &self,
        window_id: TmuxWindowId,
        layout: &str,
    ) -> anyhow::Result<bool> {
        if !self.pending_splits.lock().is_empty() {
            log::debug!("deferring tmux layout-change; GUI split is in progress");
            return Ok(false);
        }

        let tab_id = {
            let gui_tabs = self.gui_tabs.lock();
            let Some(tab) = gui_tabs.get(&window_id) else {
                return Ok(false);
            };
            tab.tab_id
        };

        let node = parse_tmux_layout(layout)?;
        let new_ids = node.pane_ids();
        let old_ids = {
            let gui_tabs = self.gui_tabs.lock();
            let Some(tab) = gui_tabs.get(&window_id) else {
                return Ok(false);
            };
            if tab.panes == new_ids {
                return Ok(false);
            }
            tab.panes.clone()
        };

        let mux = Mux::get();
        let Some(tab) = mux.get_tab(tab_id) else {
            anyhow::bail!("tmux window {window_id} has no local tab");
        };

        let active_tmux = self.tmux_id_for_local(tab.get_active_pane().map(|pane| pane.pane_id()));
        let zoomed_tmux = self.tmux_id_for_local(tab.get_zoomed_pane().map(|pane| pane.pane_id()));
        let current = tab.get_size();
        let cell_width = if current.cols == 0 {
            0
        } else {
            current.pixel_width / current.cols
        };
        let cell_height = if current.rows == 0 {
            0
        } else {
            current.pixel_height / current.rows
        };
        let root_size = layout_term_size(
            node.outer_height(),
            node.outer_width(),
            cell_width,
            cell_height,
            current.dpi,
        );

        for pane_id in new_ids.difference(&old_ids) {
            let (width, height, left, top) = node.leaf_bounds(*pane_id).ok_or_else(|| {
                anyhow!("tmux layout for window {window_id} is missing pane {pane_id}")
            })?;
            let item = PaneItem {
                session_id: 0,
                window_id,
                pane_id: *pane_id,
                _pane_index: 0,
                cursor_x: 0,
                cursor_y: 0,
                pane_width: width,
                pane_height: height,
                pane_left: left,
                pane_top: top,
                pane_active: false,
            };
            let pane = self
                .create_pane(&item)
                .context("failed to create pane from tmux layout")?;
            mux.add_pane(&pane).context("failed to add tmux pane")?;
            self.add_attached_pane(window_id, *pane_id)?;
            self.flush_backlog(*pane_id);
        }

        let pane_node = node.to_pane_node(
            tab.tab_id(),
            active_tmux,
            zoomed_tmux,
            cell_width,
            cell_height,
            current.dpi,
        );
        tab.sync_with_pane_tree(root_size, pane_node, |entry| {
            let tmux_id = entry.pane_id as TmuxPaneId;
            let local_id = self
                .remote_panes
                .lock()
                .get(&tmux_id)
                .map(|pane| pane.lock().local_pane_id)
                .unwrap_or_else(|| panic!("missing remote tmux pane {}", tmux_id));
            Mux::get().get_pane(local_id).unwrap_or_else(|| {
                panic!("missing local pane {} for tmux pane {}", local_id, tmux_id)
            })
        });

        Ok(true)
    }

    fn sync_pane_state(&self, panes: &[PaneItem]) -> anyhow::Result<()> {
        let Some(current_session) = *self.tmux_session.lock() else {
            return Ok(());
        };
        let mux = Mux::get();

        for pane in panes.iter() {
            if pane.session_id != current_session
                || !self.check_pane_attached(pane.window_id, pane.pane_id)
            {
                continue;
            }

            // We now have the cursor information, fix the cursor position
            let pane_map = self.remote_panes.lock();
            let local_pane = match pane_map.get(&pane.pane_id) {
                Some(p) => {
                    let local_pane_id = p.lock().local_pane_id;
                    mux.get_pane(local_pane_id)
                }
                None => None,
            };

            if let Some(local_pane) = local_pane {
                let c = local_pane.get_cursor_position();
                // no capture, output case
                if (c.x + c.y as usize) == 0 {
                    if let Some(text) = self.backlog.lock().remove(&pane.pane_id) {
                        if let Some(ref_pane) = pane_map.get(&pane.pane_id) {
                            let mut ref_pane = ref_pane.lock();
                            if let Err(err) = ref_pane.output_write.write_all(&text) {
                                log::error!("Failed to write tmux data to output: {:#}", err);
                            }
                        }
                    }
                } else {
                    // we have capture, so remove the backlog
                    let _ = self.backlog.lock().remove(&pane.pane_id);
                    if (pane.cursor_x + pane.cursor_y) != 0 {
                        self.set_pane_cursor_position(
                            &local_pane,
                            pane.cursor_x as usize,
                            pane.cursor_y as usize,
                        );
                    }
                }
                if pane.pane_active {
                    let gui_tabs = self.gui_tabs.lock();

                    let Some(local_tab) = gui_tabs.get(&pane.window_id) else {
                        anyhow::bail!("invalid tmux window id {}", pane.window_id);
                    };

                    match mux.get_tab(local_tab.tab_id) {
                        Some(tab) => {
                            tab.set_active_pane(&local_pane);
                            mux.notify(MuxNotification::PaneFocused(local_pane.pane_id()));
                        }
                        None => {}
                    }
                }
            }

            log::info!("new pane synced, id: {}", pane.pane_id);
        }

        Ok(())
    }

    fn sync_window_state(&self, windows: &[WindowItem], new_window: bool) -> anyhow::Result<()> {
        let Some(current_session) = *self.tmux_session.lock() else {
            return Ok(());
        };
        let mux = Mux::get();

        self.create_gui_window();
        let mut gui_window = self.gui_window.lock();
        let gui_window_id = match gui_window.as_mut() {
            Some(x) => x,
            None => {
                anyhow::bail!("No tmux gui created");
            }
        };

        for window in windows.iter() {
            if window.session_id != current_session {
                continue;
            }

            let size = TerminalSize {
                rows: window.window_height as usize,
                cols: window.window_width as usize,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 0,
            };

            let tab = Arc::new(Tab::new(&size));
            tab.set_title(&format!("{}", &window.window_name));
            mux.add_tab_no_panes(&tab);

            let _ = self.add_attached_window(window, &tab.tab_id())?;

            let mut split_stack;
            let mut split_direction;

            let mut split_pane_index = 1;
            for l in &window.layout {
                match l {
                    WindowLayout::SinglePane(x) => {
                        let p = PaneItem {
                            session_id: window.session_id,
                            window_id: window.window_id,
                            _pane_index: 0,
                            cursor_x: 0,
                            cursor_y: 0,
                            pane_active: false,
                            pane_id: x.pane_id,
                            pane_width: x.pane_width,
                            pane_height: x.pane_height,
                            pane_left: x.pane_left,
                            pane_top: x.pane_top,
                        };
                        let local_pane = self.create_pane(&p).context("failed to create pane")?;
                        tab.assign_pane(&local_pane);
                        self.add_attached_pane(p.window_id, p.pane_id)?;
                        let _ = mux.add_pane(&local_pane);
                        break;
                    }

                    WindowLayout::SplitHorizontal(x) => {
                        split_direction = SplitDirection::Horizontal;
                        split_stack = x;
                    }

                    WindowLayout::SplitVertical(x) => {
                        split_direction = SplitDirection::Vertical;
                        split_stack = x;
                    }
                }

                for x in split_stack {
                    let p = PaneItem {
                        session_id: window.session_id,
                        window_id: window.window_id,
                        _pane_index: 0,
                        cursor_x: 0,
                        cursor_y: 0,
                        pane_active: false,
                        pane_id: x.pane_id,
                        pane_width: x.pane_width,
                        pane_height: x.pane_height,
                        pane_left: x.pane_left,
                        pane_top: x.pane_top,
                    };
                    let local_pane;
                    if !self.check_pane_attached(p.window_id, p.pane_id) {
                        local_pane = self.create_pane(&p).context("failed to create pane")?;
                        self.add_attached_pane(p.window_id, p.pane_id)?;
                        let _ = mux.add_pane(&local_pane);
                        if let None = tab.get_active_pane() {
                            tab.assign_pane(&local_pane);
                            split_pane_index = tab.get_active_idx();
                            continue;
                        }

                        split_pane_index = tab.split_and_insert(
                            split_pane_index,
                            SplitRequest {
                                direction: split_direction,
                                target_is_second: false,
                                top_level: false,
                                size: SplitSize::Cells(
                                    if split_direction == SplitDirection::Horizontal {
                                        p.pane_width as usize
                                    } else {
                                        p.pane_height as usize
                                    },
                                ),
                            },
                            local_pane.clone(),
                        )? + 1;
                    } else {
                        let pane_map = self.remote_panes.lock();
                        let local_pane_id = match pane_map.get(&p.pane_id) {
                            Some(x) => x.lock().local_pane_id,
                            None => anyhow::bail!("cannot find the local pane for {}", p.pane_id),
                        };

                        split_pane_index = match tab
                            .iter_panes_ignoring_zoom()
                            .iter()
                            .find(|x| x.pane.pane_id() == local_pane_id)
                        {
                            Some(x) => x.index,
                            None => {
                                log::info!("invalid pane id {local_pane_id}");
                                continue;
                            }
                        };
                        continue;
                    }
                }
            }

            mux.add_tab_to_window(&tab, **gui_window_id)?;
            gui_window_id.notify();

            let gui_tabs = self.gui_tabs.lock();
            let local_tab = match gui_tabs.get(&window.window_id) {
                Some(x) => x,
                None => {
                    log::info!(
                        "cannot find the local tab for tmux window {}",
                        window.window_id
                    );
                    continue;
                }
            };

            // For new window, we wait for natural output instead of capturing
            if !new_window {
                for p in local_tab.panes.iter() {
                    self.cmd_queue.lock().push_back(Box::new(CapturePane {
                        pane_id: *p,
                        history_limit: window.history_limit,
                    }));
                }
            }

            // To keep the active window last one to make it active after set the focus pane
            if !window.window_active {
                self.cmd_queue.lock().push_back(Box::new(ListAllPanes {
                    window_id: window.window_id,
                    prune: false,
                    layout_csum: window.layout_csum.clone(),
                    layout: String::new(),
                }));
            }
        }

        // To keep the active window last one to make it active after set the focus pane
        match windows.iter().find(|w| w.window_active) {
            Some(window) => {
                self.cmd_queue.lock().push_back(Box::new(ListAllPanes {
                    window_id: window.window_id,
                    prune: false,
                    layout_csum: window.layout_csum.clone(),
                    layout: String::new(),
                }));
            }
            None => {}
        }

        if *self.attach_state.lock() == AttachState::Init {
            self.cmd_queue.lock().push_back(Box::new(AttachDone));
        }

        TmuxDomainState::schedule_send_next_command(self.domain_id);

        Ok(())
    }

    pub fn subscribe_notification(&self) {
        let mux = Mux::get();
        let domain_id = self.domain_id;
        mux.subscribe(move |n| {
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get();
                let domain = match mux.get_domain(domain_id) {
                    Some(d) => d,
                    None => return,
                };
                let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
                    Some(t) => t,
                    None => return,
                };

                if *tmux_domain.inner.attach_state.lock() == AttachState::Init {
                    return;
                }

                match n {
                    MuxNotification::PaneFocused(pane_id) => {
                        let tmux_pane_id = match tmux_domain
                            .inner
                            .remote_panes
                            .lock()
                            .iter()
                            .find(|(_, p)| p.lock().local_pane_id == pane_id)
                        {
                            Some((_, p)) => Some(p.lock().pane_id),
                            None => None,
                        };

                        if let Some(pane_id) = tmux_pane_id {
                            tmux_domain
                                .inner
                                .cmd_queue
                                .lock()
                                .push_back(Box::new(SelectPane { pane_id: pane_id }));
                            TmuxDomainState::schedule_send_next_command(domain_id);
                        }
                    }
                    MuxNotification::WindowInvalidated(window_id) => {
                        if let Some(window) = mux.get_window(window_id) {
                            let Some(tab) = window.get_active_tab() else {
                                return;
                            };
                            let tmux_window_id = match tmux_domain
                                .inner
                                .gui_tabs
                                .lock()
                                .iter()
                                .find(|(_, t)| t.tab_id == tab.tab_id())
                            {
                                Some((_, t)) => Some(t.tmux_window_id),
                                None => None,
                            };
                            if let Some(window_id) = tmux_window_id {
                                tmux_domain.inner.cmd_queue.lock().push_back(Box::new(
                                    SelectWindow {
                                        window_id: window_id,
                                    },
                                ));
                                TmuxDomainState::schedule_send_next_command(domain_id);
                            }
                        }
                    }
                    _ => {}
                }
            })
            .detach();
            true
        });
    }
}

fn parse_sigil_number(text: &str) -> anyhow::Result<u64> {
    let num = text
        .get(1..)
        .ok_or_else(|| anyhow!("wrong prefixed id"))?
        .parse()?;

    Ok(num)
}

#[derive(Debug)]
pub(crate) struct ListAllPanes {
    pub window_id: TmuxWindowId,
    pub prune: bool,
    pub layout_csum: String,
    /// Layout body from `%layout-change`, without the 4-char checksum prefix.
    /// Empty when this command was queued for the initial attach sync.
    pub layout: String,
}

impl TmuxCommand for ListAllPanes {
    fn get_command(&self, domain_id: DomainId) -> String {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => return "".to_string(),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => return "".to_string(),
        };

        let mut gui_tabs = tmux_domain.inner.gui_tabs.lock();

        let Some(local_tab) = gui_tabs.get_mut(&self.window_id) else {
            return "".to_string();
        };

        if local_tab.layout_csum.eq(&self.layout_csum) {
            if self.prune {
                return "".to_string();
            }
        } else {
            local_tab.layout_csum = self.layout_csum.clone();
        }

        format!(
            "list-panes -F '#{{session_id}} #{{window_id}} #{{pane_id}} \
            #{{pane_index}} #{{cursor_x}} #{{cursor_y}} #{{pane_width}} #{{pane_height}} \
            #{{pane_left}} #{{pane_top}} #{{pane_active}}' -t @{}\n",
            self.window_id
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("list-pane in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        let mut items = vec![];
        let mut pane_set = HashSet::new();
        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id =
                parse_sigil_number(fields.next().ok_or_else(|| anyhow!("missing session_id"))?)?;
            let window_id =
                parse_sigil_number(fields.next().ok_or_else(|| anyhow!("missing window_id"))?)?;
            let pane_id =
                parse_sigil_number(fields.next().ok_or_else(|| anyhow!("missing pane_id"))?)?;
            let _pane_index = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_index"))?
                .parse()?;
            let cursor_x = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_x"))?
                .parse()?;
            let cursor_y = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_y"))?
                .parse()?;
            let pane_width = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_width"))?
                .parse()?;
            let pane_height = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_height"))?
                .parse()?;
            let pane_left = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_left"))?
                .parse()?;
            let pane_top = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_top"))?
                .parse()?;
            let pane_active = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_active"))?
                .parse::<usize>()?;

            let pane_active = pane_active == 1;

            pane_set.insert(pane_id);

            items.push(PaneItem {
                session_id,
                window_id,
                pane_id,
                _pane_index,
                cursor_x,
                cursor_y,
                pane_width,
                pane_height,
                pane_left,
                pane_top,
                pane_active,
            });
        }

        log::debug!("panes in domain_id {}: {:?}", domain_id, items);
        let mux = Mux::get();
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                if !self.prune {
                    return tmux_domain.inner.sync_pane_state(&items);
                } else {
                    // A scripted `split-window` (or pane close) shows up as
                    // `%layout-change`. Pruning only removes panes; apply the
                    // layout so newly created panes appear without re-attach.
                    let changed = if self.layout.is_empty() {
                        false
                    } else {
                        match tmux_domain
                            .inner
                            .reflect_layout_change(self.window_id, &self.layout)
                        {
                            Ok(changed) => changed,
                            Err(err) => {
                                log::error!(
                                    "tmux layout-change for window {} failed: {:#}",
                                    self.window_id,
                                    err
                                );
                                false
                            }
                        }
                    };
                    tmux_domain
                        .inner
                        .remove_detached_pane(self.window_id, &pane_set)?;
                    if changed {
                        tmux_domain.inner.sync_pane_state(&items)?;
                    }
                    return Ok(());
                }
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}

#[derive(Debug)]
pub(crate) struct ListAllWindows {
    pub session_id: TmuxSessionId,
    pub window_id: Option<TmuxWindowId>,
}

impl TmuxCommand for ListAllWindows {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!(
            "list-windows -F \
                '#{{session_id}} #{{window_id}} \
                #{{window_width}} #{{window_height}} \
                #{{window_active}} \
                #{{window_name}} \
                #{{window_layout}} \
                #{{history_limit}}' -t ${}\n",
            self.session_id
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("list-window in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        let mut items = vec![];

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            let session_id =
                parse_sigil_number(fields.next().ok_or_else(|| anyhow!("missing session_id"))?)?;
            let window_id =
                parse_sigil_number(fields.next().ok_or_else(|| anyhow!("missing window_id"))?)?;
            let window_width = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_width"))?
                .parse()?;
            let window_height = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_height"))?
                .parse()?;
            let window_active = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_active"))?
                .parse::<usize>()?;

            let window_name = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_name"))?;

            let window_layout = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_layout"))?;

            let history_limit = fields
                .next()
                .ok_or_else(|| anyhow!("missing history_limit"))?
                .parse::<isize>()?;

            let window_active = window_active == 1;

            if let Some(x) = self.window_id {
                if x != window_id {
                    continue;
                }
            }

            let layout_csum = window_layout
                .get(0..4)
                .ok_or_else(|| anyhow!("missing window_layout"))?;
            let window_layout = window_layout
                .get(5..)
                .ok_or_else(|| anyhow!("missing window_layout"))?;

            let layout = parse_layout(window_layout)?;

            items.push(WindowItem {
                session_id,
                window_id,
                window_width,
                window_height,
                window_active,
                window_name: window_name.to_string(),
                layout,
                layout_csum: layout_csum.to_string(),
                history_limit,
            });
        }

        log::debug!("layout in domain_id {}: {:#?}", domain_id, items);
        let mux = Mux::get();
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                let new_window = if let Some(_x) = self.window_id {
                    true
                } else {
                    false
                };
                return tmux_domain.inner.sync_window_state(&items, new_window);
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}

#[derive(Debug)]
pub(crate) struct Resize {
    pub pane_id: TmuxPaneId,
    pub size: PtySize,
}

impl TmuxCommand for Resize {
    fn get_command(&self, domain_id: DomainId) -> String {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => return "".to_string(),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => return "".to_string(),
        };

        // Not in stable state for now, don't do resizing, otherwise it will cause tmux output
        // unexpected content.
        if *tmux_domain.inner.attach_state.lock() == AttachState::Init {
            return "".to_string();
        }

        let pane_map = tmux_domain.inner.remote_panes.lock();
        {
            let mut pane = match pane_map.get(&self.pane_id) {
                Some(x) => x.lock(),
                None => return "".to_string(),
            };

            if pane.pane_width == self.size.cols as u64 && pane.pane_height == self.size.rows as u64
            {
                return "".to_string();
            } else {
                pane.pane_width = self.size.cols as u64;
                pane.pane_height = self.size.rows as u64;
            }
        }

        let tmux_window_id = match pane_map.get(&self.pane_id) {
            Some(x) => x.lock().window_id,
            None => return "".to_string(),
        };

        let gui_tabs = tmux_domain.inner.gui_tabs.lock();
        let local_tab = match gui_tabs.get(&tmux_window_id) {
            Some(t) => t,
            None => return "".to_string(),
        };

        let size = match mux.get_tab(local_tab.tab_id) {
            Some(x) => x.get_size(),
            None => return "".to_string(),
        };

        let support_commands = tmux_domain.inner.support_commands.lock();

        if let Some(_x) = support_commands.get("resize-window") {
            format!(
                "resize-window -x {} -y {} -t @{}\nresize-pane -x {} -y {} -t %{}\n",
                size.cols, size.rows, tmux_window_id, self.size.cols, self.size.rows, self.pane_id
            )
        } else if let Some(x) = support_commands.get("refresh-client") {
            if x.contains("-C XxY") {
                format!(
                    "refresh-client -C {}x{}\nresize-pane -x {} -y {} -t %{}\n",
                    size.cols, size.rows, self.size.cols, self.size.rows, self.pane_id
                )
            } else {
                format!(
                    "refresh-client -C {},{}\nresize-pane -x {} -y {} -t %{}\n",
                    size.cols, size.rows, self.size.cols, self.size.rows, self.pane_id
                )
            }
        } else {
            log::info!("The tmux version is not supported");
            return "".to_string();
        }
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("resize-pane in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct CapturePane {
    pane_id: TmuxPaneId,
    history_limit: isize,
}

impl TmuxCommand for CapturePane {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!(
            "capture-pane -p -t %{} -e -C -S {}\n",
            self.pane_id,
            self.history_limit * -1
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("capture-pane in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => anyhow::bail!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => anyhow::bail!("Tmux domain lost"),
        };

        let unescaped = termwiz::tmux_cc::unvis(&result.output).context("unescape pane content")?;
        // capturep contents returned from guarded lines which always contain a tailing '\n'
        let unescaped = &unescaped[0..unescaped.len().saturating_sub(1)].replace("\n", "\r\n");

        let pane_map = tmux_domain.inner.remote_panes.lock();
        if let Some(pane) = pane_map.get(&self.pane_id) {
            let mut pane = pane.lock();
            if let Some(p) = mux.get_pane(pane.local_pane_id) {
                tmux_domain.inner.set_pane_cursor_position(&p, 0, 0);
            }

            pane.output_write
                .write_all(unescaped.as_bytes())
                .context("writing capture pane result to output")?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SendKeys {
    pub keys: Vec<u8>,
    pub pane: TmuxPaneId,
}
impl TmuxCommand for SendKeys {
    fn get_command(&self, _domain_id: DomainId) -> String {
        let mut s = String::new();
        for &byte in self.keys.iter() {
            write!(&mut s, "0x{:X} ", byte).expect("unable to write key");
        }
        format!("send-keys -H -t %{} {}\n", self.pane, s)
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("send-key in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct NewWindow;
impl TmuxCommand for NewWindow {
    fn get_command(&self, _domain_id: DomainId) -> String {
        "new-window\n".to_owned()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("new-window in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ListCommands;
impl TmuxCommand for ListCommands {
    fn get_command(&self, _domain_id: DomainId) -> String {
        "list-commands\n".to_owned()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("list-command in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => anyhow::bail!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => anyhow::bail!("Tmux domain lost"),
        };

        let mut support_commands = tmux_domain.inner.support_commands.lock();

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let v: Vec<&str> = line.split(' ').collect();
            support_commands.insert(v[0].to_string(), line.to_string());
        }

        let mut cmd_queue = tmux_domain.inner.cmd_queue.as_ref().lock();
        if let Some(session) = *tmux_domain.inner.tmux_session.lock() {
            cmd_queue.push_back(Box::new(ListAllWindows {
                session_id: session,
                window_id: None,
            }));
            TmuxDomainState::schedule_send_next_command(domain_id);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SplitPane {
    pub pane_id: TmuxPaneId,
    pub direction: SplitDirection,
}

impl TmuxCommand for SplitPane {
    fn get_command(&self, _domain_id: DomainId) -> String {
        if self.direction == SplitDirection::Horizontal {
            format!("split-window -h -t %{}\n", self.pane_id)
        } else {
            format!("split-window -v -t %{}\n", self.pane_id)
        }
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("split-window in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SelectWindow {
    pub window_id: TmuxWindowId,
}

impl TmuxCommand for SelectWindow {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!("select-window -t @{}\n", self.window_id)
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("select-window in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SelectPane {
    pub pane_id: TmuxPaneId,
}

impl TmuxCommand for SelectPane {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!("select-pane -t %{}\n", self.pane_id)
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("select-pane in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

// This is a dummy command which indicates the attaching is done, it prevents the tmux output
// the unexpected and unnecessary content when syncing with back end in attaching stage.
#[derive(Debug)]
pub(crate) struct AttachDone;
impl TmuxCommand for AttachDone {
    fn get_command(&self, _domain_id: DomainId) -> String {
        // The command doesn't matter, just give a legal simple command to let process_result called.
        "list-session\n".to_string()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            let error = format!("list-session in domain={domain_id} failed: {result:#?}");
            log::error!("{error}");
            anyhow::bail!("{error}");
        }
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => anyhow::bail!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => anyhow::bail!("Tmux domain lost"),
        };

        // Do nothing, just change the state.
        *tmux_domain.inner.attach_state.lock() = AttachState::Done;
        Ok(())
    }
}

/// Split a `%layout-change` payload (`checksum,spec`) into its checksum and spec.
/// The spec is what `parse_tmux_layout` accepts. If the checksum prefix is missing,
/// the body is empty and callers keep the previous prune-only behavior.
pub(crate) fn split_layout_spec(layout: &str) -> (String, String) {
    if let Some((checksum, body)) = layout.split_once(',') {
        if checksum.len() == 4
            && checksum.chars().all(|c| c.is_ascii_hexdigit())
            && body.starts_with(|c: char| c.is_ascii_digit())
        {
            return (checksum.to_string(), body.to_string());
        }
    }
    (layout.chars().take(4).collect(), String::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TmuxLayoutNode {
    Leaf {
        pane_id: TmuxPaneId,
        width: u64,
        height: u64,
        x: u64,
        y: u64,
    },
    Split {
        /// `{...}` is side-by-side. `[...]` is stacked.
        horizontal: bool,
        width: u64,
        height: u64,
        x: u64,
        y: u64,
        children: Vec<TmuxLayoutNode>,
    },
}

impl TmuxLayoutNode {
    fn outer_width(&self) -> u64 {
        match self {
            Self::Leaf { width, .. } | Self::Split { width, .. } => *width,
        }
    }

    fn outer_height(&self) -> u64 {
        match self {
            Self::Leaf { height, .. } | Self::Split { height, .. } => *height,
        }
    }

    fn pane_ids(&self) -> HashSet<TmuxPaneId> {
        let mut ids = HashSet::new();
        self.collect_pane_ids(&mut ids);
        ids
    }

    fn collect_pane_ids(&self, ids: &mut HashSet<TmuxPaneId>) {
        match self {
            Self::Leaf { pane_id, .. } => {
                ids.insert(*pane_id);
            }
            Self::Split { children, .. } => {
                for child in children {
                    child.collect_pane_ids(ids);
                }
            }
        }
    }

    fn leaf_bounds(&self, pane_id: TmuxPaneId) -> Option<(u64, u64, u64, u64)> {
        match self {
            Self::Leaf {
                pane_id: id,
                width,
                height,
                x,
                y,
            } if *id == pane_id => Some((*width, *height, *x, *y)),
            Self::Leaf { .. } => None,
            Self::Split { children, .. } => {
                children.iter().find_map(|child| child.leaf_bounds(pane_id))
            }
        }
    }

    fn to_pane_node(
        &self,
        tab_id: TabId,
        active_tmux: Option<TmuxPaneId>,
        zoomed_tmux: Option<TmuxPaneId>,
        cell_width: usize,
        cell_height: usize,
        dpi: u32,
    ) -> PaneNode {
        match self {
            Self::Leaf {
                pane_id,
                width,
                height,
                x,
                y,
            } => PaneNode::Leaf(PaneEntry {
                window_id: 0,
                tab_id,
                // Stash the tmux pane id; the sync callback maps it to a local pane.
                pane_id: *pane_id as PaneId,
                title: String::new(),
                size: layout_term_size(*height, *width, cell_width, cell_height, dpi),
                working_dir: None,
                is_active_pane: active_tmux == Some(*pane_id),
                is_zoomed_pane: zoomed_tmux == Some(*pane_id),
                workspace: String::new(),
                cursor_pos: StableCursorPosition::default(),
                physical_top: 0,
                top_row: *y as usize,
                left_col: *x as usize,
                tty_name: None,
            }),
            Self::Split {
                horizontal,
                children,
                ..
            } => binary_layout_split(
                children,
                *horizontal,
                tab_id,
                active_tmux,
                zoomed_tmux,
                cell_width,
                cell_height,
                dpi,
            ),
        }
    }
}

fn layout_term_size(
    rows: u64,
    cols: u64,
    cell_width: usize,
    cell_height: usize,
    dpi: u32,
) -> TerminalSize {
    TerminalSize {
        rows: rows as usize,
        cols: cols as usize,
        pixel_width: (cols as usize).saturating_mul(cell_width),
        pixel_height: (rows as usize).saturating_mul(cell_height),
        dpi,
    }
}

/// Fold an n-ary tmux split into WezTerm's binary split tree.
/// Child outer sizes already include borders inside that child, and each
/// binary node accounts for the single border between its two sides.
fn binary_layout_split(
    children: &[TmuxLayoutNode],
    horizontal: bool,
    tab_id: TabId,
    active_tmux: Option<TmuxPaneId>,
    zoomed_tmux: Option<TmuxPaneId>,
    cell_width: usize,
    cell_height: usize,
    dpi: u32,
) -> PaneNode {
    let mut iter = children.iter();
    let Some(first) = iter.next() else {
        return PaneNode::Empty;
    };
    let mut acc = first.to_pane_node(
        tab_id,
        active_tmux,
        zoomed_tmux,
        cell_width,
        cell_height,
        dpi,
    );
    let mut acc_w = first.outer_width();
    let mut acc_h = first.outer_height();
    for child in iter {
        let right = child.to_pane_node(
            tab_id,
            active_tmux,
            zoomed_tmux,
            cell_width,
            cell_height,
            dpi,
        );
        let (direction, first_size, second_size, new_w, new_h) = if horizontal {
            (
                SplitDirection::Horizontal,
                layout_term_size(acc_h, acc_w, cell_width, cell_height, dpi),
                layout_term_size(
                    child.outer_height(),
                    child.outer_width(),
                    cell_width,
                    cell_height,
                    dpi,
                ),
                acc_w + child.outer_width() + 1,
                acc_h.max(child.outer_height()),
            )
        } else {
            (
                SplitDirection::Vertical,
                layout_term_size(acc_h, acc_w, cell_width, cell_height, dpi),
                layout_term_size(
                    child.outer_height(),
                    child.outer_width(),
                    cell_width,
                    cell_height,
                    dpi,
                ),
                acc_w.max(child.outer_width()),
                acc_h + child.outer_height() + 1,
            )
        };
        acc = PaneNode::Split {
            left: Box::new(acc),
            right: Box::new(right),
            node: SplitDirectionAndSize {
                direction,
                first: first_size,
                second: second_size,
            },
        };
        acc_w = new_w;
        acc_h = new_h;
    }
    acc
}

fn parse_tmux_layout(layout: &str) -> anyhow::Result<TmuxLayoutNode> {
    let (node, rest) = parse_layout_node(layout)?;
    if !rest.is_empty() {
        anyhow::bail!("trailing tmux layout data: {rest}");
    }
    Ok(node)
}

fn parse_layout_node(s: &str) -> anyhow::Result<(TmuxLayoutNode, &str)> {
    let (width, s) = parse_layout_u64(s)?;
    let s = expect_layout_char(s, 'x')?;
    let (height, s) = parse_layout_u64(s)?;
    let s = expect_layout_char(s, ',')?;
    let (x, s) = parse_layout_u64(s)?;
    let s = expect_layout_char(s, ',')?;
    let (y, s) = parse_layout_u64(s)?;

    if s.starts_with('{') || s.starts_with('[') {
        let horizontal = s.starts_with('{');
        let mut s = &s[1..];
        let mut children = Vec::new();
        loop {
            let (child, rest) = parse_layout_node(s)?;
            children.push(child);
            s = rest;
            if let Some(rest) = s.strip_prefix(',') {
                s = rest;
                continue;
            }
            let close = if horizontal { '}' } else { ']' };
            if let Some(rest) = s.strip_prefix(close) {
                s = rest;
                break;
            }
            anyhow::bail!("malformed tmux layout near '{s}'");
        }
        if children.is_empty() {
            anyhow::bail!("empty tmux layout split");
        }
        Ok((
            TmuxLayoutNode::Split {
                horizontal,
                width,
                height,
                x,
                y,
                children,
            },
            s,
        ))
    } else if let Some(rest) = s.strip_prefix(',') {
        let (pane_id, s) = parse_layout_u64(rest)?;
        Ok((
            TmuxLayoutNode::Leaf {
                pane_id,
                width,
                height,
                x,
                y,
            },
            s,
        ))
    } else {
        anyhow::bail!("malformed tmux layout near '{s}'");
    }
}

fn parse_layout_u64(s: &str) -> anyhow::Result<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end == 0 {
        anyhow::bail!("expected number in tmux layout near '{s}'");
    }
    let n = s[..end].parse()?;
    Ok((n, &s[end..]))
}

fn expect_layout_char(s: &str, expected: char) -> anyhow::Result<&str> {
    match s.strip_prefix(expected) {
        Some(rest) => Ok(rest),
        None => anyhow::bail!("expected '{expected}' in tmux layout near '{s}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::{TmuxDomain, TmuxTab};
    use crate::Mux;
    use std::sync::{Mutex, OnceLock};

    fn scheduler_queue() -> &'static Mutex<Vec<promise::spawn::Runnable>> {
        static QUEUE: OnceLock<Mutex<Vec<promise::spawn::Runnable>>> = OnceLock::new();
        QUEUE.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn install_test_scheduler() {
        if promise::spawn::is_scheduler_configured() {
            return;
        }
        promise::spawn::set_schedulers(
            Box::new(|task| {
                scheduler_queue().lock().unwrap().push(task);
            }),
            Box::new(|task| {
                scheduler_queue().lock().unwrap().push(task);
            }),
        );
    }

    fn drain_test_scheduler() {
        loop {
            let tasks: Vec<_> = scheduler_queue().lock().unwrap().drain(..).collect();
            if tasks.is_empty() {
                break;
            }
            for task in tasks {
                task.run();
            }
        }
    }

    #[test]
    fn split_layout_spec_strips_checksum() {
        assert_eq!(
            split_layout_spec("b25d,80x24,0,0,0"),
            ("b25d".to_string(), "80x24,0,0,0".to_string())
        );
        assert_eq!(
            split_layout_spec("cafd,120x29,0,0{60x29,0,0,0,59x29,61,0,1}"),
            (
                "cafd".to_string(),
                "120x29,0,0{60x29,0,0,0,59x29,61,0,1}".to_string()
            )
        );
        assert_eq!(
            split_layout_spec("not a layout"),
            ("not ".to_string(), String::new())
        );
    }

    #[test]
    fn parse_layout_keeps_every_pane_id() {
        let single = parse_tmux_layout("158x40,0,0,72").unwrap();
        assert_eq!(single.pane_ids(), HashSet::from([72]));

        let horizontal = parse_tmux_layout("80x24,0,0{40x24,0,0,0,39x24,41,0,1}").unwrap();
        assert_eq!(horizontal.pane_ids(), HashSet::from([0, 1]));
        assert!(matches!(
            horizontal,
            TmuxLayoutNode::Split {
                horizontal: true,
                ..
            }
        ));

        let vertical = parse_tmux_layout("80x24,0,0[80x12,0,0,0,80x11,0,13,1]").unwrap();
        assert_eq!(vertical.pane_ids(), HashSet::from([0, 1]));
        assert!(matches!(
            vertical,
            TmuxLayoutNode::Split {
                horizontal: false,
                ..
            }
        ));

        let nested = parse_tmux_layout(
            "158x40,0,0{79x40,0,0[79x20,0,0,74,79x19,0,21{39x19,0,21,76,39x19,40,21,77}],78x40,80,0,75}",
        )
        .unwrap();
        assert_eq!(nested.pane_ids(), HashSet::from([74, 75, 76, 77]));

        let nested_vertical = parse_tmux_layout(
            "158x40,0,0[158x20,0,0,69,158x19,0,21{79x19,0,21,70,78x19,80,21[78x9,80,21,71,78x9,80,31,73]}]",
        )
        .unwrap();
        assert_eq!(nested_vertical.pane_ids(), HashSet::from([69, 70, 71, 73]));
    }

    #[test]
    fn layout_change_creates_pane_for_scripted_split() {
        install_test_scheduler();
        let _mux = Mux::new(None);
        let mux = std::sync::Arc::new(_mux);
        Mux::set_mux(&mux);

        let domain = TmuxDomain::new(1);
        let size = TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        };
        let tab = std::sync::Arc::new(Tab::new(&size));
        mux.add_tab_no_panes(&tab);

        let initial = PaneItem {
            session_id: 1,
            window_id: 7,
            pane_id: 0,
            _pane_index: 0,
            cursor_x: 0,
            cursor_y: 0,
            pane_width: 80,
            pane_height: 24,
            pane_left: 0,
            pane_top: 0,
            pane_active: true,
        };
        let pane = domain
            .inner
            .create_pane(&initial)
            .expect("create initial pane");
        let original_local_id = pane.pane_id();
        tab.assign_pane(&pane);
        mux.add_pane(&pane).expect("add initial pane");
        domain.inner.gui_tabs.lock().insert(
            7,
            TmuxTab {
                tab_id: tab.tab_id(),
                tmux_window_id: 7,
                layout_csum: "old0".to_string(),
                panes: HashSet::from([0]),
            },
        );
        *domain.inner.tmux_session.lock() = Some(1);

        // Same pane set (a resize) must not invent panes.
        assert_eq!(
            domain
                .inner
                .reflect_layout_change(7, "80x24,0,0,0")
                .expect("same layout"),
            false
        );
        assert_eq!(tab.count_panes(), Some(1));

        // A GUI split owns pane creation until its promise resolves.
        domain
            .inner
            .pending_splits
            .lock()
            .push_back(promise::Promise::new());
        assert_eq!(
            domain
                .inner
                .reflect_layout_change(7, "80x24,0,0{40x24,0,0,0,39x24,41,0,1}")
                .expect("deferred layout"),
            false
        );
        assert_eq!(tab.count_panes(), Some(1));
        domain.inner.pending_splits.lock().clear();

        assert_eq!(
            domain
                .inner
                .reflect_layout_change(7, "80x24,0,0{40x24,0,0,0,39x24,41,0,1}")
                .expect("horizontal split"),
            true
        );
        let panes = tab.iter_panes();
        assert_eq!(panes.len(), 2);
        let mut widths: Vec<_> = panes.iter().map(|pane| pane.width).collect();
        widths.sort_unstable();
        assert_eq!(widths, vec![39, 40]);
        assert!(panes
            .iter()
            .any(|pane| pane.pane.pane_id() == original_local_id));
        assert_eq!(
            domain.inner.gui_tabs.lock().get(&7).unwrap().panes,
            HashSet::from([0, 1])
        );

        // The layout-change path and the GUI split path must not double-insert.
        let again = domain
            .inner
            .split_pane(
                tab.tab_id(),
                original_local_id,
                1,
                SplitRequest {
                    direction: SplitDirection::Horizontal,
                    target_is_second: true,
                    top_level: false,
                    size: SplitSize::Cells(39),
                },
            )
            .expect("existing pane");
        assert_eq!(tab.count_panes(), Some(2));
        assert!(panes
            .iter()
            .any(|pane| pane.pane.pane_id() == again.pane_id()));

        assert_eq!(
            domain
                .inner
                .reflect_layout_change(7, "80x24,0,0[80x12,0,0,0,80x11,0,13,2]")
                .expect("vertical split"),
            true
        );
        let panes = tab.iter_panes();
        assert_eq!(panes.len(), 2);
        let mut heights: Vec<_> = panes.iter().map(|pane| pane.height).collect();
        heights.sort_unstable();
        assert_eq!(heights, vec![11, 12]);

        // Closing the extra pane collapses back to the survivor.
        assert_eq!(
            domain
                .inner
                .reflect_layout_change(7, "80x24,0,0,0")
                .expect("close split"),
            true
        );
        assert_eq!(tab.count_panes(), Some(1));
        let alive = HashSet::from([0]);
        domain
            .inner
            .remove_detached_pane(7, &alive)
            .expect("prune closed pane");
        assert_eq!(
            domain.inner.gui_tabs.lock().get(&7).unwrap().panes,
            HashSet::from([0])
        );

        assert!(domain
            .inner
            .reflect_layout_change(7, "not-a-layout")
            .is_err());

        // Run resize notifications while the mux is still alive. Dropping the
        // domain later closes pane pipes; those EOF tasks stay queued.
        drain_test_scheduler();
    }
}
