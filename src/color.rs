//! Colors for attributes

use palette;
use palette::Srgb;
use serde::{self, Deserialize, Deserializer};
use std::result::Result;

#[derive(Debug, Clone, Copy, FromPrimitive)]
#[repr(u8)]
/// These correspond to the classic ANSI color indices and are
/// used for convenience/readability in code
pub enum AnsiColor {
    Black = 0,
    Maroon,
    Green,
    Olive,
    Navy,
    Purple,
    Teal,
    Silver,
    Grey,
    Red,
    Lime,
    Yellow,
    Blue,
    Fuschia,
    Aqua,
    White,
}

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

    /// Construct a color from an SVG/CSS3 color name.
    /// Returns None if the supplied name is not recognized.
    /// The list of names can be found here:
    /// https://ogeon.github.io/docs/palette/master/palette/named/index.html
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorSpec {
    Default,
    /// Use either a raw number, or use values from the `AnsiColor` enum
    PaletteIndex(u8),
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

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct ColorAttribute {
    /// Used if the terminal supports full color
    pub full: Option<RgbColor>,
    /// If the terminal doesn't support full color, or the full color
    /// spec is_none, use old school ansi color number.
    pub ansi: ColorSpec,
}

impl From<AnsiColor> for ColorAttribute {
    fn from(col: AnsiColor) -> Self {
        Self {
            full: None,
            ansi: ColorSpec::PaletteIndex(col as u8),
        }
    }
}

impl From<ColorSpec> for ColorAttribute {
    fn from(spec: ColorSpec) -> Self {
        Self {
            full: None,
            ansi: spec,
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
