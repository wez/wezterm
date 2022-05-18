use crate::object::Object;
use crate::value::Value;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, HashMap};

/// The ToDynamic trait allows a type to emit a representation of itself
/// as the Value type.
/// This trait can be derived.
pub trait ToDynamic {
    fn to_dynamic(&self) -> Value;
}

/// The PlaceDynamic trait is used by derived implementations of FromDynamic
/// to implement flattened conversions.
/// Deriving FromDynamic for a struct will usually also derive
/// PlaceDynamic for the same struct.
/// You do not typically consume PlaceDynamic directly.
pub trait PlaceDynamic {
    /// Convert from Self to Value, by storing directly into the
    /// target Object.
    fn place_dynamic(&self, place: &mut Object);
}

impl ToDynamic for Value {
    fn to_dynamic(&self) -> Value {
        self.clone()
    }
}

impl ToDynamic for ordered_float::NotNan<f64> {
    fn to_dynamic(&self) -> Value {
        Value::F64(OrderedFloat::from(**self))
    }
}

impl ToDynamic for std::time::Duration {
    fn to_dynamic(&self) -> Value {
        Value::F64(OrderedFloat(self.as_secs_f64()))
    }
}

impl<K: ToDynamic + ToString + 'static, T: ToDynamic> ToDynamic for HashMap<K, T> {
    fn to_dynamic(&self) -> Value {
        Value::Object(
            self.iter()
                .map(|(k, v)| (k.to_dynamic(), v.to_dynamic()))
                .collect::<BTreeMap<_, _>>()
                .into(),
        )
    }
}

impl<T: ToDynamic> ToDynamic for std::sync::Arc<T> {
    fn to_dynamic(&self) -> Value {
        self.as_ref().to_dynamic()
    }
}

impl<T: ToDynamic> ToDynamic for Box<T> {
    fn to_dynamic(&self) -> Value {
        self.as_ref().to_dynamic()
    }
}

impl<T: ToDynamic> ToDynamic for Option<T> {
    fn to_dynamic(&self) -> Value {
        match self {
            None => Value::Null,
            Some(t) => t.to_dynamic(),
        }
    }
}

impl<T: ToDynamic, const N: usize> ToDynamic for [T; N] {
    fn to_dynamic(&self) -> Value {
        Value::Array(
            self.iter()
                .map(T::to_dynamic)
                .collect::<Vec<Value>>()
                .into(),
        )
    }
}

impl<T: ToDynamic> ToDynamic for Vec<T> {
    fn to_dynamic(&self) -> Value {
        Value::Array(
            self.iter()
                .map(T::to_dynamic)
                .collect::<Vec<Value>>()
                .into(),
        )
    }
}

impl ToDynamic for () {
    fn to_dynamic(&self) -> Value {
        Value::Null
    }
}

impl ToDynamic for bool {
    fn to_dynamic(&self) -> Value {
        Value::Bool(*self)
    }
}

impl ToDynamic for str {
    fn to_dynamic(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl ToDynamic for std::path::PathBuf {
    fn to_dynamic(&self) -> Value {
        Value::String(self.to_string_lossy().to_string())
    }
}

impl ToDynamic for String {
    fn to_dynamic(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl ToDynamic for char {
    fn to_dynamic(&self) -> Value {
        Value::String(self.to_string())
    }
}

impl ToDynamic for isize {
    fn to_dynamic(&self) -> Value {
        Value::I64((*self).try_into().unwrap())
    }
}

impl ToDynamic for i8 {
    fn to_dynamic(&self) -> Value {
        Value::I64((*self).into())
    }
}

impl ToDynamic for i16 {
    fn to_dynamic(&self) -> Value {
        Value::I64((*self).into())
    }
}

impl ToDynamic for i32 {
    fn to_dynamic(&self) -> Value {
        Value::I64((*self).into())
    }
}

impl ToDynamic for i64 {
    fn to_dynamic(&self) -> Value {
        Value::I64(*self)
    }
}

impl ToDynamic for usize {
    fn to_dynamic(&self) -> Value {
        Value::U64((*self).try_into().unwrap())
    }
}

impl ToDynamic for u8 {
    fn to_dynamic(&self) -> Value {
        Value::U64((*self).into())
    }
}

impl ToDynamic for u16 {
    fn to_dynamic(&self) -> Value {
        Value::U64((*self).into())
    }
}

impl ToDynamic for u32 {
    fn to_dynamic(&self) -> Value {
        Value::U64((*self).into())
    }
}

impl ToDynamic for u64 {
    fn to_dynamic(&self) -> Value {
        Value::U64(*self)
    }
}

impl ToDynamic for f64 {
    fn to_dynamic(&self) -> Value {
        Value::F64(OrderedFloat(*self))
    }
}

impl ToDynamic for f32 {
    fn to_dynamic(&self) -> Value {
        Value::F64(OrderedFloat((*self).into()))
    }
}
