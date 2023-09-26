use anyhow::anyhow;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Function, Lua, MetaMethod, UserData, UserDataMethods};
use smol::prelude::*;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("read_dir", lua.create_async_function(read_dir)?)?;
    wezterm_mod.set("glob", lua.create_async_function(glob)?)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MetaData(pub smol::fs::Metadata);

impl UserData for MetaData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("MetaData({:?})", this))
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

async fn read_dir<'lua>(
    _: &'lua Lua,
    (path, function): (String, Option<Function<'lua>>),
) -> mlua::Result<Vec<String>> {
    let mut dir = smol::fs::read_dir(path)
        .await
        .map_err(mlua::Error::external)?;
    let mut entries = vec![];
    while let Some(entry) = dir.next().await {
        let entry = entry.map_err(mlua::Error::external)?;
        if let Some(utf8) = entry.path().to_str() {
            let meta = entry.metadata().await.map_err(mlua::Error::external)?;

            // default behavior is include everything in the directory
            let mut include_entry = true;
            if let Some(func) = &function {
                include_entry = match func.call((utf8.to_string(), MetaData(meta))) {
                    Ok(mlua::Value::Boolean(b)) => b,
                    Err(err) => {
                        return Err(mlua::Error::external(format!(
                            "the optional read_dir function returns the error: {}",
                            err
                        )));
                    }
                    _ => true,
                }
            }

            if include_entry {
                entries.push(utf8.to_string());
            } // if include_entry is false, don't add entry to entries
        } else {
            return Err(mlua::Error::external(anyhow!(
                "path entry {} is not representable as utf8",
                entry.path().display()
            )));
        }
    }
    Ok(entries)
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
