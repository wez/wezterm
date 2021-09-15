use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitDirection, Tab, TabId};
use crate::tmux_commands::{ListAllPanes, TmuxCommand};
use crate::window::WindowId;
use crate::{Mux, MuxWindowBuilder};
use async_trait::async_trait;
use flume;
use portable_pty::{CommandBuilder, PtySize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use tmux_cc::*;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum State {
    WaitForInitialGuard,
    Idle,
    WaitingForResponse,
}

#[derive(Debug)]
pub(crate) struct TmuxRemotePane {
    // members for local
    pub local_pane_id: PaneId,
    pub tx: flume::Sender<String>,
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

pub(crate) struct TmuxTab {
    pub tab_id: TabId, // local tab ID
    pub tmux_window_id: TmuxWindowId,
    pub panes: HashSet<TmuxPaneId>, // tmux panes within tmux window
}

pub(crate) type TmuxCmdQueue = VecDeque<Box<dyn TmuxCommand>>;
pub(crate) struct TmuxDomainState {
    pub pane_id: PaneId,
    pub domain_id: DomainId,
    // parser: RefCell<Parser>,
    state: RefCell<State>,
    pub cmd_queue: Arc<Mutex<TmuxCmdQueue>>,
    pub gui_window: RefCell<Option<MuxWindowBuilder>>,
    pub gui_tabs: RefCell<Vec<TmuxTab>>,
    pub remote_panes: RefCell<HashMap<TmuxPaneId, RefTmuxRemotePane>>,
    pub tmux_session: RefCell<Option<TmuxSessionId>>,
}

pub struct TmuxDomain {
    pub(crate) inner: Arc<TmuxDomainState>,
}

impl TmuxDomainState {
    pub fn advance(&self, events: Box<Vec<Event>>) {
        for event in events.iter() {
            let state = *self.state.borrow();
            log::info!("tmux: {:?} in state {:?}", event, state);
            match event {
                Event::Guarded(response) => match state {
                    State::WaitForInitialGuard => {
                        *self.state.borrow_mut() = State::Idle;
                    }
                    State::WaitingForResponse => {
                        let mut cmd_queue = self.cmd_queue.as_ref().lock().unwrap();
                        let cmd = cmd_queue.pop_front().unwrap();
                        let domain_id = self.domain_id;
                        *self.state.borrow_mut() = State::Idle;
                        let resp = response.clone();
                        promise::spawn::spawn(async move {
                            if let Err(err) = cmd.process_result(domain_id, &resp) {
                                log::error!("Tmux processing command result error: {}", err);
                            }
                        })
                        .detach();
                    }
                    State::Idle => {}
                },
                Event::Output { pane, text } => {
                    let pane_map = self.remote_panes.borrow_mut();
                    if let Some(ref_pane) = pane_map.get(pane) {
                        let tmux_pane = ref_pane.lock().unwrap();
                        tmux_pane
                            .tx
                            .send(text.to_string())
                            .expect("send to tmux pane failed");
                    } else {
                        log::error!("Tmux pane {} havn't been attached", pane);
                    }
                }
                Event::WindowAdd { window: _ } => {
                    self.create_gui_window();
                }
                Event::SessionChanged { session, name: _ } => {
                    *self.tmux_session.borrow_mut() = Some(*session);
                    log::info!("tmux session changed:{}", session);
                }
                Event::Exit { reason: _ } => {
                    let mut pane_map = self.remote_panes.borrow_mut();
                    for (_, v) in pane_map.iter_mut() {
                        let remote_pane = v.lock().unwrap();
                        let (lock, condvar) = &*remote_pane.active_lock;
                        let mut released = lock.lock().unwrap();
                        *released = true;
                        condvar.notify_all();
                    }
                }
                _ => {}
            }
        }

        // send pending commands to tmux
        let cmd_queue = self.cmd_queue.as_ref().lock().unwrap();
        if *self.state.borrow() == State::Idle && !cmd_queue.is_empty() {
            TmuxDomainState::kick_off(self.domain_id);
        }
    }

    fn send_next_command(&self) {
        if *self.state.borrow() != State::Idle {
            return;
        }
        let cmd_queue = self.cmd_queue.as_ref().lock().unwrap();
        if let Some(first) = cmd_queue.front() {
            let cmd = first.get_command();
            log::error!("sending cmd {:?}", cmd);
            let mux = Mux::get().expect("to be called on main thread");
            if let Some(pane) = mux.get_pane(self.pane_id) {
                let mut writer = pane.writer();
                let _ = write!(writer, "{}", cmd);
            }
            *self.state.borrow_mut() = State::WaitingForResponse;
        }
    }

    pub fn kick_off(domain_id: usize) {
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().expect("to be called on main thread");
            if let Some(domain) = mux.get_domain(domain_id) {
                if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                    tmux_domain.send_next_command();
                }
            }
        })
        .detach();
    }

    pub fn create_gui_window(&self) {
        if self.gui_window.borrow().is_none() {
            let mux = Mux::get().expect("should be call at main thread");
            let window_builder = mux.new_empty_window();
            log::info!("Tmux create window id {}", window_builder.window_id);
            {
                let mut window_id = self.gui_window.borrow_mut();
                *window_id = Some(window_builder); // keep the builder so it won't be purged
            }
        };
    }
}

impl TmuxDomain {
    pub fn new(pane_id: PaneId) -> Self {
        let domain_id = alloc_domain_id();
        // let parser = RefCell::new(Parser::new());
        let mut cmd_queue = VecDeque::<Box<dyn TmuxCommand>>::new();
        cmd_queue.push_back(Box::new(ListAllPanes));
        let inner = Arc::new(TmuxDomainState {
            domain_id,
            pane_id,
            // parser,
            state: RefCell::new(State::WaitForInitialGuard),
            cmd_queue: Arc::new(Mutex::new(cmd_queue)),
            gui_window: RefCell::new(None),
            gui_tabs: RefCell::new(Vec::default()),
            remote_panes: RefCell::new(HashMap::default()),
            tmux_session: RefCell::new(None),
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
        _size: PtySize,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _window: WindowId,
    ) -> anyhow::Result<Rc<Tab>> {
        anyhow::bail!("Spawn not yet implemented for TmuxDomain");
    }

    async fn split_pane(
        &self,
        _command: Option<CommandBuilder>,
        _command_dir: Option<String>,
        _tab: TabId,
        _pane_id: PaneId,
        _direction: SplitDirection,
    ) -> anyhow::Result<Rc<dyn Pane>> {
        anyhow::bail!("split_pane not yet implemented for TmuxDomain");
    }

    fn domain_id(&self) -> DomainId {
        self.inner.domain_id
    }

    fn domain_name(&self) -> &str {
        "tmux"
    }

    async fn attach(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        anyhow::bail!("detach not implemented for TmuxDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
