#![macro_use]

use crate::config::{FontAttributes, TextStyle};
use anyhow::anyhow;
use mlua::{Lua, Table, Value};
use std::path::Path;

mod serde_lua;

pub use serde_lua::from_lua_value;
pub use serde_lua::ser::to_lua_value;

/// Implement lua conversion traits for a type.
/// This implementation requires that the type implement
/// serde Serialize and Deserialize.
/// Why do we need these traits?  They allow `create_function` to
/// operate in terms of our internal types rather than forcing
/// the implementer to use generic Value parameter or return values.
macro_rules! impl_lua_conversion {
    ($struct:ident) => {
        impl<'lua> mlua::ToLua<'lua> for $struct {
            fn to_lua(self, lua: &'lua mlua::Lua) -> Result<mlua::Value<'lua>, mlua::Error> {
                Ok(crate::scripting::to_lua_value(lua, self)?)
            }
        }

        impl<'lua> mlua::FromLua<'lua> for $struct {
            fn from_lua(
                value: mlua::Value<'lua>,
                _lua: &'lua mlua::Lua,
            ) -> Result<Self, mlua::Error> {
                Ok(crate::scripting::from_lua_value(value)?)
            }
        }
    };
}

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
pub fn make_lua_context(config_dir: &Path) -> anyhow::Result<Lua> {
    let lua = Lua::new();

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

        prefix_path(&mut path_array, &crate::config::HOME_DIR.join(".wezterm"));
        prefix_path(
            &mut path_array,
            &crate::config::HOME_DIR.join(".config").join("wezterm"),
        );
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

        wezterm_mod.set(
            "config_dir",
            config_dir
                .to_str()
                .ok_or_else(|| anyhow!("config dir path is not UTF-8"))?,
        )?;

        wezterm_mod.set("target_triple", env!("VERGEN_TARGET_TRIPLE"))?;
        wezterm_mod.set("version", crate::wezterm_version())?;
        wezterm_mod.set("home_dir", crate::config::HOME_DIR.to_str())?;
        wezterm_mod.set(
            "running_under_wsl",
            lua.create_function(|_, ()| Ok(crate::running_under_wsl()))?,
        )?;

        wezterm_mod.set(
            "log_error",
            lua.create_function(|_, msg: String| {
                log::error!("lua: {}", msg);
                Ok(())
            })?,
        )?;

        wezterm_mod.set("font", lua.create_function(font)?)?;
        wezterm_mod.set(
            "font_with_fallback",
            lua.create_function(font_with_fallback)?,
        )?;
        wezterm_mod.set("hostname", lua.create_function(hostname)?)?;
        wezterm_mod.set("action", lua.create_function(action)?)?;

        wezterm_mod.set("read_dir", lua.create_function(read_dir)?)?;
        wezterm_mod.set("glob", lua.create_function(glob)?)?;

        package.set("path", path_array.join(";"))?;

        let loaded: Table = package.get("loaded")?;
        loaded.set("wezterm", wezterm_mod)?;
    }

    Ok(lua)
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
    (family, map_defaults): (String, Option<TextStyle>),
) -> mlua::Result<TextStyle> {
    let mut text_style = map_defaults.unwrap_or_else(TextStyle::default);

    text_style.font.clear();
    text_style.font.push(FontAttributes {
        family,
        ..Default::default()
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
    (fallback, map_defaults): (Vec<String>, Option<TextStyle>),
) -> mlua::Result<TextStyle> {
    let mut text_style = map_defaults.unwrap_or_else(TextStyle::default);

    text_style.font.clear();
    for family in fallback {
        text_style.font.push(FontAttributes {
            family,
            ..Default::default()
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
fn action<'lua>(lua: &'lua Lua, action: Table<'lua>) -> mlua::Result<crate::config::KeyAction> {
    let wrapper = lua.create_table()?;
    wrapper.set("Action", action)?;
    Ok(from_lua_value(Value::Table(wrapper))?)
}

fn read_dir<'lua>(_: &'lua Lua, path: String) -> mlua::Result<Vec<String>> {
    let dir = std::fs::read_dir(path).map_err(|e| mlua::Error::external(e))?;
    let mut entries = vec![];
    for entry in dir {
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

fn glob<'lua>(
    _: &'lua Lua,
    (pattern, path): (String, Option<String>),
) -> mlua::Result<Vec<String>> {
    let mut entries = vec![];
    let glob = filenamegen::Glob::new(&pattern).map_err(|e| mlua::Error::external(e))?;
    for path in glob.walk(path.as_ref().map(|s| s.as_str()).unwrap_or(".")) {
        if let Some(utf8) = path.to_str() {
            entries.push(utf8.to_string());
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                path.display()
            )));
        }
    }
    Ok(entries)
}
