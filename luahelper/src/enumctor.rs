use crate::{dynamic_to_lua_value, lua_value_to_dynamic};
use mlua::{IntoLua, Lua, MetaMethod, UserData, UserDataMethods, Value};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use wezterm_dynamic::{
    Error as DynError, FromDynamic, FromDynamicOptions, ToDynamic, UnknownFieldAction,
    Value as DynValue,
};

struct EnumVariant<T> {
    phantom: PhantomData<T>,
    variant: String,
}

// Safety: <T> is used only in PhantomData so it doesn't actually
// need to be Send.
unsafe impl<T> Send for EnumVariant<T> {}

impl<T> EnumVariant<T>
where
    T: FromDynamic,
    T: ToDynamic,
    T: std::fmt::Debug,
    T: 'static,
{
    fn new(variant: String) -> Self {
        Self {
            phantom: PhantomData,
            variant,
        }
    }

    fn call_impl<'lua>(variant: &str, lua: &'lua Lua, table: Value) -> mlua::Result<Value<'lua>> {
        let value = lua_value_to_dynamic(table)?;

        let mut obj = BTreeMap::new();
        obj.insert(DynValue::String(variant.to_string()), value);
        let value = DynValue::Object(obj.into());

        let _action = T::from_dynamic(
            &value,
            FromDynamicOptions {
                unknown_fields: UnknownFieldAction::Deny,
                deprecated_fields: UnknownFieldAction::Warn,
            },
        )
        .map_err(|e| mlua::Error::external(e.to_string()))?;
        dynamic_to_lua_value(lua, value)
    }
}

impl<T> UserData for EnumVariant<T>
where
    T: FromDynamic,
    T: ToDynamic,
    T: std::fmt::Debug,
    T: 'static,
{
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Call, |lua, myself, table: Value| {
            Self::call_impl(&myself.variant, lua, table)
        });
    }
}

/// This type is used as an enum constructor for type `T`.
/// The primary usage is to enable `wezterm.action` to have the following
/// behaviors for KeyAssignment:
///
/// `wezterm.action{QuickSelectArgs={}}` -> compatibility with prior versions;
/// the table is passed through and from_dynamic -> lua conversion is attempted.
///
/// `wezterm.action.QuickSelectArgs` -> since the `QuickSelectArgs` variant
/// has a payload that impl Default, this is equivalent to the call above.
///
/// `wezterm.action.QuickSelectArgs{}` -> equivalent to the call above, but
/// explicitly calls the constructor with no parameters.
///
/// `wezterm.action.QuickSelectArgs{alphabet="abc"}` -> configures the alphabet.
///
/// This dynamic behavior is implemented using metatables.
///
/// `Enum<T>` implements __call to handle that first case above, and __index
/// to handle the remaining cases.
///
/// The __index implementation will return a simple string value for unit variants,
/// which is how they are encoded by to_dynamic.
///
/// Otherwise, a table will be built with the equivalent value representation.
/// That table will also have a metatable assigned to it, which allows for
/// either using the value as-is or passing additional parameters to it.
///
/// If parameters are required, an EnumVariant<T> is returned instead of the
/// table, and it has a __call method that will perform that final stage
/// of construction.
pub struct Enum<T> {
    phantom: PhantomData<T>,
}

// Safety: <T> is used only in PhantomData so it doesn't actually
// need to be Send.
unsafe impl<T> Send for Enum<T> {}

impl<T> Enum<T> {
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T> UserData for Enum<T>
where
    T: FromDynamic,
    T: ToDynamic,
    T: std::fmt::Debug,
    T: 'static,
{
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Call, |lua, _myself, table: Value| {
            let value = lua_value_to_dynamic(table)?;
            let _action = T::from_dynamic(
                &value,
                FromDynamicOptions {
                    unknown_fields: UnknownFieldAction::Deny,
                    deprecated_fields: UnknownFieldAction::Warn,
                },
            )
            .map_err(|e| mlua::Error::external(e.to_string()))?;
            dynamic_to_lua_value(lua, value)
        });

        methods.add_meta_method(MetaMethod::Index, |lua, _myself, field: String| {
            // Step 1: see if this is a unit variant.
            // A unit variant will be convertible from string
            if let Ok(_) = T::from_dynamic(
                &DynValue::String(field.to_string()),
                FromDynamicOptions {
                    unknown_fields: UnknownFieldAction::Deny,
                    deprecated_fields: UnknownFieldAction::Ignore,
                },
            ) {
                return Ok(field.into_lua(lua)?);
            }

            // Step 2: see if this is a valid variant, and whether we can
            // default-construct it with an empty table.
            let mut obj = BTreeMap::new();
            obj.insert(
                DynValue::String(field.to_string()),
                DynValue::Object(BTreeMap::<DynValue, DynValue>::new().into()),
            );
            match T::from_dynamic(
                &DynValue::Object(obj.into()),
                FromDynamicOptions {
                    unknown_fields: UnknownFieldAction::Deny,
                    deprecated_fields: UnknownFieldAction::Ignore,
                },
            ) {
                Ok(defaulted) => {
                    let defaulted = defaulted.to_dynamic();
                    match dynamic_to_lua_value(lua, defaulted)? {
                        Value::Table(t) => {
                            let mt = lua.create_table()?;
                            mt.set(
                                "__call",
                                lua.create_function(move |lua, (_mt, table): (Value, Value)| {
                                    EnumVariant::<T>::call_impl(&field, lua, table)
                                })?,
                            )?;

                            t.set_metatable(Some(mt));
                            return Ok(Value::Table(t));
                        }
                        wat => Err(mlua::Error::external(format!(
                            "unexpected type {}",
                            wat.type_name()
                        ))),
                    }
                }
                err @ Err(DynError::InvalidVariantForType { .. }) => {
                    Err(mlua::Error::external(err.unwrap_err().to_string()))
                }
                _ => {
                    // Must be a valid variant, but requires arguments
                    let variant_ctor = lua.create_userdata(EnumVariant::<T>::new(field))?;
                    Ok(Value::UserData(variant_ctor))
                }
            }
        });
    }
}
