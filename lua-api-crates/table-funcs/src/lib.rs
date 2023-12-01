use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Integer, Lua, Table, Value as LuaValue};
use luahelper::ValuePrinter;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let table = get_or_create_sub_module(lua, "table")?;
    table.set("merge", lua.create_function(merge)?)?;
    table.set("clone", lua.create_function(clone)?)?;
    table.set("flatten", lua.create_function(flatten)?)?;
    table.set("length", lua.create_function(length)?)?;
    table.set("has_key", lua.create_function(has_key)?)?;
    table.set("has_value", lua.create_function(has_value)?)?;
    table.set("to_string", lua.create_function(to_string)?)?;
    table.set(
        "to_string_fallback",
        lua.create_function(to_string_fallback)?,
    )?;

    Ok(())
}

// merge tables
// (in case of overlap of the tables, we default to taking the key-value pair from the last table)
// Note that we don't use a HashMap since we want to keep the order of the tables, which
// can be useful in some cases
fn merge<'lua>(
    lua: &'lua Lua,
    (array_of_tables, keep_first): (Vec<Table<'lua>>, Option<bool>),
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

    let keep_first = match keep_first {
        Some(b) => b,
        None => false, // default behavior is to keep_last set value
    };
    for (key, value) in tbl_vec {
        // Note that we override previously set key values if we have
        // the same key showing up more than once
        if keep_first {
            if !tbl.contains_key(key.clone())? {
                tbl.set(key, value)?;
            }
        } else {
            tbl.set(key, value)?;
        }
    }
    Ok(tbl)
}

fn clone<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<Table<'lua>> {
    Ok(table.clone())
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

fn to_string_fallback<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<String> {
    Ok(format!("{:#?}", table))
}

fn to_string<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<String> {
    let res = ValuePrinter(LuaValue::Table(table));
    Ok(format!("{:#?}", res).to_string())
}
