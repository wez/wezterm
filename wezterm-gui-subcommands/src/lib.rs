use config::FontLocatorSelection;
use config::FontRasterizerSelection;
use config::FontShaperSelection;
use config::{FrontEndSelection, SshParameters};
use std::ffi::OsString;
use structopt::StructOpt;

/// Helper for parsing config overrides
pub fn name_equals_value(arg: &str) -> Result<(String, String), String> {
    if let Some(eq) = arg.find('=') {
        let (left, right) = arg.split_at(eq);
        let left = left.trim();
        let right = right[1..].trim();
        if left.is_empty() || right.is_empty() {
            return Err(format!(
                "Got empty name/value `{}`; expected name=value",
                arg
            ));
        }
        Ok((left.to_string(), right.to_string()))
    } else {
        Err(format!("Expected name=value, but got {}", arg))
    }
}

#[derive(Debug, StructOpt, Default, Clone)]
pub struct StartCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    pub front_end: Option<FrontEndSelection>,

    #[structopt(
        long = "font-locator",
        possible_values = &FontLocatorSelection::variants(),
        case_insensitive = true
    )]
    pub font_locator: Option<FontLocatorSelection>,

    #[structopt(
        long = "font-rasterizer",
        possible_values = &FontRasterizerSelection::variants(),
        case_insensitive = true
    )]
    pub font_rasterizer: Option<FontRasterizerSelection>,

    #[structopt(
        long = "font-shaper",
        possible_values = &FontShaperSelection::variants(),
        case_insensitive = true
    )]
    pub font_shaper: Option<FontShaperSelection>,

    /// If true, do not connect to domains marked as connect_automatically
    /// in your wezterm.toml configuration file.
    #[structopt(long = "no-auto-connect")]
    pub no_auto_connect: bool,

    /// Specify the current working directory for the initially
    /// spawned program
    #[structopt(long = "cwd", parse(from_os_str))]
    pub cwd: Option<OsString>,

    /// Override the default windowing system class.
    /// The default is "org.wezfurlong.wezterm".
    /// Under X11 and Windows this changes the window class.
    /// Under Wayland this changes the app_id.
    /// This changes the class for all windows spawned by this
    /// instance of wezterm, including error, update and ssh
    /// authentication dialogs.
    #[structopt(long = "class")]
    pub class: Option<String>,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm start -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    pub prog: Vec<OsString>,
}

#[derive(Debug, StructOpt, Clone)]
pub struct SshCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    pub front_end: Option<FrontEndSelection>,

    /// Specifies the remote system using the form:
    /// `[username@]host[:port]`.
    /// If `username@` is omitted, then your local $USER is used
    /// instead.
    /// If `:port` is omitted, then the standard ssh port (22) is
    /// used instead.
    pub user_at_host_and_port: SshParameters,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm ssh user@host -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    pub prog: Vec<OsString>,
}

#[derive(Debug, StructOpt, Clone)]
pub struct SerialCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    pub front_end: Option<FrontEndSelection>,

    /// Set the baud rate.  The default is 9600 baud.
    #[structopt(long = "baud")]
    pub baud: Option<usize>,

    /// Specifies the serial device name.
    /// On Windows systems this can be a name like `COM0`.
    /// On posix systems this will be something like `/dev/ttyUSB0`
    #[structopt(parse(from_os_str))]
    pub port: OsString,
}

#[derive(Debug, StructOpt, Clone)]
pub struct ConnectCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    pub front_end: Option<FrontEndSelection>,

    /// Name of the multiplexer domain section from the configuration
    /// to which you'd like to connect
    pub domain_name: String,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm start -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    pub prog: Vec<OsString>,
}
