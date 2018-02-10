//! Configuration for the gui portion of the terminal

use failure::Error;
use std;
use std::fs;
use std::io::prelude::*;
use toml;

use term;


#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// The font size, measured in points
    #[serde(default = "default_font_size")]
    pub font_size: f64,

    /// The DPI to assume
    #[serde(default = "default_dpi")]
    pub dpi: f64,

    /// The baseline font to use
    #[serde(default)]
    pub font: TextStyle,

    /// An optional set of style rules to select the font based
    /// on the cell attributes
    #[serde(default)]
    pub font_rules: Vec<StyleRule>,

    /// The color palette
    pub colors: Option<Palette>,
}

fn default_font_size() -> f64 {
    10.0
}

fn default_dpi() -> f64 {
    96.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            dpi: default_dpi(),
            font: TextStyle::default(),
            font_rules: Vec::new(),
            colors: None,
        }
    }
}

/// Represents textual styling.
/// TODO: I want to add some rules so that a user can specify the font
/// and colors to use in some situations.  For example, xterm has
/// a bold color option; I'd like to be able to express something
/// like "when text is bold, use this font pattern and set the text
/// color to X".  There are some interesting possibilities here;
/// instead of just setting the color to a specific value we could
/// apply a transform to the color attribute and make it X% brighter.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// A font config pattern to parse to locate the font.
    /// Note that the dpi and current font_size for the terminal
    /// will be set on the parsed result.
    pub fontconfig_pattern: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { fontconfig_pattern: "monospace".into() }
    }
}

/// Defines a rule that can be used to select a TextStyle given
/// an input CellAttributes value.  The logic that applies the
/// matching can be found in src/font/mod.rs.  The concept is that
/// the user can specify something like this:
///
/// ```
/// [[font_rules]]
/// italic = true
/// font = { fontconfig_pattern = "Operator Mono SSm Lig:style=Italic" }
/// ```
///
/// The above is translated as: "if the CellAttributes have the italic bit
/// set, then use the italic style of font rather than the default", and
/// stop processing further font rules.
#[derive(Debug, Deserialize, Clone)]
pub struct StyleRule {
    /// If present, this rule matches when CellAttributes::intensity holds
    /// a value that matches this rule.  Valid values are "Bold", "Normal",
    /// "Half".
    pub intensity: Option<term::Intensity>,
    /// If present, this rule matches when CellAttributes::underline holds
    /// a value that matches this rule.  Valid values are "None", "Single",
    /// "Double".
    pub underline: Option<term::Underline>,
    /// If present, this rule matches when CellAttributes::italic holds
    /// a value that matches this rule.
    pub italic: Option<bool>,
    /// If present, this rule matches when CellAttributes::blink holds
    /// a value that matches this rule.
    pub blink: Option<bool>,
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

impl Config {
    pub fn load() -> Result<Self, Error> {
        let home = std::env::home_dir().ok_or_else(
            || format_err!("can't find home dir"),
        )?;

        let paths = [
            home.join(".config").join("wezterm").join("wezterm.toml"),
            home.join(".wezterm.toml"),
        ];

        for p in paths.iter() {
            let mut file = match fs::File::open(p) {
                Ok(file) => file,
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::NotFound => continue,
                        _ => bail!("Error opening {}: {:?}", p.display(), err),
                    }
                }
            };

            let mut s = String::new();
            file.read_to_string(&mut s)?;

            return toml::from_str(&s).map_err(|e| {
                format_err!("Error parsing TOML from {}: {:?}", p.display(), e)
            });
        }

        Ok(Self::default())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Palette {
    /// The text color to use when the attributes are reset to default
    pub foreground: Option<term::color::RgbColor>,
    /// The background color to use when the attributes are reset to default
    pub background: Option<term::color::RgbColor>,
    /// The color of the cursor
    pub cursor: Option<term::color::RgbColor>,
    /// A list of 8 colors corresponding to the basic ANSI palette
    pub ansi: Option<[term::color::RgbColor; 8]>,
    /// A list of 8 colors corresponding to bright versions of the
    /// ANSI palette
    pub brights: Option<[term::color::RgbColor; 8]>,
}

impl From<Palette> for term::color::ColorPalette {
    fn from(cfg: Palette) -> term::color::ColorPalette {
        let mut p = term::color::ColorPalette::default();
        if let Some(foreground) = cfg.foreground {
            p.foreground = foreground;
        }
        if let Some(background) = cfg.background {
            p.background = background;
        }
        if let Some(cursor) = cfg.cursor {
            p.cursor = cursor;
        }
        if let Some(ansi) = cfg.ansi {
            for (idx, col) in ansi.iter().enumerate() {
                p.colors[idx] = *col;
            }
        }
        if let Some(brights) = cfg.brights {
            for (idx, col) in brights.iter().enumerate() {
                p.colors[idx + 8] = *col;
            }
        }
        p
    }
}
