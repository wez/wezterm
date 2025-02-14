use anyhow::anyhow;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Function, Lua};
use config::HOME_DIR;
use smol::prelude::*;
use std::path::PathBuf;

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
    wezterm_mod.set("try_exists", lua.create_function(try_exists)?)?;
    wezterm_mod.set("glob", lua.create_async_function(glob)?)?;
    wezterm_mod.set("to_path", lua.create_function(to_path)?)?;
    // TODO: Should we include home_path?
    wezterm_mod.set("home_path", Path(HOME_DIR.to_path_buf()))?;
    Ok(())
}

async fn read_dir<'lua>(
    _: &'lua Lua,
    (path, function): (Path, Option<Function<'lua>>),
) -> mlua::Result<Vec<Path>> {
    let mut dir = smol::fs::read_dir(&path)
        .await
        .map_err(mlua::Error::external)?;
    // let mut entries: Vec<String> = vec![];
    let mut entries: Vec<Path> = vec![];
    let mut sort = false; // assume we are not sorting
                          // TODO: Does it make sense to allow multiple sorts?
    let mut sort_by_vec: Vec<Vec<i64>> = vec![];
    while let Some(entry) = dir.next().await {
        let entry = entry.map_err(mlua::Error::external)?;
        let mut entry_path = Path(entry.path());
        let meta = entry.metadata().await.map_err(mlua::Error::external)?;

        // default behavior is include everything in the directory
        let mut include_entry = true;
        let mut sort_by: Vec<i64> = vec![];
        if let Some(func) = &function {
            match func.call((entry_path.clone(), MetaData(meta))) {
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
                            entry_path =
                                Path(PathBuf::from(s.to_str().map_err(mlua::Error::external)?))
                        }
                        Some(Ok(mlua::Value::UserData(u))) => {
                            entry_path = u.take::<Path>().map_err(mlua::Error::external)?;
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
                    return Err(mlua::Error::runtime(format!(
                        "the optional read_dir function returns the error: {}",
                        err
                    )));
                }
                _ => (),
            }
        }

        if include_entry {
            // TODO: Should we return Strings instead of Paths?
            // entries.push(entry_path.0.to_str().ok_or(
            //     mlua::Error::runtime(
            //         format!("the entry {} is not valid utf8", entry_path.0.display())
            //     )
            // )?.to_string());
            entries.push(entry_path);
            if sort {
                sort_by_vec.push(sort_by);
            }
        } // if include_entry is false, don't add entry to entries
    }

    if sort {
        // let mut sorted: Vec<String> = vec![];
        // for i in 0..sort_by_vec[0].len() {
        //     let sort_by_ivec: Vec<i64> = sort_by_vec.iter().map(|v| v[i]).collect();
        //
        //     let mut zipped: Vec<(&String, &i64)> =
        //         entries.iter().zip(sort_by_ivec.iter()).collect();
        //     zipped.sort_by_key(|pair| pair.1);
        //
        //     sorted = zipped.iter().map(|pair| pair.0.to_owned()).collect();
        // }
        let mut sorted: Vec<Path> = vec![];
        for i in 0..sort_by_vec[0].len() {
            let sort_by_ivec: Vec<i64> = sort_by_vec.iter().map(|v| v[i]).collect();

            let mut zipped: Vec<(&Path, &i64)> = entries.iter().zip(sort_by_ivec.iter()).collect();
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
    let b = path.0.ends_with("..");
    let root_or_dots = match b {
        true => "..",
        false => path.0.to_str().ok_or(mlua::Error::external(format!(
            "path entry {} is not representable as utf8",
            path.0.display()
        )))?,
    };
    let basename = path
        .0
        .file_name()
        // file_name returns None if the path name ends in `..` or is root
        // but the unix utility return `..` or root in those cases, so we do too
        .unwrap_or(std::ffi::OsStr::new(root_or_dots))
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

// NOTE: smol::fs doesn't include an async try_exists
fn try_exists<'lua>(_: &'lua Lua, path: Path) -> mlua::Result<bool> {
    let exists = path.0.try_exists().map_err(mlua::Error::external)?;
    Ok(exists)
}
