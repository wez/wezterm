use config::lua::get_or_create_module;
use config::lua::mlua::{Lua, Value, Variadic};
use luahelper::ValuePrinter;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;

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

    wezterm_mod.set(
        "to_string",
        lua.create_function(|_, arg: Value| {
            let res = ValuePrinter(arg);
            Ok(format!("{:#?}", res).to_string())
        })?,
    )?;

    lua.globals().set(
        "print",
        lua.create_function(|_, args: Variadic<Value>| {
            let output = print_helper(args);
            log::info!("lua: {}", output);
            Ok(())
        })?,
    )?;

    Ok(())
}

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
                let item = format!("{:#?}", ValuePrinter(item));
                output.push_str(&item);
            }
        }
    }
    output
}
