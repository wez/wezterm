use crate::parser::ParsedFont;
use crate::units::*;
use config::FontRasterizerSelection;

/// The amount, as a number in [0,1], to horizontally skew a glyph when rendering synthetic
/// italics
pub(crate) const FAKE_ITALIC_SKEW: f64 = 0.2;

pub mod freetype;
pub mod harfbuzz;

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
    pixel_geometry: config::DisplayPixelGeometry,
) -> anyhow::Result<Box<dyn FontRasterizer>> {
    match rasterizer {
        FontRasterizerSelection::FreeType => Ok(Box::new(
            freetype::FreeTypeRasterizer::from_locator(handle, pixel_geometry)?,
        )),
        FontRasterizerSelection::Harfbuzz => Ok(Box::new(
            harfbuzz::HarfbuzzRasterizer::from_locator(handle)?,
        )),
    }
}
