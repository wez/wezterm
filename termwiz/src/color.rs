//! Colors for attributes
// for FromPrimitive
#![cfg_attr(feature = "cargo-clippy", allow(clippy::useless_attribute))]

use num_derive::*;
use serde::{self, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
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

lazy_static::lazy_static! {
    static ref NAMED_COLORS: HashMap<String, RgbColor> = build_colors();
}

fn build_colors() -> HashMap<String, RgbColor> {
    let mut map = HashMap::new();
    let rgb_txt = include_str!("rgb.txt");
    for line in rgb_txt.lines() {
        let mut fields = line.split_ascii_whitespace();
        let red = fields.next().unwrap();
        let green = fields.next().unwrap();
        let blue = fields.next().unwrap();
        let name = fields.collect::<Vec<&str>>().join(" ");

        let name = name.to_ascii_lowercase();
        map.insert(
            name,
            RgbColor::new(
                red.parse().unwrap(),
                green.parse().unwrap(),
                blue.parse().unwrap(),
            ),
        );
    }

    map
}

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

    pub fn to_tuple_rgba(self) -> RgbaTuple {
        (
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
            1.0,
        )
    }

    pub fn to_linear_tuple_rgba(self) -> RgbaTuple {
        // See https://docs.rs/palette/0.5.0/src/palette/encoding/srgb.rs.html#43
        fn to_linear(v: u8) -> f32 {
            let v = v as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        (
            to_linear(self.red),
            to_linear(self.green),
            to_linear(self.blue),
            1.0,
        )
    }

    /// Construct a color from an X11/SVG/CSS3 color name.
    /// Returns None if the supplied name is not recognized.
    /// The list of names can be found here:
    /// <https://en.wikipedia.org/wiki/X11_color_names>
    pub fn from_named(name: &str) -> Option<RgbColor> {
        NAMED_COLORS.get(&name.to_ascii_lowercase()).cloned()
    }

    /// Returns a string of the form `#RRGGBB`
    pub fn to_rgb_string(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
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
        } else if s.starts_with("rgb:") && s.len() == 12 {
            let mut chars = s.chars().skip(4);

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
            macro_rules! slash {
                () => {{
                    match chars.next() {
                        Some('/') => {}
                        _ => return None,
                    }
                }};
            }
            let red = digit!();
            slash!();
            let green = digit!();
            slash!();
            let blue = digit!();

            Some(Self::new(red, green, blue))
        } else {
            None
        }
    }

    /// Construct a color from an SVG/CSS3 color name.
    /// or from a string of the form `#RRGGBB` where
    /// R, G and B are all hex digits.
    /// Returns None if the supplied name is not recognized.
    /// The list of names can be found here:
    /// <https://ogeon.github.io/docs/palette/master/palette/named/index.html>
    pub fn from_named_or_rgb_string(s: &str) -> Option<Self> {
        RgbColor::from_rgb_str(&s).or_else(|| RgbColor::from_named(&s))
    }
}

/// This is mildly unfortunate: in order to round trip RgbColor with serde
/// we need to provide a Serialize impl equivalent to the Deserialize impl
/// below.  We use the impl below to allow more flexible specification of
/// color strings in the config file.  A side effect of doing it this way
/// is that we have to serialize RgbColor as a 7-byte string when we could
/// otherwise serialize it as a 3-byte array.  There's probably a way
/// to make this work more efficiently, but for now this will do.
impl Serialize for RgbColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.to_rgb_string();
        s.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D>(deserializer: D) -> Result<RgbColor, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RgbColor::from_named_or_rgb_string(&s)
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
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
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

    #[test]
    fn from_rgb() {
        assert!(RgbColor::from_rgb_str("#xyxyxy").is_none());

        let black = RgbColor::from_rgb_str("#000000").unwrap();
        assert_eq!(black.red, 0);
        assert_eq!(black.green, 0);
        assert_eq!(black.blue, 0);

        let grey = RgbColor::from_rgb_str("rgb:D6/D6/D6").unwrap();
        assert_eq!(grey.red, 0xd6);
        assert_eq!(grey.green, 0xd6);
        assert_eq!(grey.blue, 0xd6);
    }

    #[test]
    fn roundtrip_rgbcolor() {
        let data = varbincode::serialize(&RgbColor::from_named("DarkGreen").unwrap()).unwrap();
        eprintln!("serialized as {:?}", data);
        let _decoded: RgbColor = varbincode::deserialize(data.as_slice()).unwrap();
    }
}
