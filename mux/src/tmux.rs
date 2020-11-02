use crate::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::pane::{Pane, PaneId};
use crate::renderable::*;
use crate::tab::{SplitDirection, Tab, TabId};
use crate::window::WindowId;
use async_trait::async_trait;
use portable_pty::{CommandBuilder, PtySize};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::surface::Line;
use wezterm_term::{CellAttributes, StableRowIndex};

pub(crate) struct TmuxPlaceholderRenderable {}

impl Renderable for TmuxPlaceholderRenderable {
    fn get_cursor_position(&self) -> StableCursorPosition {
        StableCursorPosition {
            x: 0,
            y: 0,
            shape: Default::default(),
            visibility: termwiz::surface::CursorVisibility::Hidden,
        }
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        let mut dirty = RangeSet::new();
        for i in lines {
            dirty.add(i);
        }
        dirty
    }

    fn get_lines(&mut self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let line = Line::from_text(
            "This pane is running tmux control mode",
            &CellAttributes::default(),
        );
        (0, vec![line])
    }

    /// Returns render related dimensions
    fn get_dimensions(&self) -> RenderableDimensions {
        RenderableDimensions {
            cols: 32,
            viewport_rows: 1,
            scrollback_rows: 0,
            physical_top: 0,
            scrollback_top: 0,
        }
    }
}

pub(crate) struct TmuxDomainState {
    pane_id: PaneId,
    pub domain_id: DomainId,
    parser: RefCell<tmux_cc::Parser>,
    pub renderable: RefCell<TmuxPlaceholderRenderable>,
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
        let renderable = RefCell::new(TmuxPlaceholderRenderable {});
        let inner = Arc::new(TmuxDomainState {
            domain_id,
            pane_id,
            parser,
            renderable,
        });
        Self { inner }
    }
}

#[async_trait(?Send)]
impl Domain for TmuxDomain {
    async fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        window: WindowId,
    ) -> anyhow::Result<Rc<Tab>> {
        anyhow::bail!("Spawn not yet implemented for TmuxDomain");
    }

    async fn split_pane(
        &self,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        tab: TabId,
        pane_id: PaneId,
        direction: SplitDirection,
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
