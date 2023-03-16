use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua, ToLua, Value as LuaValue};
use serde_json::{Map, Value as JValue};
use std::collections::HashSet;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("json_parse", lua.create_function(json_parse)?)?;
    wezterm_mod.set("json_encode", lua.create_function(json_encode)?)?;
    Ok(())
}

fn json_value_to_lua_value<'lua>(lua: &'lua Lua, value: JValue) -> mlua::Result<LuaValue> {
    Ok(match value {
        JValue::Null => LuaValue::Nil,
        JValue::Bool(b) => LuaValue::Boolean(b),
        JValue::Number(n) => match n.as_i64() {
            Some(n) => LuaValue::Integer(n),
            None => match n.as_f64() {
                Some(n) => LuaValue::Number(n),
                None => {
                    return Err(mlua::Error::external(format!(
                        "cannot represent {n:#?} as either i64 or f64"
                    )))
                }
            },
        },
        JValue::String(s) => s.to_lua(lua)?,
        JValue::Array(arr) => {
            let tbl = lua.create_table_with_capacity(arr.len() as i32, 0)?;
            for (idx, value) in arr.into_iter().enumerate() {
                tbl.set(idx + 1, json_value_to_lua_value(lua, value)?)?;
            }
            LuaValue::Table(tbl)
        }
        JValue::Object(map) => {
            let tbl = lua.create_table_with_capacity(0, map.len() as i32)?;
            for (key, value) in map.into_iter() {
                let key = key.to_lua(lua)?;
                let value = json_value_to_lua_value(lua, value)?;
                tbl.set(key, value)?;
            }
            LuaValue::Table(tbl)
        }
    })
}

fn json_parse<'lua>(lua: &'lua Lua, text: String) -> mlua::Result<LuaValue> {
    let value =
        serde_json::from_str(&text).map_err(|err| mlua::Error::external(format!("{err:#}")))?;
    json_value_to_lua_value(lua, value)
}

fn lua_value_to_json_value(value: LuaValue, visited: &mut HashSet<usize>) -> mlua::Result<JValue> {
    if let LuaValue::Table(_) = &value {
        let ptr = value.to_pointer() as usize;
        if visited.contains(&ptr) {
            // Skip this one, as we've seen it before.
            // Treat it as a Null value.
            return Ok(JValue::Null);
        }
        visited.insert(ptr);
    }
    Ok(match value {
        LuaValue::Nil => JValue::Null,
        LuaValue::String(s) => JValue::String(s.to_str()?.to_string()),
        LuaValue::Boolean(b) => JValue::Bool(b),
        LuaValue::Integer(i) => JValue::Number(i.into()),
        LuaValue::Number(i) => {
            if let Some(n) = serde_json::value::Number::from_f64(i) {
                JValue::Number(n)
            } else {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "number",
                    to: "JsonValue",
                    message: Some(format!("unable to represent {i} as json float")),
                });
            }
        }
        // Handle our special Null userdata case and map it to Null
        LuaValue::LightUserData(ud) if ud.0.is_null() => JValue::Null,
        LuaValue::LightUserData(_) | LuaValue::UserData(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "userdata",
                to: "JsonValue",
                message: None,
            })
        }
        LuaValue::Function(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "function",
                to: "JsonValue",
                message: None,
            })
        }
        LuaValue::Thread(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "thread",
                to: "JsonValue",
                message: None,
            })
        }
        LuaValue::Error(e) => return Err(e),
        LuaValue::Table(table) => {
            if let Ok(true) = table.contains_key(1) {
                let mut array = vec![];
                let pairs = table.clone();
                for value in table.sequence_values() {
                    array.push(lua_value_to_json_value(value?, visited)?);
                }

                for pair in pairs.pairs::<LuaValue, LuaValue>() {
                    let (key, _value) = pair?;
                    match &key {
                        LuaValue::Integer(n) if *n >= 1 && *n as usize <= array.len() => {
                            // Ok!
                        }
                        _ => {
                            let type_name = key.type_name();
                            let key = luahelper::ValuePrinter(key);
                            return Err(mlua::Error::FromLuaConversionError {
                                from: type_name,
                                to: "numeric array index",
                                message: Some(format!(
                                    "Unexpected key {key:?} for array style table"
                                )),
                            });
                        }
                    }
                }

                JValue::Array(array.into())
            } else {
                let mut obj = Map::default();
                for pair in table.pairs::<LuaValue, LuaValue>() {
                    let (key, value) = pair?;
                    let key_type = key.type_name();
                    let key = match lua_value_to_json_value(key, visited)? {
                        JValue::String(s) => s,
                        _ => {
                            return Err(mlua::Error::FromLuaConversionError {
                                from: key_type,
                                to: "string",
                                message: Some("json object keys must be strings".to_string()),
                            });
                        }
                    };
                    let lua_type = value.type_name();
                    let value = lua_value_to_json_value(value, visited).map_err(|e| {
                        mlua::Error::FromLuaConversionError {
                            from: lua_type,
                            to: "value",
                            message: Some(format!("while processing {key:?}: {e}")),
                        }
                    })?;
                    obj.insert(key, value);
                }
                JValue::Object(obj.into())
            }
        }
    })
}

fn json_encode<'lua>(_: &'lua Lua, value: LuaValue) -> mlua::Result<String> {
    let mut visited = HashSet::new();
    let json = lua_value_to_json_value(value, &mut visited)?;
    serde_json::to_string(&json).map_err(|err| mlua::Error::external(format!("{err:#}")))
}
