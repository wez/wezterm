use crate::*;
use luahelper::impl_lua_conversion_dynamic;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use termwiz::cell::CellAttributes;
use termwiz::color::ColorSpec as TWColorSpec;
pub use termwiz::color::{AnsiColor, ColorAttribute, RgbColor, SrgbaTuple};
use wezterm_dynamic::{FromDynamic, ToDynamic};
use wezterm_term::color::ColorPalette;

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub struct HsbTransform {
    #[dynamic(default = "default_one_point_oh")]
    pub hue: f32,
    #[dynamic(default = "default_one_point_oh")]
    pub saturation: f32,
    #[dynamic(default = "default_one_point_oh")]
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, FromDynamic, ToDynamic)]
#[dynamic(try_from = "String", into = "String")]
pub struct RgbaColor {
    #[dynamic(flatten)]
    color: SrgbaTuple,
}

impl From<RgbColor> for RgbaColor {
    fn from(color: RgbColor) -> Self {
        Self {
            color: color.into(),
        }
    }
}

impl From<SrgbaTuple> for RgbaColor {
    fn from(color: SrgbaTuple) -> Self {
        Self { color }
    }
}

impl From<(u8, u8, u8)> for RgbaColor {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        let color: SrgbaTuple = (r, g, b).into();
        Self { color }
    }
}

impl std::ops::Deref for RgbaColor {
    type Target = SrgbaTuple;
    fn deref(&self) -> &SrgbaTuple {
        &self.color
    }
}

impl From<&RgbaColor> for String {
    fn from(val: &RgbaColor) -> Self {
        val.color.to_string()
    }
}

impl From<RgbaColor> for String {
    fn from(val: RgbaColor) -> Self {
        val.color.to_string()
    }
}

impl From<RgbaColor> for SrgbaTuple {
    fn from(val: RgbaColor) -> Self {
        val.color
    }
}

impl TryFrom<String> for RgbaColor {
    type Error = anyhow::Error;
    fn try_from(s: String) -> anyhow::Result<RgbaColor> {
        Ok(RgbaColor {
            color: SrgbaTuple::from_str(&s)
                .map_err(|_| anyhow::anyhow!("failed to parse {} as RgbaColor", &s))?,
        })
    }
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpec {
    AnsiColor(AnsiColor),
    Color(RgbaColor),
    Default,
}

impl From<AnsiColor> for ColorSpec {
    fn from(color: AnsiColor) -> ColorSpec {
        Self::AnsiColor(color)
    }
}

impl From<ColorSpec> for ColorAttribute {
    fn from(val: ColorSpec) -> Self {
        match val {
            ColorSpec::AnsiColor(c) => ColorAttribute::PaletteIndex(c.into()),
            ColorSpec::Color(RgbaColor { color }) => {
                ColorAttribute::TrueColorWithDefaultFallback(color)
            }
            ColorSpec::Default => ColorAttribute::Default,
        }
    }
}

impl From<ColorSpec> for TWColorSpec {
    fn from(val: ColorSpec) -> Self {
        match val {
            ColorSpec::AnsiColor(c) => c.into(),
            ColorSpec::Color(RgbaColor { color }) => TWColorSpec::TrueColor(color),
            ColorSpec::Default => TWColorSpec::Default,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
pub struct Palette {
    /// The text color to use when the attributes are reset to default
    pub foreground: Option<RgbaColor>,
    /// The background color to use when the attributes are reset to default
    pub background: Option<RgbaColor>,
    /// The color of the cursor
    pub cursor_fg: Option<RgbaColor>,
    pub cursor_bg: Option<RgbaColor>,
    pub cursor_border: Option<RgbaColor>,
    /// The color of selected text
    pub selection_fg: Option<RgbaColor>,
    pub selection_bg: Option<RgbaColor>,
    /// A list of 8 colors corresponding to the basic ANSI palette
    pub ansi: Option<[RgbaColor; 8]>,
    /// A list of 8 colors corresponding to bright versions of the
    /// ANSI palette
    pub brights: Option<[RgbaColor; 8]>,
    /// A map for setting arbitrary colors ranging from 16 to 256 in the color
    /// palette
    #[dynamic(default)]
    pub indexed: HashMap<u8, RgbaColor>,
    /// Configure the colors and styling of the tab bar
    pub tab_bar: Option<TabBarColors>,
    /// The color of the "thumb" of the scrollbar; the segment that
    /// represents the current viewable area
    pub scrollbar_thumb: Option<RgbaColor>,
    /// The color of the split line between panes
    pub split: Option<RgbaColor>,
    /// The color of the visual bell. If unspecified, the foreground
    /// color is used instead.
    pub visual_bell: Option<RgbaColor>,
    /// The color to use for the cursor when a dead key or leader state is active
    pub compose_cursor: Option<RgbaColor>,

    pub copy_mode_active_highlight_fg: Option<ColorSpec>,
    pub copy_mode_active_highlight_bg: Option<ColorSpec>,
    pub copy_mode_inactive_highlight_fg: Option<ColorSpec>,
    pub copy_mode_inactive_highlight_bg: Option<ColorSpec>,

    pub quick_select_label_fg: Option<ColorSpec>,
    pub quick_select_label_bg: Option<ColorSpec>,
    pub quick_select_match_fg: Option<ColorSpec>,
    pub quick_select_match_bg: Option<ColorSpec>,
}
impl_lua_conversion_dynamic!(Palette);

impl Palette {
    pub fn overlay_with(&self, other: &Self) -> Self {
        macro_rules! overlay {
            ($name:ident) => {
                if let Some(c) = &other.$name {
                    Some(c.clone())
                } else {
                    self.$name.clone()
                }
            };
        }
        Self {
            foreground: overlay!(foreground),
            background: overlay!(background),
            cursor_fg: overlay!(cursor_fg),
            cursor_bg: overlay!(cursor_bg),
            cursor_border: overlay!(cursor_border),
            selection_fg: overlay!(selection_fg),
            selection_bg: overlay!(selection_bg),
            ansi: overlay!(ansi),
            brights: overlay!(brights),
            tab_bar: match (&self.tab_bar, &other.tab_bar) {
                (Some(a), Some(b)) => Some(a.overlay_with(&b)),
                (None, Some(b)) => Some(b.clone()),
                (Some(a), None) => Some(a.clone()),
                (None, None) => None,
            },
            indexed: {
                let mut map = self.indexed.clone();
                for (k, v) in &other.indexed {
                    map.insert(*k, *v);
                }
                map
            },
            scrollbar_thumb: overlay!(scrollbar_thumb),
            split: overlay!(split),
            visual_bell: overlay!(visual_bell),
            compose_cursor: overlay!(compose_cursor),
            copy_mode_active_highlight_fg: overlay!(copy_mode_active_highlight_fg),
            copy_mode_active_highlight_bg: overlay!(copy_mode_active_highlight_bg),
            copy_mode_inactive_highlight_fg: overlay!(copy_mode_inactive_highlight_fg),
            copy_mode_inactive_highlight_bg: overlay!(copy_mode_inactive_highlight_bg),
            quick_select_label_fg: overlay!(quick_select_label_fg),
            quick_select_label_bg: overlay!(quick_select_label_bg),
            quick_select_match_fg: overlay!(quick_select_match_fg),
            quick_select_match_bg: overlay!(quick_select_match_bg),
        }
    }
}

impl From<ColorPalette> for Palette {
    fn from(cp: ColorPalette) -> Palette {
        let mut p = Palette::default();
        macro_rules! apply_color {
            ($name:ident) => {
                p.$name = Some(cp.$name.into());
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

        let mut ansi = [RgbaColor::default(); 8];
        for (idx, col) in cp.colors.0[0..8].iter().enumerate() {
            ansi[idx] = (*col).into();
        }
        p.ansi = Some(ansi);

        let mut brights = [RgbaColor::default(); 8];
        for (idx, col) in cp.colors.0[8..16].iter().enumerate() {
            brights[idx] = (*col).into();
        }
        p.brights = Some(brights);

        for (idx, col) in cp.colors.0.iter().enumerate().skip(16) {
            p.indexed.insert(idx as u8, (*col).into());
        }

        p
    }
}

impl From<Palette> for ColorPalette {
    fn from(cfg: Palette) -> ColorPalette {
        let mut p = ColorPalette::default();
        macro_rules! apply_color {
            ($name:ident) => {
                if let Some($name) = cfg.$name {
                    p.$name = $name.into();
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
                p.colors.0[idx] = (*col).into();
            }
        }
        if let Some(brights) = cfg.brights {
            for (idx, col) in brights.iter().enumerate() {
                p.colors.0[idx + 8] = (*col).into();
            }
        }
        for (&idx, &col) in &cfg.indexed {
            if idx < 16 {
                log::warn!(
                    "Ignoring invalid colors.indexed index {}; \
                           use `ansi` or `brights` to specify lower indices",
                    idx
                );
                continue;
            }
            p.colors.0[idx as usize] = col.into();
        }
        p
    }
}

/// Specify the text styling for a tab in the tab bar
#[derive(Debug, Clone, Default, PartialEq, FromDynamic, ToDynamic)]
pub struct TabBarColor {
    /// Specifies the intensity attribute for the tab title text
    #[dynamic(default)]
    pub intensity: wezterm_term::Intensity,
    /// Specifies the underline attribute for the tab title text
    #[dynamic(default)]
    pub underline: wezterm_term::Underline,
    /// Specifies the italic attribute for the tab title text
    #[dynamic(default)]
    pub italic: bool,
    /// Specifies the strikethrough attribute for the tab title text
    #[dynamic(default)]
    pub strikethrough: bool,
    /// The background color for the tab
    pub bg_color: RgbaColor,
    /// The forgeground/text color for the tab
    pub fg_color: RgbaColor,
}

impl TabBarColor {
    pub fn as_cell_attributes(&self) -> CellAttributes {
        let mut attr = CellAttributes::default();
        attr.set_intensity(self.intensity)
            .set_underline(self.underline)
            .set_italic(self.italic)
            .set_strikethrough(self.strikethrough)
            .set_background(TWColorSpec::TrueColor(*self.bg_color))
            .set_foreground(TWColorSpec::TrueColor(*self.fg_color));
        attr
    }
}

/// Specifies the colors to use for the tab bar portion of the UI.
/// These are not part of the terminal model and cannot be updated
/// in the same way that the dynamic color schemes are.
#[derive(Default, Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
pub struct TabBarColors {
    /// The background color for the tab bar
    #[dynamic(default)]
    pub background: Option<RgbaColor>,

    /// Styling for the active tab
    #[dynamic(default)]
    pub active_tab: Option<TabBarColor>,

    /// Styling for other inactive tabs
    #[dynamic(default)]
    pub inactive_tab: Option<TabBarColor>,

    /// Styling for an inactive tab with a mouse hovering
    #[dynamic(default)]
    pub inactive_tab_hover: Option<TabBarColor>,

    /// Styling for the new tab button
    #[dynamic(default)]
    pub new_tab: Option<TabBarColor>,

    /// Styling for the new tab button with a mouse hovering
    #[dynamic(default)]
    pub new_tab_hover: Option<TabBarColor>,

    #[dynamic(default)]
    pub inactive_tab_edge: Option<RgbaColor>,

    #[dynamic(default)]
    pub inactive_tab_edge_hover: Option<RgbaColor>,
}

impl TabBarColors {
    pub fn background(&self) -> RgbaColor {
        self.background.unwrap_or_else(default_background)
    }

    pub fn active_tab(&self) -> TabBarColor {
        self.active_tab.clone().unwrap_or_else(default_active_tab)
    }

    pub fn inactive_tab(&self) -> TabBarColor {
        self.inactive_tab
            .clone()
            .unwrap_or_else(default_inactive_tab)
    }

    pub fn inactive_tab_hover(&self) -> TabBarColor {
        self.inactive_tab_hover
            .clone()
            .unwrap_or_else(default_inactive_tab_hover)
    }

    pub fn new_tab(&self) -> TabBarColor {
        self.new_tab.clone().unwrap_or_else(default_inactive_tab)
    }

    pub fn new_tab_hover(&self) -> TabBarColor {
        self.new_tab_hover
            .clone()
            .unwrap_or_else(default_inactive_tab_hover)
    }

    pub fn inactive_tab_edge(&self) -> RgbaColor {
        self.inactive_tab_edge
            .unwrap_or_else(default_inactive_tab_edge)
    }

    pub fn inactive_tab_edge_hover(&self) -> RgbaColor {
        self.inactive_tab_edge_hover
            .unwrap_or_else(default_inactive_tab_edge_hover)
    }

    pub fn overlay_with(&self, other: &Self) -> Self {
        macro_rules! overlay {
            ($name:ident) => {
                if let Some(c) = &other.$name {
                    Some(c.clone())
                } else {
                    self.$name.clone()
                }
            };
        }
        Self {
            active_tab: overlay!(active_tab),
            background: overlay!(background),
            inactive_tab: overlay!(inactive_tab),
            inactive_tab_hover: overlay!(inactive_tab_hover),
            inactive_tab_edge: overlay!(inactive_tab_edge),
            inactive_tab_edge_hover: overlay!(inactive_tab_edge_hover),
            new_tab: overlay!(new_tab),
            new_tab_hover: overlay!(new_tab_hover),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
#[dynamic(try_from = "String")]
pub enum IntegratedTitleButtonColor {
    #[default]
    Auto,
    Custom(RgbaColor),
}

impl Into<String> for IntegratedTitleButtonColor {
    fn into(self) -> String {
        match self {
            Self::Auto => "auto".to_string(),
            Self::Custom(color) => color.into(),
        }
    }
}

impl TryFrom<String> for IntegratedTitleButtonColor {
    type Error = <RgbaColor as TryFrom<String>>::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else {
            Ok(Self::Custom(RgbaColor::try_from(value)?))
        }
    }
}

fn default_background() -> RgbaColor {
    (0x33, 0x33, 0x33).into()
}

fn default_inactive_tab_edge() -> RgbaColor {
    RgbColor::new_8bpc(0x57, 0x57, 0x57).into()
}

fn default_inactive_tab_edge_hover() -> RgbaColor {
    RgbColor::new_8bpc(0x36, 0x36, 0x36).into()
}

fn default_inactive_tab() -> TabBarColor {
    TabBarColor {
        bg_color: (0x33, 0x33, 0x33).into(),
        fg_color: (0x80, 0x80, 0x80).into(),
        ..TabBarColor::default()
    }
}
fn default_inactive_tab_hover() -> TabBarColor {
    TabBarColor {
        bg_color: (0x1f, 0x1f, 0x1f).into(),
        fg_color: (0x90, 0x90, 0x90).into(),
        italic: true,
        ..TabBarColor::default()
    }
}
fn default_active_tab() -> TabBarColor {
    TabBarColor {
        bg_color: (0x00, 0x00, 0x00).into(),
        fg_color: (0xc0, 0xc0, 0xc0).into(),
        ..TabBarColor::default()
    }
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct TabBarStyle {
    #[dynamic(default = "default_new_tab")]
    pub new_tab: String,
    #[dynamic(default = "default_new_tab")]
    pub new_tab_hover: String,
    #[dynamic(default = "default_window_hide")]
    pub window_hide: String,
    #[dynamic(default = "default_window_hide")]
    pub window_hide_hover: String,
    #[dynamic(default = "default_window_maximize")]
    pub window_maximize: String,
    #[dynamic(default = "default_window_maximize")]
    pub window_maximize_hover: String,
    #[dynamic(default = "default_window_close")]
    pub window_close: String,
    #[dynamic(default = "default_window_close")]
    pub window_close_hover: String,
}

impl Default for TabBarStyle {
    fn default() -> Self {
        Self {
            new_tab: default_new_tab(),
            new_tab_hover: default_new_tab(),
            window_hide: default_window_hide(),
            window_hide_hover: default_window_hide(),
            window_maximize: default_window_maximize(),
            window_maximize_hover: default_window_maximize(),
            window_close: default_window_close(),
            window_close_hover: default_window_close(),
        }
    }
}

fn default_new_tab() -> String {
    " + ".to_string()
}

fn default_window_hide() -> String {
    " . ".to_string()
}

fn default_window_maximize() -> String {
    " - ".to_string()
}

fn default_window_close() -> String {
    " X ".to_string()
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct WindowFrameConfig {
    #[dynamic(default = "default_inactive_titlebar_bg")]
    pub inactive_titlebar_bg: RgbaColor,
    #[dynamic(default = "default_active_titlebar_bg")]
    pub active_titlebar_bg: RgbaColor,
    #[dynamic(default = "default_inactive_titlebar_fg")]
    pub inactive_titlebar_fg: RgbaColor,
    #[dynamic(default = "default_active_titlebar_fg")]
    pub active_titlebar_fg: RgbaColor,
    #[dynamic(default = "default_inactive_titlebar_border_bottom")]
    pub inactive_titlebar_border_bottom: RgbaColor,
    #[dynamic(default = "default_active_titlebar_border_bottom")]
    pub active_titlebar_border_bottom: RgbaColor,
    #[dynamic(default = "default_button_fg")]
    pub button_fg: RgbaColor,
    #[dynamic(default = "default_button_bg")]
    pub button_bg: RgbaColor,
    #[dynamic(default = "default_button_hover_fg")]
    pub button_hover_fg: RgbaColor,
    #[dynamic(default = "default_button_hover_bg")]
    pub button_hover_bg: RgbaColor,

    #[dynamic(default)]
    pub font: Option<TextStyle>,
    #[dynamic(default)]
    pub font_size: Option<f64>,

    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_zero_pixel")]
    pub border_left_width: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_zero_pixel")]
    pub border_right_width: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_zero_pixel")]
    pub border_top_height: Dimension,
    #[dynamic(try_from = "crate::units::PixelUnit", default = "default_zero_pixel")]
    pub border_bottom_height: Dimension,

    pub border_left_color: Option<RgbaColor>,
    pub border_right_color: Option<RgbaColor>,
    pub border_top_color: Option<RgbaColor>,
    pub border_bottom_color: Option<RgbaColor>,
}

const fn default_zero_pixel() -> Dimension {
    Dimension::Pixels(0.)
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
            button_fg: default_button_fg().into(),
            button_bg: default_button_bg().into(),
            button_hover_fg: default_button_hover_fg(),
            button_hover_bg: default_button_hover_bg(),
            font: None,
            font_size: None,
            border_left_width: default_zero_pixel(),
            border_right_width: default_zero_pixel(),
            border_top_height: default_zero_pixel(),
            border_bottom_height: default_zero_pixel(),
            border_left_color: None,
            border_right_color: None,
            border_top_color: None,
            border_bottom_color: None,
        }
    }
}

fn default_inactive_titlebar_bg() -> RgbaColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33).into()
}

fn default_active_titlebar_bg() -> RgbaColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33).into()
}

fn default_inactive_titlebar_fg() -> RgbaColor {
    RgbColor::new_8bpc(0xcc, 0xcc, 0xcc).into()
}

fn default_active_titlebar_fg() -> RgbaColor {
    RgbColor::new_8bpc(0xff, 0xff, 0xff).into()
}

fn default_inactive_titlebar_border_bottom() -> RgbaColor {
    RgbColor::new_8bpc(0x2b, 0x20, 0x42).into()
}

fn default_active_titlebar_border_bottom() -> RgbaColor {
    RgbColor::new_8bpc(0x2b, 0x20, 0x42).into()
}

fn default_button_hover_fg() -> RgbaColor {
    RgbColor::new_8bpc(0xff, 0xff, 0xff).into()
}

fn default_button_fg() -> RgbaColor {
    RgbColor::new_8bpc(0xcc, 0xcc, 0xcc).into()
}

fn default_button_hover_bg() -> RgbaColor {
    RgbColor::new_8bpc(0x1f, 0x1f, 0x1f).into()
}

fn default_button_bg() -> RgbaColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33).into()
}

#[derive(Debug, Default, Clone, Eq, PartialEq, FromDynamic, ToDynamic)]
pub struct ColorSchemeMetaData {
    pub name: Option<String>,
    pub author: Option<String>,
    pub origin_url: Option<String>,
    pub wezterm_version: Option<String>,
    #[dynamic(default)]
    pub aliases: Vec<String>,
}
impl_lua_conversion_dynamic!(ColorSchemeMetaData);

#[derive(Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
pub struct ColorSchemeFile {
    /// The color palette
    pub colors: Palette,
    /// Info about the scheme
    #[dynamic(default)]
    pub metadata: ColorSchemeMetaData,
}
impl_lua_conversion_dynamic!(ColorSchemeFile);

fn dynamic_to_toml(value: Value) -> anyhow::Result<toml::Value> {
    Ok(match value {
        Value::Null => anyhow::bail!("cannot map Null to toml"),
        Value::Bool(b) => toml::Value::Boolean(b),
        Value::String(s) => toml::Value::String(s),
        Value::Array(a) => {
            let mut arr = vec![];
            for v in a {
                arr.push(dynamic_to_toml(v)?);
            }
            toml::Value::Array(arr)
        }
        Value::Object(o) => {
            let mut map = toml::map::Map::new();
            for (k, v) in o {
                let k = match k {
                    Value::String(s) => s,
                    Value::U64(u) => u.to_string(),
                    Value::I64(u) => u.to_string(),
                    Value::F64(u) => u.to_string(),
                    _ => anyhow::bail!("toml keys must be strings {k:?}"),
                };
                let v = match v {
                    Value::Null => continue,
                    other => dynamic_to_toml(other)?,
                };
                map.insert(k, v);
            }
            toml::Value::Table(map)
        }
        Value::U64(i) => toml::Value::Integer(i.try_into()?),
        Value::I64(i) => toml::Value::Integer(i.try_into()?),
        Value::F64(f) => toml::Value::Float(*f),
    })
}

impl ColorSchemeFile {
    pub fn from_toml_value(value: &toml::Value) -> anyhow::Result<Self> {
        let scheme = Self::from_dynamic(&crate::toml_to_dynamic(value), Default::default())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        anyhow::ensure!(
            scheme.colors.ansi.is_some(),
            "scheme is missing ANSI colors"
        );

        Ok(scheme)
    }

    pub fn from_toml_str(s: &str) -> anyhow::Result<Self> {
        let scheme: toml::Value = toml::from_str(s)?;
        Self::from_toml_value(&scheme)
    }

    pub fn to_toml_value(&self) -> anyhow::Result<toml::Value> {
        let value = self.to_dynamic();
        Ok(dynamic_to_toml(value)?)
    }

    pub fn from_json_value(value: &serde_json::Value) -> anyhow::Result<Self> {
        Self::from_dynamic(&crate::json_to_dynamic(value), Default::default())
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let value = self.to_toml_value()?;
        let text = toml::to_string_pretty(&value)?;
        std::fs::write(&path, text)
            .with_context(|| format!("writing toml to {}", path.as_ref().display()))
    }
}

#[cfg(test)]
#[test]
fn test_indexed_colors() {
    let scheme = r##"
[colors]
foreground = "#005661"
background = "#fef8ec"
cursor_bg = "#005661"
cursor_border = "#005661"
cursor_fg = "#ffffff"
selection_bg = "#cfe7f0"
selection_fg = "#005661"

ansi = [ "#8ca6a6" ,"#e64100" ,"#00b368" ,"#fa8900" ,"#0095a8" ,"#ff5792" ,"#00bdd6" ,"#005661" ]
brights = [ "#8ca6a6" ,"#e5164a" ,"#00b368" ,"#b3694d" ,"#0094f0" ,"#ff5792" ,"#00bdd6" ,"#004d57" ]

[colors.indexed]
52 = "#fbdada" # minus
88 = "#f6b6b6" # minus emph
22 = "#d6ffd6" # plus
28 = "#adffad" # plus emph
53 = "#feecf7" # purple
17 = "#e5dff6" # blue
23 = "#d8fdf6" # cyan
58 = "#f4ffe0" # yellow
"##;
    let scheme = ColorSchemeFile::from_toml_str(scheme).unwrap();
    assert_eq!(
        scheme.colors.indexed.get(&52),
        Some(&RgbColor::new_8bpc(0xfb, 0xda, 0xda).into())
    );
}
