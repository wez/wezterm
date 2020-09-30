use crate::config::*;
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
        Self::Subpixel
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
}
impl_lua_conversion!(FontAttributes);

impl FontAttributes {
    pub fn new(family: &str) -> Self {
        Self {
            family: family.into(),
            bold: false,
            italic: false,
        }
    }
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".into(),
            bold: false,
            italic: false,
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

        let default_font = FontAttributes::default();

        // Insert our bundled default JetBrainsMono as a fallback
        // in case their preference doesn't match anything.
        // But don't add it if it is already their preference.
        if font.iter().position(|f| *f == default_font).is_none() {
            font.push(default_font);
        }

        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new("Apple Color Emoji"));
        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new("Apple Symbols"));
        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new("Zapf Dingbats"));
        #[cfg(target_os = "macos")]
        font.push(FontAttributes::new("Apple LiGothic"));

        // Fallback font that has unicode replacement character
        #[cfg(windows)]
        font.push(FontAttributes::new("Segoe UI"));
        #[cfg(windows)]
        font.push(FontAttributes::new("Segoe UI Emoji"));
        #[cfg(windows)]
        font.push(FontAttributes::new("Segoe UI Symbol"));

        // We bundle this emoji font as an in-memory fallback
        font.push(FontAttributes::new("Noto Color Emoji"));

        font
    }
}

/// Defines a rule that can be used to select a `TextStyle` given
/// an input `CellAttributes` value.  The logic that applies the
/// matching can be found in src/font/mod.rs.  The concept is that
/// the user can specify something like this:
///
/// ```
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
