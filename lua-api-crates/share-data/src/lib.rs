use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua, UserData, UserDataMethods};
use std::collections::HashMap;
use std::sync::Mutex;
use wezterm_dynamic::Value;

lazy_static::lazy_static! {
    static ref GLOBALS: Mutex<HashMap<String, Value>> = Mutex::new(HashMap::new());
}

struct Global {}

impl UserData for Global {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(
            mlua::MetaMethod::Index,
            |lua: &Lua, _, key: String| -> mlua::Result<Option<mlua::Value>> {
                match GLOBALS.lock().unwrap().get(key.as_str()) {
                    Some(value) => Ok(Some(
                        luahelper::dynamic_to_lua_value(lua, value.clone())
                            .map_err(|e| mlua::Error::external(format!("{:#}", e)))?,
                    )),
                    None => Ok(None),
                }
            },
        );
        methods.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, (key, value): (String, mlua::Value)| -> mlua::Result<()> {
                let value = luahelper::lua_value_to_dynamic(value)?;
                GLOBALS.lock().unwrap().insert(key, value);
                Ok(())
            },
        );
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("GLOBAL", Global {})?;
    Ok(())
}
