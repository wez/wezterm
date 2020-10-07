use crate::{FontAttributes, TextStyle};
use anyhow::anyhow;
use bstr::BString;
pub use luahelper::*;
use mlua::{Lua, Table, Value};
use serde::*;
use std::path::Path;

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

        wezterm_mod.set(
            "config_dir",
            config_dir
                .to_str()
                .ok_or_else(|| anyhow!("config dir path is not UTF-8"))?,
        )?;

        wezterm_mod.set("target_triple", crate::wezterm_target_triple())?;
        wezterm_mod.set("version", crate::wezterm_version())?;
        wezterm_mod.set("home_dir", crate::HOME_DIR.to_str())?;
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

        wezterm_mod.set("utf16_to_utf8", lua.create_function(utf16_to_utf8)?)?;
        wezterm_mod.set("split_by_newlines", lua.create_function(split_by_newlines)?)?;
        wezterm_mod.set("run_child_process", lua.create_function(run_child_process)?)?;
        wezterm_mod.set("on", lua.create_function(register_event)?)?;
        wezterm_mod.set("emit", lua.create_async_function(emit_event)?)?;

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

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
struct TextStyleAttributes {
    /// Whether the font should be a bold variant
    #[serde(default)]
    pub bold: bool,
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
    (family, map_defaults): (String, Option<TextStyleAttributes>),
) -> mlua::Result<TextStyle> {
    let attrs = map_defaults.unwrap_or_else(TextStyleAttributes::default);
    let mut text_style = TextStyle::default();

    text_style.font.clear();
    text_style.font.push(FontAttributes {
        family,
        bold: attrs.bold,
        italic: attrs.italic,
    });
    text_style.foreground = attrs.foreground;

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
    (fallback, map_defaults): (Vec<String>, Option<TextStyleAttributes>),
) -> mlua::Result<TextStyle> {
    let attrs = map_defaults.unwrap_or_else(TextStyleAttributes::default);
    let mut text_style = TextStyle::default();

    text_style.font.clear();
    for family in fallback {
        text_style.font.push(FontAttributes {
            family,
            bold: attrs.bold,
            italic: attrs.italic,
        });
    }
    text_style.foreground = attrs.foreground;

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

fn run_child_process<'lua>(
    _: &'lua Lua,
    args: Vec<String>,
) -> mlua::Result<(bool, BString, BString)> {
    let mut cmd = std::process::Command::new(&args[0]);

    if args.len() > 1 {
        cmd.args(&args[1..]);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|e| mlua::Error::external(e))?;

    Ok((
        output.status.success(),
        output.stdout.into(),
        output.stderr.into(),
    ))
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

        let lua = make_lua_context(&std::env::current_dir()?)?;

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
