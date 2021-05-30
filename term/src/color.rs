//! Colors for attributes

#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::result::Result;
pub use termwiz::color::{AnsiColor, ColorAttribute, RgbColor, RgbaTuple};

#[derive(Clone, PartialEq, Eq)]
pub struct Palette256(pub [RgbColor; 256]);

#[cfg(feature = "use_serde")]
impl Serialize for Palette256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

#[cfg(feature = "use_serde")]
impl<'de> Deserialize<'de> for Palette256 {
    fn deserialize<D>(deserializer: D) -> Result<Palette256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Vec::<RgbColor>::deserialize(deserializer)?;
        use std::convert::TryInto;
        Ok(Self(s.try_into().map_err(|_| {
            serde::de::Error::custom("Palette256 size mismatch")
        })?))
    }
}

impl std::iter::FromIterator<RgbColor> for Palette256 {
    fn from_iter<I: IntoIterator<Item = RgbColor>>(iter: I) -> Self {
        let mut colors = [RgbColor::default(); 256];
        for (s, d) in iter.into_iter().zip(colors.iter_mut()) {
            *d = s;
        }
        Self(colors)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct ColorPalette {
    pub colors: Palette256,
    pub foreground: RgbColor,
    pub background: RgbColor,
    pub cursor_fg: RgbColor,
    pub cursor_bg: RgbColor,
    pub cursor_border: RgbColor,
    pub selection_fg: RgbColor,
    pub selection_bg: RgbColor,
    pub scrollbar_thumb: RgbColor,
    pub split: RgbColor,
}

/// Adjust the color to make it appear disabled.
/// This is not defined on RgbColor itself in order
/// to avoid termwiz requiring a dep on the palette crate.
fn grey_out(color: RgbColor) -> RgbColor {
    use palette::{Lch, Saturate, Srgba};
    let color = Srgba::new(color.red, color.green, color.blue, 0xff);
    let color: Srgba = color.into_format();
    let color = color.into_linear();

    let mut desaturated = Lch::from(color).desaturate(0.2);
    desaturated.l *= 0.8;

    let result = Srgba::from_linear(desaturated.into());
    let result = Srgba::<u8>::from_format(result);

    RgbColor::new(result.red, result.green, result.blue)
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
    pub fn resolve_fg(&self, color: ColorAttribute) -> RgbColor {
        match color {
            ColorAttribute::Default => self.foreground,
            ColorAttribute::PaletteIndex(idx) => self.colors.0[idx as usize],
            ColorAttribute::TrueColorWithPaletteFallback(color, _)
            | ColorAttribute::TrueColorWithDefaultFallback(color) => color,
        }
    }
    pub fn resolve_bg(&self, color: ColorAttribute) -> RgbColor {
        match color {
            ColorAttribute::Default => self.background,
            ColorAttribute::PaletteIndex(idx) => self.colors.0[idx as usize],
            ColorAttribute::TrueColorWithPaletteFallback(color, _)
            | ColorAttribute::TrueColorWithDefaultFallback(color) => color,
        }
    }

    /// Returns a greyed out version of the whole palette
    pub fn grey_out(&self) -> Self {
        Self {
            colors: self.colors.0.iter().map(|&c| grey_out(c)).collect(),
            foreground: grey_out(self.foreground),
            background: grey_out(self.background),
            cursor_fg: grey_out(self.cursor_fg),
            cursor_bg: grey_out(self.cursor_bg),
            cursor_border: grey_out(self.cursor_border),
            selection_fg: grey_out(self.selection_fg),
            selection_bg: grey_out(self.selection_bg),
            scrollbar_thumb: grey_out(self.scrollbar_thumb),
            split: grey_out(self.split),
        }
    }
}

lazy_static::lazy_static! {
    static ref DEFAULT_PALETTE: ColorPalette = ColorPalette::compute_default();
}

impl Default for ColorPalette {
    /// Construct a default color palette
    fn default() -> ColorPalette {
        DEFAULT_PALETTE.clone()
    }
}

impl ColorPalette {
    fn compute_default() -> Self {
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

        // 216 color cube.
        // This isn't the perfect color cube, but it matches the values used
        // by xterm, which are slightly brighter.
        static RAMP6: [u8; 6] = [0, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
        for idx in 0..216 {
            let blue = RAMP6[idx % 6];
            let green = RAMP6[idx / 6 % 6];
            let red = RAMP6[idx / 6 / 6 % 6];

            colors[16 + idx] = RgbColor { red, green, blue };
        }

        // 24 grey scales
        static GREYS: [u8; 24] = [
            0x08, 0x12, 0x1c, 0x26, 0x30, 0x3a, 0x44, 0x4e, 0x58, 0x62, 0x6c, 0x76, 0x80, 0x8a,
            0x94, 0x9e, 0xa8, 0xb2, /* Grey70 */
            0xbc, 0xc6, 0xd0, 0xda, 0xe4, 0xee,
        ];

        for idx in 0..24 {
            let grey = GREYS[idx];
            colors[232 + idx] = RgbColor::new(grey, grey, grey);
        }

        let foreground = colors[249]; // Grey70
        let background = colors[AnsiColor::Black as usize];

        let cursor_bg = RgbColor::new(0x52, 0xad, 0x70);
        let cursor_border = RgbColor::new(0x52, 0xad, 0x70);
        let cursor_fg = colors[AnsiColor::Black as usize];

        let selection_fg = colors[AnsiColor::Black as usize];
        let selection_bg = RgbColor::new(0xff, 0xfa, 0xcd);

        let scrollbar_thumb = RgbColor::new(0x22, 0x22, 0x22);
        let split = RgbColor::new(0x44, 0x44, 0x44);

        ColorPalette {
            colors: Palette256(colors),
            foreground,
            background,
            cursor_fg,
            cursor_bg,
            cursor_border,
            selection_fg,
            selection_bg,
            scrollbar_thumb,
            split,
        }
    }
}
