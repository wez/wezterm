use mlua::{Table, Value};
use serde::de::value::{MapDeserializer, SeqDeserializer};
use serde::de::{
    DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, Error as SerdeDeError,
    IntoDeserializer, Unexpected, VariantAccess, Visitor,
};
use serde::{serde_if_integer128, Deserialize};
use std::convert::TryInto;
use thiserror::*;

pub mod ser;

/// This is the key function from this module; it uses serde to
/// "parse" a lua value into a Rust type that implements Deserialize.
pub fn from_lua_value<T>(value: Value) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    T::deserialize(ValueWrapper(value))
}

fn unexpected<'lua>(v: &'lua Value<'lua>) -> Unexpected<'lua> {
    match v {
        Value::Nil => Unexpected::Other("lua nil"),
        Value::Boolean(b) => Unexpected::Bool(*b),
        Value::LightUserData(_) => Unexpected::Other("lua lightuserdata"),
        Value::Integer(i) => Unexpected::Signed(*i),
        Value::Number(n) => Unexpected::Float(*n),
        Value::String(s) => match s.to_str() {
            Ok(s) => Unexpected::Str(s),
            Err(_) => Unexpected::Bytes(s.as_bytes()),
        },
        Value::Table(t) => match t.contains_key(1) {
            Ok(true) => Unexpected::Other("lua array-like table"),
            Ok(false) => Unexpected::Other("lua map-like table"),
            Err(_) => Unexpected::Other(
                "lua table (but encountered an error while testing if it is array- or map-like)",
            ),
        },
        Value::Function(_) => Unexpected::Other("lua function"),
        Value::Thread(_) => Unexpected::Other("lua thread"),
        Value::UserData(_) => Unexpected::Other("lua userdata"),
        Value::Error(_) => Unexpected::Other("lua error"),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{}", msg)]
    Custom { msg: String },
}

impl SerdeDeError for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Custom {
            msg: msg.to_string(),
        }
    }
}

impl From<Error> for mlua::Error {
    fn from(e: Error) -> mlua::Error {
        mlua::Error::external(e)
    }
}

#[derive(Debug)]
struct ValueWrapper<'lua>(Value<'lua>);

impl<'de, 'lua> IntoDeserializer<'de, Error> for ValueWrapper<'lua> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

fn visit_table<'de, 'lua, V>(
    table: Table<'lua>,
    visitor: V,
    struct_name: Option<&'static str>,
    allowed_fields: Option<&'static [&'static str]>,
) -> Result<V::Value, Error>
where
    V: Visitor<'de>,
{
    // First we need to determine whether this table looks like an array
    // or whether it looks like a map.  Lua allows for either or both in
    // the same table.
    // Since array like tables start with index 1 we look for that key
    // and assume that if it has that that it is an array.
    if let Ok(true) = table.contains_key(1) {
        // Treat it as an array
        let mut values = vec![];
        for value in table.sequence_values() {
            match value {
                Ok(value) => values.push(ValueWrapper(value)),
                Err(err) => {
                    return Err(Error::custom(format!(
                        "while retrieving an array element: {}",
                        err
                    )))
                }
            }
        }

        let mut deser = SeqDeserializer::new(values.into_iter());
        let seq = match visitor.visit_seq(&mut deser) {
            Ok(seq) => seq,
            Err(err) => return Err(err),
        };

        deser.end()?;
        Ok(seq)
    } else {
        // Treat it as a map
        let mut pairs = vec![];
        for pair in table.pairs::<String, Value>() {
            match pair {
                Ok(pair) => {
                    // When deserializing into a struct with known field names,
                    // we don't want to hard error if the user gave a bogus field
                    // name; we'd rather generate a warning somewhere and attempt
                    // to proceed.  This makes the config a bit more forgiving of
                    // typos and also makes it easier to use a given config in
                    // a future version of wezterm where the configuration may
                    // evolve over time.
                    if let Some(allowed_fields) = allowed_fields {
                        if !allowed_fields.iter().any(|&name| name == &pair.0) {
                            // The field wasn't one of the allowed fields in this
                            // context.  Generate an error message that is hopefully
                            // helpful; we'll suggest the set of most similar field
                            // names (ordered by similarity) and list out the remaining
                            // possible field names in alpha order

                            // Produce similar field name list
                            let mut candidates: Vec<(f64, &str)> = allowed_fields
                                .iter()
                                .map(|&name| (strsim::jaro_winkler(&pair.0, name), name))
                                .filter(|(confidence, _)| *confidence > 0.8)
                                .collect();
                            candidates.sort_by(|a, b| {
                                b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                            });
                            let suggestions: Vec<&str> =
                                candidates.into_iter().map(|(_, name)| name).collect();

                            // Filter the suggestions out of the allowed field names
                            // and sort what remains.
                            let mut fields: Vec<&str> = allowed_fields
                                .iter()
                                .filter(|&name| {
                                    !suggestions.iter().any(|candidate| candidate == name)
                                })
                                .map(|&name| name)
                                .collect();
                            fields.sort();

                            let mut message = String::new();

                            match suggestions.len() {
                                0 => {}
                                1 => {
                                    message.push_str(&format!("Did you mean `{}`?", suggestions[0]))
                                }
                                _ => {
                                    message.push_str("Did you mean one of ");
                                    for (idx, candidate) in suggestions.iter().enumerate() {
                                        if idx > 0 {
                                            message.push_str(", ");
                                        }
                                        message.push('`');
                                        message.push_str(candidate);
                                        message.push('`');
                                    }
                                    message.push_str("?");
                                }
                            }
                            if !fields.is_empty() {
                                if suggestions.is_empty() {
                                    message.push_str("Possible fields are ");
                                } else {
                                    message.push_str(" Other possible fields are ");
                                }
                                for (idx, candidate) in fields.iter().enumerate() {
                                    if idx > 0 {
                                        message.push_str(", ");
                                    }
                                    message.push('`');
                                    message.push_str(candidate);
                                    message.push('`');
                                }
                                message.push('.');
                            }
                            log::error!(
                                "Ignoring unknown field `{}` in struct of type `{}`. {}",
                                pair.0,
                                struct_name.unwrap_or("<unknown>"),
                                message
                            );

                            continue;
                        }
                    }
                    pairs.push((pair.0, ValueWrapper(pair.1)))
                }
                Err(err) => {
                    return Err(Error::custom(format!(
                        "while retrieving map element: {}",
                        err
                    )))
                }
            }
        }
        let mut deser = MapDeserializer::new(pairs.into_iter());
        let seq = match visitor.visit_map(&mut deser) {
            Ok(seq) => seq,
            Err(err) => return Err(err),
        };

        deser.end()?;
        Ok(seq)
    }
}

macro_rules! int {
    ($name:ident, $ty:ty, $visit:ident) => {
        fn $name<V>(self, v: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Integer(i) => v.$visit(i.try_into().map_err(|e| {
                    Error::custom(format!(
                        "lua Integer value {} doesn't fit \
                             in specified type: {}",
                        i, e
                    ))
                })?),
                _ => Err(serde::de::Error::invalid_type(
                    unexpected(&self.0),
                    &"integer",
                )),
            }
        }
    };
}

impl<'de, 'lua> Deserializer<'de> for ValueWrapper<'lua> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Nil => visitor.visit_unit(),
            Value::Boolean(v) => visitor.visit_bool(v),
            Value::Integer(i) => visitor.visit_i64(i),
            Value::Number(n) => visitor.visit_f64(n),
            Value::String(s) => match s.to_str() {
                Ok(s) => visitor.visit_str(s),
                Err(_) => visitor.visit_bytes(s.as_bytes()),
            },
            Value::Table(t) => visit_table(t, visitor, None, None),
            Value::UserData(_) | Value::LightUserData(_) => Err(Error::custom(
                "cannot represent userdata in the serde data model",
            )),
            Value::Thread(_) => Err(Error::custom(
                "cannot represent thread in the serde data model",
            )),
            Value::Function(_) => Err(Error::custom(
                "cannot represent lua function in the serde data model",
            )),
            Value::Error(e) => Err(Error::custom(format!(
                "cannot represent lua error {} in the serde data model",
                e
            ))),
        }
    }

    fn deserialize_bool<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Boolean(b) => v.visit_bool(b),
            _ => Err(serde::de::Error::invalid_type(unexpected(&self.0), &"bool")),
        }
    }

    fn deserialize_option<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Nil => v.visit_none(),
            _ => v.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Nil => v.visit_unit(),
            _ => v.visit_some(self),
        }
    }

    fn deserialize_ignored_any<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        v.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &str, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(v)
    }

    fn deserialize_newtype_struct<V>(self, _name: &str, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        v.visit_newtype_struct(self)
    }

    fn deserialize_char<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(v)
    }

    fn deserialize_str<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(v)
    }

    fn deserialize_identifier<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(v)
    }

    fn deserialize_string<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::String(s) => match s.to_str() {
                Ok(s) => v.visit_str(s),
                Err(_) => Err(Error::custom(
                    "expected String but found a non-UTF8 lua string",
                )),
            },
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"string",
            )),
        }
    }

    fn deserialize_bytes<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(v)
    }

    fn deserialize_byte_buf<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::String(s) => match s.to_str() {
                Ok(s) => v.visit_str(s),
                Err(_) => v.visit_bytes(s.as_bytes()),
            },
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"bytes",
            )),
        }
    }

    fn deserialize_seq<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Table(t) => visit_table(t, v, None, None),
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"sequence/array",
            )),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(v)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        v: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(v)
    }

    int!(deserialize_i8, i8, visit_i8);
    int!(deserialize_u8, u8, visit_u8);
    int!(deserialize_i16, i16, visit_i16);
    int!(deserialize_u16, u16, visit_u16);
    int!(deserialize_i32, i32, visit_i32);
    int!(deserialize_u32, u32, visit_u32);
    int!(deserialize_i64, i64, visit_i64);
    int!(deserialize_u64, u64, visit_u64);

    serde_if_integer128! {
        int!(deserialize_i128, i128, visit_i128);
        int!(deserialize_u128, u128, visit_u128);
    }

    fn deserialize_f64<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Number(i) => v.visit_f64(i),
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"floating point number",
            )),
        }
    }

    fn deserialize_f32<V>(self, v: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Number(i) => v.visit_f32(i as f32),
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"floating point number",
            )),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let (variant, value) = match self.0 {
            Value::Table(t) => {
                let mut iter = t.pairs::<String, Value>();
                let (variant, value) = match iter.next() {
                    Some(Ok(v)) => v,
                    Some(Err(e)) => {
                        return Err(Error::custom(format!(
                            "failed to retrieve enum pair from map: {}",
                            e
                        )));
                    }
                    None => {
                        return Err(serde::de::Error::invalid_value(
                            Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                };

                // enums are encoded in serde_json as maps with a single
                // key:value pair, so we mirror that here
                if iter.next().is_some() {
                    return Err(serde::de::Error::invalid_value(
                        Unexpected::Map,
                        &"map with a single key",
                    ));
                }
                (variant, Some(value))
            }
            Value::String(s) => match s.to_str() {
                Ok(s) => (s.to_owned(), None),
                Err(_) => {
                    return Err(serde::de::Error::invalid_value(
                        Unexpected::Bytes(s.as_bytes()),
                        &"UTF-8 string key",
                    ))
                }
            },
            _ => {
                return Err(serde::de::Error::invalid_type(
                    Unexpected::Other("?"),
                    &"string or map",
                ));
            }
        };

        visitor.visit_enum(EnumDeserializer { variant, value })
    }

    fn deserialize_map<V>(self, v: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Table(t) => visit_table(t, v, None, None),
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"a map",
            )),
        }
    }

    fn deserialize_struct<V>(
        self,
        struct_name: &'static str,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Table(t) => match visit_table(t, v, Some(struct_name), Some(fields)) {
                Ok(v) => Ok(v),
                Err(err) => Err(Error::custom(format!(
                    "{} (while processing a struct of type `{}`)",
                    err, struct_name
                ))),
            },
            _ => Err(serde::de::Error::invalid_type(
                unexpected(&self.0),
                &"a map",
            )),
        }
    }
}

struct EnumDeserializer<'lua> {
    variant: String,
    value: Option<Value<'lua>>,
}

impl<'de, 'lua> EnumAccess<'de> for EnumDeserializer<'lua> {
    type Error = Error;
    type Variant = VariantDeserializer<'lua>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, VariantDeserializer<'lua>), Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = self.variant.into_deserializer();
        let visitor = VariantDeserializer { value: self.value };
        seed.deserialize(variant).map(|v| (v, visitor))
    }
}

struct VariantDeserializer<'lua> {
    value: Option<Value<'lua>>,
}

impl<'de, 'lua> VariantAccess<'de> for VariantDeserializer<'lua> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            Some(value) => Deserialize::deserialize(ValueWrapper(value)),
            None => Ok(()),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(ValueWrapper(value)),
            None => Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Value::Table(table)) => {
                if let Ok(true) = table.contains_key(1) {
                    let mut values = vec![];
                    for value in table.sequence_values() {
                        match value {
                            Ok(value) => values.push(ValueWrapper(value)),
                            Err(err) => {
                                return Err(Error::custom(format!(
                                    "while retrieving an array element: {}",
                                    err
                                )))
                            }
                        }
                    }

                    let deser = SeqDeserializer::new(values.into_iter());
                    serde::Deserializer::deserialize_any(deser, visitor)
                } else {
                    Err(serde::de::Error::invalid_type(
                        Unexpected::Map,
                        &"tuple variant",
                    ))
                }
            }
            Some(v) => Err(serde::de::Error::invalid_type(
                unexpected(&v),
                &"tuple variant",
            )),
            None => Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            )),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Value::Table(table)) => {
                if let Ok(false) = table.contains_key(1) {
                    let mut pairs = vec![];
                    for pair in table.pairs::<String, Value>() {
                        match pair {
                            Ok(pair) => pairs.push((pair.0, ValueWrapper(pair.1))),
                            Err(err) => {
                                return Err(Error::custom(format!(
                                    "while retrieving map element: {}",
                                    err
                                )))
                            }
                        }
                    }
                    let deser = MapDeserializer::new(pairs.into_iter());
                    serde::Deserializer::deserialize_any(deser, visitor)
                } else {
                    Err(serde::de::Error::invalid_type(
                        Unexpected::Seq,
                        &"struct variant",
                    ))
                }
            }
            Some(v) => Err(serde::de::Error::invalid_type(
                unexpected(&v),
                &"struct variant",
            )),
            _ => Err(serde::de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use mlua::Lua;
    use serde::Serialize;

    fn round_trip<
        T: Serialize + DeserializeOwned + ?Sized + PartialEq + std::fmt::Debug + Clone,
    >(
        value: T,
    ) {
        let lua = Lua::new();
        let lua_value: Value = ser::to_lua_value(&lua, value.clone()).unwrap();
        let round_tripped: T = from_lua_value(lua_value).unwrap();
        assert_eq!(value, round_tripped);
    }

    #[test]
    fn test_bool() {
        let lua = Lua::new();
        let res: bool = from_lua_value(lua.load("true").eval().unwrap()).unwrap();
        assert_eq!(res, true);
        round_trip(res);

        let res: bool = from_lua_value(lua.load("false").eval().unwrap()).unwrap();
        assert_eq!(res, false);
        round_trip(res);
    }

    #[test]
    fn test_nil() {
        let lua = Lua::new();
        let res: () = from_lua_value(lua.load("nil").eval().unwrap()).unwrap();
        round_trip(res);
    }

    #[test]
    fn test_int() {
        let lua = Lua::new();
        let res: i64 = from_lua_value(lua.load("123").eval().unwrap()).unwrap();
        assert_eq!(res, 123);
        round_trip(res);

        let res: i32 = from_lua_value(lua.load("123").eval().unwrap()).unwrap();
        assert_eq!(res, 123);
        round_trip(res);

        let res: i16 = from_lua_value(lua.load("123").eval().unwrap()).unwrap();
        assert_eq!(res, 123);
        round_trip(res);

        let res: i8 = from_lua_value(lua.load("123").eval().unwrap()).unwrap();
        assert_eq!(res, 123);
        round_trip(res);
    }

    #[test]
    fn test_float() {
        let lua = Lua::new();
        let res: f64 = from_lua_value(lua.load("123.5").eval().unwrap()).unwrap();
        assert_eq!(res, 123.5);
        round_trip(res);
    }

    #[test]
    fn test_string() {
        let lua = Lua::new();
        let res: String = from_lua_value(lua.load("\"hello\"").eval().unwrap()).unwrap();
        assert_eq!(res, "hello");
        round_trip(res);
    }

    #[test]
    fn test_array_table() {
        let lua = Lua::new();
        let res: Vec<i8> = from_lua_value(lua.load("{1, 2, 3}").eval().unwrap()).unwrap();
        assert_eq!(res, vec![1, 2, 3]);
        round_trip(res);
    }

    #[test]
    fn test_map_table() {
        let lua = Lua::new();

        #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
        struct MyMap {
            hello: String,
            age: usize,
        };

        let res: MyMap =
            from_lua_value(lua.load("{hello=\"hello\", age=42}").eval().unwrap()).unwrap();
        assert_eq!(
            res,
            MyMap {
                hello: "hello".to_owned(),
                age: 42
            }
        );
        round_trip(res);

        let err = from_lua_value::<MyMap>(lua.load("{hello=\"hello\", age=true}").eval().unwrap())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid type: boolean `true`, expected integer (\
            while processing a struct of type `MyMap`)"
        );
    }

    #[test]
    fn test_enum() {
        #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
        enum MyEnum {
            Foo,
            Bar,
        };
        let lua = Lua::new();
        let res: MyEnum = from_lua_value(lua.load("\"Foo\"").eval().unwrap()).unwrap();
        assert_eq!(res, MyEnum::Foo);
        round_trip(res);

        let res: MyEnum = from_lua_value(lua.load("\"Bar\"").eval().unwrap()).unwrap();
        assert_eq!(res, MyEnum::Bar);
        round_trip(res);

        let err = from_lua_value::<MyEnum>(lua.load("\"Invalid\"").eval().unwrap()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown variant `Invalid`, expected `Foo` or `Bar`"
        );
    }
}
