use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods};
use luahelper::mlua::Function;
use mlua::Value as LuaValue;
use std::ffi::OsStr;
use std::path::{Path as StdPath, PathBuf};

use crate::MetaData;

#[derive(Debug, Clone)]
pub struct Path(pub PathBuf);

impl AsRef<StdPath> for Path {
    fn as_ref(&self) -> &StdPath {
        self.0.as_path()
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

impl<'lua> mlua::FromLua<'lua> for Path {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
        match value {
            LuaValue::String(s) => {
                let string: String = mlua::FromLua::from_lua(LuaValue::String(s), lua)
                    .map_err(mlua::Error::external)?;
                Ok(Path(PathBuf::from(&string)))
            }
            LuaValue::UserData(u) => {
                let p = u.take::<Path>().map_err(mlua::Error::external);
                p
            }
            other => Err(mlua::Error::external(format!(
                "Wrong type. Expected string or Path, but got: {}",
                other.type_name()
            ))),
        }
    }
}

impl UserData for Path {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            let s = this
                .0
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {}",
                    this.0.display()
                )))?
                .to_string();
            Ok(s)
        });
        methods.add_meta_method(MetaMethod::Concat, |_, this, path: Path| {
            // Could alternatively, just do:
            // let p = this.0.join(&path);
            // Ok(Path(p))
            let mut p: String = this
                .0
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {}",
                    this.0.display()
                )))?
                .to_string();
            p.push_str(path.0.to_str().ok_or(mlua::Error::external(format!(
                "path entry is not representable as utf8: {}",
                path.0.display()
            )))?);
            Ok(Path(PathBuf::from(&p)))
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
            this.0.push(&path);
            Ok(())
        });
        methods.add_method("join", |_, this, path: Path| {
            let p = this.0.join(&path);
            Ok(Path(p))
        });
        methods.add_async_method("metadata", |_, this, _: ()| async move {
            let m = smol::fs::metadata(&this)
                .await
                .map_err(mlua::Error::external)?;
            Ok(MetaData(m))
        });
        methods.add_async_method("symlink_metadata", |_, this, _: ()| async move {
            let m = smol::fs::symlink_metadata(&this)
                .await
                .map_err(mlua::Error::external)?;
            Ok(MetaData(m))
        });
        methods.add_async_method("canonicalize", |_, this, _: ()| async move {
            let p = smol::fs::canonicalize(&this)
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
                .map(|c| Path(AsRef::<StdPath>::as_ref(&c).to_path_buf()))
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
                    "path entry is not representable as utf8: {}",
                    this.0.display()
                )))?;
            Ok(Path(PathBuf::from(basename)))
        });
        methods.add_method("dirname", |_, this, _: ()| {
            let dirname = this
                .0
                .parent()
                .unwrap_or(&this.0) // parent returns None if the path terminates in a root or a
                // prefix
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {}",
                    this.0.display()
                )))?;
            Ok(Path(PathBuf::from(dirname)))
        });
        methods.add_method("file_stem", |_, this, _: ()| {
            let file_stem = this
                .0
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new(""))
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry is not representable as utf8: {}",
                    this.0.display()
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
                    "path entry is not representable as utf8: {}",
                    this.0.display()
                )))?
                .to_string();
            Ok(extension)
        });
        methods.add_async_method(
            "read_dir",
            |lua: &'lua Lua, this, function: Option<Function<'lua>>| {
                crate::read_dir(lua, (this.clone(), function))
            },
        )
    }
}
