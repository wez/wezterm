use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods};
use mlua::{MultiValue as LuaMultiValue, String as LuaString, Value as LuaValue};
use std::ffi::OsStr;
use std::path::{Path as StdPath, PathBuf};

use crate::MetaData;

#[derive(Debug, PartialEq, Clone)]
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
                let p = u
                    .borrow::<Path>()
                    .map_err(mlua::Error::external)?
                    .to_owned();
                Ok(p)
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
                    "path entry {} is not representable as utf8",
                    this.0.display()
                )))?
                .to_string();
            Ok(s)
        });
        methods.add_meta_method(MetaMethod::Concat, |_, this, path: Path| {
            // TODO:
            // Could alternatively, just do:
            // let p = this.0.join(&path);
            // Ok(Path(p))
            // But we probably want this more string like behavior as an option too.
            let mut p: String = this
                .0
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry {} is not representable as utf8",
                    this.0.display()
                )))?
                .to_string();
            p.push_str(path.0.to_str().ok_or(mlua::Error::external(format!(
                "path entry {} is not representable as utf8",
                path.0.display()
            )))?);
            Ok(Path(PathBuf::from(&p)))
        });
        methods.add_meta_method(
            MetaMethod::Eq,
            |_, this, maybe_path: LuaValue| match maybe_path {
                LuaValue::UserData(u) => {
                    let p = u.borrow::<Path>();
                    match p {
                        Ok(p) => Ok(this.eq(&p)),
                        Err(_) => Ok(false),
                    }
                }
                LuaValue::Error(e) => Err(mlua::Error::external(e)),
                _ => Ok(false),
            },
        );
        // TODO: Should these be included here too? They are not async.
        // methods.add_method("is_dir", |_, this, _: ()| {
        //     let b = this.0.is_dir();
        //     Ok(b)
        // });
        // methods.add_method("is_file", |_, this, _: ()| {
        //     let b = this.0.is_file();
        //     Ok(b)
        // });
        // methods.add_method("is_symlink", |_, this, _: ()| {
        //     let b = this.0.is_symlink();
        //     Ok(b)
        // });
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
        methods.add_method("starts_with", |_, this, path: Path| {
            let b = this.0.starts_with(&path);
            Ok(b)
        });
        methods.add_method("ends_with", |_, this, path: Path| {
            let b = this.0.ends_with(&path);
            Ok(b)
        });
        methods.add_method("join", |_, this, path: Path| {
            let p = this.0.join(&path);
            Ok(Path(p))
        });
        methods.add_method_mut("strip_prefix", |_, this, base: Path| {
            let p = this.0.strip_prefix(&base).unwrap_or(&this.0).to_path_buf();
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
        methods.add_async_method("canonical_path", |_, this, _: ()| async move {
            let p = smol::fs::canonicalize(&this).await.map_err(|err| {
                mlua::Error::external(format!("'{}':canonical_path(): {err:#}", this.0.display()))
            })?;
            Ok(Path(p))
        });
        methods.add_method_mut("set_extension", |_, this, path: Path| {
            let b = this.0.set_extension(&path);
            Ok(b)
        });
        methods.add_method_mut("set_filename", |_, this, path: Path| {
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
        methods.add_method("basename", |lua: &'lua Lua, this, _: ()| {
            crate::basename(lua, this.clone())
        });
        methods.add_method("dirname", |lua: &'lua Lua, this, _: ()| {
            crate::dirname(lua, this.clone())
        });
        // TODO: Add this when stable:
        // methods.add_method("file_prefix", |_, this, _: ()| {
        //     let file_stem = this
        //         .0
        //         .file_prefix()
        //         .unwrap_or(std::ffi::OsStr::new(""))
        //         .to_str()
        //         .ok_or(mlua::Error::external(format!(
        //             "path entry {} is not representable as utf8",
        //             this.0.display()
        //         )))?
        //         .to_string();
        //     Ok(file_stem)
        // });
        methods.add_method("file_stem", |_, this, _: ()| {
            let file_stem = this
                .0
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new(""))
                .to_str()
                .ok_or(mlua::Error::external(format!(
                    "path entry {} is not representable as utf8",
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
                    "path entry {} is not representable as utf8",
                    this.0.display()
                )))?
                .to_string();
            Ok(extension)
        });
        methods.add_method("try_exists", |lua: &'lua Lua, this, _: ()| {
            crate::try_exists(lua, this.clone())
        });
        methods.add_async_method(
            "read_dir",
            |lua: &'lua Lua, this, function: Option<mlua::Function<'lua>>| {
                crate::read_dir(lua, (this.clone(), function))
            },
        );
        methods.add_async_method("read_link", |_, this, _: ()| async move {
            let link = smol::fs::read_link(&this)
                .await
                .map_err(mlua::Error::external)?;
            Ok(Path(link))
        });
        methods.add_method("clone", |_, this, _: ()| Ok(this.clone()));

        // # String methods:
        // Lua comes with the following string functions in wezterm:
        // - byte
        // - char (first argument not a string)
        // - dump (first argument not a string)
        // - find
        // - format (first argument a format string)
        // - gmatch
        // - gsub
        // - len
        // - lower
        // - match
        // - pack (first argument a format string)
        // - packsize (first argument a format string)
        // - rep
        // - reverse
        // - sub
        // - unpack (first argument a format string)
        // - upper
        // It doesn't make sense to have methods for the cases where the first
        // argument is not a string or is a format string.
        methods.add_method("byte", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let byte = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("byte")?;
            byte.call::<_, LuaMultiValue>((lua_str, multi))
        });
        methods.add_method(
            "find",
            |lua: &'lua Lua,
             this,
             // TODO: We could do this:
             //  (find_str, opt_init, opt_plain): (
             //     LuaString,
             //     Option<mlua::Integer>,
             //     Option<bool>,
             // but I prefer the error messages this way:
             multi: LuaMultiValue| {
                let lua_str = path_to_lua_str(lua, &this)?;
                let find = lua
                    .globals()
                    .get::<_, mlua::Table>("string")?
                    .get::<_, mlua::Function>("find")?;
                // find.call::<_, LuaMultiValue>((lua_str, find_str, opt_init, opt_plain))
                find.call::<_, LuaMultiValue>((lua_str, multi))
            },
        );
        methods.add_method("gmatch", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let gmatch = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("gmatch")?;
            gmatch.call::<_, mlua::Function>((lua_str, multi))
        });
        methods.add_method("gsub", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let gsub = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("gsub")?;
            gsub.call::<_, LuaString>((lua_str, multi))
        });
        methods.add_method("len", |lua: &'lua Lua, this, _: ()| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let len = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("len")?;
            len.call::<_, mlua::Integer>(lua_str)
        });
        methods.add_method("lower", |lua: &'lua Lua, this, _: ()| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let lower = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("lower")?;
            lower.call::<_, LuaString>(lua_str)
        });
        methods.add_method("match", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let lua_match = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("match")?;
            lua_match.call::<_, LuaMultiValue>((lua_str, multi))
        });
        methods.add_method("rep", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let rep = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("rep")?;
            rep.call::<_, LuaString>((lua_str, multi))
        });
        methods.add_method("reverse", |lua: &'lua Lua, this, _: ()| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let reverse = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("reverse")?;
            reverse.call::<_, LuaString>(lua_str)
        });
        methods.add_method("sub", |lua: &'lua Lua, this, multi: LuaMultiValue| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let sub = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("sub")?;
            sub.call::<_, LuaString>((lua_str, multi))
        });
        methods.add_method("upper", |lua: &'lua Lua, this, _: ()| {
            let lua_str = path_to_lua_str(lua, &this)?;
            let upper = lua
                .globals()
                .get::<_, mlua::Table>("string")?
                .get::<_, mlua::Function>("upper")?;
            upper.call::<_, LuaString>(lua_str)
        });
    }
}

fn path_to_lua_str<'lua>(lua: &'lua Lua, path: &Path) -> mlua::Result<LuaString<'lua>> {
    lua.create_string(path.0.to_str().ok_or(mlua::Error::external(format!(
        "path entry {} is not representable as utf8",
        path.0.display()
    )))?)
}
