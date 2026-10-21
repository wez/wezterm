//! Types for representing Rust types in a more dynamic form
//! that is similar to JSON or Lua values.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod array;
mod drop;
mod error;
mod fromdynamic;
mod object;
mod todynamic;
mod value;

pub use array::Array;
pub use error::Error;
pub use fromdynamic::{FromDynamic, FromDynamicOptions, UnknownFieldAction};
pub use object::{BorrowedKey, Object, ObjectKeyTrait};
pub use todynamic::{PlaceDynamic, ToDynamic};
pub use value::Value;
pub use wezterm_dynamic_derive::{FromDynamic, ToDynamic};
