use wezterm_dynamic::{FromDynamic, ToDynamic};

/// <https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function>
#[derive(Debug, Clone, Copy, FromDynamic, ToDynamic)]
pub enum EasingFunction {
    Linear,
    CubicBezier(f32, f32, f32, f32),
    Ease,
    EaseIn,
    EaseInOut,
    EaseOut,
    Constant,
}

impl EasingFunction {
    pub fn evaluate_at_position(&self, position: f32) -> f32 {
        fn cubic_bezier(p0: f32, p1: f32, p2: f32, p3: f32, x: f32) -> f32 {
            (1.0 - x).powi(3) * p0
                + 3.0 * (1.0 - x).powi(2) * x * p1
                + 3.0 * (1.0 - x) * x.powi(2) * p2
                + x.powi(3) * p3
        }

        match self {
            Self::Constant => 0.,
            Self::Linear => cubic_bezier(0., 0., 1.0, 1.0, position),
            Self::CubicBezier(a, b, c, d) => cubic_bezier(*a, *b, *c, *d, position),
            Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, position),
            Self::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, position),
            Self::EaseInOut => cubic_bezier(0.42, 0., 0.58, 1.0, position),
            Self::EaseOut => cubic_bezier(0., 0., 0.58, 1.0, position),
        }
    }
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Ease
    }
}

#[derive(Default, Debug, Clone, FromDynamic, ToDynamic)]
pub struct VisualBell {
    #[dynamic(default)]
    pub fade_in_duration_ms: u64,
    #[dynamic(default)]
    pub fade_in_function: EasingFunction,
    #[dynamic(default)]
    pub fade_out_duration_ms: u64,
    #[dynamic(default)]
    pub fade_out_function: EasingFunction,
    #[dynamic(default)]
    pub target: VisualBellTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, FromDynamic, ToDynamic)]
pub enum VisualBellTarget {
    BackgroundColor,
    CursorColor,
}

impl Default for VisualBellTarget {
    fn default() -> VisualBellTarget {
        Self::BackgroundColor
    }
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub enum AudibleBell {
    SystemBeep,
    Disabled,
}

impl Default for AudibleBell {
    fn default() -> AudibleBell {
        Self::SystemBeep
    }
}
