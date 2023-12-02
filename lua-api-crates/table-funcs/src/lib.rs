use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Integer, Lua, Table, Value as LuaValue};
use luahelper::{impl_lua_conversion_dynamic, ValuePrinter};
use wezterm_dynamic::{FromDynamic, ToDynamic};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let table = get_or_create_sub_module(lua, "table")?;
    table.set("extend", lua.create_function(extend)?)?;
    table.set("deep_extend", lua.create_function(deep_extend)?)?;
    table.set("clone", lua.create_function(clone)?)?;
    table.set("flatten", lua.create_function(flatten)?)?;
    table.set("length", lua.create_function(length)?)?;
    table.set("has_key", lua.create_function(has_key)?)?;
    table.set("has_value", lua.create_function(has_value)?)?;
    table.set("equal", lua.create_function(equal)?)?;
    table.set("to_string", lua.create_function(to_string)?)?;
    table.set(
        "to_string_fallback",
        lua.create_function(to_string_fallback)?,
    )?;

    Ok(())
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, PartialEq, Eq)]
enum ConflictMode {
    Keep,
    Force,
    Error,
}
impl_lua_conversion_dynamic!(ConflictMode);

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

    match behavior {
        Some(ConflictMode::Keep) => {
            for (key, value) in tbl_vec {
                if !tbl.contains_key(key.clone())? {
                    tbl.set(key, value)?;
                }
            }
        }
        // default behavior is to keep last set value
        Some(ConflictMode::Force) | None => {
            for (key, value) in tbl_vec {
                tbl.set(key, value)?;
            }
        }
        Some(ConflictMode::Error) => {
            for (key, value) in tbl_vec {
                if tbl.contains_key(key.clone())? {
                    return Err(mlua::Error::runtime(format!(
                        "The key {} is in more than one of the tables.",
                        key.to_string()?
                    )));
                }
                tbl.set(key, value)?;
            }
        }
    };

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

    match behavior {
        Some(ConflictMode::Keep) => {
            for (key, value) in tbl_vec {
                if !tbl.contains_key(key.clone())? {
                    tbl.set(key, value)?;
                } else if let LuaValue::Table(t) = value {
                    let inner_tbl = deep_extend(
                        lua,
                        (vec![tbl.get(key.clone())?, t], Some(ConflictMode::Keep)),
                    )?;
                    tbl.set(key, inner_tbl)?;
                }
            }
        }
        // default behavior is to keep last set value
        Some(ConflictMode::Force) | None => {
            for (key, value) in tbl_vec {
                if !tbl.contains_key(key.clone())? {
                    tbl.set(key, value)?;
                } else if let LuaValue::Table(t) = value {
                    let inner_tbl = deep_extend(
                        lua,
                        (vec![tbl.get(key.clone())?, t], Some(ConflictMode::Force)),
                    )?;
                    tbl.set(key, inner_tbl)?;
                } else {
                    tbl.set(key, value)?;
                }
            }
        }
        Some(ConflictMode::Error) => {
            for (key, value) in tbl_vec {
                if !tbl.contains_key(key.clone())? {
                    tbl.set(key, value)?;
                } else if let LuaValue::Table(t) = value {
                    let inner_tbl = deep_extend(
                        lua,
                        (vec![tbl.get(key.clone())?, t], Some(ConflictMode::Keep)),
                    )?;
                    tbl.set(key, inner_tbl)?;
                } else {
                    return Err(mlua::Error::runtime(format!(
                        "The key {} is in more than one of the tables.",
                        key.to_string()?
                    )));
                }
            }
        }
    };

    Ok(tbl)
}

fn clone<'lua>(lua: &'lua Lua, table: Table<'lua>) -> mlua::Result<Table<'lua>> {
    let table_len = table.clone().pairs::<LuaValue, LuaValue>().count();
    let res: Table<'lua> = lua.create_table_with_capacity(0, table_len)?;

    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair?;
        if let LuaValue::Table(tbl) = value {
            let inner_res = clone(lua, tbl)?;
            res.set(key, inner_res)?;
        } else {
            res.set(key, value)?;
        }
    }
    Ok(res)
}

fn flatten<'lua>(lua: &'lua Lua, arrays: Vec<LuaValue<'lua>>) -> mlua::Result<Vec<LuaValue<'lua>>> {
    let mut flat_vec: Vec<LuaValue> = vec![];
    for item in arrays {
        match item {
            LuaValue::Table(tbl) => {
                let tbl_as_vec = tbl.sequence_values().filter_map(|x| x.ok()).collect();
                let flat = flatten(lua, tbl_as_vec)?;
                for j in flat {
                    flat_vec.push(j);
                }
            }
            LuaValue::Nil => (),
            LuaValue::Thread(_) => (),
            LuaValue::Error(err) => {
                return Err(err);
            }
            other => {
                flat_vec.push(other);
            }
        }
    }
    Ok(flat_vec)
}

fn length<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<Integer> {
    // note that # only works correctly on arrays in Lua
    let len = table.pairs::<LuaValue, LuaValue>().count() as i64;
    Ok(len)
}

fn has_key<'lua>(_: &'lua Lua, (table, key): (Table<'lua>, LuaValue)) -> mlua::Result<bool> {
    Ok(table.contains_key(key)?)
}

fn has_value<'lua>(_: &'lua Lua, (table, value): (Table<'lua>, LuaValue)) -> mlua::Result<bool> {
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (_, tbl_value) = pair?;
        if tbl_value == value {
            return Ok(true);
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

fn to_string_fallback<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<String> {
    Ok(format!("{:#?}", table))
}

fn to_string<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<String> {
    let res = ValuePrinter(LuaValue::Table(table));
    Ok(format!("{:#?}", res).to_string())
}
