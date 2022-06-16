//! A Domain represents an instance of a multiplexer.
//! For example, the gui frontend has its own domain,
//! and we can connect to a domain hosted by a mux server
//! that may be local, running "remotely" inside a WSL
//! container or actually remote, running on the other end
//! of an ssh session somewhere.

use crate::localpane::LocalPane;
use crate::pane::{alloc_pane_id, Pane, PaneId};
use crate::tab::{SplitRequest, Tab, TabId};
use crate::window::WindowId;
use crate::Mux;
use anyhow::{bail, Error};
use async_trait::async_trait;
use config::{configuration, WslDomain};
use downcast_rs::{impl_downcast, Downcast};
use portable_pty::{native_pty_system, CommandBuilder, PtySystem};
use std::ffi::OsString;
use std::rc::Rc;
use wezterm_term::TerminalSize;

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

#[derive(Debug, Clone, PartialEq)]
pub enum SplitSource {
    Spawn {
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    },
    MovePane(PaneId),
}

#[async_trait(?Send)]
pub trait Domain: Downcast {
    /// Spawn a new command within this domain
    async fn spawn(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        window: WindowId,
    ) -> anyhow::Result<Rc<Tab>> {
        let pane = self.spawn_pane(size, command, command_dir).await?;

        let tab = Rc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        let mux = Mux::get().unwrap();
        mux.add_tab_and_active_pane(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    async fn split_pane(
        &self,
        source: SplitSource,
        tab: TabId,
        pane_id: PaneId,
        split_request: SplitRequest,
    ) -> anyhow::Result<Rc<dyn Pane>> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_tab(tab) {
            Some(t) => t,
            None => anyhow::bail!("Invalid tab id {}", tab),
        };

        let pane_index = match tab
            .iter_panes()
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

        let pane = match source {
            SplitSource::Spawn {
                command,
                command_dir,
            } => {
                self.spawn_pane(split_size.second, command, command_dir)
                    .await?
            }
            SplitSource::MovePane(src_pane_id) => {
                let (_domain, _window, src_tab) = mux
                    .resolve_pane_id(src_pane_id)
                    .ok_or_else(|| anyhow::anyhow!("pane {} not found", src_pane_id))?;
                let src_tab = match mux.get_tab(src_tab) {
                    Some(t) => t,
                    None => anyhow::bail!("Invalid tab id {}", src_tab),
                };

                let pane = src_tab.remove_pane(src_pane_id).ok_or_else(|| {
                    anyhow::anyhow!("pane {} not found in its containing tab!?", src_pane_id)
                })?;

                if src_tab.is_dead() {
                    mux.remove_tab(src_tab.tab_id());
                }

                pane
            }
        };

        tab.split_and_insert(pane_index, split_request, Rc::clone(&pane))?;
        Ok(pane)
    }

    async fn spawn_pane(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    ) -> anyhow::Result<Rc<dyn Pane>>;

    /// Returns false if the `spawn` method will never succeed.
    /// There are some internal placeholder domains that are
    /// pre-created with local UI that we do not want to allow
    /// to show in the launcher/menu as launchable items.
    fn spawnable(&self) -> bool {
        true
    }

    /// Returns the domain id, which is useful for obtaining
    /// a handle on the domain later.
    fn domain_id(&self) -> DomainId;

    /// Returns the name of the domain.
    /// Should be a short identifier.
    fn domain_name(&self) -> &str;

    /// Returns a label describing the domain.
    fn domain_label(&self) -> &str {
        self.domain_name()
    }

    /// Re-attach to any tabs that might be pre-existing in this domain
    async fn attach(&self, window_id: Option<WindowId>) -> anyhow::Result<()>;

    /// Detach all tabs
    fn detach(&self) -> anyhow::Result<()>;

    /// Indicates the state of the domain
    fn state(&self) -> DomainState;

    /// Called to advise the domain that a local window is closing.
    /// This allows the domain the opportunity to eg: detach/hide
    /// its tabs/panes rather than actually killing them off
    fn local_window_is_closing(&self, _window_id: WindowId) {}
}
impl_downcast!(Domain);

pub struct LocalDomain {
    pty_system: Box<dyn PtySystem>,
    id: DomainId,
    name: String,
    wsl: Option<WslDomain>,
}

impl LocalDomain {
    pub fn new(name: &str) -> Result<Self, Error> {
        Ok(Self::with_pty_system(name, native_pty_system()))
    }

    pub fn with_pty_system(name: &str, pty_system: Box<dyn PtySystem>) -> Self {
        let id = alloc_domain_id();
        Self {
            pty_system,
            id,
            name: name.to_string(),
            wsl: None,
        }
    }

    pub fn new_wsl(wsl: WslDomain) -> Result<Self, Error> {
        let mut dom = Self::new(&wsl.name)?;
        dom.wsl.replace(wsl);
        Ok(dom)
    }

    #[cfg(unix)]
    fn is_conpty(&self) -> bool {
        false
    }

    #[cfg(windows)]
    fn is_conpty(&self) -> bool {
        self.pty_system
            .downcast_ref::<portable_pty::win::conpty::ConPtySystem>()
            .is_some()
    }

    fn fixup_command(&self, cmd: &mut CommandBuilder) {
        if let Some(wsl) = &self.wsl {
            let mut args: Vec<OsString> = cmd.get_argv().clone();

            if args.is_empty() {
                if let Some(def_prog) = &wsl.default_prog {
                    for arg in def_prog {
                        args.push(arg.into());
                    }
                }
            }

            let mut argv: Vec<OsString> = vec![
                "wsl.exe".into(),
                "--distribution".into(),
                wsl.distribution
                    .as_deref()
                    .unwrap_or(wsl.name.as_str())
                    .into(),
            ];

            if let Some(cwd) = cmd.get_cwd() {
                argv.push("--cd".into());
                argv.push(cwd.into());
            }

            if let Some(user) = &wsl.username {
                argv.push("--user".into());
                argv.push(user.into());
            }

            if !args.is_empty() {
                argv.push("--exec".into());
                for arg in args {
                    argv.push(arg);
                }
            }

            // TODO: process env list and update WLSENV so that they
            // get passed through

            cmd.clear_cwd();
            *cmd.get_argv_mut() = argv;
        } else if let Some(dir) = cmd.get_cwd() {
            // I'm not normally a fan of existence checking, but not checking here
            // can be painful; in the case where a tab is local but has connected
            // to a remote system and that remote has used OSC 7 to set a path
            // that doesn't exist on the local system, process spawning can fail.
            // Another situation is `sudo -i` has the pane with set to a cwd
            // that is not accessible to the user.
            if let Err(err) = std::path::Path::new(&dir).read_dir() {
                log::warn!(
                    "Directory {:?} is not readable and will not be \
                     used for the command we are spawning: {:#}",
                    dir,
                    err
                );
                cmd.clear_cwd();
            }
        }
    }

    fn build_command(
        &self,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    ) -> anyhow::Result<CommandBuilder> {
        let config = configuration();
        let mut cmd = match command {
            Some(mut cmd) => {
                config.apply_cmd_defaults(&mut cmd, config.default_cwd.as_ref());
                cmd
            }
            None => config.build_prog(
                None,
                self.wsl
                    .as_ref()
                    .map(|wsl| wsl.default_prog.as_ref())
                    .unwrap_or(config.default_prog.as_ref()),
                self.wsl
                    .as_ref()
                    .map(|wsl| wsl.default_cwd.as_ref())
                    .unwrap_or(config.default_cwd.as_ref()),
            )?,
        };
        if let Some(dir) = command_dir {
            cmd.cwd(dir);
        }
        self.fixup_command(&mut cmd);
        Ok(cmd)
    }
}

#[async_trait(?Send)]
impl Domain for LocalDomain {
    async fn spawn_pane(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    ) -> anyhow::Result<Rc<dyn Pane>> {
        let mut cmd = self.build_command(command, command_dir)?;
        let pair = self
            .pty_system
            .openpty(crate::terminal_size_to_pty_size(size)?)?;
        let pane_id = alloc_pane_id();
        cmd.env("WEZTERM_PANE", pane_id.to_string());

        let command_line = cmd
            .as_unix_command_line()
            .unwrap_or_else(|err| format!("error rendering command line: {:?}", err));
        let command_description = format!(
            "\"{}\" in domain \"{}\"",
            if command_line.is_empty() {
                cmd.get_shell()?
            } else {
                command_line
            },
            self.name
        );
        let child = pair.slave.spawn_command(cmd)?;
        log::trace!("spawned: {:?}", child);

        let writer = pair.master.try_clone_writer()?;

        let mut terminal = wezterm_term::Terminal::new(
            size,
            std::sync::Arc::new(config::TermConfig::new()),
            "WezTerm",
            config::wezterm_version(),
            Box::new(writer),
        );
        if self.is_conpty() {
            terminal.enable_conpty_quirks();
        }

        let pane: Rc<dyn Pane> = Rc::new(LocalPane::new(
            pane_id,
            terminal,
            child,
            pair.master,
            self.id,
            command_description,
        ));

        let mux = Mux::get().unwrap();
        mux.add_pane(&pane)?;

        Ok(pane)
    }

    fn domain_id(&self) -> DomainId {
        self.id
    }

    fn domain_name(&self) -> &str {
        &self.name
    }

    async fn attach(&self, _window_id: Option<WindowId>) -> anyhow::Result<()> {
        Ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented for LocalDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
