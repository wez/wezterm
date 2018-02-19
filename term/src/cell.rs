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
    bitfield!(underline, set_underline, Underline, 0b11, 2);
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

#[cfg(test)]
mod test {
    use super::*;

    /// Make sure that the bit shifting stuff works correctly
    #[test]
    fn cell_attributes() {
        let mut attrs = CellAttributes::default();
        attrs.set_underline(Underline::None);
        assert_eq!(Underline::None, attrs.underline());
        attrs.set_underline(Underline::Single);
        assert_eq!(Underline::Single, attrs.underline());
    }
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cell {
    len: u8,
    bytes: [u8; 7],
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Cell {
        let bytes = [b' ', 0, 0, 0, 0, 0, 0];
        Cell {
            len: 1,
            bytes,
            attrs: CellAttributes::default()
        }
    }
}

impl Cell {
    pub fn new(s: &str, attrs: &CellAttributes) -> Cell {
        let mut bytes = [0u8; 7];
        let len = s.len().min(7);
        bytes[0..len].copy_from_slice(s.as_bytes());
        Cell {
            len: len as u8,
            bytes,
            attrs: attrs.clone(),
        }
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[0..self.len as usize]
    }

    pub fn from_char(c: char, attr: &CellAttributes) -> Cell {
        let mut bytes = [0u8; 7];
        let len = if c == 0 as char {
            0u8
        } else if c < 0x80 as char {
            bytes[0] = c as u8;
            1u8
        } else {
            c.encode_utf8(&mut bytes).len() as u8
        };
        Cell {
            len,
            bytes,
            attrs: attr.clone(),
        }
    }

    #[inline]
    pub fn str(&self) -> &str {
        str::from_utf8(self.bytes()).unwrap_or("?")
    }

    pub fn width(&self) -> usize {
        if self.len <= 1 {
            self.len as usize
        } else {
            use unicode_width::UnicodeWidthStr;
            self.str().width()
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.len = 1;
        self.bytes[0] = b' ';
        self.attrs = CellAttributes::default();
    }
}

impl From<char> for Cell {
    fn from(c: char) -> Cell {
        Cell::from_char(c, &CellAttributes::default())
    }
}
