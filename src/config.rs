//! Configuration for the gui portion of the terminal

use crate::font::FontSystemSelection;
use crate::frontend::guicommon::host::KeyAssignment;
use crate::frontend::FrontEndSelection;
use crate::get_shell;
use failure::{bail, err_msg, format_err, Error, Fallible};
use lazy_static::lazy_static;
use portable_pty::{CommandBuilder, PtySystemSelection};
use serde::{Deserialize, Deserializer};
use serde_derive::*;
use std;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsStr;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use term;
use term::color::RgbColor;
use termwiz::hyperlink;
use termwiz::input::{KeyCode, Modifiers};
use toml;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// The font size, measured in points
    #[serde(default = "default_font_size")]
    pub font_size: f64,

    /// The DPI to assume
    #[serde(default = "default_dpi")]
    pub dpi: f64,

    /// The baseline font to use
    #[serde(default)]
    pub font: TextStyle,

    /// An optional set of style rules to select the font based
    /// on the cell attributes
    #[serde(default)]
    pub font_rules: Vec<StyleRule>,

    /// The color palette
    pub colors: Option<Palette>,

    /// How many lines of scrollback you want to retain
    pub scrollback_lines: Option<usize>,

    /// If no `prog` is specified on the command line, use this
    /// instead of running the user's shell.
    /// For example, to have `wezterm` always run `top` by default,
    /// you'd use this:
    ///
    /// ```
    /// default_prog = ["top"]
    /// ```
    ///
    /// `default_prog` is implemented as an array where the 0th element
    /// is the command to run and the rest of the elements are passed
    /// as the positional arguments to that command.
    pub default_prog: Option<Vec<String>>,

    #[serde(default = "default_hyperlink_rules")]
    pub hyperlink_rules: Vec<hyperlink::Rule>,

    /// What to set the TERM variable to
    #[serde(default = "default_term")]
    pub term: String,

    #[serde(default)]
    pub font_system: FontSystemSelection,

    #[serde(default)]
    pub front_end: FrontEndSelection,

    #[serde(default)]
    pub pty: PtySystemSelection,

    /// When using the MuxServer, this specifies the path to the unix
    /// domain socket to use to communicate with the mux server.
    pub mux_server_unix_domain_socket_path: Option<String>,

    /// When using the MuxServer with the NetListener, specifies
    /// the address and port combination on which it should listen
    pub mux_server_bind_address: Option<String>,

    /// When using the MuxServer with the NetListener, specifies
    /// the path to an x509 PEM encoded private key file
    pub mux_server_pem_private_key: Option<PathBuf>,

    /// When using the MuxServer with the NetListener, specifies
    /// the path to an x509 PEM encoded certificate file
    pub mux_server_pem_cert: Option<PathBuf>,

    /// When using the MuxServer with the NetListener, specifies
    /// the path to an x509 PEM encoded CA chain file
    pub mux_server_pem_ca: Option<PathBuf>,

    /// When using the mux client domain, identifies the host:port
    /// pair of the remote server.
    pub mux_server_remote_address: Option<String>,

    /// When using the mux client domain:
    /// the path to an x509 PEM encoded private key file
    pub mux_client_pem_private_key: Option<PathBuf>,

    /// When using the mux client domain:
    /// the path to an x509 PEM encoded certificate file
    pub mux_client_pem_cert: Option<PathBuf>,

    /// When using the mux client domain:
    /// the path to an x509 PEM encoded CA chain file
    pub mux_client_pem_ca: Option<PathBuf>,

    pub mux_pem_root_certs: Option<Vec<PathBuf>>,

    /// When using the mux client domain, explicitly control whether
    /// the client checks that the certificate presented by the
    /// server matches the hostname portion of mux_server_remote_address.
    /// The default is true.
    /// This option is made available for troubleshooting purposes and
    /// should not be used outside of a controlled environment as it
    /// weakens the security of the TLS channel.
    pub mux_client_accept_invalid_hostnames: Option<bool>,

    /// When connecting to a mux server, the hostname string that we
    /// expect to match against the common name field in the certificate
    /// presented by the server.  This defaults to the hostname portion
    /// of the `mux_server_bind_address` configuration and you should
    /// not normally need to override this value.
    pub mux_client_expected_cn: Option<String>,

    /// Constrains the rate at which output from a child command is
    /// processed and applied to the terminal model.
    /// This acts as a brake in the case of a command spewing a
    /// ton of output and allows for the UI to remain responsive
    /// so that you can hit CTRL-C to interrupt it if desired.
    /// The default value is 2MB/s.
    pub ratelimit_output_bytes_per_second: Option<u32>,

    /// Constrains the rate at which the multiplexer server will
    /// unilaterally push data to the client.
    /// This helps to avoid saturating the link between the client
    /// and server.
    /// Each time the screen is updated as a result of the child
    /// command outputting data (rather than in response to input
    /// from the client), the server considers whether to push
    /// the result to the client.
    /// That decision is throttled by this configuration value
    /// which has a default value of 10/s
    pub ratelimit_mux_output_pushes_per_second: Option<u32>,

    /// Constrain how often the mux server scans the terminal
    /// model to compute a diff to send to the mux client.
    /// The default value is 100/s
    pub ratelimit_mux_output_scans_per_second: Option<u32>,

    #[serde(default)]
    pub keys: Vec<Key>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Key {
    #[serde(deserialize_with = "de_keycode")]
    pub key: KeyCode,
    #[serde(deserialize_with = "de_modifiers")]
    pub mods: Modifiers,
    pub action: KeyAction,
    pub arg: Option<String>,
}

impl std::convert::TryInto<KeyAssignment> for &Key {
    type Error = Error;
    fn try_into(self) -> Result<KeyAssignment, Error> {
        Ok(match self.action {
            KeyAction::SpawnTab => KeyAssignment::SpawnTab,
            KeyAction::SpawnTabInCurrentTabDomain => KeyAssignment::SpawnTabInCurrentTabDomain,
            KeyAction::SpawnWindow => KeyAssignment::SpawnWindow,
            KeyAction::ToggleFullScreen => KeyAssignment::ToggleFullScreen,
            KeyAction::Copy => KeyAssignment::Copy,
            KeyAction::Paste => KeyAssignment::Paste,
            KeyAction::Hide => KeyAssignment::Hide,
            KeyAction::Show => KeyAssignment::Show,
            KeyAction::IncreaseFontSize => KeyAssignment::IncreaseFontSize,
            KeyAction::DecreaseFontSize => KeyAssignment::DecreaseFontSize,
            KeyAction::ResetFontSize => KeyAssignment::ResetFontSize,
            KeyAction::Nop => KeyAssignment::Nop,
            KeyAction::CloseCurrentTab => KeyAssignment::CloseCurrentTab,
            KeyAction::ActivateTab => KeyAssignment::ActivateTab(
                self.arg
                    .as_ref()
                    .ok_or_else(|| format_err!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::ActivateTabRelative => KeyAssignment::ActivateTabRelative(
                self.arg
                    .as_ref()
                    .ok_or_else(|| format_err!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::SendString => KeyAssignment::SendString(
                self.arg
                    .as_ref()
                    .ok_or_else(|| format_err!("missing arg for {:?}", self))?
                    .to_owned(),
            ),
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum KeyAction {
    SpawnTab,
    SpawnTabInCurrentTabDomain,
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    Paste,
    ActivateTabRelative,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ActivateTab,
    SendString,
    Nop,
    Hide,
    Show,
    CloseCurrentTab,
}

fn de_keycode<'de, D>(deserializer: D) -> Result<KeyCode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    macro_rules! m {
        ($($val:ident),* $(,)?) => {
            $(
            if s == stringify!($val) {
                return Ok(KeyCode::$val);
            }
            )*
        }
    }

    m!(
        Hyper,
        Super,
        Meta,
        Cancel,
        Backspace,
        Tab,
        Clear,
        Enter,
        Shift,
        Escape,
        LeftShift,
        RightShift,
        Control,
        LeftControl,
        RightControl,
        Alt,
        LeftAlt,
        RightAlt,
        Menu,
        LeftMenu,
        RightMenu,
        Pause,
        CapsLock,
        PageUp,
        PageDown,
        End,
        Home,
        LeftArrow,
        RightArrow,
        UpArrow,
        DownArrow,
        Select,
        Print,
        Execute,
        PrintScreen,
        Insert,
        Delete,
        Help,
        LeftWindows,
        RightWindows,
        Applications,
        Sleep,
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        Multiply,
        Add,
        Separator,
        Subtract,
        Decimal,
        Divide,
        NumLock,
        ScrollLock,
        BrowserBack,
        BrowserForward,
        BrowserRefresh,
        BrowserStop,
        BrowserSearch,
        BrowserFavorites,
        BrowserHome,
        VolumeMute,
        VolumeDown,
        VolumeUp,
        MediaNextTrack,
        MediaPrevTrack,
        MediaStop,
        MediaPlayPause,
        ApplicationLeftArrow,
        ApplicationRightArrow,
        ApplicationUpArrow,
        ApplicationDownArrow,
    );

    if s.len() > 1 && s.starts_with('F') {
        let num: u8 = s[1..].parse().map_err(|_| {
            serde::de::Error::custom(format!(
                "expected F<NUMBER> function key string, got: {}",
                s
            ))
        })?;
        return Ok(KeyCode::Function(num));
    }

    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        Ok(KeyCode::Char(chars[0]))
    } else {
        Err(serde::de::Error::custom(format!(
            "invalid KeyCode string {}",
            s
        )))
    }
}

fn de_modifiers<'de, D>(deserializer: D) -> Result<Modifiers, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let mut mods = Modifiers::NONE;
    for ele in s.split('|') {
        if ele == "SHIFT" {
            mods |= Modifiers::SHIFT;
        } else if ele == "ALT" || ele == "OPT" || ele == "META" {
            mods |= Modifiers::ALT;
        } else if ele == "CTRL" {
            mods |= Modifiers::CTRL;
        } else if ele == "SUPER" || ele == "CMD" || ele == "WIN" {
            mods |= Modifiers::SUPER;
        } else {
            return Err(serde::de::Error::custom(format!(
                "invalid modifier name {} in {}",
                ele, s
            )));
        }
    }
    Ok(mods)
}

fn default_hyperlink_rules() -> Vec<hyperlink::Rule> {
    vec![
        // URL with a protocol
        hyperlink::Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap(),
        // implicit mailto link
        hyperlink::Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
    ]
}

fn default_term() -> String {
    "xterm-256color".into()
}

fn default_font_size() -> f64 {
    11.0
}

fn default_dpi() -> f64 {
    96.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            dpi: default_dpi(),
            font: TextStyle::default(),
            font_rules: Vec::new(),
            font_system: FontSystemSelection::default(),
            front_end: FrontEndSelection::default(),
            pty: PtySystemSelection::default(),
            colors: None,
            scrollback_lines: None,
            hyperlink_rules: default_hyperlink_rules(),
            term: default_term(),
            default_prog: None,
            mux_server_unix_domain_socket_path: None,
            mux_server_bind_address: None,
            mux_server_pem_private_key: None,
            mux_server_pem_cert: None,
            mux_server_pem_ca: None,
            mux_server_remote_address: None,
            mux_client_pem_private_key: None,
            mux_client_pem_cert: None,
            mux_client_pem_ca: None,
            mux_client_accept_invalid_hostnames: None,
            mux_client_expected_cn: None,
            ratelimit_output_bytes_per_second: None,
            ratelimit_mux_output_pushes_per_second: None,
            ratelimit_mux_output_scans_per_second: None,
            mux_pem_root_certs: None,
            keys: vec![],
        }
    }
}

#[cfg(target_os = "macos")]
const FONT_FAMILY: &str = "Menlo";
#[cfg(windows)]
const FONT_FAMILY: &str = "Consolas";
#[cfg(all(not(target_os = "macos"), not(windows)))]
const FONT_FAMILY: &str = "monospace";

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct FontAttributes {
    /// The font family name
    pub family: String,
    /// Whether the font should be a bold variant
    pub bold: Option<bool>,
    /// Whether the font should be an italic variant
    pub italic: Option<bool>,
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: FONT_FAMILY.into(),
            bold: None,
            italic: None,
        }
    }
}

fn empty_font_attributes() -> Vec<FontAttributes> {
    Vec::new()
}

/// Represents textual styling.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct TextStyle {
    #[serde(default = "empty_font_attributes")]
    pub font: Vec<FontAttributes>,

    /// If set, when rendering text that is set to the default
    /// foreground color, use this color instead.  This is most
    /// useful in a `[[font_rules]]` section to implement changing
    /// the text color for eg: bold text.
    pub foreground: Option<RgbColor>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            foreground: None,
            font: vec![FontAttributes::default()],
        }
    }
}

impl TextStyle {
    /// Make a version of this style with bold enabled.
    fn make_bold(&self) -> Self {
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .map(|attr| {
                    let mut attr = attr.clone();
                    attr.bold = Some(true);
                    attr
                })
                .collect(),
        }
    }

    /// Make a version of this style with italic enabled.
    fn make_italic(&self) -> Self {
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .map(|attr| {
                    let mut attr = attr.clone();
                    attr.italic = Some(true);
                    attr
                })
                .collect(),
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::let_and_return))]
    pub fn font_with_fallback(&self) -> Vec<FontAttributes> {
        #[allow(unused_mut)]
        let mut font = self.font.clone();

        if font.is_empty() {
            // This can happen when migratin from the old fontconfig_pattern
            // configuration syntax; ensure that we have something likely
            // sane in the font configuration
            font.push(FontAttributes::default());
        }

        #[cfg(target_os = "macos")]
        font.push(FontAttributes {
            family: "Apple Color Emoji".into(),
            bold: None,
            italic: None,
        });
        #[cfg(target_os = "macos")]
        font.push(FontAttributes {
            family: "Apple Symbols".into(),
            bold: None,
            italic: None,
        });
        #[cfg(target_os = "macos")]
        font.push(FontAttributes {
            family: "Zapf Dingbats".into(),
            bold: None,
            italic: None,
        });
        #[cfg(windows)]
        font.push(FontAttributes {
            family: "Segoe UI Emoji".into(),
            bold: None,
            italic: None,
        });
        #[cfg(windows)]
        font.push(FontAttributes {
            family: "Segoe UI Symbol".into(),
            bold: None,
            italic: None,
        });

        font
    }
}

/// Defines a rule that can be used to select a `TextStyle` given
/// an input `CellAttributes` value.  The logic that applies the
/// matching can be found in src/font/mod.rs.  The concept is that
/// the user can specify something like this:
///
/// ```
/// [[font_rules]]
/// italic = true
/// font = { font = [{family = "Operator Mono SSm Lig", italic=true}]}
/// ```
///
/// The above is translated as: "if the `CellAttributes` have the italic bit
/// set, then use the italic style of font rather than the default", and
/// stop processing further font rules.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct StyleRule {
    /// If present, this rule matches when CellAttributes::intensity holds
    /// a value that matches this rule.  Valid values are "Bold", "Normal",
    /// "Half".
    pub intensity: Option<term::Intensity>,
    /// If present, this rule matches when CellAttributes::underline holds
    /// a value that matches this rule.  Valid values are "None", "Single",
    /// "Double".
    pub underline: Option<term::Underline>,
    /// If present, this rule matches when CellAttributes::italic holds
    /// a value that matches this rule.
    pub italic: Option<bool>,
    /// If present, this rule matches when CellAttributes::blink holds
    /// a value that matches this rule.
    pub blink: Option<term::Blink>,
    /// If present, this rule matches when CellAttributes::reverse holds
    /// a value that matches this rule.
    pub reverse: Option<bool>,
    /// If present, this rule matches when CellAttributes::strikethrough holds
    /// a value that matches this rule.
    pub strikethrough: Option<bool>,
    /// If present, this rule matches when CellAttributes::invisible holds
    /// a value that matches this rule.
    pub invisible: Option<bool>,

    /// When this rule matches, `font` specifies the styling to be used.
    pub font: TextStyle,
}

fn compute_runtime_dir() -> Result<PathBuf, Error> {
    if let Some(runtime) = dirs::runtime_dir() {
        return Ok(runtime.join("wezterm"));
    }

    let home = dirs::home_dir().ok_or_else(|| err_msg("can't find home dir"))?;
    Ok(home.join(".local/share/wezterm"))
}

lazy_static! {
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("can't find HOME dir");
    static ref RUNTIME_DIR: PathBuf = compute_runtime_dir().unwrap();
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        // Note that the directories crate has methods for locating project
        // specific config directories, but only returns one of them, not
        // multiple.  In addition, it spawns a lot of subprocesses,
        // so we do this bit "by-hand"
        let paths = [
            HOME_DIR
                .join(".config")
                .join("wezterm")
                .join("wezterm.toml"),
            HOME_DIR.join(".wezterm.toml"),
        ];

        for p in &paths {
            let mut file = match fs::File::open(p) {
                Ok(file) => file,
                Err(err) => match err.kind() {
                    std::io::ErrorKind::NotFound => continue,
                    _ => bail!("Error opening {}: {:?}", p.display(), err),
                },
            };

            let mut s = String::new();
            file.read_to_string(&mut s)?;

            let cfg: Self = toml::from_str(&s)
                .map_err(|e| format_err!("Error parsing TOML from {}: {:?}", p.display(), e))?;

            // Compute but discard the key bindings here so that we raise any
            // problems earlier than we use them.
            let _ = cfg.key_bindings()?;
            return Ok(cfg.compute_extra_defaults());
        }

        Ok(Self::default().compute_extra_defaults())
    }

    pub fn default_config() -> Self {
        Self::default().compute_extra_defaults()
    }

    pub fn key_bindings(&self) -> Fallible<HashMap<(KeyCode, Modifiers), KeyAssignment>> {
        let mut map = HashMap::new();

        for k in &self.keys {
            let value = k.try_into()?;
            map.insert((k.key, k.mods), value);
        }

        Ok(map)
    }

    /// In some cases we need to compute expanded values based
    /// on those provided by the user.  This is where we do that.
    fn compute_extra_defaults(&self) -> Self {
        let mut cfg = self.clone();

        if cfg.mux_server_unix_domain_socket_path.is_none() {
            cfg.mux_server_unix_domain_socket_path =
                RUNTIME_DIR.join("sock").to_str().map(str::to_owned);
        }

        if cfg.font_rules.is_empty() {
            // Expand out some reasonable default font rules
            let bold = self.font.make_bold();
            let italic = self.font.make_italic();
            let bold_italic = bold.make_italic();

            cfg.font_rules.push(StyleRule {
                italic: Some(true),
                font: italic,
                ..Default::default()
            });

            cfg.font_rules.push(StyleRule {
                intensity: Some(term::Intensity::Bold),
                font: bold,
                ..Default::default()
            });

            cfg.font_rules.push(StyleRule {
                italic: Some(true),
                intensity: Some(term::Intensity::Bold),
                font: bold_italic,
                ..Default::default()
            });
        }

        cfg
    }

    /// On macOS, we get launched from: eg: spotlight or alfred
    /// or the finder with whatever SHELL was set to at login time
    /// (which have been subsequently changed via `chsh`) and with
    /// a cwd=/.
    /// That feels a bit broken, so we follow the lead of
    /// Terminal.app and use `login -pf $USER` as the default
    /// program to run.
    /// This function computes and returns that command.
    /// We don't do this on Linux because the linux `login`
    /// program refuses to run except when started by root.
    #[cfg(target_os = "macos")]
    fn macos_login() -> Result<Vec<String>, Error> {
        let ent = unsafe { libc::getpwuid(libc::getuid()) };
        if ent.is_null() {
            bail!("unable to resolve my own uid");
        } else {
            let name = unsafe { std::ffi::CStr::from_ptr((*ent).pw_name) };
            let name = name.to_str().map(str::to_owned)?;
            Ok(vec!["login".to_owned(), "-pf".to_owned(), name])
        }
    }

    pub fn default_prog(&self) -> Result<Vec<String>, Error> {
        if let Some(prog) = self.default_prog.as_ref() {
            Ok(prog.clone())
        } else {
            #[cfg(target_os = "macos")]
            {
                if let Ok(login) = Self::macos_login() {
                    return Ok(login);
                }
            }
            Ok(vec![get_shell()?])
        }
    }

    pub fn build_prog(&self, prog: Option<Vec<&OsStr>>) -> Result<CommandBuilder, Error> {
        let mut cmd = match prog {
            Some(args) => {
                let mut args = args.iter();
                let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                cmd.args(args);
                cmd
            }
            None => {
                let prog = self.default_prog()?;
                let mut args = prog.iter();
                let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                cmd.args(args);
                cmd
            }
        };

        cmd.env("TERM", &self.term);

        Ok(cmd)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Palette {
    /// The text color to use when the attributes are reset to default
    pub foreground: Option<RgbColor>,
    /// The background color to use when the attributes are reset to default
    pub background: Option<RgbColor>,
    /// The color of the cursor
    pub cursor_fg: Option<RgbColor>,
    pub cursor_bg: Option<RgbColor>,
    /// The color of selected text
    pub selection_fg: Option<RgbColor>,
    pub selection_bg: Option<RgbColor>,
    /// A list of 8 colors corresponding to the basic ANSI palette
    pub ansi: Option<[RgbColor; 8]>,
    /// A list of 8 colors corresponding to bright versions of the
    /// ANSI palette
    pub brights: Option<[RgbColor; 8]>,
}

impl From<Palette> for term::color::ColorPalette {
    fn from(cfg: Palette) -> term::color::ColorPalette {
        let mut p = term::color::ColorPalette::default();
        macro_rules! apply_color {
            ($name:ident) => {
                if let Some($name) = cfg.$name {
                    p.$name = $name;
                }
            };
        }
        apply_color!(foreground);
        apply_color!(background);
        apply_color!(cursor_fg);
        apply_color!(cursor_bg);
        apply_color!(selection_fg);
        apply_color!(selection_bg);

        if let Some(ansi) = cfg.ansi {
            for (idx, col) in ansi.iter().enumerate() {
                p.colors.0[idx] = *col;
            }
        }
        if let Some(brights) = cfg.brights {
            for (idx, col) in brights.iter().enumerate() {
                p.colors.0[idx + 8] = *col;
            }
        }
        p
    }
}
