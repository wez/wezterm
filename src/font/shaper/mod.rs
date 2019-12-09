use crate::font::system::GlyphInfo;
use failure::Fallible;

pub mod harfbuzz;

pub trait FontShaper {
    /// Shape text and return a vector of GlyphInfo
    fn shape(&self, text: &str) -> Fallible<Vec<GlyphInfo>>;
}
