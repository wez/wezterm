use crate::{default_one_point_oh, Config, HsbTransform};
use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundSource {
    Gradient(Gradient),
    File(String),
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct BackgroundLayer {
    pub source: BackgroundSource,

    /// Where the top left corner of the background begins
    #[dynamic(default)]
    pub origin: BackgroundOrigin,

    #[dynamic(default)]
    pub attachment: BackgroundAttachment,

    #[dynamic(default)]
    pub repeat_x: BackgroundRepeat,

    #[dynamic(default)]
    pub repeat_y: BackgroundRepeat,

    #[dynamic(default)]
    pub vertical_align: BackgroundVerticalAlignment,

    #[dynamic(default)]
    pub horizontal_align: BackgroundHorizontalAlignment,

    /// Additional alpha modifier
    #[dynamic(default = "default_one_point_oh")]
    pub opacity: f32,

    /// Additional hsb transform
    #[dynamic(default)]
    pub hsb: HsbTransform,

    #[dynamic(default)]
    pub width: BackgroundSize,

    #[dynamic(default)]
    pub height: BackgroundSize,
}

impl BackgroundLayer {
    pub fn with_legacy(cfg: &Config) -> Option<Self> {
        let source = if let Some(gradient) = &cfg.window_background_gradient {
            BackgroundSource::Gradient(gradient.clone())
        } else if let Some(path) = &cfg.window_background_image {
            BackgroundSource::File(path.to_string_lossy().to_string())
        } else {
            return None;
        };
        Some(BackgroundLayer {
            source,
            opacity: cfg.window_background_opacity,
            hsb: cfg.window_background_image_hsb.unwrap_or_default(),
            origin: Default::default(),
            attachment: Default::default(),
            repeat_x: Default::default(),
            repeat_y: Default::default(),
            vertical_align: Default::default(),
            horizontal_align: Default::default(),
            width: BackgroundSize::Percent(100),
            height: BackgroundSize::Percent(100),
        })
    }
}

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/background-size>
#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundSize {
    /// Scales image as large as possible without cropping or stretching.
    /// If the container is larger than the image, tiles the image unless
    /// the correspond `repeat` is NoRepeat.
    Contain,
    /// Scale the image (preserving aspect ratio) to the smallest possible
    /// size to the fill the container leaving no empty space.
    /// If the aspect ratio differs from the background, the image is
    /// cropped.
    Cover,
    /// Stretches the image to the specified length in pixels
    Length(u32),
    /// Stretches the image to a percentage of the background size
    /// as determined by the `origin` property.
    Percent(u8),
}

impl Default for BackgroundSize {
    fn default() -> Self {
        Self::Cover
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundHorizontalAlignment {
    Left,
    Center,
    Right,
}

impl Default for BackgroundHorizontalAlignment {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundVerticalAlignment {
    Top,
    Middle,
    Bottom,
}

impl Default for BackgroundVerticalAlignment {
    fn default() -> Self {
        Self::Top
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic, PartialEq, Eq)]
pub enum BackgroundRepeat {
    /// Repeat as much as possible to cover the area.
    /// The last image will be clipped if it doesn't fit.
    Repeat,
    /*
    /// Repeat as much as possible without clipping.
    /// The first and last images are aligned with the edges,
    /// with any gaps being distributed evenly between
    /// the images.
    /// The `position` property is ignored unless only
    /// a single image an be displayed without clipping.
    /// Clipping will only occur when there isn't enough
    /// room to display a single image.
    Space,
    /// As the available space increases, the images will
    /// stretch until there is room (space >= 50% of image
    /// size) for another one to be added. When adding a
    /// new image, the current images compress to allow
    /// room.
    Round,
    */
    /// The image is not repeated.
    /// The position of the image is defined by the
    /// `position` property
    NoRepeat,
}

impl Default for BackgroundRepeat {
    fn default() -> Self {
        Self::Repeat
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundAttachment {
    Fixed,
    Scroll,
}

impl Default for BackgroundAttachment {
    fn default() -> Self {
        Self::Fixed
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BackgroundOrigin {
    BorderBox,
    PaddingBox,
}

impl Default for BackgroundOrigin {
    fn default() -> Self {
        Self::BorderBox
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum Interpolation {
    Linear,
    Basis,
    CatmullRom,
}

impl Default for Interpolation {
    fn default() -> Self {
        Interpolation::Linear
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum BlendMode {
    Rgb,
    LinearRgb,
    Hsv,
    Oklab,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Rgb
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum GradientOrientation {
    Horizontal,
    Vertical,
    Linear {
        angle: Option<f64>,
    },
    Radial {
        radius: Option<f64>,
        cx: Option<f64>,
        cy: Option<f64>,
    },
}

impl Default for GradientOrientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic)]
pub enum GradientPreset {
    Blues,
    BrBg,
    BuGn,
    BuPu,
    Cividis,
    Cool,
    CubeHelixDefault,
    GnBu,
    Greens,
    Greys,
    Inferno,
    Magma,
    OrRd,
    Oranges,
    PiYg,
    Plasma,
    PrGn,
    PuBu,
    PuBuGn,
    PuOr,
    PuRd,
    Purples,
    Rainbow,
    RdBu,
    RdGy,
    RdPu,
    RdYlBu,
    RdYlGn,
    Reds,
    Sinebow,
    Spectral,
    Turbo,
    Viridis,
    Warm,
    YlGn,
    YlGnBu,
    YlOrBr,
    YlOrRd,
}

impl GradientPreset {
    fn build(self) -> colorgrad::Gradient {
        match self {
            Self::Blues => colorgrad::blues(),
            Self::BrBg => colorgrad::br_bg(),
            Self::BuGn => colorgrad::bu_gn(),
            Self::BuPu => colorgrad::bu_pu(),
            Self::Cividis => colorgrad::cividis(),
            Self::Cool => colorgrad::cool(),
            Self::CubeHelixDefault => colorgrad::cubehelix_default(),
            Self::GnBu => colorgrad::gn_bu(),
            Self::Greens => colorgrad::greens(),
            Self::Greys => colorgrad::greys(),
            Self::Inferno => colorgrad::inferno(),
            Self::Magma => colorgrad::magma(),
            Self::OrRd => colorgrad::or_rd(),
            Self::Oranges => colorgrad::oranges(),
            Self::PiYg => colorgrad::pi_yg(),
            Self::Plasma => colorgrad::plasma(),
            Self::PrGn => colorgrad::pr_gn(),
            Self::PuBu => colorgrad::pu_bu(),
            Self::PuBuGn => colorgrad::pu_bu_gn(),
            Self::PuOr => colorgrad::pu_or(),
            Self::PuRd => colorgrad::pu_rd(),
            Self::Purples => colorgrad::purples(),
            Self::Rainbow => colorgrad::rainbow(),
            Self::RdBu => colorgrad::rd_bu(),
            Self::RdGy => colorgrad::rd_gy(),
            Self::RdPu => colorgrad::rd_pu(),
            Self::RdYlBu => colorgrad::rd_yl_bu(),
            Self::RdYlGn => colorgrad::rd_yl_gn(),
            Self::Reds => colorgrad::reds(),
            Self::Sinebow => colorgrad::sinebow(),
            Self::Spectral => colorgrad::spectral(),
            Self::Turbo => colorgrad::turbo(),
            Self::Viridis => colorgrad::viridis(),
            Self::Warm => colorgrad::warm(),
            Self::YlGn => colorgrad::yl_gn(),
            Self::YlGnBu => colorgrad::yl_gn_bu(),
            Self::YlOrBr => colorgrad::yl_or_br(),
            Self::YlOrRd => colorgrad::yl_or_rd(),
        }
    }
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct Gradient {
    #[dynamic(default)]
    pub orientation: GradientOrientation,

    #[dynamic(default)]
    pub colors: Vec<String>,

    #[dynamic(default)]
    pub preset: Option<GradientPreset>,

    #[dynamic(default)]
    pub interpolation: Interpolation,

    #[dynamic(default)]
    pub blend: BlendMode,

    #[dynamic(default)]
    pub segment_size: Option<usize>,

    #[dynamic(default)]
    pub segment_smoothness: Option<f64>,

    #[dynamic(default)]
    pub noise: Option<usize>,
}
impl_lua_conversion_dynamic!(Gradient);

impl Gradient {
    pub fn build(&self) -> anyhow::Result<colorgrad::Gradient> {
        use colorgrad::{BlendMode as CGMode, Interpolation as CGInterp};
        let g = match &self.preset {
            Some(p) => p.build(),
            None => {
                let colors: Vec<&str> = self.colors.iter().map(|s| s.as_str()).collect();
                let mut g = colorgrad::CustomGradient::new();
                g.html_colors(&colors);
                g.mode(match self.blend {
                    BlendMode::Rgb => CGMode::Rgb,
                    BlendMode::LinearRgb => CGMode::LinearRgb,
                    BlendMode::Hsv => CGMode::Hsv,
                    BlendMode::Oklab => CGMode::Oklab,
                });
                g.interpolation(match self.interpolation {
                    Interpolation::Linear => CGInterp::Linear,
                    Interpolation::Basis => CGInterp::Basis,
                    Interpolation::CatmullRom => CGInterp::CatmullRom,
                });
                g.build()?
            }
        };
        match (self.segment_size, self.segment_smoothness) {
            (Some(size), Some(smoothness)) => Ok(g.sharp(size, smoothness)),
            (None, None) => Ok(g),
            _ => anyhow::bail!(
                "Gradient must either specify both segment_size and segment_smoothness, or neither"
            ),
        }
    }
}
