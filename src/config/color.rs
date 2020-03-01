use crate::config::*;
use termwiz::cell::CellAttributes;
use termwiz::color::{ColorSpec, RgbColor};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Palette {
    /// The text color to use when the attributes are reset to default
    pub foreground: Option<RgbColor>,
    /// The background color to use when the attributes are reset to default
    pub background: Option<RgbColor>,
    /// The color of the cursor
    pub cursor_fg: Option<RgbColor>,
    pub cursor_bg: Option<RgbColor>,
    pub cursor_border: Option<RgbColor>,
    /// The color of selected text
    pub selection_fg: Option<RgbColor>,
    pub selection_bg: Option<RgbColor>,
    /// A list of 8 colors corresponding to the basic ANSI palette
    pub ansi: Option<[RgbColor; 8]>,
    /// A list of 8 colors corresponding to bright versions of the
    /// ANSI palette
    pub brights: Option<[RgbColor; 8]>,
    /// Configure the colors and styling of the tab bar
    pub tab_bar: Option<TabBarColors>,
    /// The color of the "thumb" of the scrollbar; the segment that
    /// represents the current viewable area
    pub scrollbar_thumb: Option<RgbColor>,
}
impl_lua_conversion!(Palette);

impl From<Palette> for term::color::ColorPalette {
    fn from(cfg: Palette) -> term::color::ColorPalette {
        let mut p = term::color::ColorPalette::default();
        macro_rules! apply_color {
            ($name:ident) => {
                if let Some($name) = cfg.$name {
                    p.$name = $name;
                }
            };
        }
        apply_color!(foreground);
        apply_color!(background);
        apply_color!(cursor_fg);
        apply_color!(cursor_bg);
        apply_color!(cursor_border);
        apply_color!(selection_fg);
        apply_color!(selection_bg);
        apply_color!(scrollbar_thumb);

        if let Some(ansi) = cfg.ansi {
            for (idx, col) in ansi.iter().enumerate() {
                p.colors.0[idx] = *col;
            }
        }
        if let Some(brights) = cfg.brights {
            for (idx, col) in brights.iter().enumerate() {
                p.colors.0[idx + 8] = *col;
            }
        }
        p
    }
}

/// Specify the text styling for a tab in the tab bar
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TabBarColor {
    /// Specifies the intensity attribute for the tab title text
    #[serde(default)]
    pub intensity: term::Intensity,
    /// Specifies the underline attribute for the tab title text
    #[serde(default)]
    pub underline: term::Underline,
    /// Specifies the italic attribute for the tab title text
    #[serde(default)]
    pub italic: bool,
    /// Specifies the strikethrough attribute for the tab title text
    #[serde(default)]
    pub strikethrough: bool,
    /// The background color for the tab
    pub bg_color: RgbColor,
    /// The forgeground/text color for the tab
    pub fg_color: RgbColor,
}
impl_lua_conversion!(TabBarColor);

impl TabBarColor {
    pub fn as_cell_attributes(&self) -> CellAttributes {
        let mut attr = CellAttributes::default();
        attr.set_intensity(self.intensity)
            .set_underline(self.underline)
            .set_italic(self.italic)
            .set_strikethrough(self.strikethrough)
            .set_background(ColorSpec::TrueColor(self.bg_color))
            .set_foreground(ColorSpec::TrueColor(self.fg_color));
        attr
    }
}

/// Specifies the colors to use for the tab bar portion of the UI.
/// These are not part of the terminal model and cannot be updated
/// in the same way that the dynamic color schemes are.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TabBarColors {
    /// The background color for the tab bar
    #[serde(default = "default_background")]
    pub background: RgbColor,

    /// Styling for the active tab
    #[serde(default = "default_active_tab")]
    pub active_tab: TabBarColor,

    /// Styling for other inactive tabs
    #[serde(default = "default_inactive_tab")]
    pub inactive_tab: TabBarColor,

    /// Styling for an inactive tab with a mouse hovering
    #[serde(default = "default_inactive_tab_hover")]
    pub inactive_tab_hover: TabBarColor,
}
impl_lua_conversion!(TabBarColors);

fn default_background() -> RgbColor {
    RgbColor::new(0x0b, 0x00, 0x22)
}

fn default_inactive_tab() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new(0x1b, 0x10, 0x32),
        fg_color: RgbColor::new(0x80, 0x80, 0x80),
        ..TabBarColor::default()
    }
}
fn default_inactive_tab_hover() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new(0x3b, 0x30, 0x52),
        fg_color: RgbColor::new(0x90, 0x90, 0x90),
        italic: true,
        ..TabBarColor::default()
    }
}
fn default_active_tab() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new(0x2b, 0x20, 0x42),
        fg_color: RgbColor::new(0xc0, 0xc0, 0xc0),
        ..TabBarColor::default()
    }
}

impl Default for TabBarColors {
    fn default() -> Self {
        Self {
            background: default_background(),
            inactive_tab: default_inactive_tab(),
            inactive_tab_hover: default_inactive_tab_hover(),
            active_tab: default_active_tab(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ColorSchemeFile {
    /// The color palette
    pub colors: Palette,
}
impl_lua_conversion!(ColorSchemeFile);
