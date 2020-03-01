use anyhow::anyhow;
use mlua::{Lua, Table, Value};
use std::path::Path;

mod serde_lua;

pub use serde_lua::from_lua_value;
pub use serde_lua::ser::to_lua_value;

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
    lua: &'lua Lua,
    (family, map_defaults): (String, Option<Table<'lua>>),
) -> mlua::Result<Value<'lua>> {
    use crate::config::{FontAttributes, TextStyle};

    let mut text_style: TextStyle = match map_defaults {
        Some(def) => from_lua_value(Value::Table(def))?,
        None => TextStyle::default(),
    };

    text_style.font.clear();
    text_style.font.push(FontAttributes {
        family,
        bold: false,
        italic: false,
    });

    Ok(to_lua_value(lua, text_style)?)
}

/// Given a list of font family names in order of preference, return a
/// text style instance for that font configuration.
///
/// `wezterm.font_with_fallback({"Operator Mono", "DengXian"})`
///
/// The second optional argument is a list of other TextStyle fields,
/// as described by the `wezterm.font` documentation.
fn font_with_fallback<'lua>(
    lua: &'lua Lua,
    (fallback, map_defaults): (Vec<String>, Option<Table<'lua>>),
) -> mlua::Result<Value<'lua>> {
    use crate::config::{FontAttributes, TextStyle};

    let mut text_style: TextStyle = match map_defaults {
        Some(def) => from_lua_value(Value::Table(def))?,
        None => TextStyle::default(),
    };

    text_style.font.clear();
    for family in fallback {
        text_style.font.push(FontAttributes {
            family,
            bold: false,
            italic: false,
        });
    }

    Ok(to_lua_value(lua, text_style)?)
}
