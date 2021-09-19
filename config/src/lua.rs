use crate::Gradient;
use crate::{FontAttributes, FontStretch, FontWeight, TextStyle};
use anyhow::anyhow;
use bstr::BString;
pub use luahelper::*;
use mlua::{FromLua, Lua, Table, ToLua, ToLuaMulti, Value, Variadic};
use serde::*;
use smol::prelude::*;
use std::path::Path;
use termwiz::cell::{grapheme_column_width, unicode_column_width, AttributeChange, CellAttributes};
use termwiz::color::{AnsiColor, ColorAttribute, ColorSpec, RgbColor};
use termwiz::input::Modifiers;
use termwiz::surface::change::Change;
use unicode_segmentation::UnicodeSegmentation;

static LUA_REGISTRY_USER_CALLBACK_COUNT: &str = "wezterm-user-callback-count";

/// Set up a lua context for executing some code.
/// The path to the directory containing the configuration is
/// passed in and is used to pre-set some global values in
/// the environment.
///
/// The `package.path` is configured to search the user's
/// wezterm specific config paths for lua modules, should
/// they choose to `require` additional code from their config.
///
/// A `wezterm` module is registered so that the script can
/// `require "wezterm"` and call into functions provided by
/// wezterm.  The wezterm module contains:
/// * `executable_dir` - the directory containing the wezterm
///   executable.  This is potentially useful for portable
///   installs on Windows.
/// * `config_dir` - the directory containing the wezterm
///   configuration.
/// * `log_error` - a function that logs to stderr (or the server
///   log file for daemonized wezterm).
/// * `target_triple` - the rust compilation target triple.
/// * `version` - the version of the running wezterm instance.
/// * `home_dir` - the path to the user's home directory
///
/// In addition to this, the lua standard library, except for
/// the `debug` module, is also available to the script.
pub fn make_lua_context(config_file: &Path) -> anyhow::Result<Lua> {
    let lua = Lua::new();

    let config_dir = config_file.parent().unwrap_or_else(|| Path::new("/"));

    {
        let globals = lua.globals();
        // This table will be the `wezterm` module in the script
        let wezterm_mod = lua.create_table()?;

        let package: Table = globals.get("package")?;
        let package_path: String = package.get("path")?;
        let mut path_array: Vec<String> = package_path.split(";").map(|s| s.to_owned()).collect();

        fn prefix_path(array: &mut Vec<String>, path: &Path) {
            array.insert(0, format!("{}/?.lua", path.display()));
            array.insert(1, format!("{}/?/init.lua", path.display()));
        }

        prefix_path(&mut path_array, &crate::HOME_DIR.join(".wezterm"));
        prefix_path(&mut path_array, &crate::CONFIG_DIR);
        if let Ok(exe) = std::env::current_exe() {
            if let Some(path) = exe.parent() {
                wezterm_mod.set(
                    "executable_dir",
                    path.to_str().ok_or_else(|| anyhow!("path is not UTF-8"))?,
                )?;
                if cfg!(windows) {
                    // For a portable windows install, force in this path ahead
                    // of the rest
                    prefix_path(&mut path_array, &path.join("wezterm_modules"));
                }
            }
        }
        let config_file_str = config_file
            .to_str()
            .ok_or_else(|| anyhow!("config file path is not UTF-8"))?;

        wezterm_mod.set("config_file", config_file_str)?;
        wezterm_mod.set(
            "config_dir",
            config_dir
                .to_str()
                .ok_or_else(|| anyhow!("config dir path is not UTF-8"))?,
        )?;

        lua.set_named_registry_value("wezterm-watch-paths", Vec::<String>::new())?;
        wezterm_mod.set(
            "add_to_config_reload_watch_list",
            lua.create_function(|lua, args: Variadic<String>| {
                let mut watch_paths: Vec<String> =
                    lua.named_registry_value("wezterm-watch-paths")?;
                watch_paths.extend_from_slice(&args);
                lua.set_named_registry_value("wezterm-watch-paths", watch_paths)
            })?,
        )?;

        wezterm_mod.set("target_triple", crate::wezterm_target_triple())?;
        wezterm_mod.set("version", crate::wezterm_version())?;
        wezterm_mod.set("home_dir", crate::HOME_DIR.to_str())?;
        wezterm_mod.set(
            "running_under_wsl",
            lua.create_function(|_, ()| Ok(crate::running_under_wsl()))?,
        )?;

        fn print_helper(args: Variadic<Value>) -> String {
            let mut output = String::new();
            for (idx, item) in args.into_iter().enumerate() {
                if idx > 0 {
                    output.push(' ');
                }

                match item {
                    Value::String(s) => match s.to_str() {
                        Ok(s) => output.push_str(s),
                        Err(_) => {
                            let item = String::from_utf8_lossy(s.as_bytes());
                            output.push_str(&item);
                        }
                    },
                    item @ _ => {
                        let item = format!("{:?}", ValueWrapper(item));
                        output.push_str(&item);
                    }
                }
            }
            output
        }

        wezterm_mod.set(
            "log_error",
            lua.create_function(|_, args: Variadic<Value>| {
                let output = print_helper(args);
                log::error!("lua: {}", output);
                Ok(())
            })?,
        )?;
        wezterm_mod.set(
            "log_info",
            lua.create_function(|_, args: Variadic<Value>| {
                let output = print_helper(args);
                log::info!("lua: {}", output);
                Ok(())
            })?,
        )?;
        wezterm_mod.set(
            "log_warn",
            lua.create_function(|_, args: Variadic<Value>| {
                let output = print_helper(args);
                log::warn!("lua: {}", output);
                Ok(())
            })?,
        )?;
        globals.set(
            "print",
            lua.create_function(|_, args: Variadic<Value>| {
                let output = print_helper(args);
                log::info!("lua: {}", output);
                Ok(())
            })?,
        )?;

        wezterm_mod.set(
            "column_width",
            lua.create_function(|_, s: String| Ok(unicode_column_width(&s)))?,
        )?;

        wezterm_mod.set(
            "pad_right",
            lua.create_function(|_, (mut result, width): (String, usize)| {
                let mut len = unicode_column_width(&result);
                while len < width {
                    result.push(' ');
                    len += 1;
                }

                Ok(result)
            })?,
        )?;

        wezterm_mod.set(
            "pad_left",
            lua.create_function(|_, (mut result, width): (String, usize)| {
                let mut len = unicode_column_width(&result);
                while len < width {
                    result.insert(0, ' ');
                    len += 1;
                }

                Ok(result)
            })?,
        )?;

        wezterm_mod.set(
            "truncate_right",
            lua.create_function(|_, (s, max_width): (String, usize)| {
                let mut result = String::new();
                let mut len = 0;
                for g in s.graphemes(true) {
                    let g_len = grapheme_column_width(g);
                    if g_len + len > max_width {
                        break;
                    }
                    result.push_str(g);
                    len += g_len;
                }

                Ok(result)
            })?,
        )?;

        wezterm_mod.set(
            "truncate_left",
            lua.create_function(|_, (s, max_width): (String, usize)| {
                let mut result = vec![];
                let mut len = 0;
                for g in s.graphemes(true).rev() {
                    let g_len = grapheme_column_width(g);
                    if g_len + len > max_width {
                        break;
                    }
                    result.push(g);
                    len += g_len;
                }

                result.reverse();
                Ok(result.join(""))
            })?,
        )?;

        wezterm_mod.set("font", lua.create_function(font)?)?;
        wezterm_mod.set(
            "font_with_fallback",
            lua.create_function(font_with_fallback)?,
        )?;
        wezterm_mod.set("hostname", lua.create_function(hostname)?)?;
        wezterm_mod.set("action", lua.create_function(action)?)?;
        lua.set_named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT, 0)?;
        wezterm_mod.set("action_callback", lua.create_function(action_callback)?)?;
        wezterm_mod.set("permute_any_mods", lua.create_function(permute_any_mods)?)?;
        wezterm_mod.set(
            "permute_any_or_no_mods",
            lua.create_function(permute_any_or_no_mods)?,
        )?;

        wezterm_mod.set("read_dir", lua.create_async_function(read_dir)?)?;
        wezterm_mod.set("glob", lua.create_async_function(glob)?)?;

        wezterm_mod.set("utf16_to_utf8", lua.create_function(utf16_to_utf8)?)?;
        wezterm_mod.set("split_by_newlines", lua.create_function(split_by_newlines)?)?;
        wezterm_mod.set(
            "run_child_process",
            lua.create_async_function(run_child_process)?,
        )?;
        wezterm_mod.set("on", lua.create_function(register_event)?)?;
        wezterm_mod.set("emit", lua.create_async_function(emit_event)?)?;
        wezterm_mod.set("sleep_ms", lua.create_async_function(sleep_ms)?)?;
        wezterm_mod.set("format", lua.create_function(format)?)?;
        wezterm_mod.set("strftime", lua.create_function(strftime)?)?;
        wezterm_mod.set("battery_info", lua.create_function(battery_info)?)?;
        wezterm_mod.set("gradient_colors", lua.create_function(gradient_colors)?)?;

        package.set("path", path_array.join(";"))?;

        let loaded: Table = package.get("loaded")?;
        loaded.set("wezterm", wezterm_mod)?;
    }

    Ok(lua)
}

use termwiz::caps::{Capabilities, ColorLevel, ProbeHints};
use termwiz::render::terminfo::TerminfoRenderer;

lazy_static::lazy_static! {
    static ref CAPS: Capabilities = {
        let data = include_bytes!("../../termwiz/data/xterm-256color");
        let db = terminfo::Database::from_buffer(&data[..]).unwrap();
        Capabilities::new_with_hints(
            ProbeHints::new_from_env()
                .term(Some("xterm-256color".into()))
                .terminfo_db(Some(db))
                .color_level(Some(ColorLevel::TrueColor))
                .colorterm(None)
                .colorterm_bce(None)
                .term_program(Some("WezTerm".into()))
                .term_program_version(Some(crate::wezterm_version().into())),
        )
        .expect("cannot fail to make internal Capabilities")
    };
}

pub fn new_wezterm_terminfo_renderer() -> TerminfoRenderer {
    TerminfoRenderer::new(CAPS.clone())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(transparent)]
struct ChangeWrap(Change);
impl_lua_conversion!(ChangeWrap);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FormatColor {
    AnsiColor(AnsiColor),
    Color(String),
    Default,
}

impl FormatColor {
    fn to_attr(self) -> ColorAttribute {
        let spec: ColorSpec = self.into();
        let attr: ColorAttribute = spec.into();
        attr
    }
}

impl Into<ColorSpec> for FormatColor {
    fn into(self) -> ColorSpec {
        match self {
            FormatColor::AnsiColor(c) => c.into(),
            FormatColor::Color(s) => {
                let rgb = RgbColor::from_named_or_rgb_string(&s)
                    .unwrap_or(RgbColor::new_8bpc(0xff, 0xff, 0xff));
                rgb.into()
            }
            FormatColor::Default => ColorSpec::Default,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FormatItem {
    Foreground(FormatColor),
    Background(FormatColor),
    Attribute(AttributeChange),
    Text(String),
}
impl_lua_conversion!(FormatItem);

impl Into<Change> for FormatItem {
    fn into(self) -> Change {
        match self {
            Self::Attribute(change) => change.into(),
            Self::Text(t) => t.into(),
            Self::Foreground(c) => AttributeChange::Foreground(c.to_attr()).into(),
            Self::Background(c) => AttributeChange::Background(c.to_attr()).into(),
        }
    }
}

struct FormatTarget {
    target: Vec<u8>,
}

impl std::io::Write for FormatTarget {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.target, buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl termwiz::render::RenderTty for FormatTarget {
    fn get_size_in_cells(&mut self) -> termwiz::Result<(usize, usize)> {
        Ok((80, 24))
    }
}

fn strftime<'lua>(_: &'lua Lua, format: String) -> mlua::Result<String> {
    use chrono::prelude::*;
    let local: DateTime<Local> = Local::now();
    Ok(local.format(&format).to_string())
}

pub fn format_as_escapes(items: Vec<FormatItem>) -> anyhow::Result<String> {
    let mut changes: Vec<Change> = items.into_iter().map(Into::into).collect();
    changes.push(Change::AllAttributes(CellAttributes::default()).into());
    let mut renderer = new_wezterm_terminfo_renderer();
    let mut target = FormatTarget { target: vec![] };
    renderer.render_to(&changes, &mut target)?;
    Ok(String::from_utf8(target.target)?)
}

fn format<'lua>(_: &'lua Lua, items: Vec<FormatItem>) -> mlua::Result<String> {
    format_as_escapes(items).map_err(|e| mlua::Error::external(e))
}

#[derive(Serialize, Deserialize, Debug)]
struct BatteryInfo {
    state_of_charge: f32,
    vendor: String,
    model: String,
    state: String,
    serial: String,
    time_to_full: Option<f32>,
    time_to_empty: Option<f32>,
}
impl_lua_conversion!(BatteryInfo);

fn opt_string(s: Option<&str>) -> String {
    match s {
        Some(s) => s,
        None => "unknown",
    }
    .to_string()
}

fn battery_info<'lua>(_: &'lua Lua, _: ()) -> mlua::Result<Vec<BatteryInfo>> {
    use battery::{Manager, State};
    let manager = Manager::new().map_err(|e| mlua::Error::external(e))?;
    let mut result = vec![];
    for b in manager.batteries().map_err(|e| mlua::Error::external(e))? {
        let bat = b.map_err(|e| mlua::Error::external(e))?;
        result.push(BatteryInfo {
            state_of_charge: bat.state_of_charge().value,
            vendor: opt_string(bat.vendor()),
            model: opt_string(bat.model()),
            serial: opt_string(bat.serial_number()),
            state: match bat.state() {
                State::Charging => "Charging",
                State::Discharging => "Discharging",
                State::Empty => "Empty",
                State::Full => "Full",
                State::Unknown | _ => "Unknown",
            }
            .to_string(),
            time_to_full: bat.time_to_full().map(|q| q.value),
            time_to_empty: bat.time_to_empty().map(|q| q.value),
        })
    }
    Ok(result)
}

async fn sleep_ms<'lua>(_: &'lua Lua, milliseconds: u64) -> mlua::Result<()> {
    let duration = std::time::Duration::from_millis(milliseconds);
    smol::Timer::after(duration).await;
    Ok(())
}

/// Returns the system hostname.
/// Errors may occur while retrieving the hostname from the system,
/// or if the hostname isn't a UTF-8 string.
fn hostname<'lua>(_: &'lua Lua, _: ()) -> mlua::Result<String> {
    let hostname = hostname::get().map_err(|e| mlua::Error::external(e))?;
    match hostname.to_str() {
        Some(hostname) => Ok(hostname.to_owned()),
        None => Err(mlua::Error::external(anyhow!("hostname isn't UTF-8"))),
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
struct TextStyleAttributes {
    /// Whether the font should be a bold variant
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub weight: Option<FontWeight>,
    #[serde(default)]
    pub stretch: FontStretch,
    /// Whether the font should be an italic variant
    #[serde(default)]
    pub italic: bool,
    /// If set, when rendering text that is set to the default
    /// foreground color, use this color instead.  This is most
    /// useful in a `[[font_rules]]` section to implement changing
    /// the text color for eg: bold text.
    pub foreground: Option<termwiz::color::RgbColor>,
}
impl_lua_conversion!(TextStyleAttributes);

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
struct LuaFontAttributes {
    /// The font family name
    pub family: String,
    /// Whether the font should be a bold variant
    #[serde(default)]
    pub weight: FontWeight,
    #[serde(default)]
    pub stretch: FontStretch,
    /// Whether the font should be an italic variant
    #[serde(default)]
    pub italic: bool,
}
impl<'lua> FromLua<'lua> for LuaFontAttributes {
    fn from_lua(value: Value<'lua>, _lua: &'lua Lua) -> Result<Self, mlua::Error> {
        match value {
            Value::String(s) => {
                let mut attr = LuaFontAttributes::default();
                attr.family = s.to_str()?.to_string();
                Ok(attr)
            }
            v => Ok(from_lua_value(v)?),
        }
    }
}

/// Given a simple font family name, returns a text style instance.
/// The second optional argument is a list of the other TextStyle
/// fields, which at the time of writing includes only the
/// `foreground` color that can be used to force a particular
/// color to be used for this text style.
///
/// `wezterm.font("foo", {foreground="tomato"})`
/// yields:
/// `{ font = {{ family = "foo" }}, foreground="tomato"}`
fn font<'lua>(
    _lua: &'lua Lua,
    (mut attrs, map_defaults): (LuaFontAttributes, Option<TextStyleAttributes>),
) -> mlua::Result<TextStyle> {
    let mut text_style = TextStyle::default();
    text_style.font.clear();

    if let Some(map_defaults) = map_defaults {
        attrs.weight = match map_defaults.bold {
            Some(true) => FontWeight::BOLD,
            Some(false) => FontWeight::REGULAR,
            None => map_defaults.weight.unwrap_or(FontWeight::REGULAR),
        };
        attrs.stretch = map_defaults.stretch;
        attrs.italic = map_defaults.italic;
        text_style.foreground = map_defaults.foreground;
    }

    text_style.font.push(FontAttributes {
        family: attrs.family,
        stretch: attrs.stretch,
        weight: attrs.weight,
        italic: attrs.italic,
        is_fallback: false,
        is_synthetic: false,
    });

    Ok(text_style)
}

/// Given a list of font family names in order of preference, return a
/// text style instance for that font configuration.
///
/// `wezterm.font_with_fallback({"Operator Mono", "DengXian"})`
///
/// The second optional argument is a list of other TextStyle fields,
/// as described by the `wezterm.font` documentation.
fn font_with_fallback<'lua>(
    _lua: &'lua Lua,
    (fallback, map_defaults): (Vec<LuaFontAttributes>, Option<TextStyleAttributes>),
) -> mlua::Result<TextStyle> {
    let mut text_style = TextStyle::default();
    text_style.font.clear();

    for (idx, mut attrs) in fallback.into_iter().enumerate() {
        if let Some(map_defaults) = &map_defaults {
            attrs.weight = match map_defaults.bold {
                Some(true) => FontWeight::BOLD,
                Some(false) => FontWeight::REGULAR,
                None => map_defaults.weight.unwrap_or(FontWeight::REGULAR),
            };
            attrs.stretch = map_defaults.stretch;
            attrs.italic = map_defaults.italic;
            text_style.foreground = map_defaults.foreground;
        }

        text_style.font.push(FontAttributes {
            family: attrs.family,
            stretch: attrs.stretch,
            weight: attrs.weight,
            italic: attrs.italic,
            is_fallback: idx != 0,
            is_synthetic: false,
        });
    }

    Ok(text_style)
}

/// Helper for defining key assignment actions.
/// Usage looks like this:
///
/// ```lua
/// local wezterm = require 'wezterm';
/// return {
///    keys = {
///      {key="{", mods="SHIFT|CTRL", action=wezterm.action{ActivateTabRelative=-1}},
///      {key="}", mods="SHIFT|CTRL", action=wezterm.action{ActivateTabRelative=1}},
///    }
/// }
/// ```
fn action<'lua>(
    _lua: &'lua Lua,
    action: Table<'lua>,
) -> mlua::Result<crate::keyassignment::KeyAssignment> {
    Ok(from_lua_value(Value::Table(action))?)
}

fn action_callback<'lua>(
    lua: &'lua Lua,
    callback: mlua::Function,
) -> mlua::Result<crate::keyassignment::KeyAssignment> {
    let callback_count: i32 = lua.named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT)?;
    let user_event_id = format!("user-defined-{}", callback_count);
    lua.set_named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT, callback_count + 1)?;
    register_event(lua, (user_event_id.clone(), callback))?;
    return Ok(crate::KeyAssignment::EmitEvent(user_event_id));
}

async fn read_dir<'lua>(_: &'lua Lua, path: String) -> mlua::Result<Vec<String>> {
    let mut dir = smol::fs::read_dir(path)
        .await
        .map_err(|e| mlua::Error::external(e))?;
    let mut entries = vec![];
    for entry in dir.next().await {
        let entry = entry.map_err(|e| mlua::Error::external(e))?;
        if let Some(utf8) = entry.path().to_str() {
            entries.push(utf8.to_string());
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                entry.path().display()
            )));
        }
    }
    Ok(entries)
}

async fn glob<'lua>(
    _: &'lua Lua,
    (pattern, path): (String, Option<String>),
) -> mlua::Result<Vec<String>> {
    let entries = smol::unblock(move || {
        let mut entries = vec![];
        let glob = filenamegen::Glob::new(&pattern)?;
        for path in glob.walk(path.as_ref().map(|s| s.as_str()).unwrap_or(".")) {
            if let Some(utf8) = path.to_str() {
                entries.push(utf8.to_string());
            } else {
                return Err(anyhow!(
                    "path entry {} is not representable as utf8",
                    path.display()
                ));
            }
        }
        Ok(entries)
    })
    .await
    .map_err(|e| mlua::Error::external(e))?;
    Ok(entries)
}

fn split_by_newlines<'lua>(_: &'lua Lua, text: String) -> mlua::Result<Vec<String>> {
    Ok(text
        .lines()
        .map(|s| {
            // Ungh, `str.lines()` is supposed to split by `\n` or `\r\n`, but I've
            // found that it is necessary to have an additional trim here in order
            // to actually remove the `\r`.
            s.trim_end_matches('\r').to_string()
        })
        .collect())
}

/// This implements `wezterm.on`, whose goal is to register an event handler
/// callback.
/// The callback function may return `false` to prevent other handlers from
/// triggering.  The `false` return means "prevent the default action",
/// and thus, depending on the semantics of the emitted event, can be used
/// to override rather augment built-in behavior.
///
/// To allow the default action you can omit a return statement, or
/// explicitly return `true`.
///
/// The arguments to the handler are passed through from the corresponding
/// `wezterm.emit` call.
///
/// ```lua
/// wezterm.on("event-name", function(arg1, arg2)
///   -- do something
///   return false -- if you want to prevent other handlers running
/// end);
///
/// wezterm.emit("event-name", "foo", "bar");
/// ```
fn register_event<'lua>(
    lua: &'lua Lua,
    (name, func): (String, mlua::Function),
) -> mlua::Result<()> {
    let decorated_name = format!("wezterm-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&decorated_name)?;
    match tbl {
        mlua::Value::Nil => {
            let tbl = lua.create_table()?;
            tbl.set(1, func)?;
            lua.set_named_registry_value(&decorated_name, tbl)?;
            Ok(())
        }
        mlua::Value::Table(tbl) => {
            let len = tbl.raw_len();
            tbl.set(len + 1, func)?;
            Ok(())
        }
        _ => Err(mlua::Error::external(anyhow!(
            "registry key for {} has invalid type",
            decorated_name
        ))),
    }
}

/// This implements `wezterm.emit`.
/// The first parameter to emit is the name of a signal that may or may not
/// have previously been registered via `wezterm.on`.
/// `wezterm.emit` will call each of the registered handlers in the order
/// that they were registered and pass the remainder of the `emit` arguments
/// to those handler functions.
/// If a handler returns `false` then `wezterm.emit` will stop calling
/// any additional handlers and then return `false`.
/// Otherwise, once all handlers have been called and none of them returned
/// `false`, `wezterm.emit` will return `true`.
/// The return value indicates to the caller whether the default action
/// should take place.
pub async fn emit_event<'lua>(
    lua: &'lua Lua,
    (name, args): (String, mlua::MultiValue<'lua>),
) -> mlua::Result<bool> {
    let decorated_name = format!("wezterm-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&decorated_name)?;
    match tbl {
        mlua::Value::Table(tbl) => {
            for func in tbl.sequence_values::<mlua::Function>() {
                let func = func?;
                match func.call_async(args.clone()).await? {
                    mlua::Value::Boolean(b) if !b => {
                        // Default action prevented
                        return Ok(false);
                    }
                    _ => {
                        // Continue with other handlers
                    }
                }
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}

pub fn emit_sync_callback<'lua, A>(
    lua: &'lua Lua,
    (name, args): (String, A),
) -> mlua::Result<mlua::Value<'lua>>
where
    A: ToLuaMulti<'lua>,
{
    let decorated_name = format!("wezterm-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&decorated_name)?;
    match tbl {
        mlua::Value::Table(tbl) => {
            for func in tbl.sequence_values::<mlua::Function>() {
                let func = func?;
                return func.call(args);
            }
            Ok(mlua::Value::Nil)
        }
        _ => Ok(mlua::Value::Nil),
    }
}

/// Ungh: https://github.com/microsoft/WSL/issues/4456
fn utf16_to_utf8<'lua>(_: &'lua Lua, text: mlua::String) -> mlua::Result<String> {
    let bytes = text.as_bytes();

    if bytes.len() % 2 != 0 {
        return Err(mlua::Error::external(anyhow!(
            "input data has odd length, cannot be utf16"
        )));
    }

    // This is "safe" because we checked that the length seems reasonable,
    // and our new slice is within those same bounds.
    let wide: &[u16] =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2) };

    String::from_utf16(wide).map_err(|e| mlua::Error::external(e))
}

async fn run_child_process<'lua>(
    _: &'lua Lua,
    args: Vec<String>,
) -> mlua::Result<(bool, BString, BString)> {
    let mut cmd = smol::process::Command::new(&args[0]);

    if args.len() > 1 {
        cmd.args(&args[1..]);
    }

    #[cfg(windows)]
    {
        use smol::process::windows::CommandExt;
        cmd.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
    }

    let output = cmd.output().await.map_err(|e| mlua::Error::external(e))?;

    Ok((
        output.status.success(),
        output.stdout.into(),
        output.stderr.into(),
    ))
}

fn permute_any_mods<'lua>(
    lua: &'lua Lua,
    item: mlua::Table,
) -> mlua::Result<Vec<mlua::Value<'lua>>> {
    permute_mods(lua, item, false)
}

fn permute_any_or_no_mods<'lua>(
    lua: &'lua Lua,
    item: mlua::Table,
) -> mlua::Result<Vec<mlua::Value<'lua>>> {
    permute_mods(lua, item, true)
}

fn permute_mods<'lua>(
    lua: &'lua Lua,
    item: mlua::Table,
    allow_none: bool,
) -> mlua::Result<Vec<mlua::Value<'lua>>> {
    let mut result = vec![];
    for ctrl in &[Modifiers::NONE, Modifiers::CTRL] {
        for shift in &[Modifiers::NONE, Modifiers::SHIFT] {
            for alt in &[Modifiers::NONE, Modifiers::ALT] {
                for sup in &[Modifiers::NONE, Modifiers::SUPER] {
                    let flags = *ctrl | *shift | *alt | *sup;
                    if flags == Modifiers::NONE && !allow_none {
                        continue;
                    }

                    let new_item = lua.create_table()?;
                    for pair in item.clone().pairs::<mlua::Value, mlua::Value>() {
                        let (k, v) = pair?;
                        new_item.set(k, v)?;
                    }
                    new_item.set("mods", format!("{:?}", flags))?;
                    result.push(new_item.to_lua(lua)?);
                }
            }
        }
    }
    Ok(result)
}

fn gradient_colors<'lua>(
    _lua: &'lua Lua,
    (gradient, num_colors): (Gradient, usize),
) -> mlua::Result<Vec<String>> {
    let g = gradient.build().map_err(|e| mlua::Error::external(e))?;
    Ok(g.colors(num_colors)
        .into_iter()
        .map(|c| c.to_hex_string())
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn can_register_and_emit_multiple_events() -> anyhow::Result<()> {
        let _ = pretty_env_logger::formatted_builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        let lua = make_lua_context(Path::new("testing"))?;

        let total = Arc::new(Mutex::new(0));

        let first = lua.create_function({
            let total = total.clone();
            move |_lua: &mlua::Lua, n: i32| {
                let mut l = total.lock().unwrap();
                *l += n;
                Ok(())
            }
        })?;

        let second = lua.create_function({
            let total = total.clone();
            move |_lua: &mlua::Lua, n: i32| {
                let mut l = total.lock().unwrap();
                *l += n * 2;
                // Prevent any later functions from being called
                Ok(false)
            }
        })?;

        let third = lua.create_function({
            let total = total.clone();
            move |_lua: &mlua::Lua, n: i32| {
                let mut l = total.lock().unwrap();
                *l += n * 3;
                Ok(())
            }
        })?;

        register_event(&lua, ("foo".to_string(), first))?;
        register_event(&lua, ("foo".to_string(), second))?;
        register_event(&lua, ("foo".to_string(), third))?;
        register_event(
            &lua,
            (
                "bar".to_string(),
                lua.create_function(|_: &mlua::Lua, (a, b): (i32, String)| {
                    eprintln!("a: {}, b: {}", a, b);
                    Ok(())
                })?,
            ),
        )?;

        smol::block_on(
            lua.load(
                r#"
local wezterm = require 'wezterm';

wezterm.on('foo', function (n)
    wezterm.log_error("lua hook recording " .. n);
end);

-- one of the foo handlers returns false, so the emit
-- returns false overall, indicating that the default
-- action should not be taken
assert(wezterm.emit('foo', 2) == false)

wezterm.on('bar', function (n, str)
    wezterm.log_error("bar says " .. n .. " " .. str)
end);

-- None of the bar handlers return anything, so the
-- emit returns true to indicate that the default
-- action should be performed
assert(wezterm.emit('bar', 42, 'woot') == true)
"#,
            )
            .exec_async(),
        )?;

        assert_eq!(*total.lock().unwrap(), 6);

        Ok(())
    }
}
