use crate::parser::ParsedFont;
use crate::units::*;
use config::FontRasterizerSelection;
use image::{ImageBuffer, Rgba};

/// The amount, as a number in [0,1], to horizontally skew a glyph when rendering synthetic
/// italics
pub(crate) const FAKE_ITALIC_SKEW: f64 = 0.2;

pub mod colr;
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
    /// if true, glyphcache shouldn't need to scale the
    /// glyph to match metrics
    pub is_scaled: bool,
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

pub(crate) fn swap_red_and_blue<Container: std::ops::Deref<Target = [u8]> + std::ops::DerefMut>(
    image: &mut ImageBuffer<Rgba<u8>, Container>,
) {
    for pixel in image.pixels_mut() {
        let red = pixel[0];
        pixel[0] = pixel[2];
        pixel[2] = red;
    }
}

pub(crate) fn crop_to_non_transparent<'a, Container>(
    image: &'a mut image::ImageBuffer<Rgba<u8>, Container>,
) -> image::SubImage<&'a mut ImageBuffer<Rgba<u8>, Container>>
where
    Container: std::ops::Deref<Target = [u8]>,
{
    let width = image.width();
    let height = image.height();

    let mut first_line = None;
    let mut first_col = None;
    let mut last_col = None;
    let mut last_line = None;

    for (y, row) in image.rows().enumerate() {
        for (x, pixel) in row.enumerate() {
            let alpha = pixel[3];
            if alpha != 0 {
                if first_line.is_none() {
                    first_line = Some(y);
                }
                first_col = match first_col.take() {
                    Some(other) if x < other => Some(x),
                    Some(other) => Some(other),
                    None => Some(x),
                };
            }
        }
    }
    for (y, row) in image.rows().enumerate().rev() {
        for (x, pixel) in row.enumerate().rev() {
            let alpha = pixel[3];
            if alpha != 0 {
                if last_line.is_none() {
                    last_line = Some(y);
                }
                last_col = match last_col.take() {
                    Some(other) if x > other => Some(x),
                    Some(other) => Some(other),
                    None => Some(x),
                };
            }
        }
    }

    let first_col = first_col.unwrap_or(0) as u32;
    let first_line = first_line.unwrap_or(0) as u32;
    let last_col = last_col.unwrap_or(width as usize) as u32;
    let last_line = last_line.unwrap_or(height as usize) as u32;

    image::imageops::crop(
        image,
        first_col,
        first_line,
        last_col - first_col,
        last_line - first_line,
    )
}
