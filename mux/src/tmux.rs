use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitDirection, Tab, TabId};
use crate::tmux_commands::{ListAllPanes, TmuxCommand};
use crate::window::WindowId;
use crate::Mux;
use async_trait::async_trait;
use flume;
use portable_pty::{CommandBuilder, PtySize};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tmux_cc::*;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum State {
    WaitForInitialGuard,
    Idle,
    WaitingForResponse,
}

pub(crate) struct TmuxRemotePane {
    // members for local
    local_pane_id: PaneId,
    tx: flume::Sender<String>,
    // members sync with remote
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    pane_id: TmuxPaneId,
    pub cursor_x: u64,
    pub cursor_y: u64,
    pub pane_width: u64,
    pub pane_height: u64,
    pub pane_left: u64,
    pub pane_top: u64,
}

pub(crate) type RefTmuxRemotePane = Arc<Mutex<TmuxRemotePane>>;

pub(crate) struct TmuxTab {
    tab_id: TabId,
    tmux_window_id: TmuxWindowId,
    panes: Vec<TmuxPaneId>,
}

pub(crate) struct TmuxDomainState {
    pane_id: PaneId,
    pub domain_id: DomainId,
    // parser: RefCell<Parser>,
    state: RefCell<State>,
    cmd_queue: RefCell<VecDeque<Box<dyn TmuxCommand>>>,
    gui_window_id: RefCell<Option<usize>>,
    gui_tabs: RefCell<Vec<TmuxTab>>,
    remote_panes: RefCell<HashMap<TmuxPaneId, RefTmuxRemotePane>>,
}

pub struct TmuxDomain {
    pub(crate) inner: Arc<TmuxDomainState>,
}

impl TmuxDomainState {
    pub fn advance(&self, events: Box<Vec<Event>>) {
        for event in events.iter() {
            let state = *self.state.borrow();
            log::error!("tmux: {:?} in state {:?}", event, state);
            match event {
                Event::Guarded(response) => match state {
                    State::WaitForInitialGuard => {
                        *self.state.borrow_mut() = State::Idle;
                    }
                    State::WaitingForResponse => {
                        let cmd = self.cmd_queue.borrow_mut().pop_front().unwrap();
                        let domain_id = self.domain_id;
                        *self.state.borrow_mut() = State::Idle;
                        let resp = response.clone();
                        promise::spawn::spawn(async move {
                            if let Err(err) = cmd.process_result(domain_id, &resp) {
                                log::error!("error processing result: {}", err);
                            }
                        })
                        .detach();
                    }
                    State::Idle => {}
                },
                Event::Output { pane, text } => {
                    let pane_map = self.remote_panes.borrow_mut();
                    if let Some(ref_pane) = pane_map.get(pane) {
                        // TODO: handle escape?
                        let tmux_pane = ref_pane.lock().unwrap();
                        tmux_pane
                            .tx
                            .send(text.to_string())
                            .expect("send to tmux pane failed");
                    }
                }
                Event::WindowAdd { window: _ } => {
                    if self.gui_window_id.borrow().is_none() {
                        if let Some(mux) = Mux::get() {
                            let window_builder = mux.new_empty_window();
                            log::info!("Tmux create window id {}", window_builder.window_id);
                            {
                                let mut window_id = self.gui_window_id.borrow_mut();
                                *window_id = Some(window_builder.window_id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // send pending commands to tmux
        if *self.state.borrow() == State::Idle && !self.cmd_queue.borrow().is_empty() {
            let domain_id = self.domain_id;
            promise::spawn::spawn(async move {
                let mux = Mux::get().expect("to be called on main thread");
                if let Some(domain) = mux.get_domain(domain_id) {
                    if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                        tmux_domain.send_next_command();
                    }
                }
            })
            .detach();
        }
    }

    fn send_next_command(&self) {
        if *self.state.borrow() != State::Idle {
            return;
        }
        if let Some(first) = self.cmd_queue.borrow().front() {
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
            cmd_queue: RefCell::new(cmd_queue),
            gui_window_id: RefCell::new(None),
            gui_tabs: RefCell::new(Vec::default()),
            remote_panes: RefCell::new(HashMap::default()),
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
