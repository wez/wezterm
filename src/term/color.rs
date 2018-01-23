//! Colors for attributes

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

#[derive(Debug, Clone, Copy, Default)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {}

#[derive(Debug, Clone)]
pub enum ColorAttribute {
    Foreground,
    Background,
    PaletteIndex(u8),
    Rgb(RgbColor),
}

#[derive(Clone)]
pub struct ColorPalette {
    colors: [RgbColor; 256],
    foreground: RgbColor,
    background: RgbColor,
}

impl ColorPalette {
    pub fn resolve(&self, color: &ColorAttribute) -> RgbColor {
        match color {
            &ColorAttribute::Foreground => self.foreground,
            &ColorAttribute::Background => self.background,
            &ColorAttribute::PaletteIndex(idx) => self.colors[idx as usize],
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
            RgbColor{ red: 0x00, green: 0x00, blue: 0x00}, // Black
            RgbColor{ red: 0x80, green: 0x00, blue: 0x00}, // Maroon
            RgbColor{ red: 0x00, green: 0x80, blue: 0x00}, // Green
            RgbColor{ red: 0x80, green: 0x80, blue: 0x00}, // Olive
            RgbColor{ red: 0x00, green: 0x00, blue: 0x80}, // Navy
            RgbColor{ red: 0x80, green: 0x00, blue: 0x80}, // Purple
            RgbColor{ red: 0x00, green: 0x80, blue: 0x80}, // Teal
            RgbColor{ red: 0xc0, green: 0xc0, blue: 0xc0}, // Silver
            RgbColor{ red: 0x80, green: 0x80, blue: 0x80}, // Grey
            RgbColor{ red: 0xff, green: 0x00, blue: 0x00}, // Red
            RgbColor{ red: 0x00, green: 0xff, blue: 0x00}, // Lime
            RgbColor{ red: 0xff, green: 0xff, blue: 0x00}, // Yellow
            RgbColor{ red: 0x00, green: 0x00, blue: 0xff}, // Blue
            RgbColor{ red: 0xff, green: 0x00, blue: 0xff}, // Fuschia
            RgbColor{ red: 0x00, green: 0xff, blue: 0xff}, // Aqua
            RgbColor{ red: 0xff, green: 0xff, blue: 0xff}, // White
            ];

        colors[0..16].copy_from_slice(&ANSI);

        // 216 color cube
        static RAMP6: [u8; 6] = [0x00, 0x33, 0x66, 0x99, 0xCC, 0xFF];
        for idx in 0..216 {
            let red = RAMP6[idx % 6];
            let green = RAMP6[idx / 6 % 6];
            let blue = RAMP6[idx / 6 / 6 % 6];

            colors[16 + idx] = RgbColor { red, green, blue };
        }

        // 24 grey scales
        static GREYS: [u8; 24] = [
            0x08,
            0x12,
            0x1c,
            0x26,
            0x30,
            0x3a,
            0x44,
            0x4e,
            0x58,
            0x62,
            0x6c,
            0x76,
            0x80,
            0x8a,
            0x94,
            0x9e,
            0xa8,
            0xb2, // Grey70
            0xbc,
            0xc6,
            0xd0,
            0xda,
            0xe4,
            0xee,
        ];

        for idx in 0..24 {
            let grey = GREYS[idx];
            colors[232 + idx] = RgbColor {
                red: grey,
                green: grey,
                blue: grey,
            };
        }

        let foreground = colors[249]; // Grey70
        let background = colors[AnsiColor::Black as usize];

        ColorPalette {
            colors,
            foreground,
            background,
        }
    }
}
