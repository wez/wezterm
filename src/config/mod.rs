//! Configuration for the gui portion of the terminal

use crate::create_user_owned_dirs;
use crate::font::locator::FontLocatorSelection;
use crate::font::rasterizer::FontRasterizerSelection;
use crate::font::shaper::FontShaperSelection;
use crate::frontend::FrontEndSelection;
use crate::keyassignment::KeyAssignment;
use anyhow::{anyhow, bail, Context, Error};
use lazy_static::lazy_static;
use portable_pty::{CommandBuilder, PtySystemSelection};
use serde_derive::*;
use std;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use term;
use termwiz::hyperlink;
use termwiz::input::{KeyCode, Modifiers};
use toml;

mod color;
mod daemon;
mod font;
mod keys;
mod ssh;
mod terminal;
mod tls;
mod unix;
pub use color::*;
pub use daemon::*;
pub use font::*;
pub use keys::*;
pub use ssh::*;
pub use terminal::*;
pub use tls::*;
pub use unix::*;

lazy_static! {
    static ref HOME_DIR: PathBuf = dirs::home_dir().expect("can't find HOME dir");
    static ref RUNTIME_DIR: PathBuf = compute_runtime_dir().unwrap();
    static ref CONFIG: Configuration = Configuration::new();
}

/// Discard the current configuration and replace it with
/// the default configuration
#[allow(dead_code)]
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
        bail!("{}", error);
    }
    Ok(CONFIG.get())
}

struct ConfigInner {
    config: Arc<Config>,
    error: Option<String>,
    generation: usize,
    watcher: Option<notify::RecommendedWatcher>,
}

impl ConfigInner {
    fn new() -> Self {
        Self {
            config: Arc::new(Config::default_config()),
            error: None,
            generation: 0,
            watcher: None,
        }
    }

    fn watch_path(&mut self, path: PathBuf) {
        if self.watcher.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let watcher = notify::watcher(tx, Duration::from_millis(200)).unwrap();
            std::thread::spawn(move || {
                // block until we get an event
                use notify::DebouncedEvent;

                fn extract_path(event: DebouncedEvent) -> Option<PathBuf> {
                    match event {
                        // Defer acting until `Write`, otherwise we'll
                        // reload twice in quick succession
                        DebouncedEvent::NoticeWrite(_) => None,
                        // Likewise, defer processing a remove until after
                        // we've debounced the event.  That will give us
                        // time to pick up the new version of the config if
                        // the user's editor removes the file before writing
                        // out a new version.
                        DebouncedEvent::NoticeRemove(_) => None,
                        DebouncedEvent::Create(path)
                        | DebouncedEvent::Write(path)
                        | DebouncedEvent::Chmod(path)
                        | DebouncedEvent::Remove(path)
                        | DebouncedEvent::Rename(path, _) => Some(path),
                        DebouncedEvent::Error(_, path) => path,
                        DebouncedEvent::Rescan => None,
                    }
                }

                while let Ok(event) = rx.recv() {
                    log::trace!("event:{:?}", event);
                    if let Some(path) = extract_path(event) {
                        log::debug!("path {} changed, reload config", path.display());
                        reload();
                    }
                }
            });
            self.watcher.replace(watcher);
        }
        if let Some(watcher) = self.watcher.as_mut() {
            use notify::Watcher;
            watcher
                .watch(path, notify::RecursiveMode::NonRecursive)
                .ok();
        }
    }

    /// Attempt to load the user's configuration.
    /// On success, clear any error and replace the current
    /// configuration.
    /// On failure, retain the existing configuration but
    /// replace any captured error message.
    fn reload(&mut self) {
        match Config::load() {
            Ok((config, path)) => {
                self.config = Arc::new(config);
                self.error.take();
                self.generation += 1;
                log::debug!("Reloaded configuration! generation={}", self.generation);
                if let Some(path) = path {
                    self.watch_path(path);
                }
            }
            Err(err) => {
                let err = format!("{:#}", err);
                log::error!("While (re)loading configuration: {}", err);

                #[cfg(not(windows))]
                {
                    notify_rust::Notification::new()
                        .summary("Wezterm Configuration")
                        .body(&err)
                        // Stay on the screen until dismissed
                        .hint(notify_rust::NotificationHint::Resident(true))
                        // timeout isn't respected on macos
                        .timeout(0)
                        .show()
                        .ok();
                }

                #[cfg(windows)]
                {
                    use winrt_notification::Toast;

                    Toast::new(Toast::POWERSHELL_APP_ID)
                        .title("Wezterm Configuration")
                        .text1(&err)
                        .duration(winrt_notification::Duration::Long)
                        .show()
                        .ok();
                }

                self.error.replace(err);
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
            inner: Mutex::new(ConfigInner::new()),
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
    #[allow(dead_code)]
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

    /// When using FontKitXXX font systems, a set of directories to
    /// search ahead of the standard font locations for fonts.
    /// Relative paths are taken to be relative to the directory
    /// from which the config was loaded.
    #[serde(default)]
    pub font_dirs: Vec<PathBuf>,

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
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,

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
    pub font_locator: FontLocatorSelection,
    #[serde(default)]
    pub font_rasterizer: FontRasterizerSelection,
    #[serde(default)]
    pub font_shaper: FontShaperSelection,

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

    /// If false, do not try to use a Wayland protocol connection
    /// when starting the gui frontend, and instead use X11.
    /// This option is only considered on X11/Wayland systems and
    /// has no effect on macOS or Windows.
    /// The default is true.
    #[serde(default = "default_true")]
    pub enable_wayland: bool,
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
    pub fn load() -> Result<(Self, Option<PathBuf>), Error> {
        // Note that the directories crate has methods for locating project
        // specific config directories, but only returns one of them, not
        // multiple.  In addition, it spawns a lot of subprocesses,
        // so we do this bit "by-hand"
        let mut paths = vec![
            HOME_DIR
                .join(".config")
                .join("wezterm")
                .join("wezterm.toml"),
            HOME_DIR.join(".wezterm.toml"),
        ];
        if cfg!(windows) {
            // On Windows, a common use case is to maintain a thumb drive
            // with a set of portable tools that don't need to be installed
            // to run on a target system.  In that scenario, the user would
            // like to run with the config from their thumbdrive because
            // either the target system won't have any config, or will have
            // the config of another user.
            // So we prioritize that here: if there is a config in the same
            // dir as the executable that will take precedence.
            if let Ok(exe_name) = std::env::current_exe() {
                if let Some(exe_dir) = exe_name.parent() {
                    paths.insert(0, exe_dir.join("wezterm.toml"));
                }
            }
        }

        for p in &paths {
            log::trace!("consider config: {}", p.display());
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
                .with_context(|| format!("Error parsing TOML from {}", p.display()))?;

            // Compute but discard the key bindings here so that we raise any
            // problems earlier than we use them.
            let _ = cfg.key_bindings()?;
            return Ok((cfg.compute_extra_defaults(Some(p)), Some(p.to_path_buf())));
        }

        Ok((Self::default().compute_extra_defaults(None), None))
    }

    pub fn default_config() -> Self {
        Self::default().compute_extra_defaults(None)
    }

    pub fn key_bindings(&self) -> anyhow::Result<HashMap<(KeyCode, Modifiers), KeyAssignment>> {
        let mut map = HashMap::new();

        for k in &self.keys {
            let value = k.try_into()?;
            map.insert((k.key, k.mods), value);
        }

        Ok(map)
    }

    /// In some cases we need to compute expanded values based
    /// on those provided by the user.  This is where we do that.
    fn compute_extra_defaults(&self, config_path: Option<&Path>) -> Self {
        let mut cfg = self.clone();

        // Convert any relative font dirs to their config file relative locations
        if let Some(config_dir) = config_path.as_ref().and_then(|p| p.parent()) {
            for font_dir in &mut cfg.font_dirs {
                if !font_dir.is_absolute() {
                    let dir = config_dir.join(&font_dir);
                    *font_dir = dir;
                }
            }
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

fn default_scrollback_lines() -> usize {
    3500
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

    let home = dirs::home_dir().ok_or_else(|| anyhow!("can't find home dir"))?;
    Ok(home.join(".local/share/wezterm"))
}
