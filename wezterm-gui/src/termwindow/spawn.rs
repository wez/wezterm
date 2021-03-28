use crate::termwindow::{ClipboardHelper, MuxWindowId};
use anyhow::{anyhow, bail};
use config::keyassignment::{SpawnCommand, SpawnTabDomain};
use mux::activity::Activity;
use mux::domain::DomainState;
use mux::tab::SplitDirection;
use mux::Mux;
use portable_pty::{CommandBuilder, PtySize};
use std::sync::Arc;

#[derive(Copy, Debug, Clone, Eq, PartialEq)]
pub enum SpawnWhere {
    NewWindow,
    NewTab,
    SplitPane(SplitDirection),
}

impl super::TermWindow {
    pub fn spawn_command(&mut self, spawn: &SpawnCommand, spawn_where: SpawnWhere) {
        let size = if spawn_where == SpawnWhere::NewWindow {
            self.config.initial_size()
        } else {
            self.terminal_size
        };
        Self::spawn_command_impl(
            spawn,
            spawn_where,
            size,
            self.mux_window_id,
            ClipboardHelper {
                window: self.window.as_ref().unwrap().clone(),
                clipboard_contents: Arc::clone(&self.clipboard_contents),
            },
        )
    }

    pub fn spawn_command_impl(
        spawn: &SpawnCommand,
        spawn_where: SpawnWhere,
        size: PtySize,
        src_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
    ) {
        let spawn = spawn.clone();

        promise::spawn::spawn(async move {
            if let Err(err) =
                Self::spawn_command_internal(spawn, spawn_where, size, src_window_id, clipboard)
                    .await
            {
                log::error!("Failed to spawn: {:#}", err);
            }
        })
        .detach();
    }

    async fn spawn_command_internal(
        spawn: SpawnCommand,
        spawn_where: SpawnWhere,
        size: PtySize,
        src_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
    ) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let activity = Activity::new();
        let mux_builder;

        let target_window_id = if spawn_where == SpawnWhere::NewWindow {
            mux_builder = mux.new_empty_window();
            *mux_builder
        } else {
            src_window_id
        };

        let (domain, cwd) = match spawn.domain {
            SpawnTabDomain::DefaultDomain => {
                let cwd = mux
                    .get_active_tab_for_window(src_window_id)
                    .and_then(|tab| tab.get_active_pane())
                    .and_then(|pane| pane.get_current_working_dir());
                (mux.default_domain().clone(), cwd)
            }
            SpawnTabDomain::CurrentPaneDomain => {
                if spawn_where == SpawnWhere::NewWindow {
                    // CurrentPaneDomain is the default value for the spawn domain.
                    // It doesn't make sense to use it when spawning a new window,
                    // so we treat it as DefaultDomain instead.
                    let cwd = mux
                        .get_active_tab_for_window(src_window_id)
                        .and_then(|tab| tab.get_active_pane())
                        .and_then(|pane| pane.get_current_working_dir());
                    (mux.default_domain().clone(), cwd)
                } else {
                    let tab = match mux.get_active_tab_for_window(src_window_id) {
                        Some(tab) => tab,
                        None => bail!("window has no tabs?"),
                    };
                    let pane = tab
                        .get_active_pane()
                        .ok_or_else(|| anyhow!("current tab has no pane!?"))?;
                    (
                        mux.get_domain(pane.domain_id())
                            .ok_or_else(|| anyhow!("current tab has unresolvable domain id!?"))?,
                        pane.get_current_working_dir(),
                    )
                }
            }
            SpawnTabDomain::DomainName(name) => (
                mux.get_domain_by_name(&name).ok_or_else(|| {
                    anyhow!("spawn_tab called with unresolvable domain name {}", name)
                })?,
                None,
            ),
        };

        if domain.state() == DomainState::Detached {
            bail!("Cannot spawn a tab into a Detached domain");
        }

        let cwd = if let Some(cwd) = spawn.cwd.as_ref() {
            Some(cwd.to_str().map(|s| s.to_owned()).ok_or_else(|| {
                anyhow!(
                    "Domain::spawn requires that the cwd be unicode in {:?}",
                    cwd
                )
            })?)
        } else {
            match cwd {
                Some(url) if url.scheme() == "file" => {
                    let path = url.path().to_string();
                    // On Windows the file URI can produce a path like:
                    // `/C:\Users` which is valid in a file URI, but the leading slash
                    // is not liked by the windows file APIs, so we strip it off here.
                    let bytes = path.as_bytes();
                    if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' {
                        Some(path[1..].to_owned())
                    } else {
                        Some(path)
                    }
                }
                Some(_) | None => None,
            }
        };

        let cmd_builder = if let Some(args) = spawn.args {
            let mut builder = CommandBuilder::from_argv(args.iter().map(Into::into).collect());
            for (k, v) in spawn.set_environment_variables.iter() {
                builder.env(k, v);
            }
            if let Some(cwd) = spawn.cwd {
                builder.cwd(cwd);
            }
            Some(builder)
        } else {
            None
        };

        match spawn_where {
            SpawnWhere::SplitPane(direction) => {
                let mux = Mux::get().unwrap();
                if let Some(tab) = mux.get_active_tab_for_window(target_window_id) {
                    let pane = tab
                        .get_active_pane()
                        .ok_or_else(|| anyhow!("tab to have a pane"))?;

                    log::trace!("doing split_pane");
                    domain
                        .split_pane(cmd_builder, cwd, tab.tab_id(), pane.pane_id(), direction)
                        .await?;
                } else {
                    log::error!("there is no active tab while splitting pane!?");
                }
            }
            _ => {
                let tab = domain
                    .spawn(size, cmd_builder, cwd, target_window_id)
                    .await?;
                let tab_id = tab.tab_id();
                let pane = tab
                    .get_active_pane()
                    .ok_or_else(|| anyhow!("newly spawned tab to have a pane"))?;

                if spawn_where != SpawnWhere::NewWindow {
                    let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(clipboard);
                    pane.set_clipboard(&clipboard);
                    let mut window = mux
                        .get_window_mut(target_window_id)
                        .ok_or_else(|| anyhow!("no such window!?"))?;
                    if let Some(idx) = window.idx_by_id(tab_id) {
                        window.set_active(idx);
                    }
                }
            }
        };

        drop(activity);

        Ok(())
    }

    pub fn spawn_tab(&mut self, domain: &SpawnTabDomain) {
        self.spawn_command(
            &SpawnCommand {
                domain: domain.clone(),
                ..Default::default()
            },
            SpawnWhere::NewTab,
        );
    }
}
