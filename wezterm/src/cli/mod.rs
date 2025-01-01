use anyhow::anyhow;
use clap::Parser;
use std::ffi::OsString;
use wezterm_client::client::Client;

mod activate_pane;
mod activate_pane_direction;
mod activate_tab;
mod adjust_pane_size;
mod get_pane_direction;
mod get_text;
mod kill_pane;
mod list;
mod list_clients;
mod move_pane_to_new_tab;
mod proxy;
mod rename_workspace;
mod send_text;
mod set_tab_title;
mod set_window_title;
mod spawn_command;
mod split_pane;
mod tls_creds;
mod zoom_pane;

#[derive(Debug, Parser, Clone, Copy)]
enum CliOutputFormatKind {
    #[command(name = "table", about = "multi line space separated table")]
    Table,
    #[command(name = "json", about = "JSON format")]
    Json,
}

impl std::str::FromStr for CliOutputFormatKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<CliOutputFormatKind, Self::Err> {
        match s {
            "json" => Ok(CliOutputFormatKind::Json),
            "table" => Ok(CliOutputFormatKind::Table),
            _ => Err(anyhow::anyhow!("unknown output format")),
        }
    }
}

#[derive(Debug, Parser, Clone, Copy)]
struct CliOutputFormat {
    /// Controls the output format.
    /// "table" and "json" are possible formats.
    #[arg(long = "format", default_value = "table")]
    format: CliOutputFormatKind,
}

#[derive(Debug, Parser, Clone)]
pub struct CliCommand {
    /// Don't automatically start the server
    #[arg(long = "no-auto-start")]
    no_auto_start: bool,

    /// Prefer connecting to a background mux server.
    /// The default is to prefer connecting to a running
    /// wezterm gui instance
    #[arg(long = "prefer-mux")]
    prefer_mux: bool,

    /// When connecting to a gui instance, if you started the
    /// gui with `--class SOMETHING`, you should also pass
    /// that same value here in order for the client to find
    /// the correct gui instance.
    #[arg(long = "class")]
    class: Option<String>,

    #[command(subcommand)]
    sub: CliSubCommand,
}

#[derive(Debug, Parser, Clone)]
enum CliSubCommand {
    #[command(name = "list", about = "list windows, tabs and panes")]
    List(list::ListCommand),

    #[command(name = "list-clients", about = "list clients")]
    ListClients(list_clients::ListClientsCommand),

    #[command(name = "proxy", about = "start rpc proxy pipe")]
    Proxy(proxy::ProxyCommand),

    #[command(name = "tlscreds", about = "obtain tls credentials")]
    TlsCreds(tls_creds::TlsCredsCommand),

    #[command(
        name = "move-pane-to-new-tab",
        rename_all = "kebab",
        about = "Move a pane into a new tab"
    )]
    MovePaneToNewTab(move_pane_to_new_tab::MovePaneToNewTab),

    #[command(
        name = "split-pane",
        rename_all = "kebab",
        trailing_var_arg = true,
        about = "split the current pane.
Outputs the pane-id for the newly created pane on success"
    )]
    SplitPane(split_pane::SplitPane),

    #[command(
        name = "spawn",
        trailing_var_arg = true,
        about = "Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success"
    )]
    SpawnCommand(spawn_command::SpawnCommand),

    /// Send text to a pane as though it were pasted.
    /// If bracketed paste mode is enabled in the pane, then the
    /// text will be sent as a bracketed paste.
    #[command(name = "send-text", rename_all = "kebab")]
    SendText(send_text::SendText),

    /// Retrieves the textual content of a pane and output it to stdout
    #[command(name = "get-text", rename_all = "kebab")]
    GetText(get_text::GetText),

    /// Activate an adjacent pane in the specified direction.
    #[command(name = "activate-pane-direction", rename_all = "kebab")]
    ActivatePaneDirection(activate_pane_direction::ActivatePaneDirection),

    /// Determine the adjacent pane in the specified direction.
    ///
    /// Prints the pane id in that direction, or nothing if there
    /// is no pane in that direction.
    #[command(name = "get-pane-direction", rename_all = "kebab")]
    GetPaneDirection(get_pane_direction::GetPaneDirection),

    /// Kill a pane
    #[command(name = "kill-pane", rename_all = "kebab")]
    KillPane(kill_pane::KillPane),

    /// Activate (focus) a pane
    #[command(name = "activate-pane", rename_all = "kebab")]
    ActivatePane(activate_pane::ActivatePane),

    /// Adjust the size of a pane directionally
    #[command(name = "adjust-pane-size", rename_all = "kebab")]
    AdjustPaneSize(adjust_pane_size::CliAdjustPaneSize),

    /// Activate a tab
    #[command(name = "activate-tab", rename_all = "kebab")]
    ActivateTab(activate_tab::ActivateTab),

    /// Change the title of a tab
    #[command(name = "set-tab-title", rename_all = "kebab")]
    SetTabTitle(set_tab_title::SetTabTitle),

    /// Change the title of a window
    #[command(name = "set-window-title", rename_all = "kebab")]
    SetWindowTitle(set_window_title::SetWindowTitle),

    /// Rename a workspace
    #[command(name = "rename-workspace", rename_all = "kebab")]
    RenameWorkspace(rename_workspace::RenameWorkspace),

    /// Zoom, unzoom, or toggle zoom state
    #[command(name = "zoom-pane", rename_all = "kebab")]
    ZoomPane(zoom_pane::ZoomPane),
}

async fn run_cli_async(opts: &crate::Opt, cli: CliCommand) -> anyhow::Result<()> {
    let mut ui = mux::connui::ConnectionUI::new_headless();
    let initial = true;

    let client = Client::new_default_unix_domain(
        initial,
        &mut ui,
        cli.no_auto_start,
        cli.prefer_mux,
        cli.class
            .as_deref()
            .unwrap_or(wezterm_gui_subcommands::DEFAULT_WINDOW_CLASS),
    )?;

    match cli.sub {
        CliSubCommand::ListClients(cmd) => cmd.run(client).await,
        CliSubCommand::List(cmd) => cmd.run(client).await,
        CliSubCommand::MovePaneToNewTab(cmd) => cmd.run(client).await,
        CliSubCommand::SplitPane(cmd) => cmd.run(client).await,
        CliSubCommand::SendText(cmd) => cmd.run(client).await,
        CliSubCommand::GetText(cmd) => cmd.run(client).await,
        CliSubCommand::SpawnCommand(cmd) => cmd.run(client, &crate::init_config(opts)?).await,
        CliSubCommand::Proxy(cmd) => cmd.run(client, &crate::init_config(opts)?).await,
        CliSubCommand::TlsCreds(cmd) => cmd.run(client).await,
        CliSubCommand::ActivatePaneDirection(cmd) => cmd.run(client).await,
        CliSubCommand::GetPaneDirection(cmd) => cmd.run(client).await,
        CliSubCommand::KillPane(cmd) => cmd.run(client).await,
        CliSubCommand::ActivatePane(cmd) => cmd.run(client).await,
        CliSubCommand::AdjustPaneSize(cmd) => cmd.run(client).await,
        CliSubCommand::ActivateTab(cmd) => cmd.run(client).await,
        CliSubCommand::SetTabTitle(cmd) => cmd.run(client).await,
        CliSubCommand::SetWindowTitle(cmd) => cmd.run(client).await,
        CliSubCommand::RenameWorkspace(cmd) => cmd.run(client).await,
        CliSubCommand::ZoomPane(cmd) => cmd.run(client).await,
    }
}

pub fn run_cli(opts: &crate::Opt, cli: CliCommand) -> anyhow::Result<()> {
    let executor = promise::spawn::ScopedExecutor::new();
    match promise::spawn::block_on(executor.run(async move { run_cli_async(opts, cli).await })) {
        Ok(_) => Ok(()),
        Err(err) => crate::terminate_with_error(err),
    }
}

pub fn resolve_relative_cwd(cwd: Option<OsString>) -> anyhow::Result<Option<String>> {
    match cwd {
        None => Ok(None),
        Some(cwd) => Ok(Some(
            std::env::current_dir()?
                .join(cwd)
                .to_str()
                .ok_or_else(|| anyhow!("path is not representable as String"))?
                .to_string(),
        )),
    }
}
