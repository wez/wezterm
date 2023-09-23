use anyhow::anyhow;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua};
use smol::prelude::*;
use std::path::Path;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("read_dir", lua.create_async_function(read_dir)?)?;
    wezterm_mod.set("basename", lua.create_async_function(basename)?)?;
    wezterm_mod.set("dirname", lua.create_async_function(dirname)?)?;
    wezterm_mod.set("glob", lua.create_async_function(glob)?)?;
    Ok(())
}

async fn read_dir<'lua>(_: &'lua Lua, path: String) -> mlua::Result<Vec<String>> {
    let mut dir = smol::fs::read_dir(path)
        .await
        .map_err(mlua::Error::external)?;
    let mut entries = vec![];
    while let Some(entry) = dir.next().await {
        let entry = entry.map_err(mlua::Error::external)?;
        if let Some(utf8) = entry.path().to_str() {
            entries.push(utf8.to_string());
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                entry.path().display()
            )));
        }
    }
    Ok(entries)
}

async fn basename<'lua>(_: &'lua Lua, path: String) -> mlua::Result<String> {
    // to check if the path actually exists, we can use:
    /* let dir = smol::fs::canonicalize(path)
    .await
    .map_err(mlua::Error::external)?; */
    let path = Path::new(&path);
    if let Some(os_str_basename) = path.file_name() {
        if let Some(basename) = os_str_basename.to_str() {
            Ok(basename.to_string())
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                path.display()
            )));
        }
    } else {
        // file_name returns None if the path name ends in ..
        Ok("..".to_string())
    }
}

async fn dirname<'lua>(_: &'lua Lua, path: String) -> mlua::Result<String> {
    // to check if the path actually exists, we can use:
    /* let dir = smol::fs::canonicalize(path)
    .await
    .map_err(mlua::Error::external)?; */
    let path = Path::new(&path);
    if let Some(parent_path) = path.parent() {
        if let Some(os_str_parent) = parent_path.file_name() {
            if let Some(dirname) = os_str_parent.to_str() {
                Ok(dirname.to_string())
            } else {
                return Err(mlua::Error::external(anyhow!(
                    "path entry {} is not representable as utf8",
                    path.display()
                )));
            }
        } else {
            // file name returns None if parent_path ends in ..
            Ok("..".to_string())
        }
    } else {
        // parent returns None if the path terminates in a root or prefix
        Ok("".to_string())
    }
}

async fn glob<'lua>(
    _: &'lua Lua,
    (pattern, path): (String, Option<String>),
) -> mlua::Result<Vec<String>> {
    let entries = smol::unblock(move || {
        let mut entries = vec![];
        let glob = filenamegen::Glob::new(&pattern)?;
        for path in glob.walk(path.as_deref().unwrap_or(".")) {
            if let Some(utf8) = path.to_str() {
                entries.push(utf8.to_string());
            } else {
                return Err(anyhow!(
                    "path entry {} is not representable as utf8",
                    path.display()
                ));
            }
        }
        Ok(entries)
    })
    .await
    .map_err(mlua::Error::external)?;
    Ok(entries)
}
