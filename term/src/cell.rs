use std::mem;
use std::rc::Rc;
use std::str;

use super::color;
use super::hyperlink::Hyperlink;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CellAttributes {
    attributes: u16,
    pub foreground: color::ColorAttribute,
    pub background: color::ColorAttribute,
    pub hyperlink: Option<Rc<Hyperlink>>,
}

/// Define getter and setter for the attributes bitfield.
/// The first form is for a simple boolean value stored in
/// a single bit.  The $bitnum parameter specifies which bit.
/// The second form is for an integer value that occupies a range
/// of bits.  The $bitmask and $bitshift parameters define how
/// to transform from the stored bit value to the consumable
/// value.
macro_rules! bitfield {
    ($getter:ident, $setter:ident, $bitnum:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> bool {
            (self.attributes & (1 << $bitnum)) == (1 << $bitnum)
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: bool) {
            let attr_value = if value { 1 << $bitnum } else { 0 };
            self.attributes = (self.attributes & !(1 << $bitnum)) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> u16 {
            (self.attributes >> $bitshift) & $bitmask
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: u16) {
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $enum:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> $enum {
            unsafe { mem::transmute(((self.attributes >> $bitshift) & $bitmask) as u16)}
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: $enum) {
            let value = value as u16;
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
        }
    };
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum Intensity {
    Normal = 0,
    Bold = 1,
    Half = 2,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum Underline {
    None = 0,
    Single = 1,
    Double = 2,
}

impl CellAttributes {
    bitfield!(intensity, set_intensity, Intensity, 0b11, 0);
    bitfield!(underline, set_underline, Underline, 0b1100, 2);
    bitfield!(italic, set_italic, 4);
    bitfield!(blink, set_blink, 5);
    bitfield!(reverse, set_reverse, 6);
    bitfield!(strikethrough, set_strikethrough, 7);
    bitfield!(invisible, set_invisible, 8);
    // Allow up to 8 different font values
    //bitfield!(font, set_font, 0b111000000, 6);
}

impl Default for CellAttributes {
    fn default() -> CellAttributes {
        CellAttributes {
            attributes: 0,
            foreground: color::ColorAttribute::Foreground,
            background: color::ColorAttribute::Background,
            hyperlink: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cell {
    bytes: [u8; 8],
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::from_char(' ', &CellAttributes::default())
    }
}

impl Cell {
    pub fn new(s: &str, attrs: &CellAttributes) -> Cell {
        let mut bytes = [0u8; 8];
        let len = s.len().min(8);
        bytes[0..len].copy_from_slice(s.as_bytes());
        Cell {
            bytes,
            attrs: attrs.clone(),
        }
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        if let Some(len) = self.bytes.iter().position(|&c| c == 0) {
            &self.bytes[0..len]
        } else {
            &self.bytes
        }
    }

    pub fn from_char(c: char, attr: &CellAttributes) -> Cell {
        let mut bytes = [0u8; 8];
        c.encode_utf8(&mut bytes);
        Cell {
            bytes,
            attrs: attr.clone(),
        }
    }

    pub fn width(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        str::from_utf8(self.bytes()).unwrap_or("").width()
    }
}

impl From<char> for Cell {
    fn from(c: char) -> Cell {
        Cell::from_char(c, &CellAttributes::default())
    }
}
