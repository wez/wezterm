use anyhow::anyhow;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Integer, Lua, Table, Value as LuaValue};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let table = get_or_create_sub_module(lua, "table")?;
    table.set("merge", lua.create_function(merge)?)?;
    table.set("clone", lua.create_function(clone)?)?;
    table.set("flatten", lua.create_function(flatten)?)?;
    table.set("length", lua.create_function(length)?)?;
    table.set("has_key", lua.create_function(has_key)?)?;
    table.set("has_value", lua.create_function(has_value)?)?;
    table.set("to_string", lua.create_function(to_string)?)?;

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
            let (key, value) = pair.map_err(mlua::Error::external)?;
            tbl_vec.push((key, value));
        }
    }
    let tbl_len = tbl_vec.len();
    // note we might allocate a bit too much here, but in many use cases we will be correct
    let tbl: Table<'lua> = lua
        .create_table_with_capacity(0, tbl_len)
        .map_err(mlua::Error::external)?;

    let keep_first = match keep_first {
        Some(b) => b,
        None => false, // default behavior is to keep_last set value
    };
    for (key, value) in tbl_vec {
        // Note that we override previously set key values if we have
        // the same key showing up more than once
        if keep_first {
            if !tbl
                .contains_key(key.clone())
                .map_err(mlua::Error::external)?
            {
                tbl.set(key, value).map_err(mlua::Error::external)?;
            }
        } else {
            tbl.set(key, value).map_err(mlua::Error::external)?;
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
                let tbl_as_vec = tbl
                    .sequence_values()
                    .filter_map(|x| x.map_err(mlua::Error::external).ok())
                    .collect();
                let flat = flatten(lua, tbl_as_vec)?;
                for j in flat {
                    flat_vec.push(j);
                }
            }
            LuaValue::Nil => (),
            LuaValue::Thread(_) => (),
            LuaValue::Error(err) => {
                return Err(mlua::Error::external(err));
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
    let mut len: i64 = 0;
    for _ in table.pairs::<LuaValue, LuaValue>() {
        len += 1
    }
    Ok(len)
}

fn has_key<'lua>(_: &'lua Lua, (table, key): (Table<'lua>, LuaValue)) -> mlua::Result<bool> {
    Ok(table.contains_key(key).map_err(mlua::Error::external)?)
}

fn has_value<'lua>(_: &'lua Lua, (table, value): (Table<'lua>, LuaValue)) -> mlua::Result<bool> {
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (_, tbl_value) = pair.map_err(mlua::Error::external)?;
        if tbl_value == value {
            return Ok(true);
        }
    }
    Ok(false)
}

fn to_string<'lua>(
    lua: &'lua Lua,
    (table, indent, skip_outer_bracket): (Table<'lua>, Option<i64>, Option<bool>),
) -> mlua::Result<String> {
    if let Some(ind) = indent {
        if ind < 0 {
            return Err(mlua::Error::external(anyhow!(
                "Indent set to {ind}. Please use an indent ≥ 0."
            )));
        }
    }
    let result = to_string_impl(lua, (table, indent, skip_outer_bracket, 0));
    match result {
        Ok(res) => {
            if skip_outer_bracket == Some(true) {
                // we added indent too many spaces on each line
                let extra_spaces = match indent {
                    Some(ind) => " ".repeat(ind as usize),
                    None => " ".repeat(2),
                };
                let old = ["\n", &extra_spaces].concat();
                return Ok(res.replace(&old, "\n"));
            }
            Ok(res)
        }
        Err(err) => Err(mlua::Error::external(err)),
    }
}

fn to_string_impl<'lua>(
    lua: &'lua Lua,
    (table, indent, skip_outer_bracket, depth): (Table<'lua>, Option<i64>, Option<bool>, usize),
) -> mlua::Result<String> {
    let mut string = String::new();
    let skip_outer_bracket = match skip_outer_bracket {
        Some(b) => b,
        None => false, // defaults to keeping the outer brackets
    };
    if !skip_outer_bracket || depth != 0 {
        string.push_str("{\n");
    }

    let bracket_spaces = match indent {
        Some(ind) => " ".repeat((ind as usize) * depth),
        None => " ".repeat(2 * depth),
    };
    let content_spaces = match indent {
        Some(ind) => " ".repeat((ind as usize) * (depth + 1)),
        None => " ".repeat(2 * (depth + 1)),
    };
    for pair in table.pairs::<LuaValue, LuaValue>() {
        string.push_str(&content_spaces);

        let (key, value) = pair.map_err(mlua::Error::external)?;
        match value.clone() {
            LuaValue::Table(tbl) => {
                string.push_str(&to_string_impl(lua, (tbl, indent, None, depth + 1))?)
            }
            _ => {
                let nice_key = match key {
                    LuaValue::String(s) => s.to_str().map_err(mlua::Error::external)?.to_string(),
                    LuaValue::Number(f) => f.to_string(),
                    LuaValue::Integer(i) => i.to_string(),
                    LuaValue::Boolean(b) => b.to_string(),
                    other => format!("{other:?}"),
                };
                let nice_value = match value {
                    LuaValue::String(s) => s.to_str().map_err(mlua::Error::external)?.to_string(),
                    LuaValue::Number(f) => f.to_string(),
                    LuaValue::Integer(i) => i.to_string(),
                    LuaValue::Boolean(b) => b.to_string(),
                    other => format!("{other:?}"),
                };
                string.push_str(&format!("{nice_key} = {nice_value},\n"));
            }
        }
    }
    if depth != 0 {
        string.push_str(&bracket_spaces);
        string.push_str("},\n")
    } else if skip_outer_bracket {
        string.pop(); // remove the last newline in this case
    } else {
        string.push_str("}");
    }
    Ok(string)
}
