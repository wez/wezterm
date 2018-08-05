//! Colors for attributes

use palette;
use palette::{LinSrgba, Srgb, Srgba};
use serde::{self, Deserialize, Deserializer};
use std::result::Result;

#[derive(Debug, Clone, Copy, FromPrimitive)]
#[repr(u8)]
/// These correspond to the classic ANSI color indices and are
/// used for convenience/readability in code
pub enum AnsiColor {
    /// "Dark" black
    Black = 0,
    /// Dark red
    Maroon,
    /// Dark green
    Green,
    /// "Dark" yellow
    Olive,
    /// Dark blue
    Navy,
    /// Dark purple
    Purple,
    /// "Dark" cyan
    Teal,
    /// "Dark" white
    Silver,
    /// "Bright" black
    Grey,
    /// Bright red
    Red,
    /// Bright green
    Lime,
    /// Bright yellow
    Yellow,
    /// Bright blue
    Blue,
    /// Bright purple
    Fuschia,
    /// Bright Cyan/Aqua
    Aqua,
    /// Bright white
    White,
}

impl From<AnsiColor> for u8 {
    fn from(col: AnsiColor) -> u8 {
        col as u8
    }
}

pub type RgbaTuple = (f32, f32, f32, f32);

/// Describes a color in the SRGB colorspace using red, green and blue
/// components in the range 0-255.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {
    /// Construct a color from discrete red, green, blue values
    /// in the range 0-255.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_linear(&self) -> LinSrgba {
        Srgba::<u8>::new(self.red, self.green, self.blue, 0xff)
            .into_format()
            .into_linear()
    }

    pub fn to_tuple_rgba(&self) -> RgbaTuple {
        Srgba::<u8>::new(self.red, self.green, self.blue, 0xff)
            .into_format()
            .into_components()
    }

    pub fn to_linear_tuple_rgba(&self) -> RgbaTuple {
        self.to_linear().into_components()
    }

    /// Construct a color from an SVG/CSS3 color name.
    /// Returns None if the supplied name is not recognized.
    /// The list of names can be found here:
    /// <https://ogeon.github.io/docs/palette/master/palette/named/index.html>
    pub fn from_named(name: &str) -> Option<RgbColor> {
        palette::named::from_str(&name.to_ascii_lowercase()).map(|color| {
            let color = Srgb::<u8>::from_format(color);
            Self::new(color.red, color.green, color.blue)
        })
    }

    /// Construct a color from a string of the form `#RRGGBB` where
    /// R, G and B are all hex digits.
    pub fn from_rgb_str(s: &str) -> Option<RgbColor> {
        if s.as_bytes()[0] == b'#' && s.len() == 7 {
            let mut chars = s.chars().skip(1);

            macro_rules! digit {
                () => {{
                    let hi = match chars.next().unwrap().to_digit(16) {
                        Some(v) => (v as u8) << 4,
                        None => return None,
                    };
                    let lo = match chars.next().unwrap().to_digit(16) {
                        Some(v) => v as u8,
                        None => return None,
                    };
                    hi | lo
                }};
            }
            Some(Self::new(digit!(), digit!(), digit!()))
        } else {
            None
        }
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D>(deserializer: D) -> Result<RgbColor, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RgbColor::from_rgb_str(&s)
            .or_else(|| RgbColor::from_named(&s))
            .ok_or_else(|| format!("unknown color name: {}", s))
            .map_err(serde::de::Error::custom)
    }
}

/// An index into the fixed color palette.
pub type PaletteIndex = u8;

/// Specifies the color to be used when rendering a cell.
/// This differs from `ColorAttribute` in that this type can only
/// specify one of the possible color types at once, whereas the
/// `ColorAttribute` type can specify a TrueColor value and a fallback.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorSpec {
    Default,
    /// Use either a raw number, or use values from the `AnsiColor` enum
    PaletteIndex(PaletteIndex),
    TrueColor(RgbColor),
}

impl Default for ColorSpec {
    fn default() -> Self {
        ColorSpec::Default
    }
}

impl From<AnsiColor> for ColorSpec {
    fn from(col: AnsiColor) -> Self {
        ColorSpec::PaletteIndex(col as u8)
    }
}

impl From<RgbColor> for ColorSpec {
    fn from(col: RgbColor) -> Self {
        ColorSpec::TrueColor(col)
    }
}

/// Specifies the color to be used when rendering a cell.  This is the
/// type used in the `CellAttributes` struct and can specify an optional
/// TrueColor value, allowing a fallback to a more traditional palette
/// index if TrueColor is not available.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorAttribute {
    /// Use RgbColor when supported, falling back to the specified PaletteIndex.
    TrueColorWithPaletteFallback(RgbColor, PaletteIndex),
    /// Use RgbColor when supported, falling back to the default color
    TrueColorWithDefaultFallback(RgbColor),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_rgb() {
        let dark_green = RgbColor::from_named("DarkGreen").unwrap();
        assert_eq!(dark_green.red, 0);
        assert_eq!(dark_green.green, 0x64);
        assert_eq!(dark_green.blue, 0);
    }
}
