use crate::exec_domain::{ExecDomain, ValueOrFunc};
use crate::keyassignment::KeyAssignment;
use crate::{
    Config, FontAttributes, FontStretch, FontStyle, FontWeight, FreeTypeLoadTarget, RgbaColor,
    TextStyle,
};
use anyhow::{anyhow, Context};
use luahelper::{from_lua_value_dynamic, lua_value_to_dynamic, to_lua};
use mlua::{FromLua, IntoLuaMulti, Lua, Table, Value, Variadic};
use ordered_float::NotNan;
use portable_pty::CommandBuilder;
use std::convert::TryFrom;
use std::path::Path;
use std::sync::Mutex;
use wezterm_dynamic::{
    FromDynamic, FromDynamicOptions, ToDynamic, UnknownFieldAction, Value as DynValue,
};

pub use mlua;

static LUA_REGISTRY_USER_CALLBACK_COUNT: &str = "wezterm-user-callback-count";

pub type SetupFunc = fn(&Lua) -> anyhow::Result<()>;

lazy_static::lazy_static! {
    static ref SETUP_FUNCS: Mutex<Vec<SetupFunc>> = Mutex::new(vec![]);
}

pub fn add_context_setup_func(func: SetupFunc) {
    SETUP_FUNCS.lock().unwrap().push(func);
}

pub fn get_or_create_module<'lua>(lua: &'lua Lua, name: &str) -> anyhow::Result<mlua::Table<'lua>> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let module = lua.create_table()?;
            loaded.set(name, module.clone())?;
            Ok(module)
        }
        Value::Table(table) => Ok(table),
        wat => anyhow::bail!(
            "cannot register module {} as package.loaded.{} is already set to a value of type {}",
            name,
            name,
            wat.type_name()
        ),
    }
}

pub fn get_or_create_sub_module<'lua>(
    lua: &'lua Lua,
    name: &str,
) -> anyhow::Result<mlua::Table<'lua>> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    let sub = wezterm_mod.get(name)?;
    match sub {
        Value::Nil => {
            let sub = lua.create_table()?;
            wezterm_mod.set(name, sub.clone())?;
            Ok(sub)
        }
        Value::Table(sub) => Ok(sub),
        wat => anyhow::bail!(
            "cannot register module wezterm.{name} as it is already set to a value of type {}",
            wat.type_name()
        ),
    }
}

fn config_builder_set_strict_mode<'lua>(
    _lua: &'lua Lua,
    (myself, strict): (Table, bool),
) -> mlua::Result<()> {
    let mt = myself
        .get_metatable()
        .ok_or_else(|| mlua::Error::external("impossible that we have no metatable"))?;
    mt.set("__strict_mode", strict)
}

fn config_builder_index<'lua>(
    _lua: &'lua Lua,
    (myself, key): (Table<'lua>, mlua::Value<'lua>),
) -> mlua::Result<mlua::Value<'lua>> {
    let mt = myself
        .get_metatable()
        .ok_or_else(|| mlua::Error::external("impossible that we have no metatable"))?;
    match mt.get(key.clone()) {
        Ok(value) => Ok(value),
        _ => myself.raw_get(key),
    }
}

fn config_builder_new_index<'lua>(
    lua: &'lua Lua,
    (myself, key, value): (Table, String, Value),
) -> mlua::Result<()> {
    let stub_config = lua.create_table()?;
    stub_config.set(key.clone(), value.clone())?;

    let dvalue = lua_value_to_dynamic(Value::Table(stub_config)).map_err(|e| {
        mlua::Error::FromLuaConversionError {
            from: "table",
            to: "Config",
            message: Some(format!("lua_value_to_dynamic: {e}")),
        }
    })?;

    let mt = myself
        .get_metatable()
        .ok_or_else(|| mlua::Error::external("impossible that we have no metatable"))?;
    let strict = match mt.get("__strict_mode") {
        Ok(Value::Boolean(b)) => b,
        _ => true,
    };

    let options = FromDynamicOptions {
        unknown_fields: if strict {
            UnknownFieldAction::Deny
        } else {
            UnknownFieldAction::Warn
        },
        deprecated_fields: UnknownFieldAction::Warn,
    };

    let config_object = Config::from_dynamic(&dvalue, options).map_err(|e| {
        mlua::Error::FromLuaConversionError {
            from: "table",
            to: "Config",
            message: Some(format!("Config::from_dynamic: {e}")),
        }
    })?;

    match config_object.to_dynamic() {
        DynValue::Object(obj) => {
            match obj.get_by_str(&key) {
                None => {
                    // Show a stack trace to help them figure out where they made
                    // a mistake. This path is taken when they are not in strict
                    // mode, and we want to print some more context after the from_dynamic
                    // impl has logged a warning and suggested alternative field names.
                    let mut message =
                        format!("Attempted to set invalid config option `{key}` at:\n");
                    // Start at frame 1, our caller, as the frame for invoking this
                    // metamethod is not interesting
                    for i in 1.. {
                        if let Some(debug) = lua.inspect_stack(i) {
                            let names = debug.names();
                            let name = names.name;
                            let name_what = names.name_what;

                            let dbg_source = debug.source();
                            let source = dbg_source.source.unwrap_or_default();
                            let func_name = match (name, name_what) {
                                (Some(name), Some(name_what)) => {
                                    format!("{name_what} {name}")
                                }
                                (Some(name), None) => format!("{name}"),
                                _ => "".to_string(),
                            };

                            let line = debug.curr_line();
                            message.push_str(&format!("    [{i}] {source}:{line} {func_name}\n"));
                        } else {
                            break;
                        }
                    }
                    wezterm_dynamic::Error::warn(message);
                }
                Some(_dvalue) => {
                    myself.raw_set(key, value)?;
                }
            };
            Ok(())
        }
        _ => Err(mlua::Error::external(
            "computed config object is, impossibly, not an object",
        )),
    }
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
pub fn make_lua_context(config_file: &Path) -> anyhow::Result<Lua> {
    let lua = Lua::new();

    let config_dir = config_file.parent().unwrap_or_else(|| Path::new("/"));

    {
        let globals = lua.globals();
        // This table will be the `wezterm` module in the script
        let wezterm_mod = get_or_create_module(&lua, "wezterm")?;

        let package: Table = globals.get("package").context("get _G.package")?;
        let package_path: String = package.get("path").context("get package.path as String")?;
        let mut path_array: Vec<String> = package_path.split(";").map(|s| s.to_owned()).collect();

        fn prefix_path(array: &mut Vec<String>, path: &Path) {
            array.insert(0, format!("{}/?.lua", path.display()));
            array.insert(1, format!("{}/?/init.lua", path.display()));
        }

        prefix_path(&mut path_array, &crate::HOME_DIR.join(".wezterm"));
        for dir in crate::CONFIG_DIRS.iter() {
            prefix_path(&mut path_array, dir);
        }
        path_array.insert(
            2,
            format!("{}/plugins/?/plugin/init.lua", crate::DATA_DIR.display()),
        );

        if let Ok(exe) = std::env::current_exe() {
            if let Some(path) = exe.parent() {
                wezterm_mod
                    .set(
                        "executable_dir",
                        path.to_str()
                            .ok_or_else(|| anyhow!("current_exe path is not UTF-8"))?,
                    )
                    .context("set wezterm.executable_dir")?;
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

        // Hook into loader and arrange to watch all require'd files.
        // <https://www.lua.org/manual/5.3/manual.html#pdf-package.searchers>
        // says that the second searcher function is the one that is responsible
        // for loading lua files, so we shim around that and speculatively
        // add the name of the file that it would find (as returned from
        // package.searchpath) to the watch list, then we just call the
        // original implementation.
        lua.load(
            r#"
local orig = package.searchers[2]
package.searchers[2] = function(module)
  local name, err = package.searchpath(module, package.path)
  if name then
    package.loaded.wezterm.add_to_config_reload_watch_list(name)
  end
  return orig(module)
end
        "#,
        )
        .set_name("=searcher")
        .eval::<()>()
        .context("replace package.searchers")?;

        wezterm_mod.set(
            "config_builder",
            lua.create_function(|lua, _: ()| {
                let config = lua.create_table()?;
                let mt = lua.create_table()?;

                mt.set("__index", lua.create_function(config_builder_index)?)?;
                mt.set("__newindex", lua.create_function(config_builder_new_index)?)?;
                mt.set(
                    "set_strict_mode",
                    lua.create_function(config_builder_set_strict_mode)?,
                )?;

                config.set_metatable(Some(mt));

                Ok(config)
            })?,
        )?;

        wezterm_mod.set(
            "reload_configuration",
            lua.create_function(|_, _: ()| {
                crate::reload();
                Ok(())
            })?,
        )?;
        wezterm_mod
            .set("config_file", config_file_str)
            .context("set wezterm.config_file")?;
        wezterm_mod
            .set(
                "config_dir",
                config_dir
                    .to_str()
                    .ok_or_else(|| anyhow!("config dir path is not UTF-8"))?,
            )
            .context("set wezterm.config_dir")?;

        lua.set_named_registry_value("wezterm-watch-paths", Vec::<String>::new())?;
        wezterm_mod.set(
            "add_to_config_reload_watch_list",
            lua.create_function(add_to_config_reload_watch_list)?,
        )?;

        wezterm_mod.set("target_triple", crate::wezterm_target_triple())?;
        wezterm_mod.set("version", crate::wezterm_version())?;
        wezterm_mod.set("home_dir", crate::HOME_DIR.to_str())?;
        wezterm_mod.set(
            "running_under_wsl",
            lua.create_function(|_, ()| Ok(crate::running_under_wsl()))?,
        )?;

        wezterm_mod.set(
            "default_wsl_domains",
            lua.create_function(|_, ()| Ok(crate::WslDomain::default_domains()))?,
        )?;

        wezterm_mod.set("font", lua.create_function(font)?)?;
        wezterm_mod.set(
            "font_with_fallback",
            lua.create_function(font_with_fallback)?,
        )?;
        wezterm_mod.set("hostname", lua.create_function(hostname)?)?;
        wezterm_mod.set("action", luahelper::enumctor::Enum::<KeyAssignment>::new())?;
        wezterm_mod.set(
            "has_action",
            lua.create_function(|_lua, name: String| {
                Ok(KeyAssignment::variants().contains(&name.as_str()))
            })?,
        )?;

        lua.set_named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT, 0)?;
        wezterm_mod.set("action_callback", lua.create_function(action_callback)?)?;
        wezterm_mod.set("exec_domain", lua.create_function(exec_domain)?)?;

        wezterm_mod.set("utf16_to_utf8", lua.create_function(utf16_to_utf8)?)?;
        wezterm_mod.set("split_by_newlines", lua.create_function(split_by_newlines)?)?;
        wezterm_mod.set("on", lua.create_function(register_event)?)?;
        wezterm_mod.set("emit", lua.create_async_function(emit_event)?)?;
        wezterm_mod.set("shell_join_args", lua.create_function(shell_join_args)?)?;
        wezterm_mod.set("shell_quote_arg", lua.create_function(shell_quote_arg)?)?;
        wezterm_mod.set("shell_split", lua.create_function(shell_split)?)?;

        wezterm_mod.set(
            "default_hyperlink_rules",
            lua.create_function(move |lua, ()| {
                let rules = crate::config::default_hyperlink_rules();
                Ok(to_lua(lua, rules))
            })?,
        )?;

        // Define our own os.getenv function that knows how to resolve current
        // environment values from eg: the registry on Windows, or for
        // the current SHELL value on unix, even if the user has changed
        // those values since wezterm was started
        get_or_create_module(&lua, "os")?.set("getenv", lua.create_function(getenv)?)?;

        package
            .set("path", path_array.join(";"))
            .context("assign package.path")?;
    }

    for func in SETUP_FUNCS.lock().unwrap().iter() {
        func(&lua).context("calling SETUP_FUNCS")?;
    }

    Ok(lua)
}

/// Resolve an environment variable.
/// Lean on CommandBuilder's ability to update to current values of certain
/// environment variables that may be adjusted via the registry or implicitly
/// via eg: chsh (SHELL).
fn getenv<'lua>(_: &'lua Lua, env: String) -> mlua::Result<Option<String>> {
    let cmd = CommandBuilder::new_default_prog();
    match cmd.get_env(&env) {
        Some(s) => match s.to_str() {
            Some(s) => Ok(Some(s.to_string())),
            None => Err(mlua::Error::external(format!(
                "env var {env} is not representable as UTF-8"
            ))),
        },
        None => Ok(None),
    }
}

fn shell_split<'lua>(_: &'lua Lua, line: String) -> mlua::Result<Vec<String>> {
    shlex::split(&line).ok_or_else(|| {
        mlua::Error::external(format!("cannot tokenize `{line}` using posix shell rules"))
    })
}

fn shell_join_args<'lua>(_: &'lua Lua, args: Vec<String>) -> mlua::Result<String> {
    Ok(shlex::try_join(args.iter().map(|arg| arg.as_ref())).map_err(mlua::Error::external)?)
}

fn shell_quote_arg<'lua>(_: &'lua Lua, arg: String) -> mlua::Result<String> {
    Ok(shlex::try_quote(&arg)
        .map_err(mlua::Error::external)?
        .into_owned())
}

/// Returns the system hostname.
/// Errors may occur while retrieving the hostname from the system,
/// or if the hostname isn't a UTF-8 string.
fn hostname<'lua>(_: &'lua Lua, _: ()) -> mlua::Result<String> {
    let hostname = hostname::get().map_err(mlua::Error::external)?;
    match hostname.to_str() {
        Some(hostname) => Ok(hostname.to_owned()),
        None => Err(mlua::Error::external(anyhow!("hostname isn't UTF-8"))),
    }
}

#[derive(Debug, Default, FromDynamic, ToDynamic, Clone, PartialEq, Eq, Hash)]
struct TextStyleAttributes {
    /// Whether the font should be a bold variant
    #[dynamic(default)]
    pub bold: Option<bool>,
    #[dynamic(default)]
    pub weight: Option<FontWeight>,
    #[dynamic(default)]
    pub stretch: FontStretch,
    /// Whether the font should be an italic variant
    #[dynamic(default)]
    pub style: FontStyle,
    // Ideally we'd simply use serde's aliasing functionality on the `style`
    // field to support backwards compatibility, but aliases are invisible
    // to serde_lua, so we do a little fixup here ourselves in our from_lua impl.
    italic: Option<bool>,
    /// If set, when rendering text that is set to the default
    /// foreground color, use this color instead.  This is most
    /// useful in a `[[font_rules]]` section to implement changing
    /// the text color for eg: bold text.
    pub foreground: Option<RgbaColor>,
}
impl<'lua> FromLua<'lua> for TextStyleAttributes {
    fn from_lua(value: Value<'lua>, _lua: &'lua Lua) -> Result<Self, mlua::Error> {
        let mut attr: TextStyleAttributes = from_lua_value_dynamic(value)?;
        if let Some(italic) = attr.italic.take() {
            attr.style = if italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            };
        }
        Ok(attr)
    }
}

#[derive(Debug, Default, FromDynamic, ToDynamic, Clone, PartialEq, Eq, Hash)]
struct LuaFontAttributes {
    /// The font family name
    pub family: String,
    /// Whether the font should be a bold variant
    #[dynamic(default)]
    pub weight: FontWeight,
    #[dynamic(default)]
    pub stretch: FontStretch,
    /// Whether the font should be an italic variant
    #[dynamic(default)]
    pub style: FontStyle,
    // Ideally we'd simply use serde's aliasing functionality on the `style`
    // field to support backwards compatibility, but aliases are invisible
    // to serde_lua, so we do a little fixup here ourselves in our from_lua impl.
    #[dynamic(default)]
    italic: Option<bool>,

    #[dynamic(default)]
    pub harfbuzz_features: Option<Vec<String>>,
    #[dynamic(default)]
    pub freetype_load_target: Option<FreeTypeLoadTarget>,
    #[dynamic(default)]
    pub freetype_render_target: Option<FreeTypeLoadTarget>,
    #[dynamic(default)]
    pub freetype_load_flags: Option<String>,
    #[dynamic(default)]
    pub scale: Option<NotNan<f64>>,
    #[dynamic(default)]
    pub assume_emoji_presentation: Option<bool>,
}
impl<'lua> FromLua<'lua> for LuaFontAttributes {
    fn from_lua(value: Value<'lua>, _lua: &'lua Lua) -> Result<Self, mlua::Error> {
        match value {
            Value::String(s) => {
                let mut attr = LuaFontAttributes::default();
                attr.family = s.to_str()?.to_string();
                Ok(attr)
            }
            v => {
                let mut attr: LuaFontAttributes = from_lua_value_dynamic(v)?;
                if let Some(italic) = attr.italic.take() {
                    attr.style = if italic {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    };
                }
                Ok(attr)
            }
        }
    }
}

/// On macOS, both Menlo and Monaco fonts have ligatures for `fi` that
/// take effect for words like `find` and which are a source of
/// confusion/annoyance and issues filed on Github.
/// Let's default to disabling ligatures for these fonts unless
/// the user has explicitly specified harfbuzz_features.
/// <https://github.com/wezterm/wezterm/issues/1736>
/// <https://github.com/wezterm/wezterm/issues/1786>
fn disable_ligatures_for_menlo_or_monaco(mut attrs: FontAttributes) -> FontAttributes {
    if attrs.harfbuzz_features.is_none() && (attrs.family == "Menlo" || attrs.family == "Monaco") {
        attrs.harfbuzz_features = Some(vec![
            "kern".to_string(),
            "clig".to_string(),
            "liga=0".to_string(),
        ]);
    }
    attrs
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
        attrs.style = map_defaults.style;
        text_style.foreground = map_defaults.foreground;
    }

    text_style
        .font
        .push(disable_ligatures_for_menlo_or_monaco(FontAttributes {
            family: attrs.family,
            stretch: attrs.stretch,
            weight: attrs.weight,
            style: attrs.style,
            is_fallback: false,
            is_synthetic: false,
            harfbuzz_features: attrs.harfbuzz_features,
            freetype_load_target: attrs.freetype_load_target,
            freetype_render_target: attrs.freetype_render_target,
            freetype_load_flags: match attrs.freetype_load_flags {
                Some(flags) => Some(TryFrom::try_from(flags).map_err(mlua::Error::external)?),
                None => None,
            },
            scale: attrs.scale,
            assume_emoji_presentation: attrs.assume_emoji_presentation,
        }));

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
            attrs.style = map_defaults.style;
            text_style.foreground = map_defaults.foreground;
        }

        text_style
            .font
            .push(disable_ligatures_for_menlo_or_monaco(FontAttributes {
                family: attrs.family,
                stretch: attrs.stretch,
                weight: attrs.weight,
                style: attrs.style,
                is_fallback: idx != 0,
                is_synthetic: false,
                harfbuzz_features: attrs.harfbuzz_features,
                freetype_load_target: attrs.freetype_load_target,
                freetype_render_target: attrs.freetype_render_target,
                freetype_load_flags: match attrs.freetype_load_flags {
                    Some(flags) => Some(TryFrom::try_from(flags).map_err(mlua::Error::external)?),
                    None => None,
                },
                scale: attrs.scale,
                assume_emoji_presentation: attrs.assume_emoji_presentation,
            }));
    }

    Ok(text_style)
}

pub fn wrap_callback<'lua>(lua: &'lua Lua, callback: mlua::Function) -> mlua::Result<String> {
    let callback_count: i32 = lua.named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT)?;
    let user_event_id = format!("user-defined-{}", callback_count);
    lua.set_named_registry_value(LUA_REGISTRY_USER_CALLBACK_COUNT, callback_count + 1)?;
    register_event(lua, (user_event_id.clone(), callback))?;
    Ok(user_event_id)
}

fn action_callback<'lua>(lua: &'lua Lua, callback: mlua::Function) -> mlua::Result<KeyAssignment> {
    let user_event_id = wrap_callback(lua, callback)?;
    Ok(KeyAssignment::EmitEvent(user_event_id))
}

fn exec_domain<'lua>(
    lua: &'lua Lua,
    (name, fixup_command, label): (String, mlua::Function, Option<mlua::Value>),
) -> mlua::Result<ExecDomain> {
    let fixup_command = {
        let event_name = format!("exec-domain-{name}");
        register_event(lua, (event_name.clone(), fixup_command))?;
        event_name
    };

    let label = match label {
        Some(Value::Function(callback)) => {
            let event_name = format!("exec-domain-{name}-label");
            register_event(lua, (event_name.clone(), callback))?;
            Some(ValueOrFunc::Func(event_name))
        }
        Some(Value::String(value)) => Some(ValueOrFunc::Value(lua_value_to_dynamic(
            Value::String(value),
        )?)),
        Some(_) => {
            return Err(mlua::Error::external(
                "label function parameter must be either a string or a lua function",
            ))
        }
        None => None,
    };
    Ok(ExecDomain {
        name,
        fixup_command,
        label,
    })
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
pub fn register_event<'lua>(
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

const IS_EVENT: &str = "wezterm-is-event-emission";

/// Returns true if the current lua context is being called as part
/// of an emit_event call.
pub fn is_event_emission<'lua>(lua: &'lua Lua) -> mlua::Result<bool> {
    lua.named_registry_value(IS_EVENT)
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
    lua.set_named_registry_value(IS_EVENT, true)?;

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
    A: IntoLuaMulti<'lua>,
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

pub async fn emit_async_callback<'lua, A>(
    lua: &'lua Lua,
    (name, args): (String, A),
) -> mlua::Result<mlua::Value<'lua>>
where
    A: IntoLuaMulti<'lua>,
{
    let decorated_name = format!("wezterm-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&decorated_name)?;
    match tbl {
        mlua::Value::Table(tbl) => {
            for func in tbl.sequence_values::<mlua::Function>() {
                let func = func?;
                return func.call_async(args).await;
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

    String::from_utf16(wide).map_err(mlua::Error::external)
}

pub fn add_to_config_reload_watch_list<'lua>(
    lua: &'lua Lua,
    args: Variadic<String>,
) -> mlua::Result<()> {
    let mut watch_paths: Vec<String> = lua.named_registry_value("wezterm-watch-paths")?;
    watch_paths.extend_from_slice(&args);
    lua.set_named_registry_value("wezterm-watch-paths", watch_paths)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn can_register_and_emit_multiple_events() -> anyhow::Result<()> {
        let _ = env_logger::Builder::new()
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
    print("lua hook recording " .. n);
end);

-- one of the foo handlers returns false, so the emit
-- returns false overall, indicating that the default
-- action should not be taken
assert(wezterm.emit('foo', 2) == false)

wezterm.on('bar', function (n, str)
    print("bar says " .. n .. " " .. str)
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
