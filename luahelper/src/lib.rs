#![macro_use]

mod serde_lua;
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
        impl<'lua> mlua::ToLua<'lua> for $struct {
            fn to_lua(self, lua: &'lua mlua::Lua) -> Result<mlua::Value<'lua>, mlua::Error> {
                Ok(crate::scripting::to_lua_value(lua, self)?)
            }
        }

        impl<'lua> mlua::FromLua<'lua> for $struct {
            fn from_lua(
                value: mlua::Value<'lua>,
                _lua: &'lua mlua::Lua,
            ) -> Result<Self, mlua::Error> {
                Ok(crate::scripting::from_lua_value(value)?)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
