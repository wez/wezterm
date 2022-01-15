use crate::lua::{format_as_escapes, FormatItem};
use crate::*;
use luahelper::impl_lua_conversion;
use termwiz::cell::CellAttributes;
pub use termwiz::color::{ColorSpec, RgbColor};

#[derive(Debug, Copy, Deserialize, Serialize, Clone)]
pub struct HsbTransform {
    #[serde(default = "default_one_point_oh")]
    pub hue: f32,
    #[serde(default = "default_one_point_oh")]
    pub saturation: f32,
    #[serde(default = "default_one_point_oh")]
    pub brightness: f32,
}

impl Default for HsbTransform {
    fn default() -> Self {
        Self {
            hue: 1.,
            saturation: 1.,
            brightness: 1.,
        }
    }
}

fn de_indexed<'de, D>(deserializer: D) -> Result<HashMap<u8, RgbColor>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrap(HashMap<String, RgbColor>);
    let Wrap(map) = Wrap::deserialize(deserializer)?;

    Ok(map
        .into_iter()
        .filter_map(|(k, v)| match k.parse::<u8>() {
            Ok(n) if n >= 16 => Some((n, v)),
            _ => {
                log::warn!("Ignoring invalid color key {}", k);
                None
            }
        })
        .collect())
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
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
    /// A map for setting arbitrary colors ranging from 16 to 256 in the color
    /// palette
    #[serde(default, deserialize_with = "de_indexed")]
    pub indexed: HashMap<u8, RgbColor>,
    /// Configure the colors and styling of the tab bar
    pub tab_bar: Option<TabBarColors>,
    /// The color of the "thumb" of the scrollbar; the segment that
    /// represents the current viewable area
    pub scrollbar_thumb: Option<RgbColor>,
    /// The color of the split line between panes
    pub split: Option<RgbColor>,
    /// The color of the visual bell. If unspecified, the foreground
    /// color is used instead.
    pub visual_bell: Option<RgbColor>,
    /// The color to use for the cursor when a dead key or leader state is active
    pub compose_cursor: Option<RgbColor>,
}
impl_lua_conversion!(Palette);

impl From<Palette> for wezterm_term::color::ColorPalette {
    fn from(cfg: Palette) -> wezterm_term::color::ColorPalette {
        let mut p = wezterm_term::color::ColorPalette::default();
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
        apply_color!(split);

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
        for (&idx, &col) in &cfg.indexed {
            p.colors.0[idx as usize] = col;
        }
        p
    }
}

/// Specify the text styling for a tab in the tab bar
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TabBarColor {
    /// Specifies the intensity attribute for the tab title text
    #[serde(default)]
    pub intensity: wezterm_term::Intensity,
    /// Specifies the underline attribute for the tab title text
    #[serde(default)]
    pub underline: wezterm_term::Underline,
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

    /// Styling for the new tab button
    #[serde(default = "default_inactive_tab")]
    pub new_tab: TabBarColor,

    /// Styling for the new tab button with a mouse hovering
    #[serde(default = "default_inactive_tab_hover")]
    pub new_tab_hover: TabBarColor,

    #[serde(default = "default_inactive_tab_edge")]
    pub inactive_tab_edge: RgbColor,

    #[serde(default = "default_inactive_tab_edge_hover")]
    pub inactive_tab_edge_hover: RgbColor,
}
impl_lua_conversion!(TabBarColors);

fn default_background() -> RgbColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33)
}

fn default_inactive_tab_edge() -> RgbColor {
    RgbColor::new_8bpc(0x57, 0x57, 0x57)
}

fn default_inactive_tab_edge_hover() -> RgbColor {
    RgbColor::new_8bpc(0x36, 0x36, 0x36)
}

fn default_inactive_tab() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new_8bpc(0x33, 0x33, 0x33),
        fg_color: RgbColor::new_8bpc(0x80, 0x80, 0x80),
        ..TabBarColor::default()
    }
}
fn default_inactive_tab_hover() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new_8bpc(0x1f, 0x1f, 0x1f),
        fg_color: RgbColor::new_8bpc(0x90, 0x90, 0x90),
        italic: true,
        ..TabBarColor::default()
    }
}
fn default_active_tab() -> TabBarColor {
    TabBarColor {
        bg_color: RgbColor::new_8bpc(0x00, 0x00, 0x00),
        fg_color: RgbColor::new_8bpc(0xc0, 0xc0, 0xc0),
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
            new_tab: default_inactive_tab(),
            new_tab_hover: default_inactive_tab_hover(),
            inactive_tab_edge: default_inactive_tab_edge(),
            inactive_tab_edge_hover: default_inactive_tab_edge_hover(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TabBarStyle {
    #[serde(default = "default_new_tab")]
    pub new_tab: String,
    #[serde(default = "default_new_tab")]
    pub new_tab_hover: String,
}

impl Default for TabBarStyle {
    fn default() -> Self {
        Self {
            new_tab: default_new_tab(),
            new_tab_hover: default_new_tab(),
        }
    }
}

fn default_new_tab() -> String {
    format_as_escapes(vec![FormatItem::Text(" + ".to_string())]).unwrap()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WindowFrameConfig {
    #[serde(default = "default_inactive_titlebar_bg")]
    pub inactive_titlebar_bg: RgbColor,
    #[serde(default = "default_active_titlebar_bg")]
    pub active_titlebar_bg: RgbColor,
    #[serde(default = "default_inactive_titlebar_fg")]
    pub inactive_titlebar_fg: RgbColor,
    #[serde(default = "default_active_titlebar_fg")]
    pub active_titlebar_fg: RgbColor,
    #[serde(default = "default_inactive_titlebar_border_bottom")]
    pub inactive_titlebar_border_bottom: RgbColor,
    #[serde(default = "default_active_titlebar_border_bottom")]
    pub active_titlebar_border_bottom: RgbColor,
    #[serde(default = "default_button_fg")]
    pub button_fg: RgbColor,
    #[serde(default = "default_button_bg")]
    pub button_bg: RgbColor,
    #[serde(default = "default_button_hover_fg")]
    pub button_hover_fg: RgbColor,
    #[serde(default = "default_button_hover_bg")]
    pub button_hover_bg: RgbColor,

    #[serde(default)]
    pub font: Option<TextStyle>,
    #[serde(default)]
    pub font_size: Option<f64>,
}

impl Default for WindowFrameConfig {
    fn default() -> Self {
        Self {
            inactive_titlebar_bg: default_inactive_titlebar_bg(),
            active_titlebar_bg: default_active_titlebar_bg(),
            inactive_titlebar_fg: default_inactive_titlebar_fg(),
            active_titlebar_fg: default_active_titlebar_fg(),
            inactive_titlebar_border_bottom: default_inactive_titlebar_border_bottom(),
            active_titlebar_border_bottom: default_active_titlebar_border_bottom(),
            button_fg: default_button_fg(),
            button_bg: default_button_bg(),
            button_hover_fg: default_button_hover_fg(),
            button_hover_bg: default_button_hover_bg(),
            font: None,
            font_size: None,
        }
    }
}

fn default_inactive_titlebar_bg() -> RgbColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33)
}

fn default_active_titlebar_bg() -> RgbColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33)
}

fn default_inactive_titlebar_fg() -> RgbColor {
    RgbColor::new_8bpc(0xcc, 0xcc, 0xcc)
}

fn default_active_titlebar_fg() -> RgbColor {
    RgbColor::new_8bpc(0xff, 0xff, 0xff)
}

fn default_inactive_titlebar_border_bottom() -> RgbColor {
    RgbColor::new_8bpc(0x2b, 0x20, 0x42)
}

fn default_active_titlebar_border_bottom() -> RgbColor {
    RgbColor::new_8bpc(0x2b, 0x20, 0x42)
}

fn default_button_hover_fg() -> RgbColor {
    RgbColor::new_8bpc(0xff, 0xff, 0xff)
}

fn default_button_fg() -> RgbColor {
    RgbColor::new_8bpc(0xcc, 0xcc, 0xcc)
}

fn default_button_hover_bg() -> RgbColor {
    RgbColor::new_8bpc(0x1f, 0x1f, 0x1f)
}

fn default_button_bg() -> RgbColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33)
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ColorSchemeFile {
    /// The color palette
    pub colors: Palette,
}
impl_lua_conversion!(ColorSchemeFile);
