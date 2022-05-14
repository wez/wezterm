#![macro_use]

use std::collections::BTreeMap;
use wezterm_dynamic::{FromDynamic, ToDynamic, Value as DynValue};

mod serde_lua;
pub use mlua;
use mlua::{ToLua, Value as LuaValue};
pub use serde_lua::from_lua_value;
pub use serde_lua::ser::to_lua_value;

/// Implement lua conversion traits for a type.
/// This implementation requires that the type implement
/// serde Serialize and Deserialize.
/// Why do we need these traits?  They allow `create_function` to
/// operate in terms of our internal types rather than forcing
/// the implementer to use generic Value parameter or return values.
#[macro_export]
macro_rules! impl_lua_conversion {
    ($struct:ident) => {
        impl<'lua> $crate::mlua::ToLua<'lua> for $struct {
            fn to_lua(
                self,
                lua: &'lua $crate::mlua::Lua,
            ) -> Result<$crate::mlua::Value<'lua>, $crate::mlua::Error> {
                Ok($crate::to_lua_value(lua, self)?)
            }
        }

        impl<'lua> $crate::mlua::FromLua<'lua> for $struct {
            fn from_lua(
                value: $crate::mlua::Value<'lua>,
                _lua: &'lua $crate::mlua::Lua,
            ) -> Result<Self, $crate::mlua::Error> {
                Ok($crate::from_lua_value(value)?)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_lua_conversion_dynamic {
    ($struct:ident) => {
        impl<'lua> $crate::mlua::ToLua<'lua> for $struct {
            fn to_lua(
                self,
                lua: &'lua $crate::mlua::Lua,
            ) -> Result<$crate::mlua::Value<'lua>, $crate::mlua::Error> {
                use wezterm_dynamic::ToDynamic;
                let value = self.to_dynamic();
                $crate::dynamic_to_lua_value(lua, value)
            }
        }

        impl<'lua> $crate::mlua::FromLua<'lua> for $struct {
            fn from_lua(
                value: $crate::mlua::Value<'lua>,
                _lua: &'lua $crate::mlua::Lua,
            ) -> Result<Self, $crate::mlua::Error> {
                use wezterm_dynamic::FromDynamic;
                let lua_type = value.type_name();
                let value = $crate::lua_value_to_dynamic(value)?;
                $struct::from_dynamic(&value, Default::default()).map_err(|e| {
                    $crate::mlua::Error::FromLuaConversionError {
                        from: lua_type,
                        to: stringify!($struct),
                        message: Some(e.to_string()),
                    }
                })
            }
        }
    };
}

pub fn dynamic_to_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    value: DynValue,
) -> mlua::Result<mlua::Value> {
    Ok(match value {
        DynValue::Null => LuaValue::Nil,
        DynValue::Bool(b) => LuaValue::Boolean(b),
        DynValue::String(s) => s.to_lua(lua)?,
        DynValue::U64(u) => u.to_lua(lua)?,
        DynValue::F64(u) => u.to_lua(lua)?,
        DynValue::I64(u) => u.to_lua(lua)?,
        DynValue::Array(array) => {
            let table = lua.create_table()?;
            for (idx, value) in array.into_iter().enumerate() {
                table.set(idx + 1, dynamic_to_lua_value(lua, value)?)?;
            }
            LuaValue::Table(table)
        }
        DynValue::Object(object) => {
            let table = lua.create_table()?;
            for (key, value) in object.into_iter() {
                table.set(
                    dynamic_to_lua_value(lua, key)?,
                    dynamic_to_lua_value(lua, value)?,
                )?;
            }
            LuaValue::Table(table)
        }
    })
}

pub fn lua_value_to_dynamic(value: LuaValue) -> mlua::Result<DynValue> {
    Ok(match value {
        LuaValue::Nil => DynValue::Null,
        LuaValue::String(s) => DynValue::String(s.to_str()?.to_string()),
        LuaValue::Boolean(b) => DynValue::Bool(b),
        LuaValue::Integer(i) => DynValue::I64(i),
        LuaValue::Number(i) => DynValue::F64(i.into()),
        LuaValue::LightUserData(_) | LuaValue::UserData(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "userdata",
                to: "wezterm_dynamic::Value",
                message: None,
            })
        }
        LuaValue::Function(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "function",
                to: "wezterm_dynamic::Value",
                message: None,
            })
        }
        LuaValue::Thread(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "thread",
                to: "wezterm_dynamic::Value",
                message: None,
            })
        }
        LuaValue::Error(e) => return Err(e),
        LuaValue::Table(table) => {
            if let Ok(true) = table.contains_key(1) {
                let mut array = vec![];
                for value in table.sequence_values() {
                    array.push(lua_value_to_dynamic(value?)?);
                }
                DynValue::Array(array.into())
            } else {
                let mut obj = BTreeMap::default();
                for pair in table.pairs::<LuaValue, LuaValue>() {
                    let (key, value) = pair?;
                    obj.insert(lua_value_to_dynamic(key)?, lua_value_to_dynamic(value)?);
                }
                DynValue::Object(obj.into())
            }
        }
    })
}

pub fn from_lua_value_dynamic<T: FromDynamic>(value: LuaValue) -> mlua::Result<T> {
    let type_name = value.type_name();
    let value = lua_value_to_dynamic(value)?;
    T::from_dynamic(&value, Default::default()).map_err(|e| mlua::Error::FromLuaConversionError {
        from: type_name,
        to: "Rust Type",
        message: Some(e.to_string()),
    })
}

#[derive(FromDynamic, ToDynamic)]
pub struct ValueLua {
    pub value: wezterm_dynamic::Value,
}
impl_lua_conversion_dynamic!(ValueLua);

pub use serde_lua::ValueWrapper;
