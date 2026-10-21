//! Colors for attributes

// for FromPrimitive
#![allow(clippy::useless_attribute)]

#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
pub use wezterm_color_types::{LinearRgba, SrgbaTuple};
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use crate::alloc::string::ToString;
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub use wezterm_escape_parser::color::{AnsiColor, ColorSpec, PaletteIndex, RgbColor};

/// Specifies the color to be used when rendering a cell.  This is the
/// type used in the `CellAttributes` struct and can specify an optional
/// TrueColor value, allowing a fallback to a more traditional palette
/// index if TrueColor is not available.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Eq, PartialEq, FromDynamic, ToDynamic, Hash)]
pub enum ColorAttribute {
    /// Use RgbColor when supported, falling back to the specified PaletteIndex.
    TrueColorWithPaletteFallback(SrgbaTuple, PaletteIndex),
    /// Use RgbColor when supported, falling back to the default color
    TrueColorWithDefaultFallback(SrgbaTuple),
    /// Use the specified PaletteIndex
    PaletteIndex(PaletteIndex),
    /// Use the default color
    Default,
}

impl Default for ColorAttribute {
    fn default() -> Self {
        ColorAttribute::Default
    }
}

impl From<AnsiColor> for ColorAttribute {
    fn from(col: AnsiColor) -> Self {
        ColorAttribute::PaletteIndex(col as u8)
    }
}

impl From<ColorSpec> for ColorAttribute {
    fn from(spec: ColorSpec) -> Self {
        match spec {
            ColorSpec::Default => ColorAttribute::Default,
            ColorSpec::PaletteIndex(idx) => ColorAttribute::PaletteIndex(idx),
            ColorSpec::TrueColor(color) => ColorAttribute::TrueColorWithDefaultFallback(color),
        }
    }
}
