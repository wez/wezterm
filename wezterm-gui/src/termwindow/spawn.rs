use crate::termwindow::{ClipboardHelper, MuxWindowId};
use anyhow::{anyhow, bail, Context};
use config::keyassignment::{SpawnCommand, SpawnTabDomain};
use config::TermConfig;
use mux::activity::Activity;
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
    pub fn spawn_command(&self, spawn: &SpawnCommand, spawn_where: SpawnWhere) {
        let size = if spawn_where == SpawnWhere::NewWindow {
            self.config.initial_size()
        } else {
            self.terminal_size
        };
        let term_config = Arc::new(TermConfig::with_config(self.config.clone()));

        Self::spawn_command_impl(
            spawn,
            spawn_where,
            size,
            self.mux_window_id,
            ClipboardHelper {
                window: self.window.as_ref().unwrap().clone(),
            },
            term_config,
        )
    }

    fn spawn_command_impl(
        spawn: &SpawnCommand,
        spawn_where: SpawnWhere,
        size: PtySize,
        src_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
        term_config: Arc<TermConfig>,
    ) {
        let spawn = spawn.clone();

        promise::spawn::spawn(async move {
            if let Err(err) = Self::spawn_command_internal(
                spawn,
                spawn_where,
                size,
                src_window_id,
                clipboard,
                term_config,
            )
            .await
            {
                log::error!("Failed to spawn: {:#}", err);
            }
        })
        .detach();
    }

    pub async fn spawn_command_internal(
        spawn: SpawnCommand,
        spawn_where: SpawnWhere,
        size: PtySize,
        src_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
        term_config: Arc<TermConfig>,
    ) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let activity = Activity::new();

        let current_pane_id = if let Some(tab) = mux.get_active_tab_for_window(src_window_id) {
            tab.get_active_pane().map(|p| p.pane_id())
        } else {
            None
        };

        let cwd = if let Some(cwd) = spawn.cwd.as_ref() {
            Some(cwd.to_str().map(|s| s.to_owned()).ok_or_else(|| {
                anyhow!(
                    "Domain::spawn requires that the cwd be unicode in {:?}",
                    cwd
                )
            })?)
        } else {
            None
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

        let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(clipboard);
        let downloader: Arc<dyn wezterm_term::DownloadHandler> =
            Arc::new(crate::download::Downloader::new());
        let workspace = mux.active_workspace().clone();

        match spawn_where {
            SpawnWhere::SplitPane(direction) => {
                if let Some(tab) = mux.get_active_tab_for_window(src_window_id) {
                    let pane = tab
                        .get_active_pane()
                        .ok_or_else(|| anyhow!("tab to have a pane"))?;

                    log::trace!("doing split_pane");
                    let (pane, _size) = mux
                        .split_pane(
                            // tab.tab_id(),
                            pane.pane_id(),
                            direction,
                            cmd_builder,
                            cwd,
                            spawn.domain,
                        )
                        .await
                        .context("split_pane")?;
                    pane.set_config(term_config);
                    pane.set_clipboard(&clipboard);
                    pane.set_download_handler(&downloader);
                } else {
                    bail!("there is no active tab while splitting pane!?");
                }
            }
            _ => {
                let (_tab, pane, window_id) = mux
                    .spawn_tab_or_window(
                        match spawn_where {
                            SpawnWhere::NewWindow => None,
                            _ => Some(src_window_id),
                        },
                        spawn.domain,
                        cmd_builder,
                        cwd,
                        size,
                        current_pane_id,
                        workspace,
                    )
                    .await
                    .context("spawn_tab_or_window")?;

                // If it was created in this window, it copies our handlers.
                // Otherwise, we'll pick them up when we later respond to
                // the new window being created.
                if window_id == src_window_id {
                    pane.set_config(term_config);
                    pane.set_clipboard(&clipboard);
                    pane.set_download_handler(&downloader);
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
