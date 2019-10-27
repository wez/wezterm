//! A Domain represents an instance of a multiplexer.
//! For example, the gui frontend has its own domain,
//! and we can connect to a domain hosted by a mux server
//! that may be local, running "remotely" inside a WSL
//! container or actually remote, running on the other end
//! of an ssh session somewhere.

use crate::config::Config;
use crate::frontend::guicommon::localtab::LocalTab;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use downcast_rs::{impl_downcast, Downcast};
use failure::{Error, Fallible};
use log::info;
use portable_pty::cmdbuilder::CommandBuilder;
use portable_pty::{PtySize, PtySystem};
use std::rc::Rc;
use std::sync::Arc;

static DOMAIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type DomainId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainState {
    Detached,
    Attached,
}

pub fn alloc_domain_id() -> DomainId {
    DOMAIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

pub trait Domain: Downcast {
    /// Spawn a new command within this domain
    fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        window: WindowId,
    ) -> Result<Rc<dyn Tab>, Error>;

    /// Returns the domain id, which is useful for obtaining
    /// a handle on the domain later.
    fn domain_id(&self) -> DomainId;

    /// Returns the name of the domain
    fn domain_name(&self) -> &str;

    /// Re-attach to any tabs that might be pre-existing in this domain
    fn attach(&self) -> Fallible<()>;

    /// Detach all tabs
    fn detach(&self) -> Fallible<()>;

    /// Indicates the state of the domain
    fn state(&self) -> DomainState;
}
impl_downcast!(Domain);

pub struct LocalDomain {
    pty_system: Box<dyn PtySystem>,
    config: Arc<Config>,
    id: DomainId,
    name: String,
}

impl LocalDomain {
    pub fn new(name: &str, config: &Arc<Config>) -> Result<Self, Error> {
        let pty_system = config.pty.get()?;
        Ok(Self::with_pty_system(name, config, pty_system))
    }

    pub fn with_pty_system(
        name: &str,
        config: &Arc<Config>,
        pty_system: Box<dyn PtySystem>,
    ) -> Self {
        let config = Arc::clone(config);
        let id = alloc_domain_id();
        Self {
            pty_system,
            config,
            id,
            name: name.to_string(),
        }
    }
}

impl Domain for LocalDomain {
    fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        window: WindowId,
    ) -> Result<Rc<dyn Tab>, Error> {
        let cmd = match command {
            Some(c) => c,
            None => self.config.build_prog(None)?,
        };
        let pair = self.pty_system.openpty(size)?;
        let child = pair.slave.spawn_command(cmd)?;
        info!("spawned: {:?}", child);

        let mut terminal = term::Terminal::new(
            size.rows as usize,
            size.cols as usize,
            size.pixel_width as usize,
            size.pixel_height as usize,
            self.config.scrollback_lines.unwrap_or(3500),
            self.config.hyperlink_rules.clone(),
        );

        let mux = Mux::get().unwrap();

        if let Some(palette) = mux.config().colors.as_ref() {
            *terminal.palette_mut() = palette.clone().into();
        }

        let tab: Rc<dyn Tab> = Rc::new(LocalTab::new(terminal, child, pair.master, self.id));

        mux.add_tab(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    fn domain_id(&self) -> DomainId {
        self.id
    }

    fn domain_name(&self) -> &str {
        &self.name
    }

    fn attach(&self) -> Fallible<()> {
        Ok(())
    }

    fn detach(&self) -> Fallible<()> {
        failure::bail!("detach not implemented");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
