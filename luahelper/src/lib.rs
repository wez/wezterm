#![macro_use]

use serde::{Deserialize, Serialize};

mod serde_lua;
pub use mlua;
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

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonLua(pub serde_json::Value);
impl_lua_conversion!(JsonLua);
