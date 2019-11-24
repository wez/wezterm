//! Configuration for the gui portion of the terminal

use crate::create_user_owned_dirs;
use crate::font::FontSystemSelection;
use crate::frontend::FrontEndSelection;
use crate::keyassignment::KeyAssignment;
use failure::{bail, err_msg, format_err, Error, Fallible};
use lazy_static::lazy_static;
use portable_pty::{CommandBuilder, PtySystemSelection};
use serde_derive::*;
use std;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use term;
use termwiz::hyperlink;
use termwiz::input::{KeyCode, Modifiers};
use toml;

mod color;
mod daemon;
mod font;
mod keys;
mod ssh;
mod tls;
mod unix;
pub use color::*;
pub use daemon::*;
pub use font::*;
pub use keys::*;
pub use ssh::*;
pub use tls::*;
pub use unix::*;

lazy_static! {
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("can't find HOME dir");
    static ref RUNTIME_DIR: PathBuf = compute_runtime_dir().unwrap();
    static ref CONFIG: Configuration = Configuration::new();
}

/// Discard the current configuration and replace it with
/// the default configuration
pub fn use_default_configuration() {
    CONFIG.use_defaults();
}

/// Returns a handle to the current configuration
pub fn configuration() -> ConfigHandle {
    CONFIG.get()
}

pub fn reload() {
    CONFIG.reload();
}

/// If there was an error loading the preferred configuration,
/// return it, otherwise return the current configuration
pub fn configuration_result() -> Result<ConfigHandle, Error> {
    if let Some(error) = CONFIG.get_error() {
        failure::bail!("{}", error);
    }
    Ok(CONFIG.get())
}

struct ConfigInner {
    config: Arc<Config>,
    error: Option<String>,
    generation: usize,
}

impl ConfigInner {
    /// Attempt to load the user's configuration.
    /// On failure, capture the error message and load the
    /// default configuration instead.
    fn load() -> Self {
        match Config::load() {
            Ok(config) => Self {
                config: Arc::new(config),
                error: None,
                generation: 0,
            },
            Err(err) => Self {
                config: Arc::new(Config::default_config()),
                error: Some(err.to_string()),
                generation: 0,
            },
        }
    }

    /// Attempt to load the user's configuration.
    /// On success, clear any error and replace the current
    /// configuration.
    /// On failure, retain the existing configuration but
    /// replace any captured error message.
    fn reload(&mut self) {
        match Config::load() {
            Ok(config) => {
                self.config = Arc::new(config);
                self.error.take();
                self.generation += 1;
                log::error!("Reloaded configuration! generation={}", self.generation);
            }
            Err(err) => {
                log::error!("While reloading configuration: {}", err);
                self.error.replace(err.to_string());
            }
        }
    }

    /// Discard the current configuration and any recorded
    /// error message; replace them with the default
    /// configuration
    fn use_defaults(&mut self) {
        self.config = Arc::new(Config::default_config());
        self.error.take();
        self.generation += 1;
    }
}

pub struct Configuration {
    inner: Mutex<ConfigInner>,
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ConfigInner::load()),
        }
    }

    /// Returns the effective configuration.
    pub fn get(&self) -> ConfigHandle {
        let inner = self.inner.lock().unwrap();
        ConfigHandle {
            config: Arc::clone(&inner.config),
            generation: inner.generation,
        }
    }

    /// Reset the configuration to defaults
    pub fn use_defaults(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.use_defaults();
    }

    /// Reload the configuration
    pub fn reload(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.reload();
    }

    /// Returns a copy of any captured error message.
    /// The error message is not cleared.
    pub fn get_error(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.error.as_ref().cloned()
    }

    /// Returns any captured error message, and clears
    /// it from the config state.
    pub fn clear_error(&self) -> Option<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.error.take()
    }
}

#[derive(Clone, Debug)]
pub struct ConfigHandle {
    config: Arc<Config>,
    generation: usize,
}

impl ConfigHandle {
    /// Returns the generation number for the configuration,
    /// allowing consuming code to know whether the config
    /// has been reloading since they last derived some
    /// information from the configuration
    pub fn generation(&self) -> usize {
        self.generation
    }
}

impl std::ops::Deref for ConfigHandle {
    type Target = Config;
    fn deref(&self) -> &Config {
        &*self.config
    }
}

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

    /// The set of unix domains
    #[serde(default = "UnixDomain::default_unix_domains")]
    pub unix_domains: Vec<UnixDomain>,

    #[serde(default)]
    pub ssh_domains: Vec<SshDomain>,

    /// When running in server mode, defines configuration for
    /// each of the endpoints that we'll listen for connections
    #[serde(default)]
    pub tls_servers: Vec<TlsDomainServer>,

    /// The set of tls domains that we can connect to as a client
    #[serde(default)]
    pub tls_clients: Vec<TlsDomainClient>,

    /// Constrains the rate at which output from a child command is
    /// processed and applied to the terminal model.
    /// This acts as a brake in the case of a command spewing a
    /// ton of output and allows for the UI to remain responsive
    /// so that you can hit CTRL-C to interrupt it if desired.
    /// The default value is 200K/s.
    #[serde(default = "default_ratelimit_output_bytes_per_second")]
    pub ratelimit_output_bytes_per_second: u32,

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
    #[serde(default = "default_ratelimit_mux_output_pushes_per_second")]
    pub ratelimit_mux_output_pushes_per_second: u32,

    /// Constrain how often the mux server scans the terminal
    /// model to compute a diff to send to the mux client.
    /// The default value is 100/s
    #[serde(default = "default_ratelimit_mux_output_scans_per_second")]
    pub ratelimit_mux_output_scans_per_second: u32,

    #[serde(default)]
    pub keys: Vec<Key>,

    #[serde(default)]
    pub daemon_options: DaemonOptions,

    /// If set to true, send the system specific composed key when
    /// the ALT key is held down.  If set to false (the default)
    /// then send the key with the ALT modifier (this is typically
    /// encoded as ESC followed by the key).
    #[serde(default)]
    pub send_composed_key_when_alt_is_pressed: bool,

    /// If true, the `Backspace` and `Delete` keys generate `Delete` and `Backspace`
    /// keypresses, respectively, rather than their normal keycodes.
    /// On macOS the default for this is true because its Backspace key
    /// is labeled as Delete and things are backwards.
    #[serde(default = "default_swap_backspace_and_delete")]
    pub swap_backspace_and_delete: bool,

    /// If true, display the tab bar UI at the top of the window.
    /// The tab bar shows the titles of the tabs and which is the
    /// active tab.  Clicking on a tab activates it.
    #[serde(default = "default_true")]
    pub enable_tab_bar: bool,
}

impl Default for Config {
    fn default() -> Self {
        // Ask serde to provide the defaults based on the attributes
        // specified in the struct so that we don't have to repeat
        // the same thing in a different form down here
        toml::from_str("").unwrap()
    }
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
                    _ => bail!("Error opening {}: {}", p.display(), err),
                },
            };

            let mut s = String::new();
            file.read_to_string(&mut s)?;

            let cfg: Self = toml::from_str(&s)
                .map_err(|e| format_err!("Error parsing TOML from {}: {}", p.display(), e))?;

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

    pub fn build_prog(&self, prog: Option<Vec<&OsStr>>) -> Result<CommandBuilder, Error> {
        let mut cmd = match prog {
            Some(args) => {
                let mut args = args.iter();
                let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                cmd.args(args);
                cmd
            }
            None => {
                if let Some(prog) = self.default_prog.as_ref() {
                    let mut args = prog.iter();
                    let mut cmd = CommandBuilder::new(args.next().expect("executable name"));
                    cmd.args(args);
                    cmd
                } else {
                    CommandBuilder::new_default_prog()
                }
            }
        };

        cmd.env("TERM", &self.term);

        Ok(cmd)
    }
}

fn default_ratelimit_mux_output_scans_per_second() -> u32 {
    100
}

fn default_ratelimit_mux_output_pushes_per_second() -> u32 {
    10
}

fn default_ratelimit_output_bytes_per_second() -> u32 {
    200_000
}

fn default_true() -> bool {
    true
}

fn default_swap_backspace_and_delete() -> bool {
    cfg!(target_os = "macos")
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
    10.0
}

fn default_dpi() -> f64 {
    96.0
}

fn compute_runtime_dir() -> Result<PathBuf, Error> {
    if let Some(runtime) = dirs::runtime_dir() {
        return Ok(runtime.join("wezterm"));
    }

    let home = dirs::home_dir().ok_or_else(|| err_msg("can't find home dir"))?;
    Ok(home.join(".local/share/wezterm"))
}
