use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState, SplitSource};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitRequest, Tab, TabId};
use crate::tmux_commands::{ListAllWindows, NewWindow, SplitPane, TmuxCommand};
use crate::window::WindowId;
use crate::{Mux, MuxWindowBuilder};
use async_trait::async_trait;
use filedescriptor::FileDescriptor;
use parking_lot::{Condvar, Mutex};
use portable_pty::CommandBuilder;
use smol::channel::{bounded, Receiver, Sender};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::sync::Arc;
use termwiz::tmux_cc::*;
use wezterm_term::TerminalSize;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum State {
    WaitForInitialGuard,
    Idle,
    WaitingForResponse,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct TmuxRemotePane {
    // members for local
    pub local_pane_id: PaneId,
    pub output_write: FileDescriptor,
    pub active_lock: Arc<(Mutex<bool>, Condvar)>,
    // members sync with remote
    pub session_id: TmuxSessionId,
    pub window_id: TmuxWindowId,
    pub pane_id: TmuxPaneId,
    pub cursor_x: u64,
    pub cursor_y: u64,
    pub pane_width: u64,
    pub pane_height: u64,
    pub pane_left: u64,
    pub pane_top: u64,
}

pub(crate) type RefTmuxRemotePane = Arc<Mutex<TmuxRemotePane>>;

/// As a remote TmuxTab, keeping the TmuxPanes ID
/// within the remote tab.
#[allow(dead_code)]
pub(crate) struct TmuxTab {
    pub tab_id: TabId, // local tab ID
    pub tmux_window_id: TmuxWindowId,
    pub panes: HashSet<TmuxPaneId>, // tmux panes within tmux window
}

pub(crate) type TmuxCmdQueue = VecDeque<Box<dyn TmuxCommand>>;
pub(crate) struct TmuxDomainState {
    pub pane_id: PaneId,     // ID of the original pane
    pub domain_id: DomainId, // ID of TmuxDomain
    state: Mutex<State>,
    pub cmd_queue: Arc<Mutex<TmuxCmdQueue>>,
    pub gui_window: Mutex<Option<MuxWindowBuilder>>,
    pub gui_tabs: Mutex<Vec<TmuxTab>>,
    pub remote_panes: Mutex<HashMap<TmuxPaneId, RefTmuxRemotePane>>,
    pub tmux_session: Mutex<Option<TmuxSessionId>>,
    pub channel: (Sender<TmuxPaneId>, Receiver<TmuxPaneId>),
}

pub struct TmuxDomain {
    pub(crate) inner: Arc<TmuxDomainState>,
}

impl TmuxDomainState {
    pub fn advance(&self, events: Box<Vec<Event>>) {
        for event in events.iter() {
            let state = *self.state.lock();
            log::info!("tmux: {:?} in state {:?}", event, state);
            match event {
                Event::Guarded(response) => match state {
                    State::WaitForInitialGuard => {
                        *self.state.lock() = State::Idle;
                    }
                    State::WaitingForResponse => {
                        let mut cmd_queue = self.cmd_queue.as_ref().lock();
                        let cmd = cmd_queue.pop_front().unwrap();
                        let domain_id = self.domain_id;
                        *self.state.lock() = State::Idle;
                        let resp = response.clone();
                        promise::spawn::spawn_into_main_thread(async move {
                            if let Err(err) = cmd.process_result(domain_id, &resp) {
                                log::error!("Tmux processing command result error: {}", err);
                            }
                        })
                        .detach();
                    }
                    State::Idle => {}
                },
                Event::Output { pane, text } => {
                    let pane_map = self.remote_panes.lock();
                    if let Some(ref_pane) = pane_map.get(pane) {
                        let mut tmux_pane = ref_pane.lock();
                        if let Err(err) = tmux_pane.output_write.write_all(text.as_bytes()) {
                            log::error!("Failed to write tmux data to output: {:#}", err);
                        }
                    } else {
                        log::error!("Tmux pane {} havn't been attached", pane);
                    }
                }
                Event::WindowAdd { window: _ } => {
                    self.create_gui_window();
                }
                Event::SessionChanged { session, name: _ } => {
                    *self.tmux_session.lock() = Some(*session);
                    let mut cmd_queue = self.cmd_queue.as_ref().lock();
                    cmd_queue.push_back(Box::new(ListAllWindows {
                        session_id: *session,
                        window_id: None,
                    }));
                    log::info!("tmux session changed:{}", session);
                }
                Event::Exit { reason: _ } => {
                    let mut pane_map = self.remote_panes.lock();
                    for (_, v) in pane_map.iter_mut() {
                        let remote_pane = v.lock();
                        let (lock, condvar) = &*remote_pane.active_lock;
                        let mut released = lock.lock();
                        *released = true;
                        condvar.notify_all();
                    }
                }
                Event::SessionWindowChanged { session, window } => {
                    let mut cmd_queue = self.cmd_queue.as_ref().lock();
                    cmd_queue.push_back(Box::new(ListAllWindows {
                        session_id: *session,
                        window_id: Some(*window),
                    }));
                    log::info!("tmux window changed: {}:{}", session, window);
                }
                Event::WindowPaneChanged { window, pane } => {
                    if !self.check_pane_attached(*window, *pane) {
                        // new split pane
                        smol::block_on(async {
                            let _ = self.channel.0.send(*pane).await;
                        });
                    }
                    log::info!("tmux window pane changed: {}:{}", window, pane);
                }
                _ => {}
            }
        }

        // send pending commands to tmux
        let cmd_queue = self.cmd_queue.as_ref().lock();
        if *self.state.lock() == State::Idle && !cmd_queue.is_empty() {
            TmuxDomainState::schedule_send_next_command(self.domain_id);
        }
    }

    /// send next command at the front of cmd_queue.
    /// must be called inside main thread
    fn send_next_command(&self) {
        if *self.state.lock() != State::Idle {
            return;
        }
        let cmd_queue = self.cmd_queue.as_ref().lock();
        if let Some(first) = cmd_queue.front() {
            let cmd = first.get_command();
            log::info!("sending cmd {:?}", cmd);
            let mux = Mux::get();
            if let Some(pane) = mux.get_pane(self.pane_id) {
                let mut writer = pane.writer();
                let _ = write!(writer, "{}", cmd);
            }
            *self.state.lock() = State::WaitingForResponse;
        }
    }

    /// schedule a `send_next_command` into main thread
    pub fn schedule_send_next_command(domain_id: usize) {
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get();
            if let Some(domain) = mux.get_domain(domain_id) {
                if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                    tmux_domain.send_next_command();
                }
            }
        })
        .detach();
    }

    /// create a standalone window for tmux tabs
    pub fn create_gui_window(&self) {
        if self.gui_window.lock().is_none() {
            let mux = Mux::get();
            let window_builder = mux.new_empty_window(
                None, /* TODO: pass session here */
                None, /* position */
            );
            log::info!("Tmux create window id {}", window_builder.window_id);
            {
                let mut window_id = self.gui_window.lock();
                *window_id = Some(window_builder); // keep the builder so it won't be purged
            }
        };
    }

    /// create a tmux window
    pub fn create_tmux_window(&self, width: usize, height: usize) {
        let session_id = self.tmux_session.lock().unwrap();
        let mut cmd_queue = self.cmd_queue.as_ref().lock();
        cmd_queue.push_back(Box::new(NewWindow {
            session_id,
            width,
            height,
        }));
        TmuxDomainState::schedule_send_next_command(self.domain_id);
    }

    /// split the tmux pane
    pub fn split_tmux_pane(&self, _tab: TabId, pane_id: PaneId, split_request: SplitRequest) {
        let mut target_tmux_pane_id = None;

        let mut pane_map = self.remote_panes.lock();
        for (tmux_pane_id, v) in pane_map.iter_mut() {
            let remote_pane = v.lock();
            if remote_pane.local_pane_id == pane_id {
                target_tmux_pane_id = Some(tmux_pane_id);
                break;
            }
        }

        if let Some(id) = target_tmux_pane_id {
            let mut cmd_queue = self.cmd_queue.as_ref().lock();
            cmd_queue.push_back(Box::new(SplitPane {
                pane_id: *id,
                direction: split_request.direction,
            }));
            TmuxDomainState::schedule_send_next_command(self.domain_id);
        } else {
            log::debug!("Could not find the tmux pane peer for local pane: {pane_id}");
        }
    }
}

impl TmuxDomain {
    pub fn new(pane_id: PaneId) -> Self {
        let domain_id = alloc_domain_id();
        let cmd_queue = VecDeque::<Box<dyn TmuxCommand>>::new();
        let inner = Arc::new(TmuxDomainState {
            domain_id,
            pane_id,
            // parser,
            state: Mutex::new(State::WaitForInitialGuard),
            cmd_queue: Arc::new(Mutex::new(cmd_queue)),
            gui_window: Mutex::new(None),
            gui_tabs: Mutex::new(Vec::default()),
            remote_panes: Mutex::new(HashMap::default()),
            tmux_session: Mutex::new(None),
            channel: bounded(1),
        });

        Self { inner }
    }

    fn send_next_command(&self) {
        self.inner.send_next_command();
    }
}

#[async_trait(?Send)]
impl Domain for TmuxDomain {
    async fn spawn(
        &self,
        size: TerminalSize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _window: WindowId,
    ) -> anyhow::Result<Arc<Tab>> {
        self.inner.create_tmux_window(size.cols, size.rows);
        anyhow::bail!("Intention: we use tmux command to do so");
    }

    async fn split_pane(
        &self,
        _source: SplitSource,
        tab: TabId,
        pane_id: PaneId,
        split_request: SplitRequest,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        self.inner.split_tmux_pane(tab, pane_id, split_request);
        if let Ok(id) = self.inner.channel.1.recv().await {
            let pane = self.inner.split_pane(tab, pane_id, id, split_request);
            return pane;
        }

        anyhow::bail!("Split_pane failed");
    }

    async fn spawn_pane(
        &self,
        _size: TerminalSize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        anyhow::bail!("Spawn_pane not yet implemented for TmuxDomain");
    }

    fn domain_id(&self) -> DomainId {
        self.inner.domain_id
    }

    fn domain_name(&self) -> &str {
        "tmux"
    }

    async fn attach(&self, _window_id: Option<crate::WindowId>) -> anyhow::Result<()> {
        Ok(())
    }

    fn detachable(&self) -> bool {
        false
    }

    fn detach(&self) -> anyhow::Result<()> {
        anyhow::bail!("detach not implemented for TmuxDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
