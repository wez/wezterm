use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitDirection, Tab, TabId};
use crate::window::WindowId;
use async_trait::async_trait;
use portable_pty::{CommandBuilder, PtySize};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub(crate) struct TmuxDomainState {
    pane_id: PaneId,
    pub domain_id: DomainId,
    parser: RefCell<tmux_cc::Parser>,
}

pub struct TmuxDomain {
    pub(crate) inner: Arc<TmuxDomainState>,
}

impl TmuxDomainState {
    pub fn advance(&self, b: u8) {
        let mut parser = self.parser.borrow_mut();
        if let Some(event) = parser.advance_byte(b) {
            log::error!("tmux: {:?}", event);
        }
    }
}

impl TmuxDomain {
    pub fn new(pane_id: PaneId) -> Self {
        let domain_id = alloc_domain_id();
        let parser = RefCell::new(tmux_cc::Parser::new());
        let inner = Arc::new(TmuxDomainState {
            domain_id,
            pane_id,
            parser,
        });
        Self { inner }
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
