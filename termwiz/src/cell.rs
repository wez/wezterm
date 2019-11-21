//! Model a cell in the terminal display
use crate::color::ColorAttribute;
pub use crate::escape::osc::Hyperlink;
use crate::image::ImageCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_derive::*;
use smallvec::SmallVec;
use std;
use std::mem;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

/// Holds the attributes for a cell.
/// Most style attributes are stored internally as part of a bitfield
/// to reduce per-cell overhead.
/// The setter methods return a mutable self reference so that they can
/// be chained together.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellAttributes {
    attributes: u16,
    /// The foreground color
    pub foreground: ColorAttribute,
    /// The background color
    pub background: ColorAttribute,
    /// The hyperlink content, if any
    pub hyperlink: Option<Arc<Hyperlink>>,
    /// The image data, if any
    pub image: Option<Box<ImageCell>>,
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
        pub fn $setter(&mut self, value: bool) -> &mut Self {
            let attr_value = if value { 1 << $bitnum } else { 0 };
            self.attributes = (self.attributes & !(1 << $bitnum)) | attr_value;
            self
        }
    };

    ($getter:ident, $setter:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        pub fn $getter(&self) -> u16 {
            (self.attributes >> $bitshift) & $bitmask
        }

        #[inline]
        pub fn $setter(&mut self, value: u16) -> &mut Self {
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
            self
        }
    };

    ($getter:ident, $setter:ident, $enum:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        pub fn $getter(&self) -> $enum {
            unsafe { mem::transmute(((self.attributes >> $bitshift) & $bitmask) as u16)}
        }

        #[inline]
        pub fn $setter(&mut self, value: $enum) -> &mut Self {
            let value = value as u16;
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
            self
        }
    };
}

/// The `Intensity` of a cell describes its boldness.  Most terminals
/// implement `Intensity::Bold` by either using a bold font or by simply
/// using an alternative color.  Some terminals implement `Intensity::Half`
/// as a dimmer color variant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum Intensity {
    Normal = 0,
    Bold = 1,
    Half = 2,
}

impl Default for Intensity {
    fn default() -> Self {
        Self::Normal
    }
}

/// Specify just how underlined you want your `Cell` to be
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum Underline {
    /// The cell is not underlined
    None = 0,
    /// The cell is underlined with a single line
    Single = 1,
    /// The cell is underlined with two lines
    Double = 2,
}

impl Default for Underline {
    fn default() -> Self {
        Self::None
    }
}

/// Allow converting to boolean; true means some kind of
/// underline, false means none.  This is used in some
/// generic code to determine whether to enable underline.
impl Into<bool> for Underline {
    fn into(self) -> bool {
        self != Underline::None
    }
}

/// Specify whether you want to slowly or rapidly annoy your users
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum Blink {
    None = 0,
    Slow = 1,
    Rapid = 2,
}

/// Allow converting to boolean; true means some kind of
/// blink, false means none.  This is used in some
/// generic code to determine whether to enable blink.
impl Into<bool> for Blink {
    fn into(self) -> bool {
        self != Blink::None
    }
}

impl CellAttributes {
    bitfield!(intensity, set_intensity, Intensity, 0b11, 0);
    bitfield!(underline, set_underline, Underline, 0b11, 2);
    bitfield!(blink, set_blink, Blink, 0b11, 4);
    bitfield!(italic, set_italic, 6);
    bitfield!(reverse, set_reverse, 7);
    bitfield!(strikethrough, set_strikethrough, 8);
    bitfield!(invisible, set_invisible, 9);
    bitfield!(wrapped, set_wrapped, 10);

    /// Returns true if the attribute bits in both objects are equal.
    /// This can be used to cheaply test whether the styles of the two
    /// cells are the same, and is used by some `Renderer` implementations.
    pub fn attribute_bits_equal(&self, other: &Self) -> bool {
        self.attributes == other.attributes
    }

    /// Set the foreground color for the cell to that specified
    pub fn set_foreground<C: Into<ColorAttribute>>(&mut self, foreground: C) -> &mut Self {
        self.foreground = foreground.into();
        self
    }

    pub fn set_background<C: Into<ColorAttribute>>(&mut self, background: C) -> &mut Self {
        self.background = background.into();
        self
    }

    pub fn set_hyperlink(&mut self, link: Option<Arc<Hyperlink>>) -> &mut Self {
        self.hyperlink = link;
        self
    }

    pub fn set_image(&mut self, image: Option<Box<ImageCell>>) -> &mut Self {
        self.image = image;
        self
    }

    /// Clone the attributes, but exclude fancy extras such
    /// as hyperlinks or future sprite things
    pub fn clone_sgr_only(&self) -> Self {
        Self {
            attributes: self.attributes,
            foreground: self.foreground,
            background: self.background,
            hyperlink: None,
            image: None,
        }
    }
}

fn deserialize_smallvec<'de, D>(deserializer: D) -> Result<SmallVec<[u8; 4]>, D::Error>
where
    D: Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    Ok(SmallVec::from_slice(text.as_bytes()))
}

fn serialize_smallvec<S>(value: &SmallVec<[u8; 4]>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // unsafety: this is safe because the Cell constructor guarantees
    // that the storage is valid utf8
    let s = unsafe { std::str::from_utf8_unchecked(value) };
    s.serialize(serializer)
}

/// Models the contents of a cell on the terminal display
#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Cell {
    #[serde(
        deserialize_with = "deserialize_smallvec",
        serialize_with = "serialize_smallvec"
    )]
    text: SmallVec<[u8; 4]>,
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
    fn nerf_control_char(text: &mut SmallVec<[u8; 4]>) {
        if text.is_empty() {
            text.push(b' ');
            return;
        }

        if text.as_slice() == [b'\r', b'\n'] {
            text.remove(1);
            text[0] = b' ';
            return;
        }

        if text.len() != 1 {
            return;
        }

        if text[0] < 0x20 || text[0] == 0x7f {
            text[0] = b' ';
        }
    }

    /// Create a new cell holding the specified character and with the
    /// specified cell attributes.
    /// All control and movement characters are rewritten as a space.
    pub fn new(text: char, attrs: CellAttributes) -> Self {
        let len = text.len_utf8();
        let mut storage = SmallVec::with_capacity(len);
        unsafe {
            storage.set_len(len);
        }
        text.encode_utf8(&mut storage);
        Self::nerf_control_char(&mut storage);

        Self {
            text: storage,
            attrs,
        }
    }

    /// Create a new cell holding the specified grapheme.
    /// The grapheme is passed as a string slice and is intended to hold
    /// double-width characters, or combining unicode sequences, that need
    /// to be treated as a single logical "character" that can be cursored
    /// over.  This function technically allows for an arbitrary string to
    /// be passed but it should not be used to hold strings other than
    /// graphemes.
    pub fn new_grapheme(text: &str, attrs: CellAttributes) -> Self {
        let mut storage = SmallVec::from_slice(text.as_bytes());
        Self::nerf_control_char(&mut storage);

        Self {
            text: storage,
            attrs,
        }
    }

    /// Returns the textual content of the cell
    pub fn str(&self) -> &str {
        // unsafety: this is safe because the constructor guarantees
        // that the storage is valid utf8
        unsafe { std::str::from_utf8_unchecked(&self.text) }
    }

    /// Returns the number of cells visually occupied by this grapheme
    pub fn width(&self) -> usize {
        grapheme_column_width(self.str())
    }

    /// Returns the attributes of the cell
    pub fn attrs(&self) -> &CellAttributes {
        &self.attrs
    }

    pub fn attrs_mut(&mut self) -> &mut CellAttributes {
        &mut self.attrs
    }
}

/// Returns the number of cells visually occupied by a sequence
/// of graphemes
pub fn unicode_column_width(s: &str) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    s.graphemes(true).map(grapheme_column_width).sum()
}

/// Returns the number of cells visually occupied by a grapheme.
/// The input string must be a single grapheme.
pub fn grapheme_column_width(s: &str) -> usize {
    // Due to this issue:
    // https://github.com/unicode-rs/unicode-width/issues/4
    // we cannot simply use the unicode-width crate to compute
    // the desired value.
    // Let's check for emoji-ness for ourselves first
    use xi_unicode::EmojiExt;
    for c in s.chars() {
        if c.is_emoji_modifier_base() || c.is_emoji_modifier() {
            // treat modifier sequences as double wide
            return 2;
        }
    }
    UnicodeWidthStr::width(s)
}

/// Models a change in the attributes of a cell in a stream of changes.
/// Each variant specifies one of the possible attributes; the corresponding
/// value holds the new value to be used for that attribute.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AttributeChange {
    Intensity(Intensity),
    Underline(Underline),
    Italic(bool),
    Blink(Blink),
    Reverse(bool),
    StrikeThrough(bool),
    Invisible(bool),
    Foreground(ColorAttribute),
    Background(ColorAttribute),
    Hyperlink(Option<Arc<Hyperlink>>),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nerf_special() {
        for c in " \n\r\t".chars() {
            let cell = Cell::new(c, CellAttributes::default());
            assert_eq!(cell.str(), " ");
        }

        for g in &["", " ", "\n", "\r", "\t", "\r\n"] {
            let cell = Cell::new_grapheme(g, CellAttributes::default());
            assert_eq!(cell.str(), " ");
        }
    }

    #[test]
    fn test_width() {
        let foot = "\u{1f9b6}";
        eprintln!("foot chars");
        for c in foot.chars() {
            eprintln!("char: {:?}", c);
            use xi_unicode::EmojiExt;
            eprintln!("xi emoji: {}", c.is_emoji());
            eprintln!("xi emoji_mod: {}", c.is_emoji_modifier());
            eprintln!("xi emoji_mod_base: {}", c.is_emoji_modifier_base());
        }
        assert_eq!(unicode_column_width(foot), 2, "{} should be 2", foot);

        let women_holding_hands_dark_skin_tone_medium_light_skin_tone =
            "\u{1F469}\u{1F3FF}\u{200D}\u{1F91D}\u{200D}\u{1F469}\u{1F3FC}";

        // Ensure that we can hold this longer grapheme sequence in the cell
        // and correctly return its string contents!
        let cell = Cell::new_grapheme(
            women_holding_hands_dark_skin_tone_medium_light_skin_tone,
            CellAttributes::default(),
        );
        assert_eq!(
            cell.str(),
            women_holding_hands_dark_skin_tone_medium_light_skin_tone
        );
        assert_eq!(
            cell.width(),
            2,
            "width of {} should be 2",
            women_holding_hands_dark_skin_tone_medium_light_skin_tone
        );
    }
}
