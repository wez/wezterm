use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use config::keyassignment::SpawnTabDomain;
use config::wezterm_version;
use mux::activity::Activity;
use mux::pane::PaneId;
use mux::tab::SplitDirection;
use mux::window::WindowId;
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use serde::Serializer as _;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::rc::Rc;
use structopt::StructOpt;
use tabout::{tabulate_output, Alignment, Column};
use umask::UmaskSaver;
use wezterm_client::client::{unix_connect_with_retry, Client};
use wezterm_gui_subcommands::*;

mod asciicast;

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";

#[derive(Debug, StructOpt)]
#[structopt(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    global_setting = structopt::clap::AppSettings::ColoredHelp,
    version = wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[structopt(name = "skip-config", short = "n")]
    skip_config: bool,

    /// Specify the configuration file to use, overrides the normal
    /// configuration file resolution
    #[structopt(
        long = "config-file",
        parse(from_os_str),
        conflicts_with = "skip-config"
    )]
    config_file: Option<OsString>,

    /// Override specific configuration values
    #[structopt(
        long = "config",
        name = "name=value",
        parse(try_from_str = name_equals_value),
        number_of_values = 1)]
    config_override: Vec<(String, String)>,

    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, StructOpt, Clone)]
enum SubCommand {
    #[structopt(
        name = "start",
        about = "Start the GUI, optionally running an alternative program"
    )]
    Start(StartCommand),

    #[structopt(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[structopt(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[structopt(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[structopt(name = "ls-fonts", about = "Display information about fonts")]
    LsFonts(LsFontsCommand),

    #[structopt(name = "cli", about = "Interact with experimental mux server")]
    Cli(CliCommand),

    #[structopt(name = "imgcat", about = "Output an image to the terminal")]
    ImageCat(ImgCatCommand),

    #[structopt(
        name = "set-working-directory",
        about = "Advise the terminal of the current working directory by \
                 emitting an OSC 7 escape sequence"
    )]
    SetCwd(SetCwdCommand),

    #[structopt(name = "record", about = "Record a terminal session as an asciicast")]
    Record(asciicast::RecordCommand),

    #[structopt(name = "replay", about = "Replay an asciicast terminal session")]
    Replay(asciicast::PlayCommand),
}

#[derive(Debug, StructOpt, Clone)]
struct CliCommand {
    /// Don't automatically start the server
    #[structopt(long = "no-auto-start")]
    no_auto_start: bool,

    /// Prefer connecting to a background mux server.
    /// The default is to prefer connecting to a running
    /// wezterm gui instance
    #[structopt(long = "prefer-mux")]
    prefer_mux: bool,

    /// When connecting to a gui instance, if you started the
    /// gui with `--class SOMETHING`, you should also pass
    /// that same value here in order for the client to find
    /// the correct gui instance.
    #[structopt(long = "class")]
    class: Option<String>,

    #[structopt(subcommand)]
    sub: CliSubCommand,
}

#[derive(Debug, StructOpt, Clone, Copy)]
enum CliOutputFormatKind {
    #[structopt(name = "table", about = "multi line space separated table")]
    Table,
    #[structopt(name = "json", about = "JSON format")]
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

#[derive(Debug, StructOpt, Clone, Copy)]
struct CliOutputFormat {
    /// Controls the output format.
    /// "table" and "json" are possible formats.
    #[structopt(long = "format", default_value = "table")]
    format: CliOutputFormatKind,
}

#[derive(Debug, StructOpt, Clone)]
enum CliSubCommand {
    #[structopt(name = "list", about = "list windows, tabs and panes")]
    List(CliOutputFormat),

    #[structopt(name = "list-clients", about = "list clients")]
    ListClients(CliOutputFormat),

    #[structopt(name = "proxy", about = "start rpc proxy pipe")]
    Proxy,

    #[structopt(name = "tlscreds", about = "obtain tls credentials")]
    TlsCreds,

    #[structopt(
        name = "split-pane",
        about = "split the current pane.
Outputs the pane-id for the newly created pane on success"
    )]
    SplitPane {
        /// Specify the pane that should be split.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[structopt(long = "pane-id")]
        pane_id: Option<PaneId>,

        /// Split horizontally rather than vertically
        #[structopt(long = "horizontal")]
        horizontal: bool,

        /// Specify the current working directory for the initially
        /// spawned program
        #[structopt(long = "cwd", parse(from_os_str))]
        cwd: Option<OsString>,

        /// Instead of executing your shell, run PROG.
        /// For example: `wezterm start -- bash -l` will spawn bash
        /// as if it were a login shell.
        #[structopt(parse(from_os_str))]
        prog: Vec<OsString>,
    },

    #[structopt(
        name = "spawn",
        about = "Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success"
    )]
    SpawnCommand {
        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        /// The pane is used to determine the current domain
        /// and window.
        #[structopt(long = "pane-id")]
        pane_id: Option<PaneId>,

        #[structopt(long = "domain-name")]
        domain_name: Option<String>,

        /// Specify the window into which to spawn a tab.
        /// If omitted, the window associated with the current
        /// pane is used.
        #[structopt(long = "window-id")]
        window_id: Option<WindowId>,

        /// Spawn into a new window, rather than a new tab
        #[structopt(long = "new-window", conflicts_with = "window_id")]
        new_window: bool,

        /// Specify the current working directory for the initially
        /// spawned program
        #[structopt(long = "cwd", parse(from_os_str))]
        cwd: Option<OsString>,

        /// When creating a new window, override the default workspace name
        /// with the provided name.  The default name is "default".
        #[structopt(long = "workspace")]
        workspace: Option<String>,

        /// Instead of executing your shell, run PROG.
        /// For example: `wezterm start -- bash -l` will spawn bash
        /// as if it were a login shell.
        #[structopt(parse(from_os_str))]
        prog: Vec<OsString>,
    },

    /// Send text to a pane as though it were pasted.
    /// If bracketed paste mode is enabled in the pane, then the
    /// text will be sent as a bracketed paste.
    #[structopt(name = "send-text")]
    SendText {
        /// Specify the target pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[structopt(long = "pane-id")]
        pane_id: Option<PaneId>,

        /// The text to send. If omitted, will read the text from stdin.
        text: Option<String>,
    },
}

use termwiz::escape::osc::{
    ITermDimension, ITermFileData, ITermProprietary, OperatingSystemCommand,
};

#[derive(Debug, StructOpt, Clone)]
struct ImgCatCommand {
    /// Specify the display width; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal width.
    #[structopt(long = "width")]
    width: Option<ITermDimension>,
    /// Specify the display height; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal height.
    #[structopt(long = "height")]
    height: Option<ITermDimension>,
    /// Do not respect the aspect ratio.  The default is to respect the aspect
    /// ratio
    #[structopt(long = "no-preserve-aspect-ratio")]
    no_preserve_aspect_ratio: bool,
    /// The name of the image file to be displayed.
    /// If omitted, will attempt to read it from stdin.
    #[structopt(parse(from_os_str))]
    file_name: Option<OsString>,
}

impl ImgCatCommand {
    fn run(&self) -> anyhow::Result<()> {
        let mut data = Vec::new();
        if let Some(file_name) = self.file_name.as_ref() {
            let mut f = std::fs::File::open(file_name)
                .with_context(|| anyhow!("reading image file: {:?}", file_name))?;
            f.read_to_end(&mut data)?;
        } else {
            let mut stdin = std::io::stdin();
            stdin.read_to_end(&mut data)?;
        }

        let osc = OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
            ITermFileData {
                name: None,
                size: Some(data.len()),
                width: self.width.unwrap_or_else(Default::default),
                height: self.height.unwrap_or_else(Default::default),
                preserve_aspect_ratio: !self.no_preserve_aspect_ratio,
                inline: true,
                do_not_move_cursor: false,
                data,
            },
        )));
        println!("{}", osc);

        Ok(())
    }
}

#[derive(Debug, StructOpt, Clone)]
struct SetCwdCommand {
    /// The directory to specify.
    /// If omitted, will use the current directory of the process itself.
    #[structopt(parse(from_os_str))]
    cwd: Option<OsString>,

    /// The hostname to use in the constructed file:// URL.
    /// If omitted, the system hostname will be used.
    #[structopt(parse(from_os_str))]
    host: Option<OsString>,
}

impl SetCwdCommand {
    fn run(&self) -> anyhow::Result<()> {
        let cwd: std::path::PathBuf = match self.cwd.as_ref() {
            Some(d) => std::fs::canonicalize(d)?,
            None => std::env::current_dir()?,
        };

        let mut url = url::Url::from_directory_path(&cwd)
            .map_err(|_| anyhow::anyhow!("cwd {} is not an absolute path", cwd.display()))?;
        let host = match self.host.as_ref() {
            Some(h) => h.clone(),
            None => hostname::get()?,
        };
        let host = host.to_str().unwrap_or("localhost");
        url.set_host(Some(host))?;

        let osc = OperatingSystemCommand::CurrentWorkingDirectory(url.into());
        print!("{}", osc);
        Ok(())
    }
}

fn canon_cwd(cwd: Option<OsString>) -> anyhow::Result<Option<String>> {
    match cwd {
        None => Ok(None),
        Some(cwd) => Ok(Some(
            std::fs::canonicalize(cwd)?
                .to_str()
                .ok_or_else(|| anyhow!("path is not representable as String"))?
                .to_string(),
        )),
    }
}

fn terminate_with_error_message(err: &str) -> ! {
    log::error!("{}; terminating", err);
    std::process::exit(1);
}

fn terminate_with_error(err: anyhow::Error) -> ! {
    terminate_with_error_message(&format!("{:#}", err));
}

fn main() {
    config::designate_this_as_the_main_thread();
    config::assign_error_callback(mux::connui::show_configuration_error_message);
    if let Err(e) = run() {
        terminate_with_error(e);
    }
    Mux::shutdown();
}

fn run() -> anyhow::Result<()> {
    env_bootstrap::bootstrap();

    let saver = UmaskSaver::new();

    let opts = Opt::from_args();
    config::common_init(
        opts.config_file.as_ref(),
        &opts.config_override,
        opts.skip_config,
    );
    let config = config::configuration();

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(_)
        | SubCommand::LsFonts(_)
        | SubCommand::Ssh(_)
        | SubCommand::Serial(_)
        | SubCommand::Connect(_) => delegate_to_gui(saver),
        SubCommand::ImageCat(cmd) => cmd.run(),
        SubCommand::SetCwd(cmd) => cmd.run(),
        SubCommand::Cli(cli) => run_cli(config, cli),
        SubCommand::Record(cmd) => cmd.run(config),
        SubCommand::Replay(cmd) => cmd.run(),
    }
}

fn delegate_to_gui(saver: UmaskSaver) -> anyhow::Result<()> {
    use std::process::Command;

    // Restore the original umask
    drop(saver);

    let exe_name = if cfg!(windows) {
        "wezterm-gui.exe"
    } else {
        "wezterm-gui"
    };

    let exe = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("exe has no parent dir!?"))?
        .join(exe_name);

    let mut cmd = Command::new(exe);
    if cfg!(windows) {
        cmd.arg("--attach-parent-console");
    }

    cmd.args(std::env::args_os().skip(1));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Clean up random fds, except when we're running in an AppImage.
        // AppImage relies on child processes keeping alive an fd that
        // references the mount point and if we close it as part of execing
        // the gui binary, the appimage gets unmounted before we can exec.
        if std::env::var_os("APPIMAGE").is_none() {
            portable_pty::unix::close_random_fds();
        }
        return Err(anyhow::anyhow!("failed to exec: {:?}", cmd.exec()));
    }

    #[cfg(windows)]
    {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}

async fn resolve_pane_id(client: &Client, pane_id: Option<PaneId>) -> anyhow::Result<PaneId> {
    let pane_id: PaneId = match pane_id {
        Some(p) => p,
        None => {
            if let Ok(pane) = std::env::var("WEZTERM_PANE") {
                pane.parse()?
            } else {
                let mut clients = client.list_clients(codec::GetClientList).await?.clients;
                clients.retain(|client| client.focused_pane_id.is_some());
                clients.sort_by(|a, b| b.last_input.cmp(&a.last_input));
                if clients.is_empty() {
                    anyhow::bail!(
                        "--pane-id was not specified and $WEZTERM_PANE
                         is not set in the environment, and I couldn't
                         determine which pane was currently focused"
                    );
                }

                clients[0]
                    .focused_pane_id
                    .expect("to have filtered out above")
            }
        }
    };
    Ok(pane_id)
}

#[derive(serde::Serialize)]
struct CliListResultPtySize {
    rows: u16,
    cols: u16,
}
// This will be serialized to JSON via the 'List' command.
// As such it is intended to be a stable output format,
// Thus we need to be careful about both the fields and their types,
// herein as they are directly reflected in the output.
#[derive(serde::Serialize)]
struct CliListResultItem {
    window_id: mux::window::WindowId,
    tab_id: mux::tab::TabId,
    pane_id: mux::pane::PaneId,
    workspace: String,
    size: CliListResultPtySize,
    title: String,
    cwd: String,
}

impl From<mux::tab::PaneEntry> for CliListResultItem {
    fn from(pane: mux::tab::PaneEntry) -> CliListResultItem {
        let mux::tab::PaneEntry {
            window_id,
            tab_id,
            pane_id,
            workspace,
            title,
            working_dir,
            size: portable_pty::PtySize { rows, cols, .. },
            ..
        } = pane;

        CliListResultItem {
            window_id,
            tab_id,
            pane_id,
            workspace,
            size: CliListResultPtySize { rows, cols },
            title,
            cwd: working_dir
                .as_ref()
                .map(|url| url.url.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

// This will be serialized to JSON via the 'ListClients' command.
// As such it is intended to be a stable output format,
// Thus we need to be careful about the stability of the fields and types
// herein as they are directly reflected in the output.
#[derive(serde::Serialize)]
struct CliListClientsResultItem {
    username: String,
    hostname: String,
    pid: u32,
    connection_elapsed: std::time::Duration,
    idle_time: std::time::Duration,
    workspace: String,
    focused_pane_id: Option<mux::pane::PaneId>,
}

impl From<mux::client::ClientInfo> for CliListClientsResultItem {
    fn from(client_info: mux::client::ClientInfo) -> CliListClientsResultItem {
        let now: DateTime<Utc> = Utc::now();

        let mux::client::ClientInfo {
            connected_at,
            last_input,
            active_workspace,
            focused_pane_id,
            client_id,
            ..
        } = client_info;

        let mux::client::ClientId {
            username,
            hostname,
            pid,
            ..
        } = client_id.as_ref();

        let connection_elapsed = now - connected_at;
        let idle_time = now - last_input;

        CliListClientsResultItem {
            username: username.to_string(),
            hostname: hostname.to_string(),
            pid: *pid,
            connection_elapsed: connection_elapsed
                .to_std()
                .unwrap_or(std::time::Duration::ZERO),
            idle_time: idle_time.to_std().unwrap_or(std::time::Duration::ZERO),
            workspace: active_workspace.as_deref().unwrap_or("").to_string(),
            focused_pane_id: focused_pane_id,
        }
    }
}

async fn run_cli_async(config: config::ConfigHandle, cli: CliCommand) -> anyhow::Result<()> {
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
        CliSubCommand::ListClients(CliOutputFormat { format }) => {
            let out = std::io::stdout();
            let clients = client.list_clients(codec::GetClientList).await?;
            match format {
                CliOutputFormatKind::Json => {
                    let clients = clients
                        .clients
                        .iter()
                        .cloned()
                        .map(CliListClientsResultItem::from);
                    let mut writer = serde_json::Serializer::pretty(out.lock());
                    writer.collect_seq(clients)?;
                }
                CliOutputFormatKind::Table => {
                    let cols = vec![
                        Column {
                            name: "USER".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "HOST".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "PID".to_string(),
                            alignment: Alignment::Right,
                        },
                        Column {
                            name: "CONNECTED".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "IDLE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "WORKSPACE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "FOCUS".to_string(),
                            alignment: Alignment::Right,
                        },
                    ];
                    let mut data = vec![];
                    let now: DateTime<Utc> = Utc::now();

                    fn duration_string(d: chrono::Duration) -> String {
                        if let Ok(d) = d.to_std() {
                            format!("{:?}", d)
                        } else {
                            d.to_string()
                        }
                    }

                    for info in clients.clients {
                        let connected = now - info.connected_at;
                        let idle = now - info.last_input;
                        data.push(vec![
                            info.client_id.username.to_string(),
                            info.client_id.hostname.to_string(),
                            info.client_id.pid.to_string(),
                            duration_string(connected),
                            duration_string(idle),
                            info.active_workspace.as_deref().unwrap_or("").to_string(),
                            info.focused_pane_id
                                .map(|id| id.to_string())
                                .unwrap_or_else(String::new),
                        ]);
                    }

                    tabulate_output(&cols, &data, &mut out.lock())?;
                }
            }
        }
        CliSubCommand::List(CliOutputFormat { format }) => {
            let out = std::io::stdout();

            let mut output_items = vec![];
            let panes = client.list_panes().await?;

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        output_items.push(CliListResultItem::from(entry.clone()));
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }
            match format {
                CliOutputFormatKind::Json => {
                    let mut writer = serde_json::Serializer::pretty(out.lock());
                    writer.collect_seq(output_items.iter())?;
                }
                CliOutputFormatKind::Table => {
                    let cols = vec![
                        Column {
                            name: "WINID".to_string(),
                            alignment: Alignment::Right,
                        },
                        Column {
                            name: "TABID".to_string(),
                            alignment: Alignment::Right,
                        },
                        Column {
                            name: "PANEID".to_string(),
                            alignment: Alignment::Right,
                        },
                        Column {
                            name: "WORKSPACE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "SIZE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "TITLE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "CWD".to_string(),
                            alignment: Alignment::Left,
                        },
                    ];
                    let data = output_items
                        .iter()
                        .map(|output_item| {
                            vec![
                                output_item.window_id.to_string(),
                                output_item.tab_id.to_string(),
                                output_item.pane_id.to_string(),
                                output_item.workspace.to_string(),
                                format!("{}x{}", output_item.size.cols, output_item.size.rows),
                                output_item.title.to_string(),
                                output_item.cwd.to_string(),
                            ]
                        })
                        .collect::<Vec<_>>();
                    tabulate_output(&cols, &data, &mut std::io::stdout().lock())?;
                }
            }
        }
        CliSubCommand::SplitPane {
            pane_id,
            cwd,
            prog,
            horizontal,
        } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;

            let spawned = client
                .split_pane(codec::SplitPane {
                    pane_id,
                    direction: if horizontal {
                        SplitDirection::Horizontal
                    } else {
                        SplitDirection::Vertical
                    },
                    domain: config::keyassignment::SpawnTabDomain::CurrentPaneDomain,
                    command: if prog.is_empty() {
                        None
                    } else {
                        let builder = CommandBuilder::from_argv(prog);
                        Some(builder)
                    },
                    command_dir: canon_cwd(cwd)?,
                })
                .await?;

            log::debug!("{:?}", spawned);
            println!("{}", spawned.pane_id);
        }
        CliSubCommand::SendText { pane_id, text } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;

            let data = match text {
                Some(text) => text,
                None => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .context("reading stdin")?;
                    text
                }
            };

            client
                .send_paste(codec::SendPaste { pane_id, data })
                .await?;
        }
        CliSubCommand::SpawnCommand {
            cwd,
            prog,
            pane_id,
            domain_name,
            window_id,
            new_window,
            workspace,
        } => {
            let window_id = if new_window {
                None
            } else {
                match window_id {
                    Some(w) => Some(w),
                    None => {
                        let pane_id = resolve_pane_id(&client, pane_id).await?;

                        let panes = client.list_panes().await?;
                        let mut window_id = None;
                        'outer: for tabroot in panes.tabs {
                            let mut cursor = tabroot.into_tree().cursor();

                            loop {
                                if let Some(entry) = cursor.leaf_mut() {
                                    if entry.pane_id == pane_id {
                                        window_id.replace(entry.window_id);
                                        break 'outer;
                                    }
                                }
                                match cursor.preorder_next() {
                                    Ok(c) => cursor = c,
                                    Err(_) => break,
                                }
                            }
                        }
                        window_id
                    }
                }
            };

            let workspace = workspace.unwrap_or_else(|| mux::DEFAULT_WORKSPACE.to_string());

            let spawned = client
                .spawn_v2(codec::SpawnV2 {
                    domain: domain_name.map_or(SpawnTabDomain::DefaultDomain, |name| {
                        SpawnTabDomain::DomainName(name)
                    }),
                    window_id,
                    command: if prog.is_empty() {
                        None
                    } else {
                        let builder = CommandBuilder::from_argv(prog);
                        Some(builder)
                    },
                    command_dir: canon_cwd(cwd)?,
                    size: config::configuration().initial_size(),
                    workspace,
                })
                .await?;

            log::debug!("{:?}", spawned);
            println!("{}", spawned.pane_id);
        }
        CliSubCommand::Proxy => {
            // The client object we created above will have spawned
            // the server if needed, so now all we need to do is turn
            // ourselves into basically netcat.
            drop(client);

            let mux = Rc::new(mux::Mux::new(None));
            Mux::set_mux(&mux);
            let unix_dom = config.unix_domains.first().unwrap();
            let target = unix_dom.target();
            let stream = unix_connect_with_retry(&target, false, None)?;

            // Spawn a thread to pull data from the socket and write
            // it to stdout
            let duped = stream.try_clone()?;
            let activity = Activity::new();
            std::thread::spawn(move || {
                let stdout = std::io::stdout();
                consume_stream_then_exit_process(duped, stdout.lock(), activity);
            });

            // and pull data from stdin and write it to the socket
            let activity = Activity::new();
            std::thread::spawn(move || {
                let stdin = std::io::stdin();
                consume_stream_then_exit_process(stdin.lock(), stream, activity);
            });

            // Wait forever; the stdio threads will terminate on EOF
            smol::future::pending().await
        }
        CliSubCommand::TlsCreds => {
            let creds = client.get_tls_creds().await?;
            codec::Pdu::GetTlsCredsResponse(creds).encode(std::io::stdout().lock(), 0)?;
        }
    }
    Ok(())
}

fn run_cli(config: config::ConfigHandle, cli: CliCommand) -> anyhow::Result<()> {
    let executor = promise::spawn::ScopedExecutor::new();
    match promise::spawn::block_on(executor.run(async move { run_cli_async(config, cli).await })) {
        Ok(_) => Ok(()),
        Err(err) => terminate_with_error(err),
    }
}

fn consume_stream<F: Read, T: Write>(mut from_stream: F, mut to_stream: T) -> anyhow::Result<()> {
    let mut buf = [0u8; 8192];

    loop {
        let size = from_stream.read(&mut buf)?;
        if size == 0 {
            break;
        }
        to_stream.write_all(&buf[0..size])?;
        to_stream.flush()?;
    }
    Ok(())
}

fn consume_stream_then_exit_process<F: Read, T: Write>(
    from_stream: F,
    to_stream: T,
    activity: Activity,
) -> ! {
    consume_stream(from_stream, to_stream).ok();
    std::thread::sleep(std::time::Duration::new(2, 0));
    drop(activity);
    std::process::exit(0);
}
