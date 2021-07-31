//! Configuration for the gui portion of the terminal

use crate::keyassignment::{KeyAssignment, MouseEventTrigger, SpawnCommand};
use anyhow::{anyhow, bail, Context, Error};
use lazy_static::lazy_static;
use luahelper::impl_lua_conversion;
use mlua::Lua;
use portable_pty::{CommandBuilder, PtySize};
use serde::{Deserialize, Deserializer, Serialize};
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::DirBuilder;
use std::io::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use termwiz::hyperlink;
use termwiz::surface::CursorShape;
use toml;
use wezterm_input_types::{KeyCode, Modifiers, WindowDecorations};

mod color;
mod daemon;
mod font;
mod frontend;
pub mod keyassignment;
mod keys;
pub mod lua;
mod ssh;
mod terminal;
mod tls;
mod unix;
mod version;

pub use color::*;
pub use daemon::*;
pub use font::*;
pub use frontend::*;
pub use keys::*;
pub use ssh::*;
pub use terminal::*;
pub use tls::*;
pub use unix::*;
pub use version::*;

type LuaFactory = fn(&Path) -> anyhow::Result<Lua>;
type ErrorCallback = fn(&str);

lazy_static! {
    pub static ref HOME_DIR: PathBuf = dirs_next::home_dir().expect("can't find HOME dir");
    pub static ref CONFIG_DIR: PathBuf = xdg_config_home();
    pub static ref RUNTIME_DIR: PathBuf = compute_runtime_dir().unwrap();
    static ref CONFIG: Configuration = Configuration::new();
    static ref CONFIG_FILE_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);
    static ref CONFIG_OVERRIDES: Mutex<Vec<(String, String)>> = Mutex::new(vec![]);
    static ref MAKE_LUA: Mutex<Option<LuaFactory>> = Mutex::new(Some(lua::make_lua_context));
    static ref SHOW_ERROR: Mutex<Option<ErrorCallback>> =
        Mutex::new(Some(|e| log::error!("{}", e)));
    static ref LUA_PIPE: LuaPipe = LuaPipe::new();
    static ref COLOR_SCHEMES: HashMap<String, Palette> = build_default_schemes();
}

thread_local! {
    static LUA_CONFIG: RefCell<Option<LuaConfigState>> = RefCell::new(None);
}

pub fn build_default_schemes() -> HashMap<String, Palette> {
    let mut color_schemes = HashMap::new();
    for (scheme_name, data) in SCHEMES.iter() {
        let scheme_name = scheme_name.to_string();
        let scheme: ColorSchemeFile = toml::from_str(data).unwrap();
        color_schemes.insert(scheme_name, scheme.colors);
    }
    color_schemes
}

struct LuaPipe {
    sender: Sender<mlua::Lua>,
    receiver: Receiver<mlua::Lua>,
}
impl LuaPipe {
    pub fn new() -> Self {
        let (sender, receiver) = smol::channel::unbounded();
        Self { sender, receiver }
    }
}

/// The implementation is only slightly crazy...
/// `Lua` is Send but !Sync.
/// We take care to reference this only from the main thread of
/// the application.
/// We also need to take care to keep this `lua` alive if a long running
/// future is outstanding while a config reload happens.
/// We have to use `Rc` to manage its lifetime, but due to some issues
/// with rust's async lifetime tracking we need to indirectly schedule
/// some of the futures to avoid it thinking that the generated future
/// in the async block needs to be Send.
///
/// A further complication is that config reloading tends to happen in
/// a background filesystem watching thread.
///
/// The result of all these constraints is that the LuaPipe struct above
/// is used as a channel to transport newly loaded lua configs to the
/// main thread.
///
/// The main thread pops the loaded configs to obtain the latest one
/// and updates LuaConfigState
struct LuaConfigState {
    lua: Option<Rc<mlua::Lua>>,
}

impl LuaConfigState {
    /// Consume any lua contexts sent to us via the
    /// config loader until we end up with the most
    /// recent one being referenced by LUA_CONFIG.
    fn update_to_latest(&mut self) {
        while let Ok(lua) = LUA_PIPE.receiver.try_recv() {
            self.lua.replace(Rc::new(lua));
        }
    }

    /// Take a reference on the latest generation of the lua context
    fn get_lua(&self) -> Option<Rc<mlua::Lua>> {
        self.lua.as_ref().map(|lua| Rc::clone(lua))
    }
}

pub fn designate_this_as_the_main_thread() {
    LUA_CONFIG.with(|lc| {
        let mut lc = lc.borrow_mut();
        if lc.is_none() {
            lc.replace(LuaConfigState { lua: None });
        }
    });
}

#[must_use = "Cancels the subscription when dropped"]
pub struct ConfigSubscription(usize);

impl Drop for ConfigSubscription {
    fn drop(&mut self) {
        CONFIG.unsub(self.0);
    }
}

pub fn subscribe_to_config_reload<F>(subscriber: F) -> ConfigSubscription
where
    F: Fn() -> bool + 'static + Send,
{
    ConfigSubscription(CONFIG.subscribe(subscriber))
}

/// Spawn a future that will run with an optional Lua state from the most
/// recently loaded lua configuration.
/// The `func` argument is passed the lua state and must return a Future.
///
/// This function MUST only be called from the main thread.
/// In exchange for the caller checking for this, the parameters to
/// this method are not required to be Send.
///
/// Calling this function from a secondary thread will panic.
/// You should use `with_lua_config` if you are triggering a
/// call from a secondary thread.
pub async fn with_lua_config_on_main_thread<F, RETF, RET>(func: F) -> anyhow::Result<RET>
where
    F: FnOnce(Option<Rc<mlua::Lua>>) -> RETF,
    RETF: Future<Output = anyhow::Result<RET>>,
{
    let lua = LUA_CONFIG.with(|lc| {
        let mut lc = lc.borrow_mut();
        let lc = lc.as_mut().expect(
            "with_lua_config_on_main_thread not called
             from main thread, use with_lua_config instead!",
        );
        lc.update_to_latest();
        lc.get_lua()
    });

    func(lua).await
}

pub fn run_immediate_with_lua_config<F, RET>(func: F) -> anyhow::Result<RET>
where
    F: FnOnce(Option<Rc<mlua::Lua>>) -> anyhow::Result<RET>,
{
    let lua = LUA_CONFIG.with(|lc| {
        let mut lc = lc.borrow_mut();
        let lc = lc.as_mut().expect(
            "with_lua_config_on_main_thread not called
             from main thread, use with_lua_config instead!",
        );
        lc.update_to_latest();
        lc.get_lua()
    });

    func(lua)
}

fn schedule_with_lua<F, RETF, RET>(func: F) -> promise::spawn::Task<anyhow::Result<RET>>
where
    F: 'static,
    RET: 'static,
    F: Fn(Option<Rc<mlua::Lua>>) -> RETF,
    RETF: Future<Output = anyhow::Result<RET>>,
{
    promise::spawn::spawn(async move { with_lua_config_on_main_thread(func).await })
}

/// Spawn a future that will run with an optional Lua state from the most
/// recently loaded lua configuration.
/// The `func` argument is passed the lua state and must return a Future.
pub async fn with_lua_config<F, RETF, RET>(func: F) -> anyhow::Result<RET>
where
    F: Fn(Option<Rc<mlua::Lua>>) -> RETF,
    RETF: Future<Output = anyhow::Result<RET>> + Send + 'static,
    F: Send + 'static,
    RET: Send + 'static,
{
    promise::spawn::spawn_into_main_thread(async move { schedule_with_lua(func).await }).await
}

pub fn assign_lua_factory(make_lua_context: LuaFactory) {
    let mut factory = MAKE_LUA.lock().unwrap();
    factory.replace(make_lua_context);
}

fn make_lua_context(path: &Path) -> anyhow::Result<Lua> {
    let factory = MAKE_LUA.lock().unwrap();
    match factory.as_ref() {
        Some(f) => f(path),
        None => anyhow::bail!("assign_lua_factory has not been called"),
    }
}

fn default_config_with_overrides_applied() -> anyhow::Result<Config> {
    // Cause the default config to be re-evaluated with the overrides applied
    let lua = make_lua_context(Path::new("override"))?;
    let table = mlua::Value::Table(lua.create_table()?);
    let config = Config::apply_overrides_to(&lua, table)?;

    let cfg: Config = luahelper::from_lua_value(config)
        .context("Error converting lua value from overrides to Config struct")?;
    // Compute but discard the key bindings here so that we raise any
    // problems earlier than we use them.
    let _ = cfg.key_bindings();

    Ok(cfg)
}

pub fn common_init(
    config_file: Option<&OsString>,
    overrides: &[(String, String)],
    skip_config: bool,
) {
    if let Some(config_file) = config_file {
        set_config_file_override(Path::new(config_file));
    }
    set_config_overrides(overrides);
    if !skip_config {
        reload();
    } else if !overrides.is_empty() {
        match default_config_with_overrides_applied() {
            Ok(cfg) => CONFIG.use_this_config(cfg),
            Err(err) => {
                log::error!(
                    "Error while applying command line \
                     configuration overrides:\n{:#}",
                    err
                );
                std::process::exit(1);
            }
        }
    }
}

pub fn assign_error_callback(cb: ErrorCallback) {
    let mut factory = SHOW_ERROR.lock().unwrap();
    factory.replace(cb);
}

pub fn show_error(err: &str) {
    let factory = SHOW_ERROR.lock().unwrap();
    if let Some(cb) = factory.as_ref() {
        cb(err)
    }
}

include!(concat!(env!("OUT_DIR"), "/scheme_data.rs"));

pub fn create_user_owned_dirs(p: &Path) -> anyhow::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    {
        builder.mode(0o700);
    }

    builder.create(p)?;
    Ok(())
}

fn xdg_config_home() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME").map(|s| PathBuf::from(s).join("wezterm")) {
        Some(p) => p,
        None => HOME_DIR.join(".config").join("wezterm"),
    }
}

pub fn set_config_file_override(path: &Path) {
    CONFIG_FILE_OVERRIDE
        .lock()
        .unwrap()
        .replace(path.to_path_buf());
}

pub fn set_config_overrides(items: &[(String, String)]) {
    *CONFIG_OVERRIDES.lock().unwrap() = items.to_vec();
}

/// Discard the current configuration and replace it with
/// the default configuration
pub fn use_default_configuration() {
    CONFIG.use_defaults();
}

/// Use a config that doesn't depend on the user's
/// environment and is suitable for unit testing
pub fn use_test_configuration() {
    CONFIG.use_test();
}

pub fn use_this_configuration(config: Config) {
    CONFIG.use_this_config(config);
}

/// Returns a handle to the current configuration
pub fn configuration() -> ConfigHandle {
    CONFIG.get()
}

/// Returns a version of the config (loaded from the config file)
/// with some field overridden based on the supplied overrides object.
pub fn overridden_config(overrides: &serde_json::Value) -> Result<ConfigHandle, Error> {
    CONFIG.overridden(overrides)
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
    subscribers: HashMap<usize, Box<dyn Fn() -> bool + Send>>,
}

impl ConfigInner {
    fn new() -> Self {
        Self {
            config: Arc::new(Config::default_config()),
            error: None,
            generation: 0,
            watcher: None,
            subscribers: HashMap::new(),
        }
    }

    fn subscribe<F>(&mut self, subscriber: F) -> usize
    where
        F: Fn() -> bool + 'static + Send,
    {
        static SUB_ID: AtomicUsize = AtomicUsize::new(0);
        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
        self.subscribers.insert(sub_id, Box::new(subscriber));
        sub_id
    }

    fn unsub(&mut self, sub_id: usize) {
        self.subscribers.remove(&sub_id);
    }

    fn notify(&mut self) {
        self.subscribers.retain(|_, notify| notify());
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
            Ok(LoadedConfig {
                config,
                file_name,
                lua,
            }) => {
                self.config = Arc::new(config);
                self.error.take();
                self.generation += 1;

                // If we loaded a user config, publish this latest version of
                // the lua state to the LUA_PIPE.  This allows a subsequent
                // call to `with_lua_config` to reference this lua context
                // even though we are (probably) resolving this from a background
                // reloading thread.
                lua.and_then(|lua| {
                    if self.config.automatically_reload_config {
                        let watch_path: String = lua.globals().get("watch_path").ok()?;
                        for path in watch_path.split(";") {
                            log::debug!("watching path: {}", &path);
                            self.watch_path(PathBuf::from(path));
                        }
                    }
                    LUA_PIPE.sender.try_send(lua).ok();
                    Some(())
                })
                .or_else(|| {
                    if self.config.automatically_reload_config {
                        Some(self.watch_path(file_name?))
                    } else {
                        None
                    }
                });

                log::debug!("Reloaded configuration! generation={}", self.generation);
                self.notify();
            }
            Err(err) => {
                let err = format!("{:#}", err);
                if self.generation > 0 {
                    // Only generate the message for an actual reload
                    show_error(&err);
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

    fn use_this_config(&mut self, cfg: Config) {
        self.config = Arc::new(cfg);
        self.error.take();
        self.generation += 1;
    }

    fn overridden(&mut self, overrides: &serde_json::Value) -> Result<ConfigHandle, Error> {
        let config = Config::load_with_overrides(overrides)?;
        Ok(ConfigHandle {
            config: Arc::new(config.config),
            generation: self.generation,
        })
    }

    fn use_test(&mut self) {
        let mut config = Config::default_config();
        config.font_locator = FontLocatorSelection::ConfigDirsOnly;
        let exe_name = std::env::current_exe().unwrap();
        let exe_dir = exe_name.parent().unwrap();
        config.font_dirs.push(exe_dir.join("../../../assets/fonts"));
        // If we're building for a specific target, the dir
        // level is one deeper.
        #[cfg(target_os = "macos")]
        config
            .font_dirs
            .push(exe_dir.join("../../../../assets/fonts"));
        // Specify the same DPI used on non-mac systems so
        // that we have consistent values regardless of the
        // operating system that we're running tests on
        config.dpi.replace(96.0);
        self.config = Arc::new(config);
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

    /// Subscribe to config reload events
    fn subscribe<F>(&self, subscriber: F) -> usize
    where
        F: Fn() -> bool + 'static + Send,
    {
        let mut inner = self.inner.lock().unwrap();
        inner.subscribe(subscriber)
    }

    fn unsub(&self, sub_id: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.unsub(sub_id);
    }

    /// Reset the configuration to defaults
    pub fn use_defaults(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.use_defaults();
    }

    fn use_this_config(&self, cfg: Config) {
        let mut inner = self.inner.lock().unwrap();
        inner.use_this_config(cfg);
    }

    fn overridden(&self, overrides: &serde_json::Value) -> Result<ConfigHandle, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.overridden(overrides)
    }

    /// Use a config that doesn't depend on the user's
    /// environment and is suitable for unit testing
    pub fn use_test(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.use_test();
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

/// Deserialize either an integer or a float as a float
fn de_number<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct Number;

    impl<'de> serde::de::Visitor<'de> for Number {
        type Value = f64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("f64 or i64")
        }

        fn visit_f64<E>(self, value: f64) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<f64, E>
        where
            E: serde::de::Error,
        {
            Ok(value as f64)
        }
    }

    deserializer.deserialize_any(Number)
}

/// Behavior when the program spawned by wezterm terminates
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum ExitBehavior {
    /// Close the associated pane
    Close,
    /// Close the associated pane if the process was successful
    CloseOnCleanExit,
    /// Hold the pane until it is explicitly closed
    Hold,
}

impl Default for ExitBehavior {
    fn default() -> Self {
        ExitBehavior::CloseOnCleanExit
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// The font size, measured in points
    #[serde(default = "default_font_size", deserialize_with = "de_number")]
    pub font_size: f64,

    #[serde(default = "default_one_point_oh_f64")]
    pub line_height: f64,

    #[serde(default)]
    pub allow_square_glyphs_to_overflow_width: AllowSquareGlyphOverflow,

    #[serde(default)]
    pub window_decorations: WindowDecorations,

    /// When using FontKitXXX font systems, a set of directories to
    /// search ahead of the standard font locations for fonts.
    /// Relative paths are taken to be relative to the directory
    /// from which the config was loaded.
    #[serde(default)]
    pub font_dirs: Vec<PathBuf>,

    #[serde(default)]
    pub color_scheme_dirs: Vec<PathBuf>,

    /// The DPI to assume
    pub dpi: Option<f64>,

    /// The baseline font to use
    #[serde(default)]
    pub font: TextStyle,

    /// An optional set of style rules to select the font based
    /// on the cell attributes
    #[serde(default)]
    pub font_rules: Vec<StyleRule>,

    /// When true (the default), PaletteIndex 0-7 are shifted to
    /// bright when the font intensity is bold.  The brightening
    /// doesn't apply to text that is the default color.
    #[serde(default = "default_true")]
    pub bold_brightens_ansi_colors: bool,

    /// The color palette
    pub colors: Option<Palette>,

    #[serde(default)]
    pub window_frame: WindowFrameConfig,

    #[serde(default)]
    pub tab_bar_style: TabBarStyle,

    #[serde(skip)]
    pub resolved_palette: Palette,

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
    /// ```toml
    /// default_prog = ["top"]
    /// ```
    ///
    /// `default_prog` is implemented as an array where the 0th element
    /// is the command to run and the rest of the elements are passed
    /// as the positional arguments to that command.
    pub default_prog: Option<Vec<String>>,

    /// Specifies the default current working directory if none is specified
    /// through configuration or OSC 7 (see docs for `default_cwd` for more
    /// info!)
    pub default_cwd: Option<PathBuf>,

    #[serde(default)]
    pub exit_behavior: ExitBehavior,

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
    pub freetype_load_target: FreeTypeLoadTarget,
    #[serde(default)]
    pub freetype_render_target: Option<FreeTypeLoadTarget>,
    #[serde(default, deserialize_with = "FreeTypeLoadFlags::de_string")]
    pub freetype_load_flags: FreeTypeLoadFlags,

    /// Selects the freetype interpret version to use.
    /// Likely values are 35, 38 and 40 which have different
    /// characteristics with respective to subpixel hinting.
    /// See https://freetype.org/freetype2/docs/subpixel-hinting.html
    pub freetype_interpreter_version: Option<u32>,

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

    /// Constrains the rate at which the multiplexer client will
    /// speculatively fetch line data.
    /// This helps to avoid saturating the link between the client
    /// and server if the server is dumping a large amount of output
    /// to the client.
    #[serde(default = "default_ratelimit_line_prefetches_per_second")]
    pub ratelimit_mux_line_prefetches_per_second: u32,

    #[serde(default)]
    pub keys: Vec<Key>,
    #[serde(
        default = "default_bypass_mouse_reporting_modifiers",
        deserialize_with = "crate::keys::de_modifiers"
    )]
    pub bypass_mouse_reporting_modifiers: Modifiers,

    #[serde(default)]
    pub debug_key_events: bool,

    #[serde(default)]
    pub disable_default_key_bindings: bool,
    pub leader: Option<LeaderKey>,

    #[serde(default)]
    pub disable_default_quick_select_patterns: bool,
    #[serde(default)]
    pub quick_select_patterns: Vec<String>,
    #[serde(default = "default_alphabet")]
    pub quick_select_alphabet: String,

    #[serde(default)]
    pub mouse_bindings: Vec<Mouse>,
    #[serde(default)]
    pub disable_default_mouse_bindings: bool,

    #[serde(default)]
    pub daemon_options: DaemonOptions,

    /// If set to true, send the system specific composed key when
    /// the ALT key is held down.  If set to false
    /// then send the key with the ALT modifier (this is typically
    /// encoded as ESC followed by the key).
    #[serde(default = "default_true")]
    pub send_composed_key_when_alt_is_pressed: bool,

    #[serde(default)]
    pub send_composed_key_when_left_alt_is_pressed: bool,

    #[serde(default = "default_true")]
    pub send_composed_key_when_right_alt_is_pressed: bool,

    #[serde(default)]
    pub treat_left_ctrlalt_as_altgr: bool,

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

    #[serde(default)]
    pub tab_bar_at_bottom: bool,

    /// If true, tab bar titles are prefixed with the tab index
    #[serde(default = "default_true")]
    pub show_tab_index_in_tab_bar: bool,

    /// If true, show_tab_index_in_tab_bar uses a zero-based index.
    /// The default is false and the tab shows a one-based index.
    #[serde(default)]
    pub tab_and_split_indices_are_zero_based: bool,

    /// Specifies the maximum width that a tab can have in the
    /// tab bar.  Defaults to 16 glyphs in width.
    #[serde(default = "default_tab_max_width")]
    pub tab_max_width: usize,

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
    #[serde(default)]
    pub enable_wayland: bool,

    /// Whether to prefer EGL over other GL implementations.
    /// EGL on Windows has jankier resize behavior than WGL (which
    /// is used if EGL is unavailable), but EGL survives graphics
    /// driver updates without breaking and losing your work.
    #[serde(default = "default_true")]
    pub prefer_egl: bool,

    #[serde(default = "default_true")]
    pub custom_block_glyphs: bool,

    /// Controls the amount of padding to use around the terminal cell area
    #[serde(default)]
    pub window_padding: WindowPadding,

    /// Specifies the path to a background image attachment file.
    /// The file can be any image format that the rust `image`
    /// crate is able to identify and load.
    /// A window background image is rendered into the background
    /// of the window before any other content.
    ///
    /// The image will be scaled to fit the window.
    #[serde(default)]
    pub window_background_image: Option<PathBuf>,
    #[serde(default)]
    pub window_background_image_hsb: Option<HsbTransform>,
    #[serde(default)]
    pub foreground_text_hsb: HsbTransform,

    /// Specifies the alpha value to use when rendering the background
    /// of the window.  The background is taken either from the
    /// window_background_image, or if there is none, the background
    /// color of the cell in the current position.
    /// The default is 1.0 which is 100% opaque.  Setting it to a number
    /// between 0.0 and 1.0 will allow for the screen behind the window
    /// to "shine through" to varying degrees.
    /// This only works on systems with a compositing window manager.
    /// Setting opacity to a value other than 1.0 can impact render
    /// performance.
    #[serde(default = "default_one_point_oh")]
    pub window_background_opacity: f32,

    /// inactive_pane_hue, inactive_pane_saturation and
    /// inactive_pane_brightness allow for transforming the color
    /// of inactive panes.
    /// The pane colors are converted to HSV values and multiplied
    /// by these values before being converted back to RGB to
    /// use in the display.
    ///
    /// The default is 1.0 which leaves the values as-is.
    ///
    /// Modifying the hue changes the hue of the color by rotating
    /// it through the color wheel.  It is not as useful as the
    /// other components, but is available "for free" as part of
    /// the colorspace conversion.
    ///
    /// Modifying the saturation can add or reduce the amount of
    /// "colorfulness".  Making the value smaller can make it appear
    /// more washed out.
    ///
    /// Modifying the brightness can be used to dim or increase
    /// the perceived amount of light.
    ///
    /// The range of these values is 0.0 and up; they are used to
    /// multiply the existing values, so the default of 1.0
    /// preserves the existing component, whilst 0.5 will reduce
    /// it by half, and 2.0 will double the value.
    ///
    /// A subtle dimming effect can be achieved by setting:
    /// inactive_pane_saturation = 0.9
    /// inactive_pane_brightness = 0.8
    #[serde(default = "default_inactive_pane_hsb")]
    pub inactive_pane_hsb: HsbTransform,

    #[serde(default = "default_one_point_oh")]
    pub text_background_opacity: f32,

    /// Specifies how often a blinking cursor transitions between visible
    /// and invisible, expressed in milliseconds.
    /// Setting this to 0 disables blinking.
    /// Note that this value is approximate due to the way that the system
    /// event loop schedulers manage timers; non-zero values will be at
    /// least the interval specified with some degree of slop.
    #[serde(default = "default_cursor_blink_rate")]
    pub cursor_blink_rate: u64,

    #[serde(default)]
    pub force_reverse_video_cursor: bool,

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

    /// Specifies how often blinking text (normal speed) transitions
    /// between visible and invisible, expressed in milliseconds.
    /// Setting this to 0 disables slow text blinking.  Note that this
    /// value is approximate due to the way that the system event loop
    /// schedulers manage timers; non-zero values will be at least the
    /// interval specified with some degree of slop.
    #[serde(default = "default_text_blink_rate")]
    pub text_blink_rate: u64,

    /// Specifies how often blinking text (rapid speed) transitions
    /// between visible and invisible, expressed in milliseconds.
    /// Setting this to 0 disables rapid text blinking.  Note that this
    /// value is approximate due to the way that the system event loop
    /// schedulers manage timers; non-zero values will be at least the
    /// interval specified with some degree of slop.
    #[serde(default = "default_text_blink_rate_rapid")]
    pub text_blink_rate_rapid: u64,

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

    #[serde(default)]
    pub use_ime: bool,
    #[serde(default = "default_true")]
    pub use_dead_keys: bool,

    #[serde(default)]
    pub use_local_build_for_proxy: bool,

    #[serde(default)]
    pub launch_menu: Vec<SpawnCommand>,

    /// When true, watch the config file and reload it automatically
    /// when it is detected as changing.
    #[serde(default = "default_true")]
    pub automatically_reload_config: bool,

    #[serde(default = "default_true")]
    pub add_wsl_distributions_to_launch_menu: bool,

    #[serde(default = "default_true")]
    pub check_for_updates: bool,
    #[serde(default)]
    pub show_update_window: bool,

    #[serde(default = "default_update_interval")]
    pub check_for_updates_interval_seconds: u64,

    /// When set to true, use the CSI-U encoding scheme as described
    /// in http://www.leonerd.org.uk/hacks/fixterms/
    /// This is off by default because @wez and @jsgf find the shift-space
    /// mapping annoying in vim :-p
    #[serde(default)]
    pub enable_csi_u_key_encoding: bool,

    #[serde(default)]
    pub window_close_confirmation: WindowCloseConfirmation,

    #[serde(default)]
    pub native_macos_fullscreen_mode: bool,

    #[serde(default = "default_word_boundary")]
    pub selection_word_boundary: String,

    #[serde(default = "default_enq_answerback")]
    pub enq_answerback: String,

    #[serde(default = "default_true")]
    pub adjust_window_size_when_changing_font_size: bool,

    #[serde(default = "default_alternate_buffer_wheel_scroll_speed")]
    pub alternate_buffer_wheel_scroll_speed: u8,

    #[serde(default = "default_status_update_interval")]
    pub status_update_interval: u64,

    #[serde(default)]
    pub experimental_shape_post_processing: bool,

    #[serde(default = "default_stateless_process_list")]
    pub skip_close_confirmation_for_processes_named: Vec<String>,

    #[serde(default = "default_true")]
    pub warn_about_missing_glyphs: bool,

    #[serde(default)]
    pub sort_fallback_fonts_by_coverage: bool,

    #[serde(default)]
    pub search_font_dirs_for_fallback: bool,

    #[serde(default)]
    pub use_cap_height_to_scale_fallback_fonts: bool,

    #[serde(default)]
    pub swallow_mouse_click_on_pane_focus: bool,

    #[serde(default)]
    pub pane_focus_follows_mouse: bool,
}
impl_lua_conversion!(Config);

fn default_stateless_process_list() -> Vec<String> {
    [
        "bash",
        "sh",
        "zsh",
        "fish",
        "tmux",
        "nu",
        "cmd.exe",
        "pwsh.exe",
        "powershell.exe",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_status_update_interval() -> u64 {
    1_000
}

fn default_alternate_buffer_wheel_scroll_speed() -> u8 {
    3
}

fn default_alphabet() -> String {
    "asdfqwerzxcvjklmiuopghtybn".to_string()
}

fn default_word_boundary() -> String {
    " \t\n{[}]()\"'`".to_string()
}

fn default_enq_answerback() -> String {
    "".to_string()
}

fn default_one_point_oh_f64() -> f64 {
    1.0
}

fn default_one_point_oh() -> f32 {
    1.0
}

fn default_tab_max_width() -> usize {
    16
}

fn default_update_interval() -> u64 {
    86400
}

fn default_inactive_pane_hsb() -> HsbTransform {
    HsbTransform {
        brightness: 0.8,
        saturation: 0.9,
        hue: 1.0,
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum DefaultCursorStyle {
    BlinkingBlock,
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}
impl_lua_conversion!(DefaultCursorStyle);

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

#[derive(Default, Deserialize, Serialize, Clone, Copy, Debug)]
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
impl_lua_conversion!(WindowPadding);

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum WindowCloseConfirmation {
    AlwaysPrompt,
    NeverPrompt,
    // TODO: something smart where we see whether the
    // running programs are stateful
}
impl_lua_conversion!(WindowCloseConfirmation);

impl Default for WindowCloseConfirmation {
    fn default() -> Self {
        WindowCloseConfirmation::AlwaysPrompt
    }
}

impl Default for Config {
    fn default() -> Self {
        // Ask serde to provide the defaults based on the attributes
        // specified in the struct so that we don't have to repeat
        // the same thing in a different form down here
        toml::from_str("").unwrap()
    }
}

pub struct LoadedConfig {
    pub config: Config,
    pub file_name: Option<PathBuf>,
    pub lua: Option<mlua::Lua>,
}

struct PathPossibility {
    path: PathBuf,
    is_required: bool,
}
impl PathPossibility {
    pub fn required(path: PathBuf) -> PathPossibility {
        PathPossibility {
            path,
            is_required: true,
        }
    }
    pub fn optional(path: PathBuf) -> PathPossibility {
        PathPossibility {
            path,
            is_required: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<LoadedConfig, Error> {
        Self::load_with_overrides(&serde_json::Value::default())
    }

    pub fn load_with_overrides(overrides: &serde_json::Value) -> Result<LoadedConfig, Error> {
        // Note that the directories crate has methods for locating project
        // specific config directories, but only returns one of them, not
        // multiple.  In addition, it spawns a lot of subprocesses,
        // so we do this bit "by-hand"

        let mut paths = vec![
            PathPossibility::optional(CONFIG_DIR.join("wezterm.lua")),
            PathPossibility::optional(HOME_DIR.join(".wezterm.lua")),
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
                    paths.insert(0, PathPossibility::optional(exe_dir.join("wezterm.lua")));
                }
            }
        }
        if let Some(path) = std::env::var_os("WEZTERM_CONFIG_FILE") {
            log::trace!("Note: WEZTERM_CONFIG_FILE is set in the environment");
            paths.insert(0, PathPossibility::required(path.into()));
        }

        if let Some(path) = CONFIG_FILE_OVERRIDE.lock().unwrap().as_ref() {
            log::trace!("Note: config file override is set");
            paths.insert(0, PathPossibility::required(path.clone()));
        }

        for path_item in &paths {
            let p = path_item.path.as_path();
            log::trace!("consider config: {}", p.display());
            let mut file = match fs::File::open(p) {
                Ok(file) => file,
                Err(err) => match err.kind() {
                    std::io::ErrorKind::NotFound if !path_item.is_required => continue,
                    _ => bail!("Error opening {}: {}", p.display(), err),
                },
            };

            let mut s = String::new();
            file.read_to_string(&mut s)?;

            let cfg: Self;

            let lua = make_lua_context(p)?;
            let config: mlua::Value = smol::block_on(
                lua.load(&s)
                    .set_name(p.to_string_lossy().as_bytes())?
                    .eval_async(),
            )?;
            let config = Self::apply_overrides_to(&lua, config)?;
            let config = Self::apply_overrides_obj_to(config, overrides)?;
            cfg = luahelper::from_lua_value(config).with_context(|| {
                format!(
                    "Error converting lua value returned by script {} to Config struct",
                    p.display()
                )
            })?;

            // Compute but discard the key bindings here so that we raise any
            // problems earlier than we use them.
            let _ = cfg.key_bindings();

            std::env::set_var("WEZTERM_CONFIG_FILE", p);
            if let Some(dir) = p.parent() {
                std::env::set_var("WEZTERM_CONFIG_DIR", dir);
            }
            return Ok(LoadedConfig {
                config: cfg.compute_extra_defaults(Some(p)),
                file_name: Some(p.to_path_buf()),
                lua: Some(lua),
            });
        }

        Ok(LoadedConfig {
            config: Self::default().compute_extra_defaults(None),
            file_name: None,
            lua: Some(make_lua_context(Path::new(""))?),
        })
    }

    fn apply_overrides_obj_to<'l>(
        mut config: mlua::Value<'l>,
        overrides: &serde_json::Value,
    ) -> anyhow::Result<mlua::Value<'l>> {
        match overrides {
            serde_json::Value::Object(obj) => {
                if let mlua::Value::Table(tbl) = &mut config {
                    for (key, value) in obj {
                        let value = luahelper::JsonLua(value.clone());
                        tbl.set(key.as_str(), value)?;
                    }
                }
                Ok(config)
            }
            _ => Ok(config),
        }
    }

    fn apply_overrides_to<'l>(
        lua: &'l mlua::Lua,
        mut config: mlua::Value<'l>,
    ) -> anyhow::Result<mlua::Value<'l>> {
        let overrides = CONFIG_OVERRIDES.lock().unwrap();
        for (key, value) in &*overrides {
            let code = format!(
                r#"
                local wezterm = require 'wezterm';
                config.{} = {};
                return config;
                "#,
                key, value
            );
            let chunk = lua.load(&code);
            let chunk = chunk.set_name(&format!("--config {}={}", key, value))?;
            lua.globals().set("config", config.clone())?;
            log::debug!("Apply {}={} to config", key, value);
            config = chunk.eval()?;
        }
        Ok(config)
    }

    pub fn default_config() -> Self {
        Self::default().compute_extra_defaults(None)
    }

    pub fn key_bindings(&self) -> HashMap<(KeyCode, Modifiers), KeyAssignment> {
        let mut map = HashMap::new();

        for k in &self.keys {
            let (key, mods) = k.key.normalize_shift(k.mods);
            map.insert((key, mods), k.action.clone());
        }

        map
    }

    pub fn mouse_bindings(&self) -> HashMap<(MouseEventTrigger, Modifiers), KeyAssignment> {
        let mut map = HashMap::new();

        for m in &self.mouse_bindings {
            map.insert((m.event.clone(), m.mods), m.action.clone());
        }

        map
    }

    /// In some cases we need to compute expanded values based
    /// on those provided by the user.  This is where we do that.
    pub fn compute_extra_defaults(&self, config_path: Option<&Path>) -> Self {
        let mut cfg = self.clone();

        // Convert any relative font dirs to their config file relative locations
        if let Some(config_dir) = config_path.as_ref().and_then(|p| p.parent()) {
            for font_dir in &mut cfg.font_dirs {
                if !font_dir.is_absolute() {
                    let dir = config_dir.join(&font_dir);
                    *font_dir = dir;
                }
            }

            if let Some(path) = self.window_background_image.as_ref() {
                if !path.is_absolute() {
                    cfg.window_background_image.replace(config_dir.join(path));
                }
            }
        }

        // Add some reasonable default font rules
        let reduced = self.font.reduce_first_font_to_family();

        let italic = reduced.make_italic();

        let bold = reduced.make_bold();
        let bold_italic = bold.make_italic();

        let half_bright = reduced.make_half_bright();
        let half_bright_italic = half_bright.make_italic();

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Half),
            font: half_bright_italic,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(false),
            intensity: Some(wezterm_term::Intensity::Half),
            font: half_bright,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(false),
            intensity: Some(wezterm_term::Intensity::Bold),
            font: bold,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Bold),
            font: bold_italic,
            ..Default::default()
        });

        cfg.font_rules.push(StyleRule {
            italic: Some(true),
            intensity: Some(wezterm_term::Intensity::Normal),
            font: italic,
            ..Default::default()
        });

        // Load any additional color schemes into the color_schemes map
        cfg.load_color_schemes(&cfg.compute_color_scheme_dirs())
            .ok();

        cfg.resolved_palette = cfg.colors.as_ref().cloned().unwrap_or(Default::default());
        // Color scheme overrides any manually specified palette
        if let Some(scheme) = cfg.color_scheme.as_ref() {
            match cfg.resolve_color_scheme() {
                None => {
                    log::error!(
                        "Your configuration specifies color_scheme=\"{}\" \
                        but that scheme was not found",
                        scheme
                    );
                }
                Some(p) => {
                    cfg.resolved_palette = p.clone();
                }
            }
        }

        cfg
    }

    fn compute_color_scheme_dirs(&self) -> Vec<PathBuf> {
        let mut paths = self.color_scheme_dirs.clone();
        paths.push(CONFIG_DIR.join("colors"));
        if cfg!(windows) {
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
            let scheme: ColorSchemeFile = toml::from_str(&s).context("parsing TOML")?;
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
                                match load_scheme(&path) {
                                    Ok(scheme) => {
                                        log::trace!(
                                            "Loaded color scheme `{}` from {}",
                                            scheme_name,
                                            path.display()
                                        );
                                        self.color_schemes
                                            .insert(scheme_name.to_string(), scheme.colors);
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "Color scheme in `{}` failed to load: {:#}",
                                            path.display(),
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn resolve_color_scheme(&self) -> Option<&Palette> {
        let scheme_name = self.color_scheme.as_ref()?;

        if let Some(palette) = self.color_schemes.get(scheme_name) {
            Some(palette)
        } else {
            COLOR_SCHEMES.get(scheme_name)
        }
    }

    pub fn initial_size(&self) -> PtySize {
        PtySize {
            rows: self.initial_rows,
            cols: self.initial_cols,
            // Guess at a plausible default set of pixel dimensions.
            // This is based on "typical" 10 point font at "normal"
            // pixel density.
            // This will get filled in by the gui layer, but there is
            // an edge case where we emit an iTerm image escape in
            // the software update banner through the mux layer before
            // the GUI has had a chance to update the pixel dimensions
            // when running under X11.
            // This is a bit gross.
            pixel_width: 8 * self.initial_cols,
            pixel_height: 16 * self.initial_rows,
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
        // Apply `default_cwd` only if `cwd` is not already set, allows `--cwd`
        // option to take precedence
        if let (None, Some(cwd)) = (cmd.get_cwd(), &self.default_cwd) {
            cmd.cwd(cwd);
        }

        // Augment WSLENV so that TERM related environment propagates
        // across the win32/wsl boundary
        let mut wsl_env = std::env::var("WSLENV").ok();

        for (k, v) in &self.set_environment_variables {
            if k == "WSLENV" {
                wsl_env.replace(v.clone());
            } else {
                cmd.env(k, v);
            }
        }

        if wsl_env.is_some() || cfg!(windows) || running_under_wsl() {
            let mut wsl_env = wsl_env.unwrap_or_else(String::new);
            if !wsl_env.is_empty() {
                wsl_env.push(':');
            }
            wsl_env.push_str("TERM:COLORTERM:TERM_PROGRAM:TERM_PROGRAM_VERSION");
            cmd.env("WSLENV", wsl_env);
        }

        #[cfg(unix)]
        cmd.umask(umask::UmaskSaver::saved_umask());
        cmd.env("TERM", &self.term);
        cmd.env("COLORTERM", "truecolor");
        // TERM_PROGRAM and TERM_PROGRAM_VERSION are an emerging
        // de-facto standard for identifying the terminal.
        cmd.env("TERM_PROGRAM", "WezTerm");
        cmd.env("TERM_PROGRAM_VERSION", crate::wezterm_version());
    }
}

fn default_ratelimit_line_prefetches_per_second() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

fn default_cursor_blink_rate() -> u64 {
    800
}

fn default_text_blink_rate() -> u64 {
    500
}

fn default_text_blink_rate_rapid() -> u64 {
    250
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
        // file://
        hyperlink::Rule::new(r"\bfile://\S*\b", "$0").unwrap(),
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
    12.0
}

fn compute_runtime_dir() -> Result<PathBuf, Error> {
    if let Some(runtime) = dirs_next::runtime_dir() {
        return Ok(runtime.join("wezterm"));
    }

    Ok(HOME_DIR.join(".local/share/wezterm"))
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

fn default_bypass_mouse_reporting_modifiers() -> Modifiers {
    Modifiers::SHIFT
}
