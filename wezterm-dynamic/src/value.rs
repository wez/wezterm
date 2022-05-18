use crate::array::Array;
use crate::object::Object;
use ordered_float::OrderedFloat;

/// Represents values of various possible other types.
/// Value is intended to be convertible to the same set
/// of types as Lua and is a superset of the types possible
/// in TOML and JSON.
#[derive(Clone, Debug, PartialEq, Hash, Eq, Ord, PartialOrd)]
pub enum Value {
    Null,
    Bool(bool),
    String(String),
    Array(Array),
    Object(Object),
    U64(u64),
    I64(i64),
    F64(OrderedFloat<f64>),
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

impl Value {
    pub fn variant_name(&self) -> &str {
        match self {
            Self::Null => "Null",
            Self::Bool(_) => "Bool",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Object(_) => "Object",
            Self::U64(_) => "U64",
            Self::I64(_) => "I64",
            Self::F64(_) => "F64",
        }
    }
}
