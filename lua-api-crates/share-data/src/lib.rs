use config::lua::get_or_create_module;
use config::lua::mlua::{
    self, IntoLua, Lua, UserData, UserDataMethods, UserDataRef, Value as LuaValue,
};
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct Object {
    inner: Arc<Mutex<BTreeMap<String, Value>>>,
}

impl Ord for Object {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_ptr = self as *const Self;
        let other_ptr = other as *const Self;
        self_ptr.cmp(&other_ptr)
    }
}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.lock().unwrap();
        let b = other.inner.lock().unwrap();
        *a == *b
    }
}

impl Eq for Object {}

impl Hash for Object {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.lock().unwrap().hash(state)
    }
}

#[derive(Debug, Clone)]
struct Array {
    inner: Arc<Mutex<Vec<Value>>>,
}

impl Ord for Array {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_ptr = self as *const Self;
        let other_ptr = other as *const Self;
        self_ptr.cmp(&other_ptr)
    }
}

impl PartialOrd for Array {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        let a = self.inner.lock().unwrap();
        let b = other.inner.lock().unwrap();
        *a == *b
    }
}

impl Eq for Array {}

impl Hash for Array {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.lock().unwrap().hash(state)
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone)]
enum Value {
    Null,
    Bool(bool),
    String(String),
    Array(Array),
    Object(Object),
    I64(i64),
    F64(OrderedFloat<f64>),
}

fn lua_value_to_gvalue(value: LuaValue) -> mlua::Result<Value> {
    let mut visited = HashSet::new();
    lua_value_to_gvalue_impl(value, &mut visited)
}

fn lua_value_to_gvalue_impl(value: LuaValue, visited: &mut HashSet<usize>) -> mlua::Result<Value> {
    if let LuaValue::Table(_) = &value {
        let ptr = value.to_pointer() as usize;
        if visited.contains(&ptr) {
            // Skip this one, as we've seen it before.
            // Treat it as a Null value.
            return Ok(Value::Null);
        }
        visited.insert(ptr);
    }
    Ok(match value {
        LuaValue::Nil => Value::Null,
        LuaValue::String(s) => Value::String(s.to_str()?.to_string()),
        LuaValue::Boolean(b) => Value::Bool(b),
        LuaValue::Integer(i) => Value::I64(i),
        LuaValue::Number(i) => Value::F64(i.into()),
        // Handle our special Null userdata case and map it to Null
        LuaValue::LightUserData(ud) if ud.0.is_null() => Value::Null,
        LuaValue::LightUserData(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "userdata",
                to: "Value",
                message: None,
            })
        }
        LuaValue::UserData(ud) => match ud.get_metatable() {
            Ok(mt) => {
                if let Ok(to_dynamic) = mt.get::<mlua::Function>("__wezterm_to_dynamic") {
                    match to_dynamic.call(LuaValue::UserData(ud.clone())) {
                        Ok(value) => {
                            return lua_value_to_gvalue_impl(value, visited);
                        }
                        Err(err) => {
                            return Err(mlua::Error::FromLuaConversionError {
                                from: "userdata",
                                to: "Value",
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
                            return lua_value_to_gvalue_impl(value, visited);
                        }
                        Err(err) => {
                            return Err(mlua::Error::FromLuaConversionError {
                                from: "userdata",
                                to: "Value",
                                message: Some(format!("error calling tostring: {err:#}")),
                            })
                        }
                    },
                    Err(err) => {
                        return Err(mlua::Error::FromLuaConversionError {
                            from: "userdata",
                            to: "Value",
                            message: Some(format!("error getting tostring: {err:#}")),
                        })
                    }
                }
            }
            Err(err) => {
                return Err(mlua::Error::FromLuaConversionError {
                    from: "userdata",
                    to: "Value",
                    message: Some(format!("error getting metatable: {err:#}")),
                })
            }
        },
        LuaValue::Function(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "function",
                to: "Value",
                message: None,
            })
        }
        LuaValue::Thread(_) => {
            return Err(mlua::Error::FromLuaConversionError {
                from: "thread",
                to: "Value",
                message: None,
            })
        }
        LuaValue::Error(e) => return Err(e),
        LuaValue::Table(table) => {
            if let Ok(true) = table.contains_key(1) {
                let mut array = vec![];
                let pairs = table.clone();
                for value in table.sequence_values() {
                    array.push(lua_value_to_gvalue(value?)?);
                }

                for pair in pairs.pairs::<LuaValue, LuaValue>() {
                    let (key, _value) = pair?;
                    match &key {
                        LuaValue::Integer(n) if *n >= 1 && *n as usize <= array.len() => {
                            // Ok!
                        }
                        _ => {
                            let type_name = key.type_name();
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

                Value::Array(Array {
                    inner: Arc::new(Mutex::new(array.into())),
                })
            } else {
                let mut obj = BTreeMap::default();
                for pair in table.pairs::<String, LuaValue>() {
                    let (key, value) = pair?;
                    let lua_type = value.type_name();
                    let value = lua_value_to_gvalue(value).map_err(|e| {
                        mlua::Error::FromLuaConversionError {
                            from: lua_type,
                            to: "value",
                            message: Some(format!("while processing {key:?}: {e}")),
                        }
                    })?;
                    obj.insert(key, value);
                }
                Value::Object(Object {
                    inner: Arc::new(Mutex::new(obj.into())),
                })
            }
        }
    })
}

lazy_static::lazy_static! {
    static ref GLOBALS: Value = Value::Object(Object{inner:Arc::new(Mutex::new(BTreeMap::new()))});
}

fn gvalue_to_lua<'lua>(lua: &'lua Lua, value: &Value) -> mlua::Result<LuaValue<'lua>> {
    match value {
        Value::Array(arr) => {
            let result = lua.create_table()?;
            let arr = arr.inner.lock().unwrap();
            for (idx, value) in arr.iter().enumerate() {
                result.set(idx + 1, gvalue_to_lua(lua, value)?)?;
            }
            Ok(LuaValue::Table(result))
        }
        Value::Object(obj) => {
            let result = lua.create_table()?;
            let obj = obj.inner.lock().unwrap();
            for (key, value) in obj.iter() {
                result.set(key.clone(), gvalue_to_lua(lua, value)?)?;
            }
            Ok(LuaValue::Table(result))
        }
        Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        Value::Null => Ok(LuaValue::Nil),
        Value::String(s) => s.to_string().into_lua(lua),
        Value::I64(i) => Ok(LuaValue::Integer(*i)),
        Value::F64(n) => n.into_lua(lua),
    }
}

impl UserData for Value {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(
            "__wezterm_to_dynamic",
            |lua: &Lua, this, _: ()| -> mlua::Result<mlua::Value> { gvalue_to_lua(lua, this) },
        );
        methods.add_meta_method(
            mlua::MetaMethod::Len,
            |lua: &Lua, this, _: ()| -> mlua::Result<mlua::Value> {
                match this {
                    Value::Array(arr) => arr.inner.lock().unwrap().len().into_lua(lua),
                    Value::Object(obj) => obj.inner.lock().unwrap().len().into_lua(lua),
                    Value::String(s) => s.to_string().into_lua(lua),
                    _ => Err(mlua::Error::external(
                        "invalid type for len operator".to_string(),
                    )),
                }
            },
        );

        methods.add_meta_method(mlua::MetaMethod::Pairs, |lua, this, ()| match this {
            Value::Array(_) => {
                let stateless_iter = lua.create_function(
                    |lua, (this, i): (UserDataRef<Value>, usize)| match &*this {
                        Value::Array(arr) => {
                            let arr = arr.inner.lock().unwrap();
                            let i = i + 1;

                            if i <= arr.len() {
                                return Ok(mlua::Variadic::from_iter(vec![
                                    i.into_lua(lua)?,
                                    arr[i - 1].clone().into_lua(lua)?,
                                ]));
                            }
                            return Ok(mlua::Variadic::new());
                        }
                        _ => unreachable!(),
                    },
                )?;
                Ok((stateless_iter, this.clone(), 0.into_lua(lua)?))
            }
            Value::Object(_) => {
                let stateless_iter = lua.create_function(
                    |lua, (this, key): (UserDataRef<Value>, Option<String>)| match &*this {
                        Value::Object(obj) => {
                            let obj = obj.inner.lock().unwrap();
                            let mut iter = obj.iter();

                            let mut this_is_key = false;

                            if key.is_none() {
                                this_is_key = true;
                            }

                            while let Some((this_key, value)) = iter.next() {
                                if this_is_key {
                                    return Ok(mlua::MultiValue::from_vec(vec![
                                        this_key.clone().into_lua(lua)?,
                                        value.clone().into_lua(lua)?,
                                    ]));
                                }
                                if Some(this_key.as_str()) == key.as_deref() {
                                    this_is_key = true;
                                }
                            }
                            return Ok(mlua::MultiValue::new());
                        }
                        _ => unreachable!(),
                    },
                )?;
                Ok((stateless_iter, this.clone(), LuaValue::Nil))
            }
            _ => Err(mlua::Error::external(
                "invalid type for __ipairs metamethod".to_string(),
            )),
        });

        methods.add_meta_method(
            mlua::MetaMethod::Index,
            |lua: &Lua, this, key: LuaValue| -> mlua::Result<mlua::Value> {
                match this {
                    Value::Array(arr) => match key {
                        LuaValue::Integer(i) => {
                            if i <= 0 {
                                return Err(mlua::Error::external(format!(
                                    "invalid array index {i}"
                                )));
                            }
                            // Convert lua 1-based indices to 0-based
                            let i = (i as usize) - 1;

                            let arr = arr.inner.lock().unwrap();
                            let value = match arr.get(i) {
                                None => return Ok(LuaValue::Nil),
                                Some(v) => v,
                            };

                            match value {
                                Value::Null => Ok(LuaValue::Nil),
                                Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
                                Value::String(s) => s.clone().into_lua(lua),
                                Value::F64(u) => u.into_lua(lua),
                                Value::I64(u) => u.into_lua(lua),
                                Value::Array(_) => value.clone().into_lua(lua),
                                Value::Object(_) => value.clone().into_lua(lua),
                            }
                        }
                        _ => Err(mlua::Error::external(
                            "can only index arrays using integer values",
                        )),
                    },
                    Value::Object(obj) => match key {
                        LuaValue::String(s) => match s.to_str() {
                            Err(e) => Err(mlua::Error::external(format!(
                                "can only index objects using unicode strings: {e:#}"
                            ))),
                            Ok(s) => {
                                let obj = obj.inner.lock().unwrap();
                                let value = match obj.get(s) {
                                    None => return Ok(LuaValue::Nil),
                                    Some(v) => v,
                                };
                                match value {
                                    Value::Null => Ok(LuaValue::Nil),
                                    Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
                                    Value::String(s) => s.clone().into_lua(lua),
                                    Value::F64(u) => u.into_lua(lua),
                                    Value::I64(u) => u.into_lua(lua),
                                    Value::Array(_) => value.clone().into_lua(lua),
                                    Value::Object(_) => value.clone().into_lua(lua),
                                }
                            }
                        },
                        _ => Err(mlua::Error::external(
                            "can only index objects using string values",
                        )),
                    },
                    _ => Err(mlua::Error::external(
                        "can only index array or object values".to_string(),
                    )),
                }
            },
        );
        methods.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, this, (key, value): (LuaValue, LuaValue)| -> mlua::Result<()> {
                match this {
                    Value::Array(arr) => match key {
                        LuaValue::Integer(i) => {
                            if i <= 0 {
                                return Err(mlua::Error::external(format!(
                                    "invalid array index {i}"
                                )));
                            }
                            // Convert lua 1-based indices to 0-based
                            let i = (i as usize) - 1;

                            let mut arr = arr.inner.lock().unwrap();
                            if i >= arr.len() {
                                return Err(mlua::Error::external(format!(
                                    "cannot make sparse array by inserting at {i} when len is {}",
                                    arr.len()
                                )));
                            }

                            let value = lua_value_to_gvalue(value)?;

                            if i == arr.len() - 1 {
                                arr.push(value);
                            } else {
                                arr[i] = value;
                            }

                            Ok(())
                        }
                        _ => Err(mlua::Error::external(
                            "can only index arrays using integer values",
                        )),
                    },
                    Value::Object(obj) => match key {
                        LuaValue::String(s) => match s.to_str() {
                            Err(e) => Err(mlua::Error::external(format!(
                                "can only index objects using unicode strings: {e:#}"
                            ))),
                            Ok(s) => {
                                let mut obj = obj.inner.lock().unwrap();
                                let value = lua_value_to_gvalue(value)?;
                                obj.insert(s.to_string(), value);
                                Ok(())
                            }
                        },
                        _ => Err(mlua::Error::external(
                            "can only index objects using string values",
                        )),
                    },
                    _ => Err(mlua::Error::external(
                        "can only index array or object values".to_string(),
                    )),
                }
            },
        );
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("GLOBAL", GLOBALS.clone())?;
    Ok(())
}
