//! Model a cell in the terminal display
use crate::color::{ColorAttribute, PaletteIndex};
pub use crate::escape::osc::Hyperlink;
use crate::image::ImageCell;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::mem;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SmallColor {
    Default,
    PaletteIndex(PaletteIndex),
}

impl Default for SmallColor {
    fn default() -> Self {
        Self::Default
    }
}

impl Into<ColorAttribute> for SmallColor {
    fn into(self) -> ColorAttribute {
        match self {
            Self::Default => ColorAttribute::Default,
            Self::PaletteIndex(idx) => ColorAttribute::PaletteIndex(idx),
        }
    }
}

/// Holds the attributes for a cell.
/// Most style attributes are stored internally as part of a bitfield
/// to reduce per-cell overhead.
/// The setter methods return a mutable self reference so that they can
/// be chained together.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Default, Clone, Eq, PartialEq)]
pub struct CellAttributes {
    attributes: u16,
    /// The foreground color
    foreground: SmallColor,
    /// The background color
    background: SmallColor,
    /// Relatively rarely used attributes spill over to a heap
    /// allocated struct in order to keep CellAttributes
    /// smaller in the common case.
    fat: Option<Box<FatAttributes>>,
}

impl std::fmt::Debug for CellAttributes {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("CellAttributes")
            .field("attributes", &self.attributes)
            .field("intensity", &self.intensity())
            .field("underline", &self.underline())
            .field("blink", &self.blink())
            .field("italic", &self.italic())
            .field("reverse", &self.reverse())
            .field("strikethrough", &self.strikethrough())
            .field("invisible", &self.invisible())
            .field("wrapped", &self.wrapped())
            .field("overline", &self.overline())
            .field("semantic_type", &self.semantic_type())
            .field("foreground", &self.foreground)
            .field("background", &self.background)
            .field("fat", &self.fat)
            .finish()
    }
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct FatAttributes {
    /// The hyperlink content, if any
    hyperlink: Option<Arc<Hyperlink>>,
    /// The image data, if any
    image: Vec<Box<ImageCell>>,
    /// The color of the underline.  If None, then
    /// the foreground color is to be used
    underline_color: ColorAttribute,
    foreground: ColorAttribute,
    background: ColorAttribute,
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
            unsafe { mem::transmute(((self.attributes >> $bitshift) & $bitmask) as u16) }
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

/// Describes the semantic "type" of the cell.
/// This categorizes cells into Output (from the actions the user is
/// taking; this is the default if left unspecified),
/// Input (that the user typed) and Prompt (effectively, "chrome" provided
/// by the shell or application that the user is interacting with.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum SemanticType {
    Output = 0,
    Input = 1,
    Prompt = 2,
}

impl Default for SemanticType {
    fn default() -> Self {
        Self::Output
    }
}

/// The `Intensity` of a cell describes its boldness.  Most terminals
/// implement `Intensity::Bold` by either using a bold font or by simply
/// using an alternative color.  Some terminals implement `Intensity::Half`
/// as a dimmer color variant.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Underline {
    /// The cell is not underlined
    None = 0,
    /// The cell is underlined with a single line
    Single = 1,
    /// The cell is underlined with two lines
    Double = 2,
    /// Curly underline
    Curly = 3,
    /// Dotted underline
    Dotted = 4,
    /// Dashed underline
    Dashed = 5,
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
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    bitfield!(underline, set_underline, Underline, 0b111, 2);
    bitfield!(blink, set_blink, Blink, 0b11, 5);
    bitfield!(italic, set_italic, 7);
    bitfield!(reverse, set_reverse, 8);
    bitfield!(strikethrough, set_strikethrough, 9);
    bitfield!(invisible, set_invisible, 10);
    bitfield!(wrapped, set_wrapped, 11);
    bitfield!(overline, set_overline, 12);
    bitfield!(semantic_type, set_semantic_type, SemanticType, 0b11, 13);

    /// Returns true if the attribute bits in both objects are equal.
    /// This can be used to cheaply test whether the styles of the two
    /// cells are the same, and is used by some `Renderer` implementations.
    pub fn attribute_bits_equal(&self, other: &Self) -> bool {
        self.attributes == other.attributes
    }

    /// Set the foreground color for the cell to that specified
    pub fn set_foreground<C: Into<ColorAttribute>>(&mut self, foreground: C) -> &mut Self {
        let foreground: ColorAttribute = foreground.into();
        match foreground {
            ColorAttribute::Default => {
                self.foreground = SmallColor::Default;
                if let Some(fat) = self.fat.as_mut() {
                    fat.foreground = ColorAttribute::Default;
                }
                self.deallocate_fat_attributes_if_none();
            }
            ColorAttribute::PaletteIndex(idx) => {
                self.foreground = SmallColor::PaletteIndex(idx);
                if let Some(fat) = self.fat.as_mut() {
                    fat.foreground = ColorAttribute::Default;
                }
                self.deallocate_fat_attributes_if_none();
            }
            foreground => {
                self.foreground = SmallColor::Default;
                self.allocate_fat_attributes();
                self.fat.as_mut().unwrap().foreground = foreground;
            }
        }

        self
    }

    pub fn foreground(&self) -> ColorAttribute {
        if let Some(fat) = self.fat.as_ref() {
            if fat.foreground != ColorAttribute::Default {
                return fat.foreground;
            }
        }
        self.foreground.into()
    }

    pub fn set_background<C: Into<ColorAttribute>>(&mut self, background: C) -> &mut Self {
        let background: ColorAttribute = background.into();
        match background {
            ColorAttribute::Default => {
                self.background = SmallColor::Default;
                if let Some(fat) = self.fat.as_mut() {
                    fat.background = ColorAttribute::Default;
                }
                self.deallocate_fat_attributes_if_none();
            }
            ColorAttribute::PaletteIndex(idx) => {
                self.background = SmallColor::PaletteIndex(idx);
                if let Some(fat) = self.fat.as_mut() {
                    fat.background = ColorAttribute::Default;
                }
                self.deallocate_fat_attributes_if_none();
            }
            background => {
                self.background = SmallColor::Default;
                self.allocate_fat_attributes();
                self.fat.as_mut().unwrap().background = background;
            }
        }

        self
    }

    pub fn background(&self) -> ColorAttribute {
        if let Some(fat) = self.fat.as_ref() {
            if fat.background != ColorAttribute::Default {
                return fat.background;
            }
        }
        self.background.into()
    }

    fn allocate_fat_attributes(&mut self) {
        if self.fat.is_none() {
            self.fat.replace(Box::new(FatAttributes {
                hyperlink: None,
                image: vec![],
                underline_color: ColorAttribute::Default,
                foreground: ColorAttribute::Default,
                background: ColorAttribute::Default,
            }));
        }
    }

    fn deallocate_fat_attributes_if_none(&mut self) {
        let deallocate = self
            .fat
            .as_ref()
            .map(|fat| {
                fat.image.is_empty()
                    && fat.hyperlink.is_none()
                    && fat.underline_color == ColorAttribute::Default
                    && fat.foreground == ColorAttribute::Default
                    && fat.background == ColorAttribute::Default
            })
            .unwrap_or(false);
        if deallocate {
            self.fat.take();
        }
    }

    pub fn set_hyperlink(&mut self, link: Option<Arc<Hyperlink>>) -> &mut Self {
        if link.is_none() && self.fat.is_none() {
            self
        } else {
            self.allocate_fat_attributes();
            self.fat.as_mut().unwrap().hyperlink = link;
            self.deallocate_fat_attributes_if_none();
            self
        }
    }

    /// Assign a single image to a cell.
    pub fn set_image(&mut self, image: Box<ImageCell>) -> &mut Self {
        self.allocate_fat_attributes();
        self.fat.as_mut().unwrap().image = vec![image];
        self
    }

    /// Clear all images from a cell
    pub fn clear_images(&mut self) -> &mut Self {
        if let Some(fat) = self.fat.as_mut() {
            fat.image.clear();
        }
        self.deallocate_fat_attributes_if_none();
        self
    }

    /// Add an image attachement, preserving any existing attachments.
    /// The list of images is maintained in z-index order
    pub fn attach_image(&mut self, image: Box<ImageCell>) -> &mut Self {
        self.allocate_fat_attributes();
        let fat = self.fat.as_mut().unwrap();
        let z_index = image.z_index();
        match fat
            .image
            .binary_search_by(|probe| probe.z_index().cmp(&z_index))
        {
            Ok(idx) | Err(idx) => fat.image.insert(idx, image),
        }
        self
    }

    pub fn set_underline_color<C: Into<ColorAttribute>>(
        &mut self,
        underline_color: C,
    ) -> &mut Self {
        let underline_color = underline_color.into();
        if underline_color == ColorAttribute::Default && self.fat.is_none() {
            self
        } else {
            self.allocate_fat_attributes();
            self.fat.as_mut().unwrap().underline_color = underline_color;
            self.deallocate_fat_attributes_if_none();
            self
        }
    }

    /// Clone the attributes, but exclude fancy extras such
    /// as hyperlinks or future sprite things
    pub fn clone_sgr_only(&self) -> Self {
        let mut res = Self {
            attributes: self.attributes,
            foreground: self.foreground,
            background: self.background,
            fat: None,
        };
        if let Some(fat) = self.fat.as_ref() {
            if fat.background != ColorAttribute::Default
                || fat.foreground != ColorAttribute::Default
            {
                res.allocate_fat_attributes();
                let mut new_fat = res.fat.as_mut().unwrap();
                new_fat.foreground = fat.foreground;
                new_fat.background = fat.background;
            }
        }
        // Reset the semantic type; clone_sgr_only is used primarily
        // to create a "blank" cell when clearing and we want that to
        // be deterministically tagged as Output so that we have an
        // easier time in get_semantic_zones.
        res.set_semantic_type(SemanticType::default());
        res.set_underline_color(self.underline_color());
        res
    }

    pub fn hyperlink(&self) -> Option<&Arc<Hyperlink>> {
        self.fat.as_ref().and_then(|fat| fat.hyperlink.as_ref())
    }

    /// Returns the list of attached images in z-index order.
    /// Returns None if there are no attached images; will
    /// never return Some(vec![]).
    pub fn images(&self) -> Option<Vec<ImageCell>> {
        let fat = self.fat.as_ref()?;
        if fat.image.is_empty() {
            return None;
        }
        Some(fat.image.iter().map(|im| im.as_ref().clone()).collect())
    }

    pub fn underline_color(&self) -> ColorAttribute {
        self.fat
            .as_ref()
            .map(|fat| fat.underline_color)
            .unwrap_or(ColorAttribute::Default)
    }
}

#[cfg(feature = "use_serde")]
fn deserialize_teenystring<'de, D>(deserializer: D) -> Result<TeenyString, D::Error>
where
    D: Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    Ok(TeenyString::from_slice(text.as_bytes()))
}

#[cfg(feature = "use_serde")]
fn serialize_teenystring<S>(value: &TeenyString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // unsafety: this is safe because the Cell constructor guarantees
    // that the storage is valid utf8
    let s = unsafe { std::str::from_utf8_unchecked(value.as_bytes()) };
    s.serialize(serializer)
}

/// TeenyString encodes string storage in a single machine word.
/// The scheme is simple but effective: strings that encode into a
/// byte slice that is 1 less byte than the machine word size can
/// be encoded directly into the usize bits stored in the struct.
/// A marker bit (LSB for big endian, MSB for little endian) is
/// set to indicate that the string is stored inline.
/// If the string is longer than this then a `Vec<u8>` is allocated
/// from the heap and the usize holds its raw pointer address.
struct TeenyString(usize);
impl TeenyString {
    fn marker_mask() -> usize {
        if cfg!(target_endian = "little") {
            cfg_if::cfg_if! {
                if #[cfg(target_pointer_width = "64")] {
                    0x7f000000_00000000
                } else if #[cfg(target_pointer_width = "32")] {
                    0x7f000000
                } else if #[cfg(target_pointer_width = "16")] {
                    0x7f00
                } else {
                    panic!("unsupported target");
                }
            }
        } else {
            // I don't have a big endian machine to verify
            // this on, but I think this is right!
            0x1
        }
    }
    fn is_marker_bit_set(word: usize) -> bool {
        let mask = Self::marker_mask();
        word & mask == mask
    }

    fn set_marker_bit(word: usize) -> usize {
        word | Self::marker_mask()
    }

    pub fn from_slice(bytes: &[u8]) -> Self {
        // De-fang the input text such that it has no special meaning
        // to a terminal.  All control and movement characters are rewritten
        // as a space.
        let bytes = if bytes.is_empty() {
            b" "
        } else if bytes == b"\r\n" {
            b" "
        } else if bytes.len() == 1 && (bytes[0] < 0x20 || bytes[0] == 0x7f) {
            b" "
        } else {
            bytes
        };
        let len = bytes.len();
        if len < std::mem::size_of::<usize>() {
            let mut word = 0usize;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    &mut word as *mut usize as *mut u8,
                    len,
                );
            }
            let word = Self::set_marker_bit(word);
            Self(word)
        } else {
            let vec = Box::new(bytes.to_vec());
            let ptr = Box::into_raw(vec);
            Self(ptr as usize)
        }
    }

    pub fn from_char(c: char) -> Self {
        let mut bytes = [0u8; 8];
        let len = c.len_utf8();
        debug_assert!(len < std::mem::size_of_val(&bytes));
        c.encode_utf8(&mut bytes);
        Self::from_slice(&bytes[0..len])
    }

    pub fn as_bytes(&self) -> &[u8] {
        if Self::is_marker_bit_set(self.0) {
            let bytes = &self.0 as *const usize as *const u8;
            let bytes =
                unsafe { std::slice::from_raw_parts(bytes, std::mem::size_of::<usize>() - 1) };
            let len = bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(std::mem::size_of::<usize>() - 1);

            &bytes[0..len]
        } else {
            let vec = self.0 as *const usize as *const Vec<u8>;
            unsafe { (*vec).as_slice() }
        }
    }
}

impl Drop for TeenyString {
    fn drop(&mut self) {
        if !Self::is_marker_bit_set(self.0) {
            let vec = unsafe { Box::from_raw(self.0 as *mut usize as *mut Vec<u8>) };
            drop(vec);
        }
    }
}

impl std::clone::Clone for TeenyString {
    fn clone(&self) -> Self {
        Self::from_slice(self.as_bytes())
    }
}

impl std::cmp::PartialEq for TeenyString {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_bytes().eq(rhs.as_bytes())
    }
}
impl std::cmp::Eq for TeenyString {}

/// Models the contents of a cell on the terminal display
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Clone, Eq, PartialEq)]
pub struct Cell {
    #[cfg_attr(
        feature = "use_serde",
        serde(
            deserialize_with = "deserialize_teenystring",
            serialize_with = "serialize_teenystring"
        )
    )]
    text: TeenyString,
    attrs: CellAttributes,
}

impl std::fmt::Debug for Cell {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("Cell")
            .field("text", &self.str())
            .field("attrs", &self.attrs)
            .finish()
    }
}

impl Default for Cell {
    fn default() -> Self {
        Cell::new(' ', CellAttributes::default())
    }
}

impl Cell {
    /// Create a new cell holding the specified character and with the
    /// specified cell attributes.
    /// All control and movement characters are rewritten as a space.
    pub fn new(text: char, attrs: CellAttributes) -> Self {
        let storage = TeenyString::from_char(text);
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
        let storage = TeenyString::from_slice(text.as_bytes());

        Self {
            text: storage,
            attrs,
        }
    }

    /// Returns the textual content of the cell
    pub fn str(&self) -> &str {
        // unsafety: this is safe because the constructor guarantees
        // that the storage is valid utf8
        unsafe { std::str::from_utf8_unchecked(self.text.as_bytes()) }
    }

    /// Returns the number of cells visually occupied by this grapheme
    pub fn width(&self) -> usize {
        let s = self.str();
        if s.len() == 1 {
            1
        } else {
            grapheme_column_width(s)
        }
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
    let mut emoji = false;
    for c in s.chars() {
        if c.is_emoji_modifier_base() || c.is_emoji_modifier() {
            // treat modifier sequences as double wide
            return 2;
        }
        if c.is_emoji() {
            emoji = true;
        }
    }
    let width = UnicodeWidthStr::width(s);
    if emoji {
        // For sequences such as "deaf man", UnicodeWidthStr::width()
        // returns 3 because of the widths of the component glyphs,
        // rather than 2 for a single double width grapheme.
        // If we saw any emoji within the characters then we assume
        // that it can be a maximum of 2 cells in width.
        width.min(2)
    } else {
        width
    }
}

/// Models a change in the attributes of a cell in a stream of changes.
/// Each variant specifies one of the possible attributes; the corresponding
/// value holds the new value to be used for that attribute.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq)]
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
    fn teeny_string() {
        let s = TeenyString::from_char('a');
        assert_eq!(s.as_bytes(), &[b'a']);

        let longer = TeenyString::from_slice(b"hellothere");
        assert_eq!(longer.as_bytes(), b"hellothere");
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn memory_usage() {
        assert_eq!(std::mem::size_of::<crate::color::RgbColor>(), 4);
        assert_eq!(std::mem::size_of::<ColorAttribute>(), 8);
        assert_eq!(std::mem::size_of::<CellAttributes>(), 16);
        assert_eq!(std::mem::size_of::<Cell>(), 24);
        assert_eq!(std::mem::size_of::<Vec<u8>>(), 24);
        assert_eq!(std::mem::size_of::<char>(), 4);
        assert_eq!(std::mem::size_of::<TeenyString>(), 8);
    }

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

        let deaf_man = "\u{1F9CF}\u{200D}\u{2642}\u{FE0F}";
        eprintln!("deaf_man chars");
        for c in deaf_man.chars() {
            eprintln!("char: {:?}", c);
            use xi_unicode::EmojiExt;
            eprintln!("xi emoji: {}", c.is_emoji());
            eprintln!("xi emoji_mod: {}", c.is_emoji_modifier());
            eprintln!("xi emoji_mod_base: {}", c.is_emoji_modifier_base());
        }
        assert_eq!(unicode_column_width(deaf_man), 2);

        // This is a codepoint in the private use area
        let font_awesome_star = "\u{f005}";
        eprintln!("font_awesome_star {}", font_awesome_star.escape_debug());
        assert_eq!(unicode_column_width(font_awesome_star), 1);
    }
}
