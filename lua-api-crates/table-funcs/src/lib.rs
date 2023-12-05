use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, MultiValue as LuaMultiValue, Table, Value as LuaValue};
use luahelper::impl_lua_conversion_dynamic;
use luahelper::mlua::AnyUserDataExt;
use wezterm_dynamic::{FromDynamic, ToDynamic};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let table = get_or_create_sub_module(lua, "table")?;
    table.set("extend", lua.create_function(extend)?)?;
    table.set("deep_extend", lua.create_function(deep_extend)?)?;
    table.set("clone", lua.create_function(clone)?)?;
    table.set("flatten", lua.create_function(flatten)?)?;
    table.set("count", lua.create_function(count)?)?;
    table.set("get", lua.create_function(get)?)?;
    table.set("has_key", lua.create_function(has_key)?)?;
    table.set("has_value", lua.create_function(has_value)?)?;
    table.set("equal", lua.create_function(equal)?)?;

    Ok(())
}

#[derive(Default, Debug, FromDynamic, ToDynamic, Clone, PartialEq, Eq, Copy)]
enum ConflictMode {
    /// Retain the existing value
    Keep,
    /// Take the latest value
    #[default]
    Force,
    /// Raise an error
    Error,
}
impl_lua_conversion_dynamic!(ConflictMode);

#[derive(Default, Debug, FromDynamic, ToDynamic, Clone, PartialEq, Eq, Copy)]
enum DepthMode {
    /// Only look at the top level of tables
    #[default]
    Shallow,
    /// Recursively go through tables
    Deep,
}
impl_lua_conversion_dynamic!(DepthMode);

// merge tables
// (in case of overlap of the tables, we default to taking the key-value pair from the last table)
fn extend<'lua>(
    lua: &'lua Lua,
    (array_of_tables, behavior): (Vec<Table<'lua>>, Option<ConflictMode>),
) -> mlua::Result<Table<'lua>> {
    let behavior = behavior.unwrap_or_default();
    let tbl: Table<'lua> = lua.create_table()?;
    for table in array_of_tables {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;

            if !tbl.contains_key(key.clone())? {
                tbl.set(key, value)?;
            } else if behavior == ConflictMode::Force {
                tbl.set(key, value)?;
            } else if behavior == ConflictMode::Error {
                return Err(mlua::Error::runtime(format!(
                    "The key '{}' is in more than one of the tables.",
                    key.to_string()?
                )));
            }
        }
    }

    Ok(tbl)
}

// merge tables entrywise recursively
// (in case of overlap of the tables, we default to taking the key-value pair from the last table)
fn deep_extend<'lua>(
    lua: &'lua Lua,
    (array_of_tables, behavior): (Vec<Table<'lua>>, Option<ConflictMode>),
) -> mlua::Result<Table<'lua>> {
    let behavior = behavior.unwrap_or_default();
    let tbl: Table<'lua> = lua.create_table()?;
    for table in array_of_tables {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            let tbl_value = tbl.get::<_, LuaValue>(key.clone())?;

            match (tbl_value, value) {
                // if tbl[key] is set to a table value and we get a table
                (LuaValue::Table(tbl_value_table), LuaValue::Table(value_table)) => {
                    let inner_tbl =
                        deep_extend(lua, (vec![tbl_value_table, value_table], Some(behavior)))?;
                    tbl.set(key, inner_tbl)?;
                }
                (tbl_val, LuaValue::Table(value_tbl)) => {
                    // if tbl[key] is set to a non-table value, but we get a table
                    if tbl_val.is_nil() {
                        tbl.set(key, clone(lua, (value_tbl, Some(DepthMode::Deep)))?)?;
                    } else if behavior == ConflictMode::Force {
                        tbl.set(key, clone(lua, (value_tbl, Some(DepthMode::Deep)))?)?;
                    } else if behavior == ConflictMode::Error {
                        return Err(mlua::Error::runtime(format!(
                            "The key '{}' is in more than one of the tables.",
                            key.to_string()?
                        )));
                    }
                }
                (LuaValue::Table(_), val) => {
                    // if tbl[key] is set to a table, but we get a non-table value
                    if behavior == ConflictMode::Force {
                        tbl.set(key, val)?;
                    } else if behavior == ConflictMode::Error {
                        return Err(mlua::Error::runtime(format!(
                            "The key '{}' is in more than one of the tables.",
                            key.to_string()?
                        )));
                    }
                }
                (tbl_val, val) => {
                    // tbl_val and val are not tables
                    if tbl_val.is_nil() {
                        tbl.set(key, val)?;
                    } else if behavior == ConflictMode::Force {
                        tbl.set(key, val)?;
                    } else if behavior == ConflictMode::Error {
                        return Err(mlua::Error::runtime(format!(
                            "The key '{}' is in more than one of the tables.",
                            key.to_string()?
                        )));
                    }
                }
            }
        }
    }

    Ok(tbl)
}

fn clone<'lua>(
    lua: &'lua Lua,
    (table, behavior): (Table<'lua>, Option<DepthMode>),
) -> mlua::Result<Table<'lua>> {
    let res: Table<'lua> = lua.create_table()?;

    let behavior = behavior.unwrap_or_default();
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair?;
        match behavior {
            DepthMode::Shallow => res.set(key, value)?,
            DepthMode::Deep => {
                if let LuaValue::Table(tbl) = value {
                    res.set(key, clone(lua, (tbl, Some(behavior)))?)?
                } else {
                    res.set(key, value)?;
                }
            }
        }
    }
    Ok(res)
}

fn flatten<'lua>(
    lua: &'lua Lua,
    (arrays, behavior): (Vec<LuaValue<'lua>>, Option<DepthMode>),
) -> mlua::Result<Vec<LuaValue<'lua>>> {
    let mut flat_vec: Vec<LuaValue> = vec![];
    let behavior = behavior.unwrap_or_default();
    for item in arrays {
        match item {
            LuaValue::Table(tbl) => {
                if behavior == DepthMode::Deep {
                    let tbl_as_vec = tbl.sequence_values().filter_map(|x| x.ok()).collect();
                    let mut flat = flatten(lua, (tbl_as_vec, Some(behavior)))?;
                    flat_vec.append(&mut flat);
                } else {
                    let mut tbl_as_vec = tbl.sequence_values().filter_map(|x| x.ok()).collect();
                    flat_vec.append(&mut tbl_as_vec);
                }
            }
            LuaValue::Nil => (),
            other => {
                flat_vec.push(other);
            }
        }
    }
    Ok(flat_vec)
}

/// note that the `#` operator only works correctly on arrays in Lua
fn count<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<usize> {
    Ok(table.pairs::<LuaValue, LuaValue>().count())
}

fn get<'lua>(
    _: &'lua Lua,
    (table, keys): (Table<'lua>, LuaMultiValue<'lua>),
) -> mlua::Result<LuaValue<'lua>> {
    if keys.is_empty() {
        return Err(mlua::Error::runtime(
            "wezterm.table.get(<table>, <keys..>) expects at least one key, but it was called with no keys."
        ));
    }

    let mut value = LuaValue::Table(table);
    for key in keys {
        match value {
            LuaValue::Table(tbl) => {
                value = tbl.get(key)?;
            }
            LuaValue::UserData(ud) => {
                value = match ud.get(key) {
                    Ok(v) => v,
                    Err(_) => return Ok(LuaValue::Nil),
                };
            }
            _ => {
                // cannot index non-table structures
                return Ok(LuaValue::Nil);
            }
        }
    }

    Ok(value)
}

fn has_key<'lua>(
    lua: &'lua Lua,
    (table, keys): (Table<'lua>, LuaMultiValue),
) -> mlua::Result<bool> {
    if keys.is_empty() {
        return Err(mlua::Error::runtime(
            "wezterm.table.has_key(<table>, <keys..>) expects at least one key, but it was called with no keys."
        ));
    }

    Ok(!get(lua, (table, keys))?.is_nil())
}

fn has_value<'lua>(
    lua: &'lua Lua,
    (table, value, behavior): (Table<'lua>, LuaValue, Option<DepthMode>),
) -> mlua::Result<bool> {
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (_, table_val) = pair?;
        let table_has_value = match (table_val.clone(), value.clone()) {
            // for tables, compare by values using our equal function
            (LuaValue::Table(table_val_tbl), LuaValue::Table(value_tbl)) => {
                lua_table_eq(table_val_tbl, value_tbl)?
            }
            // oterwise, compare using Lua '=='
            _ => table_val.eq(&value),
        };
        if table_has_value {
            return Ok(true);
        }

        if behavior == Some(DepthMode::Deep) {
            if let LuaValue::Table(new_tbl) = table_val {
                if has_value(lua, (new_tbl, value.clone(), behavior))? {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

fn lua_value_eq(value1: LuaValue, value2: LuaValue) -> mlua::Result<bool> {
    match (value1, value2) {
        (LuaValue::Table(a), LuaValue::Table(b)) => lua_table_eq(a, b),
        (a, b) => Ok(a.eq(&b)),
    }
}

fn lua_table_eq(table1: Table, table2: Table) -> mlua::Result<bool> {
    let mut table1_len = 0;
    for pair in table1.pairs::<LuaValue, LuaValue>() {
        match pair {
            Ok((key, value)) => {
                table1_len += 1;
                match table2.get(key.clone()) {
                    Ok(value2) => {
                        if !lua_value_eq(value, value2)? {
                            return Ok(false);
                        }
                    }
                    Err(_) => return Ok(false),
                }
            }
            Err(_) => return Ok(false),
        }
    }
    let table2_len = table2.pairs::<LuaValue, LuaValue>().count();
    Ok(table1_len == table2_len)
}

fn equal<'lua>(_: &'lua Lua, (table1, table2): (Table<'lua>, Table<'lua>)) -> mlua::Result<bool> {
    lua_table_eq(table1, table2)
}
