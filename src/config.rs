//! Configuration for the gui portion of the terminal

use failure::Error;
use std;
use std::fs;
use std::io::prelude::*;
use toml;


#[derive(Debug, Deserialize)]
pub struct Config {
    /// The font size, measured in points
    #[serde(default = "default_font_size")]
    pub font_size: f64,

    /// The DPI to assume
    #[serde(default = "default_dpi")]
    pub dpi: f64,

    #[serde(default)]
    pub font: TextStyle,
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
#[derive(Debug, Deserialize)]
pub struct TextStyle {
    /// A font config pattern to parse to locate the font.
    /// Note that the dpi and current font_size for the terminal
    /// will be set on the parsed result.
    pub fontconfig_pattern: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            fontconfig_pattern: "monospace".into()
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let home = std::env::home_dir().ok_or_else(|| format_err!("can't find home dir"))?;

        let paths = [
            home.join(".config").join("wezterm").join("wezterm.toml"),
            home.join(".wezterm.toml"),
        ];

        for p in paths.iter() {
            let mut file = match fs::File::open(p) {
                Ok(file) => file,
                Err(err) => match err.kind() {
                    std::io::ErrorKind::NotFound => continue,
                    _ => bail!("Error opening {}: {:?}", p.display(), err),
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
