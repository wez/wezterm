//! Colors for attributes

use palette;
use serde::{self, Deserialize, Deserializer};
use std::fmt;
use std::result::Result;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
/// These correspond to the classic ANSI color indices and are
/// used for convenience/readability here in the code
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

pub type RgbTuple = (f32, f32, f32);
pub type RgbaTuple = (f32, f32, f32, f32);

impl RgbColor {
    /// Construct a color from discrete red, green, blue values
    /// in the range 0-255.
    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn to_linear(&self) -> palette::Rgba {
        palette::Rgba::new_u8(self.red, self.green, self.blue, 0xff)
    }

    pub fn to_linear_tuple_rgb(&self) -> RgbTuple {
        self.to_linear().to_pixel()
    }

    pub fn to_linear_tuple_rgba(&self) -> RgbaTuple {
        self.to_linear().to_pixel()
    }

    /// Construct a color from an SVG/CSS3 color name.  The name
    /// must be lower case.  Returns None if the supplied name is
    /// not recognized.
    /// The list of names can be found here:
    /// https://ogeon.github.io/docs/palette/master/palette/named/index.html
    pub fn from_named(name: &str) -> Option<RgbColor> {
        palette::named::from_str(name).map(|(r, g, b)| Self::new(r, g, b))
    }

    /// Construct a color from a string of the form `#RRGGBB` where
    /// R, G and B are all hex digits.
    pub fn from_rgb_str(s: &str) -> Option<RgbColor> {
        if s.as_bytes()[0] == b'#' && s.len() == 7 {
            let mut chars = s.chars().skip(1);

            macro_rules! digit {
                () => {
                    {
                        let hi = match chars.next().unwrap().to_digit(16) {
                            Some(v) => (v as u8) << 4,
                            None => return None
                        };
                        let lo = match chars.next().unwrap().to_digit(16) {
                            Some(v) => v as u8,
                            None => return None
                        };
                        hi | lo
                    }
                }
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
            .ok_or(format!("unknown color name: {}", s))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorAttribute {
    Foreground,
    Background,
    PaletteIndex(u8),
    Rgb(RgbColor),
}

#[derive(Clone)]
pub struct Palette256(pub [RgbColor; 256]);

#[derive(Clone, Debug)]
pub struct ColorPalette {
    pub colors: Palette256,
    pub foreground: RgbColor,
    pub background: RgbColor,
    pub cursor_fg: RgbColor,
    pub cursor_bg: RgbColor,
    pub selection_fg: RgbColor,
    pub selection_bg: RgbColor,
}

impl fmt::Debug for Palette256 {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // If we wanted to dump all of the entries, we'd use this:
        // self.0[..].fmt(fmt)
        // However, we typically don't care about those and we're interested
        // in the Debug-ability of ColorPalette that embeds us.
        write!(fmt, "[suppressed]")
    }
}

impl ColorPalette {
    pub fn resolve(&self, color: &ColorAttribute) -> RgbColor {
        match color {
            &ColorAttribute::Foreground => self.foreground,
            &ColorAttribute::Background => self.background,
            &ColorAttribute::PaletteIndex(idx) => self.colors.0[idx as usize],
            &ColorAttribute::Rgb(color) => color,
        }
    }
}

impl Default for ColorPalette {
    /// Construct a default color palette
    fn default() -> ColorPalette {
        let mut colors = [RgbColor::default(); 256];

        // The XTerm ansi color set
        static ANSI: [RgbColor; 16] = [
            // Black
            RgbColor {
                red: 0x00,
                green: 0x00,
                blue: 0x00,
            },
            // Maroon
            RgbColor {
                red: 0xcc,
                green: 0x55,
                blue: 0x55,
            },
            // Green
            RgbColor {
                red: 0x55,
                green: 0xcc,
                blue: 0x55,
            },
            // Olive
            RgbColor {
                red: 0xcd,
                green: 0xcd,
                blue: 0x55,
            },
            // Navy
            RgbColor {
                red: 0x54,
                green: 0x55,
                blue: 0xcb,
            },
            // Purple
            RgbColor {
                red: 0xcc,
                green: 0x55,
                blue: 0xcc,
            },
            // Teal
            RgbColor {
                red: 0x7a,
                green: 0xca,
                blue: 0xca,
            },
            // Silver
            RgbColor {
                red: 0xcc,
                green: 0xcc,
                blue: 0xcc,
            },
            // Grey
            RgbColor {
                red: 0x55,
                green: 0x55,
                blue: 0x55,
            },
            // Red
            RgbColor {
                red: 0xff,
                green: 0x55,
                blue: 0x55,
            },
            // Lime
            RgbColor {
                red: 0x55,
                green: 0xff,
                blue: 0x55,
            },
            // Yellow
            RgbColor {
                red: 0xff,
                green: 0xff,
                blue: 0x55,
            },
            // Blue
            RgbColor {
                red: 0x55,
                green: 0x55,
                blue: 0xff,
            },
            // Fuschia
            RgbColor {
                red: 0xff,
                green: 0x55,
                blue: 0xff,
            },
            // Aqua
            RgbColor {
                red: 0x55,
                green: 0xff,
                blue: 0xff,
            },
            // White
            RgbColor {
                red: 0xff,
                green: 0xff,
                blue: 0xff,
            },
        ];

        colors[0..16].copy_from_slice(&ANSI);

        // 216 color cube
        static RAMP6: [u8; 6] = [0x00, 0x33, 0x66, 0x99, 0xCC, 0xFF];
        for idx in 0..216 {
            let blue = RAMP6[idx % 6];
            let green = RAMP6[idx / 6 % 6];
            let red = RAMP6[idx / 6 / 6 % 6];

            colors[16 + idx] = RgbColor { red, green, blue };
        }

        // 24 grey scales
        static GREYS: [u8; 24] = [
            0x08, 0x12, 0x1c, 0x26, 0x30, 0x3a, 0x44, 0x4e, 0x58, 0x62, 0x6c, 0x76, 0x80, 0x8a,
            0x94, 0x9e, 0xa8, 0xb2 /* Grey70 */, 0xbc, 0xc6, 0xd0, 0xda, 0xe4, 0xee,
        ];

        for idx in 0..24 {
            let grey = GREYS[idx];
            colors[232 + idx] = RgbColor::new(grey, grey, grey);
        }

        let foreground = colors[249]; // Grey70
        let background = colors[AnsiColor::Black as usize];

        let cursor_bg = RgbColor::new(0x52, 0xad, 0x70);
        let cursor_fg = colors[AnsiColor::Black as usize];

        let selection_fg = colors[AnsiColor::Black as usize];
        let selection_bg = RgbColor::new(0xff, 0xfa, 0xcd);

        ColorPalette {
            colors: Palette256(colors),
            foreground,
            background,
            cursor_fg,
            cursor_bg,
            selection_fg,
            selection_bg,
        }
    }
}
