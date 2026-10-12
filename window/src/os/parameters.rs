use wezterm_color_types::LinearRgba;
use wezterm_font::parser::ParsedFont;

use crate::ULength;

pub type FontAndSize = (ParsedFont, f64);

#[derive(Default, Clone, Debug)]
pub struct TitleBar {
    pub padding_left: ULength,
    pub padding_right: ULength,
    pub height: Option<ULength>,
    pub font_and_size: Option<FontAndSize>,
    /// Offset from the left edge of the window to the right edge of the
    /// macOS traffic light buttons (close/minimize/zoom).
    /// Value is in points (logical pixels).
    pub macos_traffic_light_offset: Option<f32>,
}

#[derive(Default, Clone, Debug)]
pub struct Border {
    pub top: ULength,
    pub left: ULength,
    pub bottom: ULength,
    pub right: ULength,
    pub color: LinearRgba,
}

#[derive(Default, Clone, Debug)]
pub struct Parameters {
    pub title_bar: TitleBar,
    /// If present, the application should draw it
    pub border_dimensions: Option<Border>,
}
