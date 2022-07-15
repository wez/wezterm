use crate::*;
use luahelper::impl_lua_conversion_dynamic;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use termwiz::cell::CellAttributes;
pub use termwiz::color::{ColorSpec, RgbColor, SrgbaTuple};
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

impl std::ops::Deref for RgbaColor {
    type Target = SrgbaTuple;
    fn deref(&self) -> &SrgbaTuple {
        &self.color
    }
}

impl Into<String> for &RgbaColor {
    fn into(self) -> String {
        self.color.to_string()
    }
}

impl Into<String> for RgbaColor {
    fn into(self) -> String {
        self.color.to_string()
    }
}

impl Into<SrgbaTuple> for RgbaColor {
    fn into(self) -> SrgbaTuple {
        self.color
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
}
impl_lua_conversion_dynamic!(Palette);

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
    pub bg_color: RgbColor,
    /// The forgeground/text color for the tab
    pub fg_color: RgbColor,
}

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
#[derive(Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
pub struct TabBarColors {
    /// The background color for the tab bar
    #[dynamic(default = "default_background")]
    pub background: RgbColor,

    /// Styling for the active tab
    #[dynamic(default = "default_active_tab")]
    pub active_tab: TabBarColor,

    /// Styling for other inactive tabs
    #[dynamic(default = "default_inactive_tab")]
    pub inactive_tab: TabBarColor,

    /// Styling for an inactive tab with a mouse hovering
    #[dynamic(default = "default_inactive_tab_hover")]
    pub inactive_tab_hover: TabBarColor,

    /// Styling for the new tab button
    #[dynamic(default = "default_inactive_tab")]
    pub new_tab: TabBarColor,

    /// Styling for the new tab button with a mouse hovering
    #[dynamic(default = "default_inactive_tab_hover")]
    pub new_tab_hover: TabBarColor,

    #[dynamic(default = "default_inactive_tab_edge")]
    pub inactive_tab_edge: RgbaColor,

    #[dynamic(default = "default_inactive_tab_edge_hover")]
    pub inactive_tab_edge_hover: RgbaColor,
}

fn default_background() -> RgbColor {
    RgbColor::new_8bpc(0x33, 0x33, 0x33)
}

fn default_inactive_tab_edge() -> RgbaColor {
    RgbColor::new_8bpc(0x57, 0x57, 0x57).into()
}

fn default_inactive_tab_edge_hover() -> RgbaColor {
    RgbColor::new_8bpc(0x36, 0x36, 0x36).into()
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

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct TabBarStyle {
    #[dynamic(default = "default_new_tab")]
    pub new_tab: String,
    #[dynamic(default = "default_new_tab")]
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
    " + ".to_string()
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct WindowFrameConfig {
    #[dynamic(default = "default_inactive_titlebar_bg")]
    pub inactive_titlebar_bg: RgbColor,
    #[dynamic(default = "default_active_titlebar_bg")]
    pub active_titlebar_bg: RgbColor,
    #[dynamic(default = "default_inactive_titlebar_fg")]
    pub inactive_titlebar_fg: RgbColor,
    #[dynamic(default = "default_active_titlebar_fg")]
    pub active_titlebar_fg: RgbColor,
    #[dynamic(default = "default_inactive_titlebar_border_bottom")]
    pub inactive_titlebar_border_bottom: RgbColor,
    #[dynamic(default = "default_active_titlebar_border_bottom")]
    pub active_titlebar_border_bottom: RgbColor,
    #[dynamic(default = "default_button_fg")]
    pub button_fg: RgbColor,
    #[dynamic(default = "default_button_bg")]
    pub button_bg: RgbColor,
    #[dynamic(default = "default_button_hover_fg")]
    pub button_hover_fg: RgbColor,
    #[dynamic(default = "default_button_hover_bg")]
    pub button_hover_bg: RgbColor,

    #[dynamic(default)]
    pub font: Option<TextStyle>,
    #[dynamic(default)]
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

#[derive(Debug, Default, Clone, Eq, PartialEq, FromDynamic, ToDynamic)]
pub struct ColorSchemeMetaData {
    pub name: Option<String>,
    pub author: Option<String>,
    pub origin_url: Option<String>,
    pub wezterm_version: Option<String>,
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
            let mut map = toml::value::Map::new();
            for (k, v) in o {
                let k = match k {
                    Value::String(s) => s,
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
        Self::from_dynamic(&crate::toml_to_dynamic(value), Default::default())
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn from_toml_str(s: &str) -> anyhow::Result<Self> {
        let scheme: toml::Value = toml::from_str(s)?;
        let mut scheme = ColorSchemeFile::from_toml_value(&scheme)?;

        // Little hack to extract comment style metadata from
        // iTerm2-Color-Schemes generated toml files
        if scheme.metadata.name.is_none() {
            if let Some(first_line) = s.lines().next() {
                if let Some(name) = first_line.strip_prefix("# ") {
                    scheme.metadata.name.replace(name.to_string());
                }
            }
        }
        Ok(scheme)
    }

    pub fn to_toml_value(&self) -> anyhow::Result<toml::Value> {
        let value = self.to_dynamic();
        Ok(dynamic_to_toml(value)?)
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
