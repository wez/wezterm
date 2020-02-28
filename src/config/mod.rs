//! Configuration for the gui portion of the terminal

use crate::create_user_owned_dirs;
use crate::font::locator::FontLocatorSelection;
use crate::font::rasterizer::FontRasterizerSelection;
use crate::font::shaper::FontShaperSelection;
use crate::frontend::FrontEndSelection;
use crate::keyassignment::KeyAssignment;
use anyhow::{anyhow, bail, Context, Error};
use lazy_static::lazy_static;
use portable_pty::{CommandBuilder, PtySize};
use serde::Deserialize;
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
use termwiz::surface::CursorShape;
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
    pub static ref HOME_DIR: PathBuf = dirs::home_dir().expect("can't find HOME dir");
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
            const DELAY: Duration = Duration::from_millis(200);
            let watcher = notify::watcher(tx, DELAY).unwrap();
            std::thread::spawn(move || {
                // block until we get an event
                use notify::DebouncedEvent;

                fn extract_path(event: DebouncedEvent) -> Option<PathBuf> {
                    match event {
                        // Defer acting until `Write`, otherwise we'll
                        // reload twice in quick succession
                        DebouncedEvent::NoticeWrite(_) => None,
                        DebouncedEvent::Create(path)
                        | DebouncedEvent::Write(path)
                        | DebouncedEvent::Chmod(path)
                        | DebouncedEvent::Remove(path)
                        | DebouncedEvent::Rename(path, _) => Some(path),
                        DebouncedEvent::NoticeRemove(path) => {
                            // In theory, `notify` should deliver DebouncedEvent::Remove
                            // shortly after this, but it doesn't always do so.
                            // Let's just wait a bit and report the path changed
                            // for ourselves.
                            std::thread::sleep(DELAY);
                            Some(path)
                        }
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
                crate::termwiztermtab::show_configuration_error_message(&err);
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

    #[serde(default)]
    pub color_scheme_dirs: Vec<PathBuf>,

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

    /// Use a named color scheme rather than the palette specified
    /// by the colors setting.
    pub color_scheme: Option<String>,

    /// Named color schemes
    #[serde(default)]
    pub color_schemes: HashMap<String, Palette>,

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

    /// Specifies a map of environment variables that should be set
    /// when spawning commands in the local domain.
    /// This is not used when working with remote domains.
    #[serde(default)]
    pub set_environment_variables: HashMap<String, String>,

    /// Specifies the height of a new window, expressed in character cells.
    #[serde(default = "default_initial_rows")]
    pub initial_rows: u16,

    /// Specifies the width of a new window, expressed in character cells
    #[serde(default = "default_initial_cols")]
    pub initial_cols: u16,

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
    pub font_hinting: FontHinting,
    #[serde(default)]
    pub font_antialias: FontAntiAliasing,

    /// Specify the features to enable when using harfbuzz for font shaping.
    /// There is some light documentation here:
    /// <https://harfbuzz.github.io/shaping-opentype-features.html>
    /// but it boils down to allowing opentype feature names to be specified
    /// using syntax similar to the CSS font-feature-settings options:
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-feature-settings>.
    /// The OpenType spec lists a number of features here:
    /// <https://docs.microsoft.com/en-us/typography/opentype/spec/featurelist>
    ///
    /// Options of likely interest will be:
    ///
    /// * `calt` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-calt>
    /// * `clig` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-clig>
    ///
    /// If you want to disable ligatures in most fonts, then you may want to
    /// use a setting like this:
    ///
    /// ```toml
    /// harfbuzz_features = ["calt=0", "clig=0", "liga=0"]
    /// ```
    ///
    /// Some fonts make available extended options via stylistic sets.
    /// If you use the [Fira Code font](https://github.com/tonsky/FiraCode),
    /// it lists available stylistic sets here:
    /// <https://github.com/tonsky/FiraCode/wiki/How-to-enable-stylistic-sets>
    ///
    /// and you can set them in wezterm:
    ///
    /// ```toml
    /// # Use this for a zero with a dot rather than a line through it
    /// # when using the Fira Code font
    /// harfbuzz_features = ["zero"]
    /// ```
    #[serde(default = "default_harfbuzz_features")]
    pub harfbuzz_features: Vec<String>,

    #[serde(default)]
    pub front_end: FrontEndSelection,

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

    /// Constrains the rate at which the multiplexer client will
    /// speculatively fetch line data.
    /// This helps to avoid saturating the link between the client
    /// and server if the server is dumping a large amount of output
    /// to the client.
    #[serde(default = "default_ratelimit_line_prefetches_per_second")]
    pub ratelimit_mux_line_prefetches_per_second: u32,

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

    /// If true, hide the tab bar if the window only has a single tab.
    #[serde(default)]
    pub hide_tab_bar_if_only_one_tab: bool,

    #[serde(default)]
    pub enable_scroll_bar: bool,

    /// If false, do not try to use a Wayland protocol connection
    /// when starting the gui frontend, and instead use X11.
    /// This option is only considered on X11/Wayland systems and
    /// has no effect on macOS or Windows.
    /// The default is true.
    #[serde(default = "default_true")]
    pub enable_wayland: bool,

    /// Controls the amount of padding to use around the terminal cell area
    #[serde(default)]
    pub window_padding: WindowPadding,

    /// Specifies how often a blinking cursor transitions between visible
    /// and invisible, expressed in milliseconds.
    /// Setting this to 0 disables blinking.
    /// Note that this value is approximate due to the way that the system
    /// event loop schedulers manage timers; non-zero values will be at
    /// least the interval specified with some degree of slop.
    #[serde(default = "default_cursor_blink_rate")]
    pub cursor_blink_rate: u64,

    /// Specifies the default cursor style.  various escape sequences
    /// can override the default style in different situations (eg:
    /// an editor can change it depending on the mode), but this value
    /// controls how the cursor appears when it is reset to default.
    /// The default is `SteadyBlock`.
    /// Acceptable values are `SteadyBlock`, `BlinkingBlock`,
    /// `SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
    /// and `BlinkingBar`.
    #[serde(default)]
    pub default_cursor_style: DefaultCursorStyle,

    /// If non-zero, specifies the period (in seconds) at which various
    /// statistics are logged.  Note that there is a minimum period of
    /// 10 seconds.
    #[serde(default)]
    pub periodic_stat_logging: u64,

    /// If false, do not scroll to the bottom of the terminal when
    /// you send input to the terminal.
    /// The default is to scroll to the bottom when you send input
    /// to the terminal.
    #[serde(default = "default_true")]
    pub scroll_to_bottom_on_input: bool,

    #[serde(default = "default_true")]
    pub use_ime: bool,

    #[serde(default)]
    pub use_local_build_for_proxy: bool,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub enum DefaultCursorStyle {
    BlinkingBlock,
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}

impl Default for DefaultCursorStyle {
    fn default() -> Self {
        DefaultCursorStyle::SteadyBlock
    }
}

impl DefaultCursorStyle {
    pub fn effective_shape(self, shape: CursorShape) -> CursorShape {
        match shape {
            CursorShape::Default => match self {
                Self::BlinkingBlock => CursorShape::BlinkingBlock,
                Self::SteadyBlock => CursorShape::SteadyBlock,
                Self::BlinkingUnderline => CursorShape::BlinkingUnderline,
                Self::SteadyUnderline => CursorShape::SteadyUnderline,
                Self::BlinkingBar => CursorShape::BlinkingBar,
                Self::SteadyBar => CursorShape::SteadyBar,
            },
            _ => shape,
        }
    }
}

#[derive(Default, Deserialize, Clone, Copy, Debug)]
pub struct WindowPadding {
    #[serde(default)]
    pub left: u16,
    #[serde(default)]
    pub top: u16,
    #[serde(default)]
    pub right: u16,
    #[serde(default)]
    pub bottom: u16,
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
            HOME_DIR.join(".config").join("wezterm").join("wezterm.lua"),
            HOME_DIR
                .join(".config")
                .join("wezterm")
                .join("wezterm.toml"),
            HOME_DIR.join(".wezterm.lua"),
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
                    paths.insert(0, exe_dir.join("wezterm.lua"));
                    paths.insert(1, exe_dir.join("wezterm.toml"));
                }
            }
        }
        if let Some(path) = std::env::var_os("WEZTERM_CONFIG_FILE") {
            log::trace!("Note: WEZTERM_CONFIG_FILE is set in the environment");
            paths.insert(0, path.into());
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

            let cfg: Self;

            if p.extension() == Some(OsStr::new("toml")) {
                cfg = toml::from_str(&s)
                    .with_context(|| format!("Error parsing TOML from {}", p.display()))?;
            } else if p.extension() == Some(OsStr::new("lua")) {
                let lua = crate::scripting::make_lua_context(p)?;
                let config: mlua::Value = lua
                    .load(&s)
                    .set_name(p.to_string_lossy().as_bytes())?
                    .eval()?;
                cfg = crate::scripting::from_lua_value(config).with_context(|| {
                    format!(
                        "Error converting lua value returned by script {} to Config struct",
                        p.display()
                    )
                })?;
            } else {
                unreachable!();
            }

            // Compute but discard the key bindings here so that we raise any
            // problems earlier than we use them.
            let _ = cfg.key_bindings()?;

            std::env::set_var("WEZTERM_CONFIG_FILE", p);
            if let Some(dir) = p.parent() {
                std::env::set_var("WEZTERM_CONFIG_DIR", dir);
            }
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

        // Load any additional color schemes into the color_schemes map
        cfg.load_color_schemes(&cfg.compute_color_scheme_dirs())
            .ok();

        if let Some(scheme) = cfg.color_scheme.as_ref() {
            if !cfg.color_schemes.contains_key(scheme) {
                log::error!(
                    "Your configuration specifies \
                     color_scheme=\"{}\" but that scheme \
                     was not found",
                    scheme
                );
            }
        }

        cfg
    }

    fn compute_color_scheme_dirs(&self) -> Vec<PathBuf> {
        let mut paths = self.color_scheme_dirs.clone();
        paths.push(HOME_DIR.join(".config").join("wezterm").join("colors"));

        if let Ok(exe_name) = std::env::current_exe() {
            // If running out of the source tree our executable path will be
            // something like: `.../wezterm/target/release/wezterm`.
            // It takes 3 parent calls to reach the wezterm dir; if we get
            // there, get to the `assets/colors` dir.
            if let Some(colors_dir) = exe_name
                .parent()
                .and_then(|release| release.parent())
                .and_then(|target| target.parent())
                .map(|srcdir| srcdir.join("assets").join("colors"))
            {
                paths.push(colors_dir);
            }

            // If running out of an AppImage, resolve our installed colors
            // path relative to our binary location:
            // `/usr/bin/wezterm` -> `/usr/share/wezterm/colors`
            if let Some(colors_dir) = exe_name
                .parent()
                .and_then(|bin| bin.parent())
                .map(|usr| usr.join("share").join("wezterm").join("colors"))
            {
                paths.push(colors_dir);
            }
        }

        if cfg!(target_os = "macos") {
            if let Ok(exe_name) = std::env::current_exe() {
                if let Some(colors_dir) = exe_name
                    .parent()
                    .map(|srcdir| srcdir.join("Contents").join("Resources").join("colors"))
                {
                    paths.push(colors_dir);
                }
            }
        } else if cfg!(unix) {
            paths.push(PathBuf::from("/usr/share/wezterm/colors"));
        } else if cfg!(windows) {
            // See commentary re: portable tools above!
            if let Ok(exe_name) = std::env::current_exe() {
                if let Some(exe_dir) = exe_name.parent() {
                    paths.insert(0, exe_dir.join("colors"));
                }
            }
        }
        paths
    }

    fn load_color_schemes(&mut self, paths: &[PathBuf]) -> Result<(), Error> {
        fn extract_scheme_name(name: &str) -> Option<&str> {
            if name.ends_with(".toml") {
                let len = name.len();
                Some(&name[..len - 5])
            } else {
                None
            }
        }

        fn load_scheme(path: &Path) -> Result<ColorSchemeFile, Error> {
            let s = std::fs::read_to_string(path)?;
            let scheme: ColorSchemeFile = toml::from_str(&s).with_context(|| {
                format!("Error parsing color scheme TOML from {}", path.display())
            })?;
            Ok(scheme)
        }

        for colors_dir in paths {
            if let Ok(dir) = std::fs::read_dir(colors_dir) {
                for entry in dir {
                    if let Ok(entry) = entry {
                        if let Some(name) = entry.file_name().to_str() {
                            if let Some(scheme_name) = extract_scheme_name(name) {
                                if self.color_schemes.contains_key(scheme_name) {
                                    // This scheme has already been defined
                                    continue;
                                }

                                let path = entry.path();
                                if let Ok(scheme) = load_scheme(&path) {
                                    log::trace!(
                                        "Loaded color scheme `{}` from {}",
                                        scheme_name,
                                        path.display()
                                    );
                                    self.color_schemes
                                        .insert(scheme_name.to_string(), scheme.colors);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn initial_size(&self) -> PtySize {
        PtySize {
            rows: self.initial_rows,
            cols: self.initial_cols,
            pixel_width: 0,
            pixel_height: 0,
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

        self.apply_cmd_defaults(&mut cmd);

        Ok(cmd)
    }

    pub fn apply_cmd_defaults(&self, cmd: &mut CommandBuilder) {
        for (k, v) in &self.set_environment_variables {
            cmd.env(k, v);
        }
        cmd.env("TERM", &self.term);
    }
}

fn default_ratelimit_line_prefetches_per_second() -> u32 {
    10
}

fn default_ratelimit_output_bytes_per_second() -> u32 {
    400_000
}

fn default_true() -> bool {
    true
}

fn default_cursor_blink_rate() -> u64 {
    800
}

fn default_swap_backspace_and_delete() -> bool {
    // cfg!(target_os = "macos")
    // See: https://github.com/wez/wezterm/issues/88
    false
}

fn default_scrollback_lines() -> usize {
    3500
}

fn default_initial_rows() -> u16 {
    24
}

fn default_initial_cols() -> u16 {
    80
}

fn default_hyperlink_rules() -> Vec<hyperlink::Rule> {
    vec![
        // URL with a protocol
        hyperlink::Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap(),
        // implicit mailto link
        hyperlink::Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
    ]
}

fn default_harfbuzz_features() -> Vec<String> {
    ["kern", "liga", "clig"]
        .iter()
        .map(|&s| s.to_string())
        .collect()
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

pub fn pki_dir() -> anyhow::Result<PathBuf> {
    compute_runtime_dir().map(|d| d.join("pki"))
}

fn default_read_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_write_timeout() -> Duration {
    Duration::from_secs(60)
}
