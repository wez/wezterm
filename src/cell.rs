use color;
use std::mem;
use std::rc::Rc;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hyperlink {
    pub id: String,
    pub url: String,
}

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
        pub fn $getter(&self) -> bool {
            (self.attributes & (1 << $bitnum)) == (1 << $bitnum)
        }

        #[inline]
        pub fn $setter(&mut self, value: bool) {
            let attr_value = if value { 1 << $bitnum } else { 0 };
            self.attributes = (self.attributes & !(1 << $bitnum)) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        pub fn $getter(&self) -> u16 {
            (self.attributes >> $bitshift) & $bitmask
        }

        #[inline]
        pub fn $setter(&mut self, value: u16) {
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $enum:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        pub fn $getter(&self) -> $enum {
            unsafe { mem::transmute(((self.attributes >> $bitshift) & $bitmask) as u16)}
        }

        #[inline]
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

impl Default for CellAttributes {
    fn default() -> Self {
        Self {
            attributes: 0,
            foreground: color::ColorAttribute {
                ansi: color::ColorSpec::Foreground,
                full: None,
            },
            background: color::ColorAttribute {
                ansi: color::ColorSpec::Background,
                full: None,
            },
            hyperlink: None,
        }
    }
}

impl CellAttributes {
    bitfield!(intensity, set_intensity, Intensity, 0b11, 0);
    bitfield!(underline, set_underline, Underline, 0b11, 2);
    bitfield!(italic, set_italic, 4);
    bitfield!(blink, set_blink, 5);
    bitfield!(reverse, set_reverse, 6);
    bitfield!(strikethrough, set_strikethrough, 7);
    bitfield!(invisible, set_invisible, 8);

    /// Clone the attributes, but exclude fancy extras such
    /// as hyperlinks or future sprite things
    pub fn clone_sgr_only(&self) -> Self {
        Self {
            attributes: self.attributes,
            foreground: self.foreground,
            background: self.background,
            hyperlink: None,
        }
    }
}

/// Models the contents of a cell on the terminal display
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Cell {
    text: char,
    attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Self {
        Cell::new(' ', CellAttributes::default())
    }
}

impl Cell {
    /// De-fang the input character such that it has no special meaning
    /// to a terminal.  All control and movement characters are rewritten
    /// as a space.
    pub fn nerf_control_char(text: char) -> char {
        if text < 0x20 as char || text == 0x7f as char {
            ' '
        } else {
            text
        }
    }

    pub fn new(text: char, attrs: CellAttributes) -> Self {
        Self {
            text: Self::nerf_control_char(text),
            attrs,
        }
    }

    pub fn char(&self) -> char {
        self.text
    }

    pub fn attrs(&self) -> &CellAttributes {
        &self.attrs
    }
}

/// Models a change in the attributes of a cell in a stream of changes
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AttributeChange {
    Intensity(Intensity),
    Underline(Underline),
    Italic(bool),
    Blink(bool),
    Reverse(bool),
    StrikeThrough(bool),
    Invisible(bool),
    Foreground(color::ColorAttribute),
    Background(color::ColorAttribute),
    Hyperlink(Option<Rc<Hyperlink>>),
}
