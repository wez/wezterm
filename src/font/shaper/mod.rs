use crate::font::system::GlyphInfo;
use failure::Fallible;

pub mod harfbuzz;

pub trait FontShaper {
    /// Shape text and return a vector of GlyphInfo
    fn shape(&self, text: &str, size: f64, dpi: u32) -> Fallible<Vec<GlyphInfo>>;
}
