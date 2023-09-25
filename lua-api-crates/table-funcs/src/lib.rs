use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, Table, Value as LuaValue};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let table = get_or_create_sub_module(lua, "table")?;
    table.set("merge", lua.create_function(merge)?)?;
    table.set("clone", lua.create_function(clone)?)?;
    table.set("flatten", lua.create_function(flatten)?)?;

    Ok(())
}

// merge tables
// (in case of overlap of the tables, we take the key-value pair from the last table)
//
fn merge<'lua>(lua: &'lua Lua, tables: Vec<Table<'lua>>) -> mlua::Result<Table<'lua>> {
    let mut tbl_vec: Vec<(LuaValue, LuaValue)> = vec![];
    for table in tables {
        for inner_pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = inner_pair.map_err(mlua::Error::external)?;
            tbl_vec.push((key, value));
        }
    }
    let tbl_len = tbl_vec.len();
    let tbl: Table<'lua> = lua
        .create_table_with_capacity(0, tbl_len)
        .map_err(mlua::Error::external)?;
    for (key, value) in tbl_vec {
        // Note that we override previously set key values if we have
        // the same key showing up more than once
        tbl.set(key, value).map_err(mlua::Error::external)?;
    }
    Ok(tbl)
}

fn clone<'lua>(_: &'lua Lua, table: Table<'lua>) -> mlua::Result<Table<'lua>> {
    Ok(table.clone())
}

fn flatten<'lua>(lua: &'lua Lua, table: Vec<Vec<LuaValue>>) -> mlua::Result<Table<'lua>> {
    let flat_vec: Vec<LuaValue> = table.into_iter().flatten().collect();
    let flat_table = lua
        .create_table_with_capacity(flat_vec.len(), 0)
        .map_err(mlua::Error::external)?;
    for value in flat_vec {
        flat_table.push(value).map_err(mlua::Error::external)?;
    }
    Ok(flat_table)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
