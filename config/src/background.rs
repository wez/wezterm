use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

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
