use crate::*;
use bitflags::*;
use enum_display_derive::Display;
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use termwiz::color::RgbColor;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Display, PartialOrd, Ord,
)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl FontStretch {
    pub fn from_opentype_stretch(w: u16) -> Self {
        match w {
            1 => Self::UltraCondensed,
            2 => Self::ExtraCondensed,
            3 => Self::Condensed,
            4 => Self::SemiCondensed,
            5 => Self::Normal,
            6 => Self::SemiExpanded,
            7 => Self::Expanded,
            8 => Self::ExtraExpanded,
            9 => Self::UltraExpanded,
            _ if w < 1 => Self::UltraCondensed,
            _ => Self::UltraExpanded,
        }
    }

    pub fn to_opentype_stretch(self) -> u16 {
        match self {
            Self::UltraCondensed => 1,
            Self::ExtraCondensed => 2,
            Self::Condensed => 3,
            Self::SemiCondensed => 4,
            Self::Normal => 5,
            Self::SemiExpanded => 6,
            Self::Expanded => 7,
            Self::ExtraExpanded => 8,
            Self::UltraExpanded => 9,
        }
    }
}

impl Default for FontStretch {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontWeight(u16);

enum FontWeightOrLabel {
    Weight(u16),
    Label(&'static str),
}

impl FontWeight {
    fn categorize_weight(&self) -> FontWeightOrLabel {
        let label = if *self == Self::EXTRABLACK {
            "ExtraBlack"
        } else if *self == Self::BLACK {
            "Black"
        } else if *self == Self::EXTRABOLD {
            "ExtraBold"
        } else if *self == Self::BOLD {
            "Bold"
        } else if *self == Self::DEMIBOLD {
            "DemiBold"
        } else if *self == Self::MEDIUM {
            "Medium"
        } else if *self == Self::REGULAR {
            "Regular"
        } else if *self == Self::BOOK {
            "Book"
        } else if *self == Self::DEMILIGHT {
            "DemiLight"
        } else if *self == Self::LIGHT {
            "Light"
        } else if *self == Self::EXTRALIGHT {
            "ExtraLight"
        } else if *self == Self::THIN {
            "Thin"
        } else {
            return FontWeightOrLabel::Weight(self.0);
        };
        FontWeightOrLabel::Label(label)
    }

    fn from_str(s: &str) -> Option<FontWeight> {
        Some(match s {
            "ExtraBlack" => Self::EXTRABLACK,
            "Black" => Self::BLACK,
            "ExtraBold" => Self::EXTRABOLD,
            "Bold" => Self::BOLD,
            "DemiBold" => Self::DEMIBOLD,
            "Medium" => Self::MEDIUM,
            "Regular" => Self::REGULAR,
            "Book" => Self::BOOK,
            "DemiLight" => Self::DEMILIGHT,
            "Light" => Self::LIGHT,
            "ExtraLight" => Self::EXTRALIGHT,
            "Thin" => Self::THIN,
            _ => return None,
        })
    }
}

impl Serialize for FontWeight {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.categorize_weight() {
            FontWeightOrLabel::Weight(n) => serializer.serialize_u16(n),
            FontWeightOrLabel::Label(l) => serializer.serialize_str(l),
        }
    }
}

impl<'de> Deserialize<'de> for FontWeight {
    fn deserialize<D>(deserializer: D) -> Result<FontWeight, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V {}

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FontWeight;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("string or font weight value")
            }

            fn visit_str<E>(self, value: &str) -> Result<FontWeight, E>
            where
                E: serde::de::Error,
            {
                match FontWeight::from_str(value) {
                    Some(w) => Ok(w),
                    None => Err(E::custom(format!("invalid font weight {}", value))),
                }
            }

            // Lua gives us an integer in this format
            fn visit_i64<E>(self, value: i64) -> Result<FontWeight, E>
            where
                E: serde::de::Error,
            {
                if value > 0 && value <= u16::MAX as i64 {
                    Ok(FontWeight(value as u16))
                } else {
                    Err(E::custom(format!("invalid font weight {}", value)))
                }
            }
        }

        deserializer.deserialize_any(V {})
    }
}

impl Display for FontWeight {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.categorize_weight() {
            FontWeightOrLabel::Weight(n) => write!(fmt, "{}", n),
            FontWeightOrLabel::Label(l) => write!(fmt, "\"{}\"", l),
        }
    }
}

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const EXTRALIGHT: FontWeight = FontWeight(200);
    pub const LIGHT: FontWeight = FontWeight(300);
    pub const DEMILIGHT: FontWeight = FontWeight(350);
    pub const BOOK: FontWeight = FontWeight(380);
    pub const REGULAR: FontWeight = FontWeight(400);
    pub const MEDIUM: FontWeight = FontWeight(500);
    pub const DEMIBOLD: FontWeight = FontWeight(600);
    pub const BOLD: FontWeight = FontWeight(700);
    pub const EXTRABOLD: FontWeight = FontWeight(800);
    pub const BLACK: FontWeight = FontWeight(900);
    pub const EXTRABLACK: FontWeight = FontWeight(1000);
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::REGULAR
    }
}

impl FontWeight {
    pub const fn from_opentype_weight(w: u16) -> Self {
        Self(w)
    }

    pub fn to_opentype_weight(self) -> u16 {
        self.0
    }

    pub fn lighter(self) -> Self {
        Self::from_opentype_weight(self.to_opentype_weight().saturating_sub(200))
    }

    pub fn bolder(self) -> Self {
        Self::from_opentype_weight(self.to_opentype_weight() + 200)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FreeTypeLoadTarget {
    /// This corresponds to the default hinting algorithm, optimized
    /// for standard gray-level rendering.
    Normal,
    /// A lighter hinting algorithm for non-monochrome modes. Many
    /// generated glyphs are more fuzzy but better resemble its
    /// original shape. A bit like rendering on Mac OS X.  This target
    /// implies FT_LOAD_FORCE_AUTOHINT.
    Light,
    /// Strong hinting algorithm that should only be used for
    /// monochrome output. The result is probably unpleasant if the
    /// glyph is rendered in non-monochrome modes.
    Mono,
    /// A variant of Normal optimized for horizontally decimated LCD displays.
    HorizontalLcd,
    /// A variant of Normal optimized for vertically decimated LCD displays.
    VerticalLcd,
}

impl Default for FreeTypeLoadTarget {
    fn default() -> Self {
        Self::Normal
    }
}

bitflags! {
    // Note that these are strongly coupled with deps/freetype/src/lib.rs,
    // but we can't directly reference that from here without making config
    // depend on freetype.
    #[derive(Default, Deserialize, Serialize)]
    pub struct FreeTypeLoadFlags: u32 {
        /// FT_LOAD_DEFAULT
        const DEFAULT = 0;
        /// Disable hinting. This generally generates ‘blurrier’
        /// bitmap glyph when the glyph is rendered in any of the
        /// anti-aliased modes.
        const NO_HINTING = 2;
        const NO_BITMAP = 8;
        /// Indicates that the auto-hinter is preferred over the
        /// font’s native hinter.
        const FORCE_AUTOHINT = 32;
        const MONOCHROME = 4096;
        /// Disable auto-hinter.
        const NO_AUTOHINT = 32768;
    }
}

impl FreeTypeLoadFlags {
    pub fn de_string<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let mut flags = FreeTypeLoadFlags::default();

        for ele in s.split('|') {
            let ele = ele.trim();
            match ele {
                "DEFAULT" => flags |= Self::DEFAULT,
                "NO_HINTING" => flags |= Self::NO_HINTING,
                "NO_BITMAP" => flags |= Self::NO_BITMAP,
                "FORCE_AUTOHINT" => flags |= Self::FORCE_AUTOHINT,
                "MONOCHROME" => flags |= Self::MONOCHROME,
                "NO_AUTOHINT" => flags |= Self::NO_AUTOHINT,
                _ => {
                    return Err(serde::de::Error::custom(format!(
                        "invalid FreeTypeLoadFlags {} in {}",
                        ele, s
                    )));
                }
            }
        }

        Ok(flags)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct FontAttributes {
    /// The font family name
    pub family: String,
    /// Whether the font should be a bold variant
    #[serde(default)]
    pub weight: FontWeight,
    #[serde(default)]
    pub stretch: FontStretch,
    /// Whether the font should be an italic variant
    #[serde(default)]
    pub italic: bool,
    pub is_fallback: bool,
    pub is_synthetic: bool,
}
impl_lua_conversion!(FontAttributes);

impl std::fmt::Display for FontAttributes {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "wezterm.font('{}', {{weight={}, stretch='{}', italic={}}})",
            self.family, self.weight, self.stretch, self.italic
        )
    }
}

impl FontAttributes {
    pub fn new(family: &str) -> Self {
        Self {
            family: family.into(),
            weight: FontWeight::default(),
            stretch: FontStretch::default(),
            italic: false,
            is_fallback: false,
            is_synthetic: false,
        }
    }

    pub fn new_fallback(family: &str) -> Self {
        Self {
            family: family.into(),
            weight: FontWeight::default(),
            stretch: FontStretch::default(),
            italic: false,
            is_fallback: true,
            is_synthetic: false,
        }
    }
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".into(),
            weight: FontWeight::default(),
            stretch: FontStretch::default(),
            italic: false,
            is_fallback: false,
            is_synthetic: false,
        }
    }
}

/// Represents textual styling.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct TextStyle {
    #[serde(default)]
    pub font: Vec<FontAttributes>,

    /// If set, when rendering text that is set to the default
    /// foreground color, use this color instead.  This is most
    /// useful in a `[[font_rules]]` section to implement changing
    /// the text color for eg: bold text.
    pub foreground: Option<RgbColor>,
}
impl_lua_conversion!(TextStyle);

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            foreground: None,
            font: vec![FontAttributes::default()],
        }
    }
}

impl TextStyle {
    /// Make a version of this style where the first entry
    /// has any explicitly named bold/italic components
    /// removed.  The intent is to set it up for make_bold
    /// and make_italic below.
    ///
    /// This is done heuristically based on the family name
    /// string as we cannot depend on the font parser from
    /// this crate, and even if we did have a parser, that
    /// doesn't help us know anything about the name until
    /// we have a parsed font to compare with.
    ///
    /// <https://github.com/wez/wezterm/issues/456>
    pub fn reduce_first_font_to_family(&self) -> Self {
        fn reduce(mut family: &str) -> String {
            loop {
                let start = family;

                for s in &[
                    "Black",
                    "Bold",
                    "Book",
                    "Condensed",
                    "Demi",
                    "Expanded",
                    "Extra",
                    "Italic",
                    "Light",
                    "Medium",
                    "Regular",
                    "Semi",
                    "Thin",
                    "Ultra",
                ] {
                    family = family.trim().trim_end_matches(s);
                }

                if family == start {
                    break;
                }
            }

            family.trim().to_string()
        }
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .enumerate()
                .map(|(idx, orig_attr)| {
                    let mut attr = orig_attr.clone();
                    if idx == 0 {
                        attr.family = reduce(&attr.family);
                    }
                    attr
                })
                .collect(),
        }
    }

    /// Make a version of this style with bold enabled.
    pub fn make_bold(&self) -> Self {
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .map(|attr| {
                    let mut attr = attr.clone();
                    attr.weight = attr.weight.bolder();
                    attr.is_synthetic = true;
                    attr
                })
                .collect(),
        }
    }

    pub fn make_half_bright(&self) -> Self {
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .map(|attr| {
                    let mut attr = attr.clone();
                    attr.weight = attr.weight.lighter();
                    attr.is_synthetic = true;
                    attr
                })
                .collect(),
        }
    }

    /// Make a version of this style with italic enabled.
    pub fn make_italic(&self) -> Self {
        Self {
            foreground: self.foreground,
            font: self
                .font
                .iter()
                .map(|attr| {
                    let mut attr = attr.clone();
                    attr.italic = true;
                    attr.is_synthetic = true;
                    attr
                })
                .collect(),
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::let_and_return))]
    pub fn font_with_fallback(&self) -> Vec<FontAttributes> {
        let mut font = self.font.clone();

        let mut default_font = FontAttributes::default();

        // Insert our bundled default JetBrainsMono as a fallback
        // in case their preference doesn't match anything.
        // But don't add it if it is already their preference.
        if font.iter().position(|f| *f == default_font).is_none() {
            default_font.is_fallback = true;
            font.push(default_font);
        }

        // We bundle this emoji font as an in-memory fallback
        font.push(FontAttributes::new_fallback("Noto Color Emoji"));

        // And finally, a last resort fallback font
        font.push(FontAttributes::new_fallback("Last Resort High-Efficiency"));

        font
    }
}

/// Defines a rule that can be used to select a `TextStyle` given
/// an input `CellAttributes` value.  The logic that applies the
/// matching can be found in src/font/mod.rs.  The concept is that
/// the user can specify something like this:
///
/// ```toml
/// [[font_rules]]
/// italic = true
/// font = { font = [{family = "Operator Mono SSm Lig", italic=true}]}
/// ```
///
/// The above is translated as: "if the `CellAttributes` have the italic bit
/// set, then use the italic style of font rather than the default", and
/// stop processing further font rules.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct StyleRule {
    /// If present, this rule matches when CellAttributes::intensity holds
    /// a value that matches this rule.  Valid values are "Bold", "Normal",
    /// "Half".
    pub intensity: Option<wezterm_term::Intensity>,
    /// If present, this rule matches when CellAttributes::underline holds
    /// a value that matches this rule.  Valid values are "None", "Single",
    /// "Double".
    pub underline: Option<wezterm_term::Underline>,
    /// If present, this rule matches when CellAttributes::italic holds
    /// a value that matches this rule.
    pub italic: Option<bool>,
    /// If present, this rule matches when CellAttributes::blink holds
    /// a value that matches this rule.
    pub blink: Option<wezterm_term::Blink>,
    /// If present, this rule matches when CellAttributes::reverse holds
    /// a value that matches this rule.
    pub reverse: Option<bool>,
    /// If present, this rule matches when CellAttributes::strikethrough holds
    /// a value that matches this rule.
    pub strikethrough: Option<bool>,
    /// If present, this rule matches when CellAttributes::invisible holds
    /// a value that matches this rule.
    pub invisible: Option<bool>,

    /// When this rule matches, `font` specifies the styling to be used.
    pub font: TextStyle,
}
impl_lua_conversion!(StyleRule);

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum AllowSquareGlyphOverflow {
    Never,
    Always,
    WhenFollowedBySpace,
}

impl Default for AllowSquareGlyphOverflow {
    fn default() -> Self {
        Self::WhenFollowedBySpace
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum FontLocatorSelection {
    /// Use fontconfig APIs to resolve fonts (!macos, posix systems)
    FontConfig,
    /// Use GDI on win32 systems
    Gdi,
    /// Use CoreText on macOS
    CoreText,
    /// Use only the font_dirs configuration to locate fonts
    ConfigDirsOnly,
}

impl Default for FontLocatorSelection {
    fn default() -> Self {
        if cfg!(windows) {
            FontLocatorSelection::Gdi
        } else if cfg!(target_os = "macos") {
            FontLocatorSelection::CoreText
        } else {
            FontLocatorSelection::FontConfig
        }
    }
}

impl FontLocatorSelection {
    pub fn variants() -> Vec<&'static str> {
        vec!["FontConfig", "CoreText", "ConfigDirsOnly", "Gdi"]
    }
}

impl std::str::FromStr for FontLocatorSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "fontconfig" => Ok(Self::FontConfig),
            "coretext" => Ok(Self::CoreText),
            "configdirsonly" => Ok(Self::ConfigDirsOnly),
            "gdi" => Ok(Self::Gdi),
            _ => Err(anyhow!(
                "{} is not a valid FontLocatorSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum FontRasterizerSelection {
    FreeType,
}

impl Default for FontRasterizerSelection {
    fn default() -> Self {
        FontRasterizerSelection::FreeType
    }
}

impl FontRasterizerSelection {
    pub fn variants() -> Vec<&'static str> {
        vec!["FreeType"]
    }
}

impl std::str::FromStr for FontRasterizerSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "freetype" => Ok(Self::FreeType),
            _ => Err(anyhow!(
                "{} is not a valid FontRasterizerSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum FontShaperSelection {
    Allsorts,
    Harfbuzz,
}

impl Default for FontShaperSelection {
    fn default() -> Self {
        FontShaperSelection::Harfbuzz
    }
}

impl FontShaperSelection {
    pub fn variants() -> Vec<&'static str> {
        vec!["Harfbuzz", "AllSorts"]
    }
}

impl std::str::FromStr for FontShaperSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "harfbuzz" => Ok(Self::Harfbuzz),
            "allsorts" => Ok(Self::Allsorts),
            _ => Err(anyhow!(
                "{} is not a valid FontShaperSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_reduce() {
        for family in &[
            "Inconsolata SemiCondensed ExtraBold",
            "Inconsolata SemiCondensed Regular",
            "Inconsolata SemiCondensed Medium",
            "Inconsolata SemiCondensed SemiBold",
        ] {
            let style = TextStyle {
                font: vec![FontAttributes::new(family)],
                foreground: None,
            };
            let style = style.reduce_first_font_to_family();
            assert_eq!(style.font[0].family, "Inconsolata");
        }
    }
}
