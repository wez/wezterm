//! Freetype and fonts in general make use of fixed point types
//! to represent fractional numbers without using floating point
//! types.
//! Since those types are expressed in C as integers, it can be
//! easy to misue the values without scaling/adapting them appropriately.
//! This module adopts the fixed crate to manage that more robustly.
//!
//! In order to drop those types in to the bindgen-generated bindings
//! that comprise most of this crate, we separately run bindgen to
//! extract the underlying types and alias them in here as various
//! XXXStorage types.
//!
//! Now, since those types are based on things like `c_long` and
//! `c_short`, we don't necessarily know whether those are `i32` or `i64`
//! we need to use a helper trait to allow the compiler to resolve
//! the FixedXX variant that matches the storage size.
use crate::types::{
    FT_F26Dot6 as F26Dot6Storage, FT_F2Dot14 as F2Dot14Storage, FT_Fixed as FixedStorage,
    FT_Pos as PosStorage,
};
use fixed::types::extra::{U14, U16, U6};

/// Helper trait to resolve eg: `c_long` to the `fixed::FixedIXX`
/// type that occupies the same size
pub trait SelectFixedStorage<T> {
    type Storage;
}

impl<T> SelectFixedStorage<T> for i8 {
    type Storage = fixed::FixedI8<T>;
}
impl<T> SelectFixedStorage<T> for i16 {
    type Storage = fixed::FixedI16<T>;
}
impl<T> SelectFixedStorage<T> for i32 {
    type Storage = fixed::FixedI32<T>;
}
impl<T> SelectFixedStorage<T> for i64 {
    type Storage = fixed::FixedI64<T>;
}

pub type FT_F2Dot14 = <F2Dot14Storage as SelectFixedStorage<U14>>::Storage;
pub type FT_F26Dot6 = <F26Dot6Storage as SelectFixedStorage<U6>>::Storage;
pub type FT_Fixed = <FixedStorage as SelectFixedStorage<U16>>::Storage;

/// FT_Pos is used to store vectorial coordinates. Depending on the context, these can
/// represent distances in integer font units, or 16.16, or 26.6 fixed-point pixel coordinates.
#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct FT_Pos(PosStorage);

impl FT_Pos {
    /// Return the value expressed in font-units
    pub fn font_units(self) -> PosStorage {
        self.0
    }

    /// Construct a pos expressed in font-units
    pub fn from_font_units(v: PosStorage) -> Self {
        Self(v)
    }

    /// Extract the FT_Fixed/F16Dot16 equivalent value
    pub fn f16d16(self) -> <PosStorage as SelectFixedStorage<U16>>::Storage {
        <PosStorage as SelectFixedStorage<U16>>::Storage::from_bits(self.0)
    }

    /// Extract the F26Dot6 equivalent value
    pub fn f26d6(self) -> <PosStorage as SelectFixedStorage<U6>>::Storage {
        <PosStorage as SelectFixedStorage<U6>>::Storage::from_bits(self.0)
    }
}

impl From<FT_F26Dot6> for FT_Pos {
    fn from(src: FT_F26Dot6) -> FT_Pos {
        FT_Pos(src.to_bits())
    }
}

impl From<FT_Fixed> for FT_Pos {
    fn from(src: FT_Fixed) -> FT_Pos {
        FT_Pos(src.to_bits())
    }
}
