use anyhow::anyhow;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Function, Lua};
use smol::prelude::*;
use std::path::{Path as StdPath, PathBuf};

mod metadata;
mod path;

pub use metadata::MetaData;
pub use path::Path;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("read_dir", lua.create_async_function(read_dir)?)?;
    wezterm_mod.set("basename", lua.create_function(basename)?)?;
    wezterm_mod.set("dirname", lua.create_function(dirname)?)?;
    wezterm_mod.set("canonical_path", lua.create_async_function(canonical_path)?)?;
    wezterm_mod.set("glob", lua.create_async_function(glob)?)?;
    wezterm_mod.set("to_path", lua.create_function(to_path)?)?;
    Ok(())
}

async fn read_dir<'lua>(
    _: &'lua Lua,
    (path, function): (Path, Option<Function<'lua>>),
) -> mlua::Result<Vec<String>> {
    let mut dir = smol::fs::read_dir(&path)
        .await
        .map_err(mlua::Error::external)?;
    let mut entries: Vec<String> = vec![];
    let mut sort = false; // assume we are not sorting
    let mut sort_by_vec: Vec<Vec<i64>> = vec![];
    while let Some(entry) = dir.next().await {
        let entry = entry.map_err(mlua::Error::external)?;
        if let Some(utf8) = entry.path().to_str() {
            // we need to make utf8 owned
            let mut utf8 = utf8.to_string();
            let meta = entry.metadata().await.map_err(mlua::Error::external)?;

            // default behavior is include everything in the directory
            let mut include_entry = true;
            let mut sort_by: Vec<i64> = vec![];
            if let Some(func) = &function {
                match func.call((utf8.clone(), MetaData(meta))) {
                    Ok(mlua::Value::Boolean(b)) => {
                        include_entry = b;
                    }
                    Ok(mlua::Value::Table(tbl)) => {
                        let mut iter = tbl.sequence_values();
                        match iter.next() {
                            Some(Ok(mlua::Value::Boolean(b))) => {
                                include_entry = b;
                            }
                            _ => (),
                        };
                        match iter.next() {
                            Some(Ok(mlua::Value::String(s))) => {
                                utf8 = s.to_str().map_err(mlua::Error::external)?.to_string();
                            }
                            Some(Ok(mlua::Value::Integer(i))) => {
                                sort = true;
                                sort_by.push(i);
                            }
                            _ => (),
                        }
                        while let Some(Ok(mlua::Value::Integer(i))) = iter.next() {
                            sort = true;
                            sort_by.push(i);
                        }
                    }
                    Err(err) => {
                        return Err(mlua::Error::external(format!(
                            "the optional read_dir function returns the error: {}",
                            err
                        )));
                    }
                    _ => (),
                }
            }

            if include_entry {
                entries.push(utf8);
                if sort {
                    sort_by_vec.push(sort_by);
                }
            } // if include_entry is false, don't add entry to entries
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                entry.path().display()
            )));
        }
    }

    if sort {
        let mut sorted: Vec<String> = vec![];
        for i in 0..sort_by_vec[0].len() {
            let sort_by_ivec: Vec<i64> = sort_by_vec.iter().map(|v| v[i]).collect();

            let mut zipped: Vec<(&String, &i64)> =
                entries.iter().zip(sort_by_ivec.iter()).collect();
            zipped.sort_by_key(|pair| pair.1);

            sorted = zipped.iter().map(|pair| pair.0.to_owned()).collect();
        }

        Ok(sorted)
    } else {
        Ok(entries)
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

fn to_path<'lua>(_: &'lua Lua, string: String) -> mlua::Result<Path> {
    let p = PathBuf::from(string);
    Ok(Path(p))
}

// similar (but not equal) to the shell command basename
fn basename<'lua>(_: &'lua Lua, path: Path) -> mlua::Result<Path> {
    let basename = path
        .0
        .file_name()
        // file_name returns None if the path terminates in ..
        .unwrap_or(std::ffi::OsStr::new(".."))
        .to_str()
        .ok_or(mlua::Error::external(format!(
            "path entry {} is not representable as utf8",
            path.0.display()
        )))?;
    Ok(Path(PathBuf::from(basename)))
}

// return the path without its final component if there is one
// similar to the shell command dirname
fn dirname<'lua>(_: &'lua Lua, path: Path) -> mlua::Result<Path> {
    let dirname = path
        .0
        .parent()
        // parent returns None if the path terminates in a root or a prefix
        .unwrap_or(&path.0)
        .to_str()
        .ok_or(mlua::Error::external(format!(
            "path entry {} is not representable as utf8",
            path.0.display()
        )))?;
    Ok(Path(PathBuf::from(dirname)))
}

// if path exists return the canonical form of the path with all
// intermediate components normalized and symbolic links resolved
async fn canonical_path<'lua>(_: &'lua Lua, path: Path) -> mlua::Result<Path> {
    let p = smol::fs::canonicalize(&path).await.map_err(|err| {
        mlua::Error::external(format!("canonical_path('{}'): {err:#}", path.0.display()))
    })?;
    Ok(Path(p))
}
