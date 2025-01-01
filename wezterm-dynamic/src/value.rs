use crate::array::Array;
use crate::object::Object;
use ordered_float::OrderedFloat;

/// Represents values of various possible other types.
/// Value is intended to be convertible to the same set
/// of types as Lua and is a superset of the types possible
/// in TOML and JSON.
#[derive(Clone, PartialEq, Hash, Eq, Ord, PartialOrd)]
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

impl std::fmt::Debug for Value {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::String(s) => fmt.write_fmt(format_args!("{:?}", s)),
            Self::Null => fmt.write_str("nil"),
            Self::Bool(i) => i.fmt(fmt),
            Self::I64(i) => i.fmt(fmt),
            Self::U64(i) => i.fmt(fmt),
            Self::F64(i) => i.fmt(fmt),
            Self::Array(a) => a.fmt(fmt),
            Self::Object(o) => o.fmt(fmt),
        }
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

    pub fn coerce_unsigned(&self) -> Option<u64> {
        match self {
            Self::U64(u) => Some(*u),
            Self::I64(i) => (*i).try_into().ok(),
            Self::F64(OrderedFloat(f))
                if f.fract() == 0.0 && *f >= u64::MIN as f64 && *f <= u64::MAX as f64 =>
            {
                Some(*f as u64)
            }
            _ => None,
        }
    }

    pub fn coerce_signed(&self) -> Option<i64> {
        match self {
            Self::I64(u) => Some(*u),
            Self::U64(i) => (*i).try_into().ok(),
            Self::F64(OrderedFloat(f))
                if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 =>
            {
                Some(*f as i64)
            }
            _ => None,
        }
    }

    pub fn coerce_float(&self) -> Option<f64> {
        match self {
            Self::I64(u) => Some(*u as f64),
            Self::U64(i) => Some(*i as f64),
            Self::F64(OrderedFloat(f)) => Some(*f),
            _ => None,
        }
    }
}
