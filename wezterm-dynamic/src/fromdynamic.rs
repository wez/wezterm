use crate::error::Error;
use crate::value::Value;
use ordered_float::OrderedFloat;
use std::collections::HashMap;
use std::convert::TryInto;
use std::hash::Hash;

/// Specify how FromDynamic will treat unknown fields
/// when converting from Value to a given target type
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnknownFieldAction {
    /// Don't check, don't warn, don't raise an error
    Ignore,
    /// Emit a log::warn log
    Warn,
    /// Return an Error
    Deny,
}

impl Default for UnknownFieldAction {
    fn default() -> UnknownFieldAction {
        UnknownFieldAction::Warn
    }
}

/// Specify various options for FromDynamic::from_dynamic
#[derive(Copy, Clone, Debug, Default)]
pub struct FromDynamicOptions {
    pub unknown_fields: UnknownFieldAction,
    pub deprecated_fields: UnknownFieldAction,
}

impl FromDynamicOptions {
    pub fn flatten(self) -> Self {
        Self {
            unknown_fields: UnknownFieldAction::Ignore,
            ..self
        }
    }
}

/// The FromDynamic trait allows a type to construct itself from a Value.
/// This trait can be derived.
pub trait FromDynamic {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error>
    where
        Self: Sized;
}

impl FromDynamic for Value {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        Ok(value.clone())
    }
}

impl FromDynamic for ordered_float::NotNan<f64> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        let f = f64::from_dynamic(value, options)?;
        Ok(ordered_float::NotNan::new(f).map_err(|e| Error::Message(e.to_string()))?)
    }
}

impl FromDynamic for std::time::Duration {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        let f = f64::from_dynamic(value, options)?;
        Ok(std::time::Duration::from_secs_f64(f))
    }
}

impl<T: FromDynamic> FromDynamic for Box<T> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        let value = T::from_dynamic(value, options)?;
        Ok(Box::new(value))
    }
}

impl<T: FromDynamic> FromDynamic for std::sync::Arc<T> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        let value = T::from_dynamic(value, options)?;
        Ok(std::sync::Arc::new(value))
    }
}

impl<T: FromDynamic> FromDynamic for Option<T> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Null => Ok(None),
            value => Ok(Some(T::from_dynamic(value, options)?)),
        }
    }
}

impl<T: FromDynamic, const N: usize> FromDynamic for [T; N] {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Array(arr) => {
                let v = arr
                    .iter()
                    .map(|v| T::from_dynamic(v, options))
                    .collect::<Result<Vec<T>, Error>>()?;
                v.try_into().map_err(|v: Vec<T>| Error::ArraySizeMismatch {
                    vec_size: v.len(),
                    array_size: N,
                })
            }
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "array",
            }),
        }
    }
}

impl<K: FromDynamic + Eq + Hash, T: FromDynamic> FromDynamic for HashMap<K, T> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Object(obj) => {
                let mut map = HashMap::with_capacity(obj.len());
                for (k, v) in obj.iter() {
                    map.insert(K::from_dynamic(k, options)?, T::from_dynamic(v, options)?);
                }
                Ok(map)
            }
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "HashMap",
            }),
        }
    }
}

impl<T: FromDynamic> FromDynamic for Vec<T> {
    fn from_dynamic(value: &Value, options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Array(arr) => Ok(arr
                .iter()
                .map(|v| T::from_dynamic(v, options))
                .collect::<Result<Vec<T>, Error>>()?),
            // lua uses tables for everything; we can end up here if we got an empty
            // table and treated it as an object. Allow that to stand-in for an empty
            // array instead.
            Value::Object(obj) if obj.is_empty() => Ok(vec![]),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "Vec",
            }),
        }
    }
}

impl FromDynamic for () {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Null => Ok(()),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "()",
            }),
        }
    }
}

impl FromDynamic for bool {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::Bool(b) => Ok(*b),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "bool",
            }),
        }
    }
}

impl FromDynamic for std::path::PathBuf {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::String(s) => Ok(s.into()),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "PathBuf",
            }),
        }
    }
}

impl FromDynamic for char {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::String(s) => {
                let mut iter = s.chars();
                let c = iter.next().ok_or_else(|| Error::CharFromWrongSizedString)?;
                if iter.next().is_some() {
                    Err(Error::CharFromWrongSizedString)
                } else {
                    Ok(c)
                }
            }
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "char",
            }),
        }
    }
}

impl FromDynamic for String {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::String(s) => Ok(s.to_string()),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "String",
            }),
        }
    }
}

macro_rules! int {
    ($($ty:ty),* $(,)?) => {
        $(
impl FromDynamic for $ty {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::I64(n) => match (*n).try_into() {
                Ok(n) => Ok(n),
                Err(err) => Err(Error::Message(err.to_string())),
            },
            Value::U64(n) => match (*n).try_into() {
                Ok(n) => Ok(n),
                Err(err) => Err(Error::Message(err.to_string())),
            },
            other => Err(Error::NoConversion{
                source_type:other.variant_name().to_string(),
                dest_type: stringify!($ty),
            })
        }
    }
}
        )*
    }
}

int!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl FromDynamic for f32 {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::F64(OrderedFloat(n)) => Ok((*n) as f32),
            Value::I64(n) => Ok((*n) as f32),
            Value::U64(n) => Ok((*n) as f32),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "f32",
            }),
        }
    }
}

impl FromDynamic for f64 {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, Error> {
        match value {
            Value::F64(OrderedFloat(n)) => Ok(*n),
            Value::I64(n) => Ok((*n) as f64),
            Value::U64(n) => Ok((*n) as f64),
            other => Err(Error::NoConversion {
                source_type: other.variant_name().to_string(),
                dest_type: "f64",
            }),
        }
    }
}
