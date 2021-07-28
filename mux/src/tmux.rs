use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitDirection, Tab, TabId};
use crate::window::WindowId;
use crate::Mux;
use anyhow::anyhow;
use async_trait::async_trait;
use portable_pty::{CommandBuilder, PtySize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use tmux_cc::*;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum State {
    WaitForInitialGuard,
    Idle,
    WaitingForResponse,
}

trait TmuxCommand {
    fn get_command(&self) -> String;
    fn process_result(&self, domain_id: DomainId, result: &Guarded, state: &mut TmuxInnerState) -> anyhow::Result<()>;
}

#[derive(Debug, PartialEq, Eq)]
struct TmuxPane {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    pane_id: TmuxPaneId,
    pane_index: u64,
    cursor_x: u64,
    cursor_y: u64,
    pane_width: u64,
    pane_height: u64,
    pane_left: u64,
    pane_top: u64,
}

struct ListAllPanes;
impl TmuxCommand for ListAllPanes {
    fn get_command(&self) -> String {
        "list-panes -aF '#{session_id} #{window_id} #{pane_id} \
            #{pane_index} #{cursor_x} #{cursor_y} #{pane_width} #{pane_height} \
            #{pane_left} #{pane_top}'\n"
            .to_owned()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded, state: &mut TmuxInnerState) -> anyhow::Result<()> {

        state.panes.clear();

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            let session_id = fields.next().ok_or_else(|| anyhow!("missing session_id"))?;
            let window_id = fields.next().ok_or_else(|| anyhow!("missing window_id"))?;
            let pane_id = fields.next().ok_or_else(|| anyhow!("missing pane_id"))?;
            let pane_index = fields
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

            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id = session_id[1..].parse()?;
            let window_id = window_id[1..].parse()?;
            let pane_id = pane_id[1..].parse()?;

            state.panes.push(TmuxPane {
                session_id,
                window_id,
                pane_id,
                pane_index,
                cursor_x,
                cursor_y,
                pane_width,
                pane_height,
                pane_left,
                pane_top,
            });
        }

        log::error!("panes in domain_id {}: {:?}", domain_id, state.panes);
        Ok(())
    }
}

struct TmuxInnerState {
    panes: Vec<TmuxPane>
}

pub(crate) struct TmuxDomainState {
    pane_id: PaneId,
    pub domain_id: DomainId,
    parser: RefCell<Parser>,
    state: RefCell<State>,
    inner: RefCell<TmuxInnerState>,
    cmd_queue: RefCell<VecDeque<Box<dyn TmuxCommand>>>,
}

pub struct TmuxDomain {
    pub(crate) inner: Arc<TmuxDomainState>,
}

impl TmuxDomainState {
    pub fn advance(&self, b: u8) {
        let mut parser = self.parser.borrow_mut();
        if let Some(event) = parser.advance_byte(b) {
            let state = *self.state.borrow();
            log::error!("tmux: {:?} in state {:?}", event, state);
            if let Event::Guarded(response) = event {
                match state {
                    State::WaitForInitialGuard => {
                        *self.state.borrow_mut() = State::Idle;
                    }
                    State::WaitingForResponse => {
                        let cmd = self.cmd_queue.borrow_mut().pop_front().unwrap();
                        let domain_id = self.domain_id;
                        *self.state.borrow_mut() = State::Idle;
                        if let Err(err) = cmd.process_result(domain_id, &response, &mut self.inner.borrow_mut()) {
                            log::error!("error processing result: {}", err);
                        }
                    }
                    State::Idle => {}
                }
            }
        }
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
        let parser = RefCell::new(Parser::new());
        let mut cmd_queue = VecDeque::<Box<dyn TmuxCommand>>::new();
        cmd_queue.push_back(Box::new(ListAllPanes));
        let inner = Arc::new(TmuxDomainState {
            domain_id,
            pane_id,
            parser,
            state: RefCell::new(State::WaitForInitialGuard),
            inner: RefCell::new(TmuxInnerState {
                panes: vec![]
            }),
            cmd_queue: RefCell::new(cmd_queue),
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


#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_list_panes() {
        let domain = TmuxDomain::new(0 as usize);
        let domain_state = domain.inner;
        let command = ListAllPanes;
        let result = Guarded {
            timestamp: 1604279270,
            number: 310,
            flags: 0,
            error: false,
            output: "
$0 @0 %0 0 2 37 104 38 0 0
$1 @1 %1 0 2 37 104 38 0 0
$2 @2 %2 0 2 2 80 24 0 0
            ".to_owned()
        };

        let tmux_inner_state = &mut domain_state.inner.borrow_mut();
        command.process_result(0 as usize, &result, tmux_inner_state);
        assert_eq!(
            vec![
                TmuxPane {
                    session_id: 0,
                    window_id: 0,
                    pane_id: 0,
                    pane_index: 0,
                    cursor_x: 2,
                    cursor_y: 37,
                    pane_width: 104,
                    pane_height: 38,
                    pane_left: 0,
                    pane_top: 0,
                },
                TmuxPane {
                    session_id: 1,
                    window_id: 1,
                    pane_id: 1,
                    pane_index: 0,
                    cursor_x: 2,
                    cursor_y: 37,
                    pane_width: 104,
                    pane_height: 38,
                    pane_left: 0,
                    pane_top: 0,
                },
                TmuxPane {
                    session_id: 2,
                    window_id: 2,
                    pane_id: 2,
                    pane_index: 0,
                    cursor_x: 2,
                    cursor_y: 2,
                    pane_width: 80,
                    pane_height: 24,
                    pane_left: 0,
                    pane_top: 0,
                }
            ],
            tmux_inner_state.panes
        )
    }
}