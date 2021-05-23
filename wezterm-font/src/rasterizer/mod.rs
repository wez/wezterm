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

impl RasterizedGlyph {
    /// Computes a skewed version of this glyph to produce a synthesized oblique variant
    pub fn skew(&self) -> Self {
        // This function is derived from code which is subject to the terms of the
        // Mozilla Public License, v. 2.0. http://mozilla.org/MPL/2.0/

        let factor = 0.2f64; // Skew factor
        let stride = self.width * 4;

        // Calculate the skewed horizontal offsets of the bottom and top of the glyph.
        let bottom = self.bearing_y.get().round() as f64 - self.height as f64;
        let skew_min = ((bottom + 0.5) * factor).floor();
        let skew_max = ((self.bearing_y.get() as f64 - 0.5) * factor).ceil();

        // Allocate enough extra width for the min/max skew offsets.
        let skew_width = self.width + (skew_max - skew_min) as usize;
        let mut skew_buffer = vec![0u8; skew_width * self.height * 4];
        for y in 0..self.height {
            // Calculate a skew offset at the vertical center of the current row.
            let offset = (self.bearing_y.get() - y as f64 - 0.5) * factor - skew_min;
            // Get a blend factor in 0..256 constant across all pixels in the row.
            let blend = (offset.fract() * 256.0) as u32;
            let src_row = y * stride;
            let dest_row = (y * skew_width + offset.floor() as usize) * 4;
            let mut prev_px = [0u32; 4];
            for (src, dest) in self.data[src_row..src_row + stride]
                .chunks(4)
                .zip(skew_buffer[dest_row..dest_row + stride].chunks_mut(4))
            {
                let px = [src[0] as u32, src[1] as u32, src[2] as u32, src[3] as u32];
                // Blend current pixel with previous pixel based on blend factor.
                let next_px = [px[0] * blend, px[1] * blend, px[2] * blend, px[3] * blend];
                dest[0] = ((((px[0] << 8) - next_px[0]) + prev_px[0] + 128) >> 8) as u8;
                dest[1] = ((((px[1] << 8) - next_px[1]) + prev_px[1] + 128) >> 8) as u8;
                dest[2] = ((((px[2] << 8) - next_px[2]) + prev_px[2] + 128) >> 8) as u8;
                dest[3] = ((((px[3] << 8) - next_px[3]) + prev_px[3] + 128) >> 8) as u8;
                // Save the remainder for blending onto the next pixel.
                prev_px = next_px;
            }
            // If the skew misaligns the final pixel, write out the remainder.
            if blend > 0 {
                let dest = &mut skew_buffer[dest_row + stride..dest_row + stride + 4];
                dest[0] = ((prev_px[0] + 128) >> 8) as u8;
                dest[1] = ((prev_px[1] + 128) >> 8) as u8;
                dest[2] = ((prev_px[2] + 128) >> 8) as u8;
                dest[3] = ((prev_px[3] + 128) >> 8) as u8;
            }
        }
        Self {
            data: skew_buffer,
            height: self.height,
            width: skew_width,
            bearing_x: self.bearing_x + PixelLength::new(skew_min),
            bearing_y: self.bearing_y,
            has_color: self.has_color,
        }
    }
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
