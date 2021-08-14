use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum GradientOrientation {
    Horizontal,
    Vertical,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gradient {
    #[serde(default)]
    pub orientation: GradientOrientation,

    pub colors: Vec<String>,

    #[serde(default)]
    pub interpolation: Interpolation,

    #[serde(default)]
    pub blend: BlendMode,

    #[serde(default)]
    pub segment_size: Option<usize>,

    #[serde(default)]
    pub segment_smoothness: Option<f64>,

    #[serde(default)]
    pub noise: Option<usize>,
}

impl Gradient {
    pub fn build(&self) -> anyhow::Result<colorgrad::Gradient> {
        use colorgrad::{BlendMode as CGMode, Interpolation as CGInterp};
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
        let g = g.build()?;
        match (self.segment_size, self.segment_smoothness) {
            (Some(size), Some(smoothness)) => Ok(g.sharp(size, smoothness)),
            (None, None) => Ok(g),
            _ => anyhow::bail!(
                "Gradient must either specify both segment_size and segment_smoothness, or neither"
            ),
        }
    }
}
