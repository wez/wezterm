use config::{Config, TextStyle};
use failure::{self, Error};
use font::{
    shape_with_harfbuzz, FallbackIdx, Font, FontMetrics, FontSystem, GlyphInfo, NamedFont,
    RasterizedGlyph,
};

pub type FontSystemImpl = WindowsFonts;
pub struct WindowsFonts {}
impl WindowsFonts {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for WindowsFonts {
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        unimplemented!();
    }
}
