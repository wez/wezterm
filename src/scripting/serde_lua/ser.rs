use super::ValueWrapper;
use mlua::{Lua, Table, ToLua, Value};
use serde::ser::Error as SerError;
use serde::{serde_if_integer128, Serialize, Serializer};
use thiserror::*;

pub fn to_lua_value<'lua, T>(lua: &'lua Lua, input: T) -> Result<Value<'lua>, Error>
where
    T: Serialize,
{
    let serializer = LuaSerializer { lua };
    input.serialize(serializer)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{:?}", msg)]
    Custom { msg: String },
}

impl Error {
    fn lua(e: mlua::Error) -> Error {
        Error::custom(e)
    }
}

impl From<Error> for mlua::Error {
    fn from(e: Error) -> mlua::Error {
        mlua::Error::external(e)
    }
}

impl SerError for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom {
            msg: msg.to_string(),
        }
    }
}

struct LuaSerializer<'lua> {
    lua: &'lua Lua,
}

struct LuaSeqSerializer<'lua> {
    lua: &'lua Lua,
    table: Table<'lua>,
    index: usize,
}

impl<'lua> serde::ser::SerializeSeq for LuaSeqSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let value = value.serialize(LuaSerializer { lua: self.lua })?;
        self.table.set(self.index, value).map_err(Error::lua)?;
        self.index += 1;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Table(self.table))
    }
}

impl<'lua> serde::ser::SerializeTuple for LuaSeqSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl<'lua> serde::ser::SerializeTupleStruct for LuaSeqSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value<'lua>, Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

struct LuaTupleVariantSerializer<'lua> {
    lua: &'lua Lua,
    table: Table<'lua>,
    index: usize,
    name: String,
}

impl<'lua> serde::ser::SerializeTupleVariant for LuaTupleVariantSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let value = value.serialize(LuaSerializer { lua: self.lua })?;
        self.table.set(self.index, value).map_err(Error::lua)?;
        self.index += 1;
        Ok(())
    }

    fn end(self) -> Result<Value<'lua>, Error> {
        let map = self.lua.create_table().map_err(Error::lua)?;
        map.set(self.name, self.table).map_err(Error::lua)?;
        Ok(Value::Table(map))
    }
}

struct LuaMapSerializer<'lua> {
    lua: &'lua Lua,
    table: Table<'lua>,
    key: Option<Value<'lua>>,
}

impl<'lua> serde::ser::SerializeMap for LuaMapSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        let key = key.serialize(LuaSerializer { lua: self.lua })?;
        self.key.replace(key);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let value = value.serialize(LuaSerializer { lua: self.lua })?;
        let key = self
            .key
            .take()
            .expect("serialize_key must be called before serialize_value");
        self.table.set(key, value).map_err(Error::lua)?;
        Ok(())
    }

    fn serialize_entry<K: Serialize + ?Sized, V: Serialize + ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Error> {
        let key = key.serialize(LuaSerializer { lua: self.lua })?;
        let value = value.serialize(LuaSerializer { lua: self.lua })?;
        self.table.set(key, value).map_err(Error::lua)?;
        Ok(())
    }

    fn end(self) -> Result<Value<'lua>, Error> {
        Ok(Value::Table(self.table))
    }
}

impl<'lua> serde::ser::SerializeStruct for LuaMapSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), Error> {
        serde::ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Value<'lua>, Error> {
        serde::ser::SerializeMap::end(self)
    }
}

struct LuaStructVariantSerializer<'lua> {
    lua: &'lua Lua,
    name: String,
    table: Table<'lua>,
}

impl<'lua> serde::ser::SerializeStructVariant for LuaStructVariantSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), Error> {
        let key = key.serialize(LuaSerializer { lua: self.lua })?;
        let value = value.serialize(LuaSerializer { lua: self.lua })?;
        self.table.set(key, value).map_err(Error::lua)?;
        Ok(())
    }

    fn end(self) -> Result<Value<'lua>, Error> {
        let map = self.lua.create_table().map_err(Error::lua)?;
        map.set(self.name, self.table).map_err(Error::lua)?;
        Ok(Value::Table(map))
    }
}

impl<'lua> serde::Serializer for LuaSerializer<'lua> {
    type Ok = Value<'lua>;
    type Error = Error;
    type SerializeSeq = LuaSeqSerializer<'lua>;
    type SerializeTuple = LuaSeqSerializer<'lua>;
    type SerializeTupleStruct = LuaSeqSerializer<'lua>;
    type SerializeTupleVariant = LuaTupleVariantSerializer<'lua>;
    type SerializeMap = LuaMapSerializer<'lua>;
    type SerializeStruct = LuaMapSerializer<'lua>;
    type SerializeStructVariant = LuaStructVariantSerializer<'lua>;

    fn serialize_bool(self, b: bool) -> Result<Value<'lua>, Error> {
        b.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_i8(self, i: i8) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_i16(self, i: i16) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_i32(self, i: i32) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_i64(self, i: i64) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_u8(self, i: u8) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_u16(self, i: u16) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_u32(self, i: u32) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_u64(self, i: u64) -> Result<Value<'lua>, Error> {
        i.to_lua(self.lua).map_err(Error::lua)
    }

    serde_if_integer128! {
        fn serialize_u128(self, i: u128) -> Result<Value<'lua>, Error> {
            i.to_lua(self.lua).map_err(Error::lua)
        }
        fn serialize_i128(self, i: i128) -> Result<Value<'lua>, Error> {
            i.to_lua(self.lua).map_err(Error::lua)
        }
    }

    fn serialize_f32(self, f: f32) -> Result<Value<'lua>, Error> {
        f.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_f64(self, f: f64) -> Result<Value<'lua>, Error> {
        f.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_char(self, c: char) -> Result<Value<'lua>, Error> {
        let mut s = String::new();
        s.push(c);
        self.serialize_str(&s)
    }

    fn serialize_str(self, s: &str) -> Result<Value<'lua>, Error> {
        s.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_bytes(self, b: &[u8]) -> Result<Value<'lua>, Error> {
        let b: &bstr::BStr = b.into();
        b.to_lua(self.lua).map_err(Error::lua)
    }

    fn serialize_none(self) -> Result<Value<'lua>, Error> {
        Ok(Value::Nil)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Value<'lua>, Error> {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value<'lua>, Error> {
        Ok(Value::Nil)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value<'lua>, Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value<'lua>, Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value<'lua>, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value<'lua>, Error> {
        let value = value.serialize(LuaSerializer { lua: self.lua })?;

        let table = self.lua.create_table().map_err(Error::lua)?;
        table.set(variant, value).map_err(Error::lua)?;
        Ok(Value::Table(table))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<LuaSeqSerializer<'lua>, Error> {
        self.serialize_tuple(len.unwrap_or(0))
    }

    fn serialize_tuple(self, _len: usize) -> Result<LuaSeqSerializer<'lua>, Error> {
        let table = self.lua.create_table().map_err(Error::lua)?;
        Ok(LuaSeqSerializer {
            lua: self.lua,
            table,
            index: 1,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<LuaSeqSerializer<'lua>, Error> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<LuaTupleVariantSerializer<'lua>, Error> {
        let table = self.lua.create_table().map_err(Error::lua)?;
        Ok(LuaTupleVariantSerializer {
            lua: self.lua,
            table,
            index: 1,
            name: variant.to_string(),
        })
    }

    fn serialize_map(
        self,
        _len: std::option::Option<usize>,
    ) -> Result<LuaMapSerializer<'lua>, Error> {
        let table = self.lua.create_table().map_err(Error::lua)?;
        Ok(LuaMapSerializer {
            lua: self.lua,
            table,
            key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<LuaMapSerializer<'lua>, Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<LuaStructVariantSerializer<'lua>, Error> {
        let table = self.lua.create_table().map_err(Error::lua)?;
        Ok(LuaStructVariantSerializer {
            lua: self.lua,
            table,
            name: variant.to_owned(),
        })
    }
}

impl<'lua> Serialize for ValueWrapper<'lua> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Value::Nil => serializer.serialize_unit(),
            Value::Boolean(b) => serializer.serialize_bool(*b),
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Number(n) => serializer.serialize_f64(*n),
            Value::String(s) => match s.to_str() {
                Ok(s) => serializer.serialize_str(s),
                Err(_) => serializer.serialize_bytes(s.as_bytes()),
            },
            Value::Table(table) => {
                if let Ok(true) = table.contains_key(1) {
                    let mut values = vec![];
                    for value in table.clone().sequence_values() {
                        match value {
                            Ok(value) => values.push(ValueWrapper(value)),
                            Err(err) => {
                                return Err(S::Error::custom(format!(
                                    "while retrieving an array element: {}",
                                    err
                                )))
                            }
                        }
                    }
                    values.serialize(serializer)
                } else {
                    use serde::ser::SerializeMap;
                    let mut pairs = vec![];
                    for pair in table.clone().pairs::<Value, Value>() {
                        match pair {
                            Ok(pair) => pairs.push((ValueWrapper(pair.0), ValueWrapper(pair.1))),
                            Err(err) => {
                                return Err(S::Error::custom(format!(
                                    "while retrieving map element: {}",
                                    err
                                )))
                            }
                        }
                    }

                    let mut map = serializer.serialize_map(Some(pairs.len()))?;
                    for (k, v) in pairs.into_iter() {
                        map.serialize_entry(&k, &v)?;
                    }
                    map.end()
                }
            }
            Value::UserData(_) | Value::LightUserData(_) => Err(S::Error::custom(
                "cannot represent userdata in the serde data model",
            )),
            Value::Thread(_) => Err(S::Error::custom(
                "cannot represent thread in the serde data model",
            )),
            Value::Function(_) => Err(S::Error::custom(
                "cannot represent lua function in the serde data model",
            )),
            Value::Error(e) => Err(S::Error::custom(format!(
                "cannot represent lua error {} in the serde data model",
                e
            ))),
        }
    }
}
