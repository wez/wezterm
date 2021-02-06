use crate::*;
use luahelper::impl_lua_conversion;
use termwiz::color::RgbColor;

#[derive(Debug, Copy, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub enum FontHinting {
    /// No hinting is performed
    None,
    /// Light vertical hinting is performed to fit the terminal grid
    Vertical,
    /// Vertical hinting is performed, with additional processing
    /// for subpixel anti-aliasing.
    /// This level is equivalent to Microsoft ClearType.
    VerticalSubpixel,
    /// Vertical and horizontal hinting is performed.
    Full,
}
impl_lua_conversion!(FontHinting);

impl Default for FontHinting {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Debug, Copy, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub enum FontAntiAliasing {
    None,
    Greyscale,
    Subpixel,
}
impl_lua_conversion!(FontAntiAliasing);

impl Default for FontAntiAliasing {
    fn default() -> Self {
        Self::Greyscale
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct FontAttributes {
    /// The font family name
    pub family: String,
    /// Whether the font should be a bold variant
    #[serde(default)]
    pub bold: bool,
    /// Whether the font should be an italic variant
    #[serde(default)]
    pub italic: bool,
    pub is_fallback: bool,
}
impl_lua_conversion!(FontAttributes);

impl std::fmt::Display for FontAttributes {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "wezterm.font('{}', {{bold={}, italic={}}})",
            self.family, self.bold, self.italic
        )
    }
}

impl FontAttributes {
    pub fn new(family: &str) -> Self {
        Self {
            family: family.into(),
            bold: false,
            italic: false,
            is_fallback: false,
        }
    }

    pub fn new_fallback(family: &str) -> Self {
        Self {
            family: family.into(),
            bold: false,
            italic: false,
            is_fallback: true,
        }
    }
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".into(),
            bold: false,
            italic: false,
            is_fallback: false,
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
        fn reduce(family: &str) -> String {
            family
                // Italic tends to be last in the string,
                // if present, so strip it first
                .trim_end_matches(" Italic")
                // Then the various weight names
                .trim_end_matches(" Thin")
                .trim_end_matches(" Extra Light")
                .trim_end_matches(" Normal")
                .trim_end_matches(" Medium")
                .trim_end_matches(" Semi Bold")
                .trim_end_matches(" Bold")
                .trim_end_matches(" Extra Bold")
                .trim_end_matches(" Ultra Bold")
                .trim_end_matches(" Book")
                .to_string()
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
                    attr.bold = true;
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

        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new_fallback("Apple Symbols"));
        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new_fallback("Zapf Dingbats"));

        // Fallback font that has unicode replacement character
        #[cfg(windows)]
        font.push(FontAttributes::new_fallback("Segoe UI"));
        #[cfg(windows)]
        font.push(FontAttributes::new_fallback("Segoe UI Emoji"));
        #[cfg(windows)]
        font.push(FontAttributes::new_fallback("Segoe UI Symbol"));

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

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AllowSquareGlyphOverflow {
    Never,
    Always,
    WhenFollowedBySpace,
}

impl Default for AllowSquareGlyphOverflow {
    fn default() -> Self {
        Self::Never
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
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

lazy_static::lazy_static! {
    static ref DEFAULT_LOCATOR: Mutex<FontLocatorSelection> = Mutex::new(Default::default());
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
    pub fn set_default(self) {
        let mut def = DEFAULT_LOCATOR.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_LOCATOR.lock().unwrap();
        *def
    }

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

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontRasterizerSelection {
    FreeType,
}

lazy_static::lazy_static! {
    static ref DEFAULT_RASTER: Mutex<FontRasterizerSelection> = Mutex::new(Default::default());
}

impl Default for FontRasterizerSelection {
    fn default() -> Self {
        FontRasterizerSelection::FreeType
    }
}

impl FontRasterizerSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_RASTER.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_RASTER.lock().unwrap();
        *def
    }

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

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontShaperSelection {
    Allsorts,
    Harfbuzz,
}

lazy_static::lazy_static! {
    static ref DEFAULT_SHAPER: Mutex<FontShaperSelection> = Mutex::new(Default::default());
}

impl Default for FontShaperSelection {
    fn default() -> Self {
        FontShaperSelection::Harfbuzz
    }
}

impl FontShaperSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_SHAPER.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_SHAPER.lock().unwrap();
        *def
    }

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
