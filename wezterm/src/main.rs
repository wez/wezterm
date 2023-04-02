use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::builder::{PossibleValue, ValueParser};
use clap::{Parser, ValueEnum, ValueHint};
use clap_complete::{generate as generate_completion, shells, Generator as CompletionGenerator};
use config::keyassignment::{PaneDirection, SpawnTabDomain};
use config::wezterm_version;
use mux::activity::Activity;
use mux::pane::PaneId;
use mux::tab::{SplitDirection, SplitRequest, SplitSize, TabId};
use mux::window::WindowId;
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use serde::Serializer as _;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::Arc;
use tabout::{tabulate_output, Alignment, Column};
use termwiz_funcs::lines_to_escapes;
use umask::UmaskSaver;
use wezterm_client::client::{unix_connect_with_retry, Client};
use wezterm_gui_subcommands::*;
use wezterm_term::{ScrollbackOrVisibleRowIndex, StableRowIndex, TerminalSize};

mod asciicast;

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";

#[derive(Debug, Parser)]
#[command(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    version = wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[arg(long, short = 'n')]
    skip_config: bool,

    /// Specify the configuration file to use, overrides the normal
    /// configuration file resolution
    #[arg(
        long,
        value_parser,
        conflicts_with = "skip_config",
        value_hint=ValueHint::FilePath
    )]
    config_file: Option<OsString>,

    /// Override specific configuration values
    #[arg(
        long = "config",
        name = "name=value",
        value_parser=ValueParser::new(name_equals_value),
        number_of_values = 1)]
    config_override: Vec<(String, String)>,

    #[command(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, Clone, ValueEnum)]
enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Fig,
}

impl CompletionGenerator for Shell {
    fn file_name(&self, name: &str) -> String {
        match self {
            Shell::Bash => shells::Bash.file_name(name),
            Shell::Elvish => shells::Elvish.file_name(name),
            Shell::Fish => shells::Fish.file_name(name),
            Shell::PowerShell => shells::PowerShell.file_name(name),
            Shell::Zsh => shells::Zsh.file_name(name),
            Shell::Fig => clap_complete_fig::Fig.file_name(name),
        }
    }

    fn generate(&self, cmd: &clap::Command, buf: &mut dyn std::io::Write) {
        match self {
            Shell::Bash => shells::Bash.generate(cmd, buf),
            Shell::Elvish => shells::Elvish.generate(cmd, buf),
            Shell::Fish => shells::Fish.generate(cmd, buf),
            Shell::PowerShell => shells::PowerShell.generate(cmd, buf),
            Shell::Zsh => shells::Zsh.generate(cmd, buf),
            Shell::Fig => clap_complete_fig::Fig.generate(cmd, buf),
        }
    }
}

#[derive(Debug, Parser, Clone)]
enum SubCommand {
    #[command(
        name = "start",
        about = "Start the GUI, optionally running an alternative program"
    )]
    Start(StartCommand),

    #[command(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[command(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[command(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[command(name = "ls-fonts", about = "Display information about fonts")]
    LsFonts(LsFontsCommand),

    #[command(name = "show-keys", about = "Show key assignments")]
    ShowKeys(ShowKeysCommand),

    #[command(name = "cli", about = "Interact with experimental mux server")]
    Cli(CliCommand),

    #[command(name = "imgcat", about = "Output an image to the terminal")]
    ImageCat(ImgCatCommand),

    #[command(
        name = "set-working-directory",
        about = "Advise the terminal of the current working directory by \
                 emitting an OSC 7 escape sequence"
    )]
    SetCwd(SetCwdCommand),

    #[command(name = "record", about = "Record a terminal session as an asciicast")]
    Record(asciicast::RecordCommand),

    #[command(name = "replay", about = "Replay an asciicast terminal session")]
    Replay(asciicast::PlayCommand),

    /// Generate shell completion information
    #[command(name = "shell-completion")]
    ShellCompletion {
        /// Which shell to generate for
        #[arg(long, value_parser)]
        shell: Shell,
    },
}

#[derive(Debug, Parser, Clone)]
struct CliCommand {
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
enum CliSubCommand {
    #[command(name = "list", about = "list windows, tabs and panes")]
    List(CliOutputFormat),

    #[command(name = "list-clients", about = "list clients")]
    ListClients(CliOutputFormat),

    #[command(name = "proxy", about = "start rpc proxy pipe")]
    Proxy,

    #[command(name = "tlscreds", about = "obtain tls credentials")]
    TlsCreds,

    #[command(
        name = "move-pane-to-new-tab",
        rename_all = "kebab",
        about = "Move a pane into a new tab"
    )]
    MovePaneToNewTab {
        /// Specify the pane that should be moved.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// Specify the window into which the new tab will be
        /// created.
        /// If omitted, the window associated with the current
        /// pane is used.
        #[arg(long)]
        window_id: Option<WindowId>,

        /// Create tab in a new window, rather than the window
        /// currently containing the pane.
        #[arg(long, conflicts_with = "window_id")]
        new_window: bool,

        /// If creating a new window, override the default workspace name
        /// with the provided name.  The default name is "default".
        #[arg(long)]
        workspace: Option<String>,
    },

    #[command(
        name = "split-pane",
        rename_all = "kebab",
        trailing_var_arg = true,
        about = "split the current pane.
Outputs the pane-id for the newly created pane on success"
    )]
    SplitPane {
        /// Specify the pane that should be split.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// Equivalent to `--right`. If neither this nor any other direction
        /// is specified, the default is equivalent to `--bottom`.
        #[arg(long, conflicts_with_all=&["left", "right", "top", "bottom"])]
        horizontal: bool,

        /// Split horizontally, with the new pane on the left
        #[arg(long, conflicts_with_all=&["right", "top", "bottom"])]
        left: bool,

        /// Split horizontally, with the new pane on the right
        #[arg(long, conflicts_with_all=&["left", "top", "bottom"])]
        right: bool,

        /// Split vertically, with the new pane on the top
        #[arg(long, conflicts_with_all=&["left", "right", "bottom"])]
        top: bool,

        /// Split vertically, with the new pane on the bottom
        #[arg(long, conflicts_with_all=&["left", "right", "top"])]
        bottom: bool,

        /// Rather than splitting the active pane, split the entire
        /// window.
        #[arg(long)]
        top_level: bool,

        /// The number of cells that the new split should have.
        /// If omitted, 50% of the available space is used.
        #[arg(long)]
        cells: Option<usize>,

        /// Specify the number of cells that the new split should
        /// have, expressed as a percentage of the available space.
        #[arg(long, conflicts_with = "cells")]
        percent: Option<u8>,

        /// Specify the current working directory for the initially
        /// spawned program
        #[arg(long, value_parser, value_hint=ValueHint::DirPath)]
        cwd: Option<OsString>,

        /// Instead of spawning a new command, move the specified
        /// pane into the newly created split.
        #[arg(long, conflicts_with_all=&["cwd", "prog"])]
        move_pane_id: Option<PaneId>,

        /// Instead of executing your shell, run PROG.
        /// For example: `wezterm cli split-pane -- bash -l` will spawn bash
        /// as if it were a login shell.
        #[arg(value_parser, value_hint=ValueHint::CommandWithArguments, num_args=1..)]
        prog: Vec<OsString>,
    },

    #[command(
        name = "spawn",
        trailing_var_arg = true,
        about = "Spawn a command into a new window or tab
Outputs the pane-id for the newly created pane on success"
    )]
    SpawnCommand {
        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        /// The pane is used to determine the current domain
        /// and window.
        #[arg(long)]
        pane_id: Option<PaneId>,

        #[arg(long)]
        domain_name: Option<String>,

        /// Specify the window into which to spawn a tab.
        /// If omitted, the window associated with the current
        /// pane is used.
        /// Cannot be used with `--workspace` or `--new-window`.
        #[arg(long, conflicts_with_all=&["workspace", "new_window"])]
        window_id: Option<WindowId>,

        /// Spawn into a new window, rather than a new tab.
        #[arg(long)]
        new_window: bool,

        /// Specify the current working directory for the initially
        /// spawned program
        #[arg(long, value_parser, value_hint=ValueHint::DirPath)]
        cwd: Option<OsString>,

        /// When creating a new window, override the default workspace name
        /// with the provided name.  The default name is "default".
        /// Requires `--new-window`.
        #[arg(long, requires = "new_window")]
        workspace: Option<String>,

        /// Instead of executing your shell, run PROG.
        /// For example: `wezterm cli spawn -- bash -l` will spawn bash
        /// as if it were a login shell.
        #[arg(value_parser, value_hint=ValueHint::CommandWithArguments, num_args=1..)]
        prog: Vec<OsString>,
    },

    /// Send text to a pane as though it were pasted.
    /// If bracketed paste mode is enabled in the pane, then the
    /// text will be sent as a bracketed paste.
    #[command(name = "send-text", rename_all = "kebab")]
    SendText {
        /// Specify the target pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// Send the text directly, rather than as a bracketed paste.
        #[arg(long)]
        no_paste: bool,

        /// The text to send. If omitted, will read the text from stdin.
        text: Option<String>,
    },

    /// Retrieves the textual content of a pane and output it to stdout
    #[command(name = "get-text", rename_all = "kebab")]
    GetText {
        /// Specify the target pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// The starting line number.
        /// 0 is the first line of terminal screen.
        /// Negative numbers proceed backwards into the scrollback.
        /// The default value is unspecified is 0, the first line of
        /// the terminal screen.
        #[arg(long, allow_hyphen_values = true)]
        start_line: Option<ScrollbackOrVisibleRowIndex>,

        /// The ending line number.
        /// 0 is the first line of terminal screen.
        /// Negative numbers proceed backwards into the scrollback.
        /// The default value if unspecified is the bottom of the
        /// the terminal screen.
        #[arg(long, allow_hyphen_values = true)]
        end_line: Option<ScrollbackOrVisibleRowIndex>,

        /// Include escape sequences that color and style the text.
        /// If omitted, unattributed text will be returned.
        #[arg(long)]
        escapes: bool,
    },

    /// Activate an adjacent pane in the specified direction.
    #[command(name = "activate-pane-direction", rename_all = "kebab")]
    ActivatePaneDirection {
        /// The direction to switch to.
        #[arg(value_parser=PaneDirectionParser{})]
        direction: PaneDirection,
    },

    /// Kill a pane
    #[command(name = "kill-pane", rename_all = "kebab")]
    KillPane {
        /// Specify the target pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,
    },

    /// Activate (focus) a pane
    #[command(name = "activate-pane", rename_all = "kebab")]
    ActivatePane {
        /// Specify the target pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[arg(long)]
        pane_id: Option<PaneId>,
    },

    /// Activate a tab
    #[command(name = "activate-tab", rename_all = "kebab")]
    ActivateTab {
        /// Specify the target tab by its id
        #[arg(long, conflicts_with_all=&["tab_index", "tab_relative", "no_wrap", "pane_id"])]
        tab_id: Option<TabId>,

        /// Specify the target tab by its index within the window
        /// that holds the current pane.
        /// Indices are 0-based, with 0 being the left-most tab.
        /// Negative numbers can be used to reference the right-most
        /// tab, so -1 is the right-most tab, -2 is the penultimate
        /// tab and so on.
        #[arg(long, allow_hyphen_values = true)]
        tab_index: Option<isize>,

        /// Specify the target tab by its relative offset.
        /// -1 selects the tab to the left. -2 two tabs to the left.
        /// 1 is one tab to the right and so on.
        ///
        /// Unless `--no-wrap` is specified, relative moves wrap
        /// around from the left-most to right-most and vice versa.
        #[arg(long, allow_hyphen_values = true)]
        tab_relative: Option<isize>,

        /// When used with tab-relative, prevents wrapping around
        /// and will instead clamp to the left-most when moving left
        /// or right-most when moving right.
        #[arg(long, requires = "tab_relative")]
        no_wrap: bool,

        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        ///
        /// The pane is used to figure out which window
        /// contains appropriate tabs
        #[arg(long)]
        pane_id: Option<PaneId>,
    },

    /// Change the title of a tab
    #[command(name = "set-tab-title", rename_all = "kebab")]
    SetTabTitle {
        /// Specify the target tab by its id
        #[arg(long, conflicts_with_all=&["pane_id"])]
        tab_id: Option<TabId>,
        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        ///
        /// The pane is used to figure out which tab should be renamed.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// The new title for the tab
        title: String,
    },

    /// Change the title of a window
    #[command(name = "set-window-title", rename_all = "kebab")]
    SetWindowTitle {
        /// Specify the target window by its id
        #[arg(long, conflicts_with_all=&["pane_id"])]
        window_id: Option<WindowId>,
        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        ///
        /// The pane is used to figure out which window
        /// should be renamed.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// The new title for the window
        title: String,
    },

    /// Rename a workspace
    #[command(name = "rename-workspace", rename_all = "kebab")]
    RenameWorkspace {
        /// Specify the workspace to rename
        #[arg(long)]
        workspace: Option<String>,

        /// Specify the current pane.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        ///
        /// The pane is used to figure out which workspace
        /// should be renamed.
        #[arg(long)]
        pane_id: Option<PaneId>,

        /// The new name for the workspace
        new_workspace: String,
    },
}

#[derive(Clone, Copy)]
struct PaneDirectionParser {}

impl clap::builder::TypedValueParser for PaneDirectionParser {
    type Value = PaneDirection;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        use clap::error::*;

        let value = value
            .to_str()
            .ok_or_else(|| Error::raw(ErrorKind::InvalidUtf8, "value must be a utf8 string\n"))?;
        PaneDirection::direction_from_str(value)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, format!("{e}\n")))
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue>>> {
        Some(Box::new(
            PaneDirection::variants().iter().map(PossibleValue::new),
        ))
    }
}

use termwiz::escape::osc::{
    ITermDimension, ITermFileData, ITermProprietary, OperatingSystemCommand,
};

#[derive(Debug, Parser, Clone)]
struct ImgCatCommand {
    /// Specify the display width; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal width.
    #[arg(long = "width")]
    width: Option<ITermDimension>,
    /// Specify the display height; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal height.
    #[arg(long = "height")]
    height: Option<ITermDimension>,
    /// Do not respect the aspect ratio.  The default is to respect the aspect
    /// ratio
    #[arg(long = "no-preserve-aspect-ratio")]
    no_preserve_aspect_ratio: bool,
    /// The name of the image file to be displayed.
    /// If omitted, will attempt to read it from stdin.
    #[arg(value_parser, value_hint=ValueHint::FilePath)]
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
                width: self.width.unwrap_or_default(),
                height: self.height.unwrap_or_default(),
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

#[derive(Debug, Parser, Clone)]
struct SetCwdCommand {
    /// The directory to specify.
    /// If omitted, will use the current directory of the process itself.
    #[arg(value_parser, value_hint=ValueHint::DirPath)]
    cwd: Option<OsString>,

    /// The hostname to use in the constructed file:// URL.
    /// If omitted, the system hostname will be used.
    #[arg(value_parser, value_hint=ValueHint::Hostname)]
    host: Option<OsString>,
}

impl SetCwdCommand {
    fn run(&self) -> anyhow::Result<()> {
        let mut cwd = std::env::current_dir()?;
        if let Some(dir) = &self.cwd {
            cwd.push(dir);
        }

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

fn resolve_relative_cwd(cwd: Option<OsString>) -> anyhow::Result<Option<String>> {
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

    let opts = Opt::parse();
    config::common_init(
        opts.config_file.as_ref(),
        &opts.config_override,
        opts.skip_config,
    )
    .context("config::common_init")?;
    let config = config::configuration();
    config.update_ulimit()?;

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(_)
        | SubCommand::LsFonts(_)
        | SubCommand::ShowKeys(_)
        | SubCommand::Ssh(_)
        | SubCommand::Serial(_)
        | SubCommand::Connect(_) => delegate_to_gui(saver),
        SubCommand::ImageCat(cmd) => cmd.run(),
        SubCommand::SetCwd(cmd) => cmd.run(),
        SubCommand::Cli(cli) => run_cli(config, cli),
        SubCommand::Record(cmd) => cmd.run(config),
        SubCommand::Replay(cmd) => cmd.run(),
        SubCommand::ShellCompletion { shell } => {
            use clap::CommandFactory;
            let mut cmd = Opt::command();
            let name = cmd.get_name().to_string();
            generate_completion(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
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
        let res = cmd.exec();
        return Err(anyhow::anyhow!("failed to exec {cmd:?}: {res:?}"));
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
    rows: usize,
    cols: usize,
    /// Pixel width of the pane, if known (can be zero)
    pixel_width: usize,
    /// Pixel height of the pane, if known (can be zero)
    pixel_height: usize,
    /// dpi of the pane, if known (can be zero)
    dpi: u32,
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
    /// Cursor x coordinate from top left of non-scrollback pane area
    cursor_x: usize,
    /// Cursor y coordinate from top left of non-scrollback pane area
    cursor_y: usize,
    cursor_shape: termwiz::surface::CursorShape,
    cursor_visibility: termwiz::surface::CursorVisibility,
    /// Number of cols from the left of the tab area to the left of this pane
    left_col: usize,
    /// Number of rows from the top of the tab area to the top of this pane
    top_row: usize,
    tab_title: String,
    window_title: String,
}

impl CliListResultItem {
    fn from(pane: mux::tab::PaneEntry, tab_title: &str, window_title: &str) -> CliListResultItem {
        let mux::tab::PaneEntry {
            window_id,
            tab_id,
            pane_id,
            workspace,
            title,
            working_dir,
            cursor_pos,
            physical_top,
            left_col,
            top_row,
            size:
                TerminalSize {
                    rows,
                    cols,
                    pixel_width,
                    pixel_height,
                    dpi,
                },
            ..
        } = pane;

        CliListResultItem {
            window_id,
            tab_id,
            pane_id,
            workspace,
            size: CliListResultPtySize {
                rows,
                cols,
                pixel_width,
                pixel_height,
                dpi,
            },
            title,
            cwd: working_dir
                .as_ref()
                .map(|url| url.url.as_str())
                .unwrap_or("")
                .to_string(),
            cursor_x: cursor_pos.x,
            cursor_y: cursor_pos.y.saturating_sub(physical_top) as usize,
            cursor_shape: cursor_pos.shape,
            cursor_visibility: cursor_pos.visibility,
            left_col,
            top_row,
            tab_title: tab_title.to_string(),
            window_title: window_title.to_string(),
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

            for (tabroot, tab_title) in panes.tabs.into_iter().zip(panes.tab_titles.iter()) {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        let window_title = panes
                            .window_titles
                            .get(&entry.window_id)
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        output_items.push(CliListResultItem::from(
                            entry.clone(),
                            tab_title,
                            window_title,
                        ));
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
        CliSubCommand::MovePaneToNewTab {
            pane_id,
            window_id,
            new_window,
            workspace,
        } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;
            let window_id = if new_window {
                None
            } else {
                match window_id {
                    Some(w) => Some(w),
                    None => {
                        let panes = client.list_panes().await?;
                        let mut window_id = None;
                        'outer_move: for tabroot in panes.tabs {
                            let mut cursor = tabroot.into_tree().cursor();

                            loop {
                                if let Some(entry) = cursor.leaf_mut() {
                                    if entry.pane_id == pane_id {
                                        window_id.replace(entry.window_id);
                                        break 'outer_move;
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

            let moved = client
                .move_pane_to_new_tab(codec::MovePaneToNewTab {
                    pane_id,
                    window_id,
                    workspace_for_new_window: workspace,
                })
                .await?;

            log::debug!("{:?}", moved);
        }
        CliSubCommand::SplitPane {
            pane_id,
            cwd,
            prog,
            horizontal,
            left,
            right,
            top,
            bottom,
            top_level,
            cells,
            percent,
            move_pane_id,
        } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;

            let direction = if left || right || horizontal {
                SplitDirection::Horizontal
            } else if top || bottom {
                SplitDirection::Vertical
            } else {
                SplitDirection::Vertical
            };
            let target_is_second = !(left || top);
            let size = match (cells, percent) {
                (Some(c), _) => SplitSize::Cells(c),
                (_, Some(p)) => SplitSize::Percent(p),
                (None, None) => SplitSize::Percent(50),
            };

            let split_request = SplitRequest {
                direction,
                target_is_second,
                size,
                top_level,
            };

            let spawned = client
                .split_pane(codec::SplitPane {
                    pane_id,
                    split_request,
                    domain: config::keyassignment::SpawnTabDomain::CurrentPaneDomain,
                    command: if prog.is_empty() {
                        None
                    } else {
                        let builder = CommandBuilder::from_argv(prog);
                        Some(builder)
                    },
                    command_dir: resolve_relative_cwd(cwd)?,
                    move_pane_id,
                })
                .await?;

            log::debug!("{:?}", spawned);
            println!("{}", spawned.pane_id);
        }
        CliSubCommand::SendText {
            pane_id,
            text,
            no_paste,
        } => {
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

            if no_paste {
                client
                    .write_to_pane(codec::WriteToPane {
                        pane_id,
                        data: data.as_bytes().to_vec(),
                    })
                    .await?;
            } else {
                client
                    .send_paste(codec::SendPaste { pane_id, data })
                    .await?;
            }
        }
        CliSubCommand::GetText {
            pane_id,
            start_line,
            end_line,
            escapes,
        } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;

            let info = client
                .get_dimensions(codec::GetPaneRenderableDimensions { pane_id })
                .await?;

            let start_line = match start_line {
                None => info.dimensions.physical_top,
                Some(n) if n >= 0 => info.dimensions.physical_top + n as StableRowIndex,
                Some(n) => {
                    let line = info.dimensions.physical_top as isize + n as isize;
                    if line < info.dimensions.scrollback_top as isize {
                        info.dimensions.scrollback_top
                    } else {
                        line as StableRowIndex
                    }
                }
            };

            let end_line = match end_line {
                None => {
                    info.dimensions.physical_top + info.dimensions.viewport_rows as StableRowIndex
                }
                Some(n) if n >= 0 => info.dimensions.physical_top + n as StableRowIndex,
                Some(n) => {
                    let line = info.dimensions.physical_top as isize + n as isize;
                    if line < info.dimensions.scrollback_top as isize {
                        info.dimensions.scrollback_top
                    } else {
                        line as StableRowIndex
                    }
                }
            };

            let lines = client
                .get_lines(codec::GetLines {
                    pane_id: pane_id.into(),
                    lines: vec![start_line..end_line + 1],
                })
                .await?;

            let lines = lines
                .lines
                .extract_data()
                .0
                .into_iter()
                .map(|(_idx, line)| line)
                .collect();

            if escapes {
                println!("{}", lines_to_escapes(lines)?);
            } else {
                lines.iter().for_each(|line| println!("{}", line.as_str()));
            }
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

            let workspace = workspace
                .as_deref()
                .unwrap_or(
                    config
                        .default_workspace
                        .as_deref()
                        .unwrap_or(mux::DEFAULT_WORKSPACE),
                )
                .to_string();

            let size = config.initial_size(0);

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
                    command_dir: resolve_relative_cwd(cwd)?,
                    size,
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

            let mux = Arc::new(mux::Mux::new(None));
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
        CliSubCommand::ActivatePaneDirection { direction } => {
            let pane_id = resolve_pane_id(&client, None).await?;
            client
                .activate_pane_direction(codec::ActivatePaneDirection { pane_id, direction })
                .await?;
        }
        CliSubCommand::KillPane { pane_id } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;
            client.kill_pane(codec::KillPane { pane_id }).await?;
        }
        CliSubCommand::ActivatePane { pane_id } => {
            let pane_id = resolve_pane_id(&client, pane_id).await?;
            client
                .set_focused_pane_id(codec::SetFocusedPane { pane_id })
                .await?;
        }
        CliSubCommand::ActivateTab {
            tab_id,
            tab_relative,
            tab_index,
            no_wrap,
            pane_id,
        } => {
            let panes = client.list_panes().await?;

            let mut pane_id_to_tab_id = HashMap::new();
            let mut tab_id_to_active_pane_id = HashMap::new();
            let mut tabs_by_window = HashMap::new();
            let mut window_by_tab_id = HashMap::new();

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        pane_id_to_tab_id.insert(entry.pane_id, entry.tab_id);
                        if entry.is_active_pane {
                            tab_id_to_active_pane_id.insert(entry.tab_id, entry.pane_id);
                        }
                        window_by_tab_id.insert(entry.tab_id, entry.window_id);
                        let win = tabs_by_window
                            .entry(entry.window_id)
                            .or_insert_with(Vec::new);
                        if win.last().copied() != Some(entry.tab_id) {
                            win.push(entry.tab_id);
                        }
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }

            let tab_id = if let Some(tab_id) = tab_id {
                tab_id
            } else {
                // Find the current tab from the pane id
                let pane_id = resolve_pane_id(&client, pane_id).await?;
                let current_tab_id = pane_id_to_tab_id
                    .get(&pane_id)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve current tab"))?;
                let window = window_by_tab_id
                    .get(&current_tab_id)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve current window"))?;

                let tabs = tabs_by_window
                    .get(&window)
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve tabs for current window"))?;
                let max = tabs.len();
                anyhow::ensure!(max > 0, "window has no tabs!?");

                if let Some(tab_index) = tab_index {
                    // This logic is coupled with TermWindow::activate_tab
                    // If you update this, update that!
                    let tab_idx = if tab_index < 0 {
                        max.saturating_sub(tab_index.abs() as usize)
                    } else {
                        tab_index as usize
                    };

                    tabs.get(tab_idx)
                        .copied()
                        .ok_or_else(|| anyhow::anyhow!("tab index {tab_index} is invalid"))?
                } else if let Some(delta) = tab_relative {
                    // This logic is coupled with TermWindow::activate_tab_relative
                    // If you update this, update that!
                    let wrap = !no_wrap;
                    let active = tabs
                        .iter()
                        .position(|&tab_id| tab_id == current_tab_id)
                        .ok_or_else(|| anyhow::anyhow!("current tab is not in window!?"))?
                        as isize;

                    let tab = active + delta;
                    let tab_idx = if wrap {
                        let tab = if tab < 0 { max as isize + tab } else { tab };
                        (tab as usize % max) as isize
                    } else {
                        if tab < 0 {
                            0
                        } else if tab >= max as isize {
                            max as isize - 1
                        } else {
                            tab
                        }
                    };
                    tabs.get(tab_idx as usize)
                        .copied()
                        .ok_or_else(|| anyhow::anyhow!("tab index {tab_idx} is invalid"))?
                } else {
                    anyhow::bail!("impossible arguments!");
                }
            };

            // Now that we know which tab we want to activate, figure out
            // which pane will be the active pane
            let target_pane = tab_id_to_active_pane_id
                .get(&tab_id)
                .copied()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "could not determine which pane should be active for tab {tab_id}"
                    )
                })?;

            client
                .set_focused_pane_id(codec::SetFocusedPane {
                    pane_id: target_pane,
                })
                .await?;
        }
        CliSubCommand::SetTabTitle {
            tab_id,
            pane_id,
            title,
        } => {
            let panes = client.list_panes().await?;

            let mut pane_id_to_tab_id = HashMap::new();

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        pane_id_to_tab_id.insert(entry.pane_id, entry.tab_id);
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }

            let tab_id = if let Some(tab_id) = tab_id {
                tab_id
            } else {
                // Find the current tab from the pane id
                let pane_id = resolve_pane_id(&client, pane_id).await?;
                pane_id_to_tab_id
                    .get(&pane_id)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve current tab"))?
            };

            client
                .set_tab_title(codec::TabTitleChanged { tab_id, title })
                .await?;
        }
        CliSubCommand::SetWindowTitle {
            window_id,
            pane_id,
            title,
        } => {
            let panes = client.list_panes().await?;

            let mut pane_id_to_window_id = HashMap::new();

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        pane_id_to_window_id.insert(entry.pane_id, entry.window_id);
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }

            let window_id = if let Some(window_id) = window_id {
                window_id
            } else {
                // Find the current tab from the pane id
                let pane_id = resolve_pane_id(&client, pane_id).await?;
                pane_id_to_window_id
                    .get(&pane_id)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve current window"))?
            };

            client
                .set_window_title(codec::WindowTitleChanged { window_id, title })
                .await?;
        }
        CliSubCommand::RenameWorkspace {
            workspace,
            pane_id,
            new_workspace,
        } => {
            let panes = client.list_panes().await?;

            let mut pane_id_to_workspace = HashMap::new();

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        pane_id_to_workspace.insert(entry.pane_id, entry.workspace.to_string());
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }

            let old_workspace = if let Some(workspace) = workspace {
                workspace
            } else {
                // Find the current tab from the pane id
                let pane_id = resolve_pane_id(&client, pane_id).await?;
                pane_id_to_workspace
                    .get(&pane_id)
                    .ok_or_else(|| anyhow::anyhow!("unable to resolve current workspace"))?
                    .to_string()
            };

            client
                .rename_workspace(codec::RenameWorkspace {
                    old_workspace,
                    new_workspace,
                })
                .await?;
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
