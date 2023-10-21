use std::ffi::OsStr;

use anyhow::anyhow;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Function, Lua, MetaMethod, UserData, UserDataMethods};
use smol::prelude::*;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("read_dir", lua.create_async_function(read_dir)?)?;
    wezterm_mod.set("glob", lua.create_async_function(glob)?)?;
    wezterm_mod.set("topath", lua.create_function(to_path)?)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MetaData(pub smol::fs::Metadata);

#[derive(Debug, Clone)]
pub struct Path(pub std::path::PathBuf);

impl AsRef<std::path::Path> for Path {
    fn as_ref(&self) -> &std::path::Path {
        self.0.as_path()
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

impl<'lua> mlua::FromLua<'lua> for Path {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
        let s: String = mlua::FromLua::from_lua(value, lua).map_err(mlua::Error::external)?;
        Ok(Path(std::path::PathBuf::from(&s)))
    }
}

impl UserData for MetaData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("{:#?}", this.0))
        });
        methods.add_method("is_dir", |_, this, _: ()| {
            let b = this.0.is_dir();
            Ok(b)
        });
        methods.add_method("is_file", |_, this, _: ()| {
            let b = this.0.is_file();
            Ok(b)
        });
        methods.add_method("is_symlink", |_, this, _: ()| {
            let b = this.0.is_symlink();
            Ok(b)
        });
        methods.add_method("is_readonly", |_, this, _: ()| {
            let b = this.0.permissions().readonly();
            Ok(b)
        });
        methods.add_method("secs_since_modified", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .modified()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("secs_since_accessed", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .accessed()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("secs_since_created", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .created()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("bytes", |_, this, _: ()| {
            let bytes = this.0.len();
            Ok(bytes as i64)
        });
    }
}

impl UserData for Path {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            let s = this
                .0
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {:#?}",
                    this.0
                )))?
                .to_string();
            Ok(s)
        });
        methods.add_meta_method(MetaMethod::Concat, |_, this, path: Path| {
            let p = this.0.join(&path);
            Ok(Path(p))
        });
        methods.add_method("is_dir", |_, this, _: ()| {
            let b = this.0.is_dir();
            Ok(b)
        });
        methods.add_method("is_file", |_, this, _: ()| {
            let b = this.0.is_file();
            Ok(b)
        });
        methods.add_method("is_symlink", |_, this, _: ()| {
            let b = this.0.is_symlink();
            Ok(b)
        });
        methods.add_method("is_absolute", |_, this, _: ()| {
            let b = this.0.is_absolute();
            Ok(b)
        });
        methods.add_method("is_relative", |_, this, _: ()| {
            let b = this.0.is_relative();
            Ok(b)
        });
        methods.add_method_mut("pop", |_, this, _: ()| {
            let b = this.0.pop();
            Ok(b)
        });
        methods.add_method_mut("push", |_, this, path: Path| {
            let p = this.0.push(&path);
            Ok(p)
        });
        methods.add_method("join", |_, this, path: Path| {
            let p = this.0.join(&path);
            Ok(Path(p))
        });
        methods.add_async_method("metadata", |_, this, _: ()| async move {
            let m = smol::fs::metadata(&this.0)
                .await
                .map_err(mlua::Error::external)?;
            Ok(MetaData(m))
        });
        methods.add_async_method("symlink_metadata", |_, this, _: ()| async move {
            let m = smol::fs::symlink_metadata(&this.0)
                .await
                .map_err(mlua::Error::external)?;
            Ok(MetaData(m))
        });
        methods.add_async_method("canonicalize", |_, this, _: ()| async move {
            let p = smol::fs::canonicalize(&this.0)
                .await
                .map_err(mlua::Error::external)?;
            Ok(Path(p))
        });
        methods.add_method_mut("set_extension", |_, this, path: Path| {
            let b = this.0.set_extension(&path);
            Ok(b)
        });
        methods.add_method_mut("set_file_name", |_, this, path: Path| {
            this.0.set_file_name(&path);
            Ok(())
        });
        methods.add_method("ancestors", |_, this, _: ()| {
            let ancestors: Vec<Path> = this.0.ancestors().map(|p| Path(p.to_path_buf())).collect();
            Ok(ancestors)
        });
        methods.add_method("components", |_, this, _: ()| {
            let components: Vec<Path> = this
                .0
                .components()
                .map(|c| Path(AsRef::<std::path::Path>::as_ref(&c).to_path_buf()))
                .collect();
            Ok(components)
        });
        methods.add_method("basename", |_, this, _: ()| {
            let basename = this
                .0
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("..")) // file_name returns None if the path
                // terminates in ..
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {:#?}",
                    this.0
                )))?;
            Ok(Path(std::path::PathBuf::from(basename)))
        });
        methods.add_method("dirname", |_, this, _: ()| {
            let dirname = this
                .0
                .parent()
                .unwrap_or(&this.0) // parent returns None if the path terminates in a root or a
                // prefix
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {:#?}",
                    this.0
                )))?;
            Ok(Path(std::path::PathBuf::from(dirname)))
        });
        methods.add_method("file_stem", |_, this, _: ()| {
            let file_stem = this
                .0
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new(""))
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {:#?}",
                    this.0
                )))?
                .to_string();
            Ok(file_stem)
        });
        methods.add_method("extension", |_, this, _: ()| {
            let extension = this
                .0
                .extension()
                .unwrap_or(std::ffi::OsStr::new(""))
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {:#?}",
                    this.0
                )))?
                .to_string();
            Ok(extension)
        });
    }
}

async fn read_dir<'lua>(
    _: &'lua Lua,
    (path, function): (String, Option<Function<'lua>>),
) -> mlua::Result<Vec<String>> {
    let mut dir = smol::fs::read_dir(path)
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

            let mut zipped: Vec<(String, i64)> = entries
                .clone()
                .into_iter()
                .zip(sort_by_ivec.into_iter())
                .collect();
            zipped.sort_by_key(|pair| pair.1);

            sorted = zipped
                .iter()
                .map(|pair| pair.0.clone())
                .map(|s| s.to_string())
                .collect();
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
    let p = std::path::PathBuf::from(string);
    Ok(Path(p))
}
