use crate::parser::ParsedFont;
use crate::units::*;
use config::FontRasterizerSelection;

pub mod freetype;

/// A bitmap representation of a glyph.
/// The data is stored as pre-multiplied RGBA 32bpp.
#[derive(Debug)]
pub struct RasterizedGlyph {
    pub data: Vec<u8>,
    pub height: usize,
    pub width: usize,
    pub bearing_x: PixelLength,
    pub bearing_y: PixelLength,
    pub has_color: bool,
}

/// Rasterizes the specified glyph index in the associated font
/// and returns the generated bitmap
pub trait FontRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph>;
}

pub fn new_rasterizer(
    rasterizer: FontRasterizerSelection,
    handle: &ParsedFont,
) -> anyhow::Result<Box<dyn FontRasterizer>> {
    match rasterizer {
        FontRasterizerSelection::FreeType => Ok(Box::new(
            freetype::FreeTypeRasterizer::from_locator(handle)?,
        )),
    }
}
