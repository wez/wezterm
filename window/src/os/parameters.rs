use wezterm_color_types::LinearRgba;
use wezterm_font::parser::ParsedFont;

use crate::Length;

pub struct TitleBar {
    pub padding_left: Length,
    pub padding_right: Length,
    pub height: Option<Length>,
    pub font_and_size: Option<(ParsedFont, f64)>,
}

pub struct Border {
    pub top: Length,
    pub left: Length,
    pub bottom: Length,
    pub right: Length,
    pub color: LinearRgba,
}

pub struct Parameters {
    pub title_bar: TitleBar,
    pub border_dimensions: Option<Border>, // If present, the application should draw it
}
