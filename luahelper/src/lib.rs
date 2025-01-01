#![macro_use]

pub use mlua;
use mlua::{IntoLua, Value as LuaValue};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::rc::Rc;
use wezterm_dynamic::{FromDynamic, ToDynamic, Value as DynValue};

pub mod enumctor;

pub fn to_lua<'lua, T: ToDynamic>(
    lua: &'lua mlua::Lua,
    value: T,
) -> Result<mlua::Value<'lua>, mlua::Error> {
    let value = value.to_dynamic();
    dynamic_to_lua_value(lua, value)
}

pub fn from_lua<'lua, T: FromDynamic>(value: mlua::Value<'lua>) -> Result<T, mlua::Error> {
    let lua_type = value.type_name();
    let value = lua_value_to_dynamic(value).map_err(|e| mlua::Error::FromLuaConversionError {
        from: lua_type,
        to: std::any::type_name::<T>(),
        message: Some(e.to_string()),
    })?;
    T::from_dynamic(&value, Default::default()).map_err(|e| mlua::Error::FromLuaConversionError {
        from: lua_type,
        to: std::any::type_name::<T>(),
        message: Some(e.to_string()),
    })
}

/// Implement lua conversion traits for a type.
/// This implementation requires that the type implement
/// FromDynamic and ToDynamic.
/// Why do we need these traits?  They allow `create_function` to
/// operate in terms of our internal types rather than forcing
/// the implementer to use generic Value parameter or return values.
#[macro_export]
macro_rules! impl_lua_conversion_dynamic {
    ($struct:ident) => {
        impl<'lua> $crate::mlua::IntoLua<'lua> for $struct {
            fn into_lua(
                self,
                lua: &'lua $crate::mlua::Lua,
            ) -> Result<$crate::mlua::Value<'lua>, $crate::mlua::Error> {
                $crate::to_lua(lua, self)
            }
        }

        impl<'lua> $crate::mlua::FromLua<'lua> for $struct {
            fn from_lua(
                value: $crate::mlua::Value<'lua>,
                _lua: &'lua $crate::mlua::Lua,
            ) -> Result<Self, $crate::mlua::Error> {
                $crate::from_lua(value)
            }
        }
    };
}

pub fn dynamic_to_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    value: DynValue,
) -> mlua::Result<mlua::Value<'lua>> {
    Ok(match value {
        DynValue::Null => LuaValue::Nil,
        DynValue::Bool(b) => LuaValue::Boolean(b),
        DynValue::String(s) => s.into_lua(lua)?,
        DynValue::U64(u) => u.into_lua(lua)?,
        DynValue::F64(u) => u.into_lua(lua)?,
        DynValue::I64(u) => u.into_lua(lua)?,
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
    let mut visited = HashSet::new();
    lua_value_to_dynamic_impl(value, &mut visited)
}

fn lua_value_to_dynamic_impl(
    value: LuaValue,
    visited: &mut HashSet<usize>,
) -> mlua::Result<DynValue> {
    if let LuaValue::Table(_) = &value {
        let ptr = value.to_pointer() as usize;
        if visited.contains(&ptr) {
            // Skip this one, as we've seen it before.
            // Treat it as a Null value.
            return Ok(DynValue::Null);
        }
        visited.insert(ptr);
    }
    Ok(match value {
        LuaValue::Nil => DynValue::Null,
        LuaValue::String(s) => DynValue::String(s.to_str()?.to_string()),
        LuaValue::Boolean(b) => DynValue::Bool(b),
        LuaValue::Integer(i) => DynValue::I64(i),
        LuaValue::Number(i) => DynValue::F64(i.into()),
        // Handle our special Null userdata case and map it to Null
        LuaValue::LightUserData(ud) if ud.0.is_null() => DynValue::Null,
        LuaValue::LightUserData(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "userdata",
                to: "wezterm_dynamic::Value",
                message: None,
            })
        }
        LuaValue::UserData(ud) => match ud.get_metatable() {
            Ok(mt) => {
                if let Ok(to_dynamic) = mt.get::<mlua::Function>("__wezterm_to_dynamic") {
                    match to_dynamic.call(LuaValue::UserData(ud.clone())) {
                        Ok(value) => {
                            return lua_value_to_dynamic_impl(value, visited);
                        }
                        Err(err) => {
                            return Err(mlua::Error::FromLuaConversionError {
                                from: "userdata",
                                to: "wezterm_dynamic::Value",
                                message: Some(format!(
                                    "error calling __wezterm_to_dynamic: {err:#}"
                                )),
                            })
                        }
                    }
                }

                match mt.get::<mlua::Function>(mlua::MetaMethod::ToString) {
                    Ok(to_string) => match to_string.call(LuaValue::UserData(ud.clone())) {
                        Ok(value) => {
                            return lua_value_to_dynamic_impl(value, visited);
                        }
                        Err(err) => {
                            return Err(mlua::Error::FromLuaConversionError {
                                from: "userdata",
                                to: "wezterm_dynamic::Value",
                                message: Some(format!("error calling tostring: {err:#}")),
                            })
                        }
                    },
                    Err(err) => {
                        return Err(mlua::Error::FromLuaConversionError {
                            from: "userdata",
                            to: "wezterm_dynamic::Value",
                            message: Some(format!("error getting tostring: {err:#}")),
                        })
                    }
                }
            }
            Err(err) => {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "userdata",
                    to: "wezterm_dynamic::Value",
                    message: Some(format!("error getting metatable: {err:#}")),
                })
            }
        },
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
                let pairs = table.clone();
                for value in table.sequence_values() {
                    array.push(lua_value_to_dynamic(value?)?);
                }

                for pair in pairs.pairs::<LuaValue, LuaValue>() {
                    let (key, _value) = pair?;
                    match &key {
                        LuaValue::Integer(n) if *n >= 1 && *n as usize <= array.len() => {
                            // Ok!
                        }
                        _ => {
                            let type_name = key.type_name();
                            let key = ValuePrinter(key);
                            return Err(mlua::Error::FromLuaConversionError {
                                from: type_name,
                                to: "numeric array index",
                                message: Some(format!(
                                    "Unexpected key {key:?} for array style table"
                                )),
                            });
                        }
                    }
                }

                DynValue::Array(array.into())
            } else {
                let mut obj = BTreeMap::default();
                for pair in table.pairs::<LuaValue, LuaValue>() {
                    let (key, value) = pair?;
                    let key = lua_value_to_dynamic(key)?;
                    let lua_type = value.type_name();
                    let value = lua_value_to_dynamic(value).map_err(|e| {
                        mlua::Error::FromLuaConversionError {
                            from: lua_type,
                            to: "value",
                            message: Some(format!("while processing {key:?}: {e}")),
                        }
                    })?;
                    obj.insert(key, value);
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

pub struct ValuePrinter<'lua>(pub LuaValue<'lua>);

impl<'lua> std::fmt::Debug for ValuePrinter<'lua> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        let visited = Rc::new(RefCell::new(HashSet::new()));
        ValuePrinterHelper {
            visited,
            value: self.0.clone(),
            is_cycle: false,
        }
        .fmt(fmt)
    }
}

struct ValuePrinterHelper<'lua> {
    visited: Rc<RefCell<HashSet<usize>>>,
    value: LuaValue<'lua>,
    is_cycle: bool,
}

impl<'lua> PartialEq for ValuePrinterHelper<'lua> {
    fn eq(&self, rhs: &Self) -> bool {
        self.value.eq(&rhs.value)
    }
}

impl<'lua> Eq for ValuePrinterHelper<'lua> {}

impl<'lua> PartialOrd for ValuePrinterHelper<'lua> {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        let lhs = lua_value_to_dynamic(self.value.clone()).unwrap_or(DynValue::Null);
        let rhs = lua_value_to_dynamic(rhs.value.clone()).unwrap_or(DynValue::Null);
        lhs.partial_cmp(&rhs)
    }
}

impl<'lua> Ord for ValuePrinterHelper<'lua> {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        let lhs = lua_value_to_dynamic(self.value.clone()).unwrap_or(DynValue::Null);
        let rhs = lua_value_to_dynamic(rhs.value.clone()).unwrap_or(DynValue::Null);
        lhs.cmp(&rhs)
    }
}

impl<'lua> ValuePrinterHelper<'lua> {
    fn has_cycle(&self, value: &mlua::Value) -> bool {
        self.visited
            .borrow()
            .contains(&(value.to_pointer() as usize))
    }
}

fn is_array_style_table(t: &mlua::Table) -> bool {
    let mut keys = BTreeSet::new();
    for pair in t.clone().pairs::<LuaValue, LuaValue>() {
        match pair {
            Ok((key, _)) => match key {
                LuaValue::Integer(i) if i >= 1 => {
                    keys.insert(i);
                }
                _ => return false,
            },
            Err(_) => return false,
        }
    }

    // Now see if we have contiguous keys.
    // The BTreeSet will iterate the keys in ascending order.
    let mut expect = 1;
    for key in keys {
        if key != expect {
            return false;
        }
        expect += 1;
    }

    true
}

impl<'lua> std::fmt::Debug for ValuePrinterHelper<'lua> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match &self.value {
            LuaValue::Table(_) if self.is_cycle => {
                fmt.write_fmt(format_args!("table: {:?}", self.value.to_pointer()))
            }
            LuaValue::Table(t) => {
                self.visited
                    .borrow_mut()
                    .insert(self.value.to_pointer() as usize);
                if is_array_style_table(&t) {
                    // Treat as list
                    let mut list = fmt.debug_list();
                    for value in t.clone().sequence_values() {
                        match value {
                            Ok(value) => {
                                let is_cycle = self.has_cycle(&value);
                                list.entry(&Self {
                                    visited: Rc::clone(&self.visited),
                                    value,
                                    is_cycle,
                                });
                            }
                            Err(err) => {
                                list.entry(&err);
                            }
                        }
                    }
                    list.finish()?;
                    drop(list);
                    Ok(())
                } else {
                    // Treat as map; put it into a BTreeMap so that we have a stable
                    // order for our tests.
                    let mut map = BTreeMap::new();
                    for pair in t.clone().pairs::<LuaValue, LuaValue>() {
                        match pair {
                            Ok(pair) => {
                                let is_cycle = self.has_cycle(&pair.1);
                                map.insert(
                                    Self {
                                        visited: Rc::clone(&self.visited),
                                        value: pair.0,
                                        is_cycle: false,
                                    },
                                    Self {
                                        visited: Rc::clone(&self.visited),
                                        value: pair.1,
                                        is_cycle,
                                    },
                                );
                            }
                            Err(err) => {
                                log::error!("error while retrieving map entry: {}", err);
                                break;
                            }
                        }
                    }
                    fmt.debug_map().entries(&map).finish()
                }
            }
            LuaValue::UserData(_) if self.is_cycle => {
                fmt.write_fmt(format_args!("userdata: {:?}", self.value.to_pointer()))
            }
            LuaValue::UserData(ud) => {
                if let Ok(mt) = ud.get_metatable() {
                    if let Ok(to_dynamic) = mt.get::<mlua::Function>("__wezterm_to_dynamic") {
                        return match to_dynamic.call(LuaValue::UserData(ud.clone())) {
                            Ok(value) => Self {
                                visited: Rc::clone(&self.visited),
                                value,
                                is_cycle: false,
                            }
                            .fmt(fmt),
                            Err(err) => write!(fmt, "Error calling __wezterm_to_dynamic: {err}"),
                        };
                    }
                }
                match self.value.to_string() {
                    Ok(s) => fmt.write_str(&s),
                    Err(err) => write!(fmt, "userdata ({err:#})"),
                }
            }
            LuaValue::Error(e) => fmt.write_fmt(format_args!("error {}", e)),
            LuaValue::String(s) => match s.to_str() {
                Ok(s) => fmt.write_fmt(format_args!("\"{}\"", s.escape_default())),
                Err(_) => {
                    let mut binary_string = "b\"".to_string();
                    for &b in s.as_bytes() {
                        if let Some(c) = char::from_u32(b as u32) {
                            if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c == ' ' {
                                binary_string.push(c);
                                continue;
                            }
                        }
                        binary_string.push_str(&format!("\\x{b:02x}"));
                    }
                    binary_string.push('"');
                    fmt.write_str(&binary_string)
                }
            },
            _ => match self.value.to_string() {
                Ok(s) => fmt.write_str(&s),
                Err(err) => write!(fmt, "({err:#})"),
            },
        }
    }
}
