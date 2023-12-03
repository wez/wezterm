use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, MultiValue as LuaMultiValue, Table, Value as LuaValue};
use luahelper::impl_lua_conversion_dynamic;
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
    Top,
    /// Recursively go through tables
    Deep,
}
impl_lua_conversion_dynamic!(DepthMode);

// merge tables
// (in case of overlap of the tables, we default to taking the key-value pair from the last table)
// Note that we don't use a HashMap since we want to keep the order of the tables, which
// can be useful in some cases
fn extend<'lua>(
    lua: &'lua Lua,
    (array_of_tables, behavior): (Vec<Table<'lua>>, Option<ConflictMode>),
) -> mlua::Result<Table<'lua>> {
    let mut tbl_vec: Vec<(LuaValue, LuaValue)> = vec![];
    for table in array_of_tables {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            tbl_vec.push((key, value));
        }
    }
    let tbl_len = tbl_vec.len();
    // note we might allocate a bit too much here, but in many use cases we will be correct
    let tbl: Table<'lua> = lua.create_table_with_capacity(0, tbl_len)?;

    let behavior = behavior.unwrap_or_default();
    for (key, value) in tbl_vec {
        if !tbl.contains_key(key.clone())? {
            tbl.set(key, value)?;
        } else if behavior == ConflictMode::Force {
            tbl.set(key, value)?;
        } else if behavior == ConflictMode::Error {
            return Err(mlua::Error::runtime(format!(
                "The key {} is in more than one of the tables.",
                key.to_string()?
            )));
        }
    }

    Ok(tbl)
}

// merge tables entrywise recursively
// (in case of overlap of the tables, we default to taking the key-value pair from the last table)
// Note that we don't use a HashMap since we want to keep the order of the tables, which
// can be useful in some cases
fn deep_extend<'lua>(
    lua: &'lua Lua,
    (array_of_tables, behavior): (Vec<Table<'lua>>, Option<ConflictMode>),
) -> mlua::Result<Table<'lua>> {
    let mut tbl_vec: Vec<(LuaValue, LuaValue)> = vec![];
    for table in array_of_tables {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            tbl_vec.push((key, value));
        }
    }
    let tbl_len = tbl_vec.len();
    // note we might allocate a bit too much here, but in many use cases we will be correct
    let tbl: Table<'lua> = lua.create_table_with_capacity(0, tbl_len)?;

    let behavior = behavior.unwrap_or_default();
    for (key, value) in tbl_vec {
        if !tbl.contains_key(key.clone())? {
            tbl.set(key, value)?;
        } else if let LuaValue::Table(t) = value {
            let inner_tbl = deep_extend(lua, (vec![tbl.get(key.clone())?, t], Some(behavior)))?;
            tbl.set(key, inner_tbl)?;
        } else if behavior == ConflictMode::Force {
            tbl.set(key, value)?;
        } else if behavior == ConflictMode::Error {
            return Err(mlua::Error::runtime(format!(
                "The key {} is in more than one of the tables.",
                key.to_string()?
            )));
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
            DepthMode::Top => res.set(key, value)?,
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
                    for elem in tbl.sequence_values::<LuaValue>() {
                        flat_vec.push(elem?);
                    }
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
    (table, key, mut extra_keys): (Table<'lua>, LuaValue<'lua>, LuaMultiValue<'lua>),
) -> mlua::Result<LuaValue<'lua>> {
    if extra_keys.is_empty() {
        return Ok(table.get::<_, LuaValue>(key)?);
    }

    let mut value: LuaValue = table.get(key.clone())?;

    let mut value_tbl = match table.get::<_, Table>(key) {
        Ok(t) => t,
        Err(_) => return Ok(LuaValue::Nil), // if extra_keys were empty, we wouldn't get here
    };

    while let Some(next_key) = extra_keys.pop_front() {
        value = value_tbl.get(next_key.clone())?;
        let new_val_tbl = value_tbl.get::<_, Table>(next_key);
        value_tbl = match new_val_tbl {
            Ok(t) => t,
            Err(_) => {
                if extra_keys.is_empty() {
                    return Ok(value);
                } else {
                    return Ok(LuaValue::Nil);
                }
            }
        }
    }

    Ok(value)
}

fn has_key<'lua>(
    _: &'lua Lua,
    (table, key, mut extra_keys): (Table<'lua>, LuaValue, LuaMultiValue),
) -> mlua::Result<bool> {
    if extra_keys.is_empty() {
        return Ok(table.contains_key(key)?);
    }

    let mut value_has_key = table.contains_key(key.clone())?;

    let mut value = match table.get::<_, Table>(key) {
        Ok(t) => t,
        Err(_) => return Ok(false), // if extra_keys were empty, we wouldn't get here
    };

    while let Some(next_key) = extra_keys.pop_front() {
        value_has_key = value.contains_key(next_key.clone())?;
        let new_val = value.get::<_, Table>(next_key);
        value = match new_val {
            Ok(t) => t,
            Err(_) => return Ok(value_has_key && extra_keys.is_empty()),
        };
    }

    Ok(value_has_key)
}

fn has_value<'lua>(lua: &'lua Lua, (table, value, behavior): (Table<'lua>, LuaValue, Option<DepthMode>)) -> mlua::Result<bool> {
    let behavior = behavior.unwrap_or_default();
    match behavior {
        DepthMode::Top => {
            // we don't need a clone in this case
            for pair in table.pairs::<LuaValue, LuaValue>() {
                let (_, tbl_value) = pair?;
                if tbl_value == value {
                    return Ok(true);
                }
            }
        }
        DepthMode::Deep => {
            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (key, tbl_value) = pair?;
                if tbl_value == value {
                    return Ok(true);
                }
                if tbl_value.is_table() {
                    let tbl = table.get::<_, Table>(key)?;
                    if let Ok(true) = has_value(lua, (tbl, value.clone(), Some(behavior))) {
                        return Ok(true);
                    }
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
