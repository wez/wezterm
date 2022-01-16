use super::glyphcache::GlyphCache;
use ::window::bitmaps::atlas::{OutOfTextureSpace, Sprite};
use ::window::bitmaps::{BitmapImage, Image, Texture2d};
use ::window::color::SrgbaPixel;
use ::window::{Point, Rect, Size};
use anyhow::Context;
use std::rc::Rc;
use wezterm_font::units::*;
use wezterm_font::{FontConfiguration, FontMetrics};

#[derive(Copy, Clone, Debug)]
pub struct RenderMetrics {
    pub descender: PixelLength,
    pub descender_row: IntPixelLength,
    pub descender_plus_two: IntPixelLength,
    pub underline_height: IntPixelLength,
    pub strike_row: IntPixelLength,
    pub cell_size: Size,
}

impl RenderMetrics {
    pub fn with_font_metrics(metrics: &FontMetrics) -> Self {
        let (cell_height, cell_width) = (
            metrics.cell_height.get().ceil() as usize,
            metrics.cell_width.get().ceil() as usize,
        );

        let underline_height = metrics.underline_thickness.get().round().max(1.) as isize;

        let descender_row =
            (cell_height as f64 + (metrics.descender - metrics.underline_position).get()) as isize;
        let descender_plus_two =
            (2 * underline_height + descender_row).min(cell_height as isize - underline_height);
        let strike_row = descender_row / 2;

        Self {
            descender: metrics.descender,
            descender_row,
            descender_plus_two,
            strike_row,
            cell_size: Size::new(cell_width as isize, cell_height as isize),
            underline_height,
        }
    }

    pub fn scale_line_height(&self, line_height: f64) -> Self {
        let size = euclid::size2(
            self.cell_size.width,
            (self.cell_size.height as f64 * line_height) as isize,
        );

        let adjust = (((self.descender_row as f64 * line_height) - self.descender_row as f64) / 2.0)
            as isize;
        Self {
            descender: self.descender - PixelLength::new(adjust as f64),
            descender_row: self.descender_row - adjust,
            descender_plus_two: self.descender_plus_two - adjust,
            underline_height: self.underline_height,
            strike_row: self.strike_row,
            cell_size: size,
        }
    }

    pub fn scale_cell_width(&self, scale: u8) -> Self {
        let mut scaled = self.clone();
        scaled.cell_size.width *= scale as isize;
        scaled
    }

    pub fn new(fonts: &Rc<FontConfiguration>) -> anyhow::Result<Self> {
        let metrics = fonts
            .default_font_metrics()
            .context("failed to get font metrics!?")?;

        let line_height = fonts.config().line_height;

        let (cell_height, cell_width) = (
            (metrics.cell_height.get() * line_height).ceil() as usize,
            metrics.cell_width.get().ceil() as usize,
        );

        // When line_height != 1.0, we want to adjust the baseline position
        // such that we are horizontally centered.
        let line_height_y_adjust = (cell_height as f64 - metrics.cell_height.get().ceil()) / 2.;

        let underline_height = metrics.underline_thickness.get().round().max(1.) as isize;

        let descender_row = (cell_height as f64
            + (metrics.descender - metrics.underline_position).get()
            - line_height_y_adjust) as isize;
        let descender_plus_two =
            (2 * underline_height + descender_row).min(cell_height as isize - underline_height);
        let strike_row = descender_row / 2;

        Ok(Self {
            descender: metrics.descender - PixelLength::new(line_height_y_adjust),
            descender_row,
            descender_plus_two,
            strike_row,
            cell_size: Size::new(cell_width as isize, cell_height as isize),
            underline_height,
        })
    }
}

pub struct UtilSprites<T: Texture2d> {
    pub white_space: Sprite<T>,
    pub filled_box: Sprite<T>,
}

impl<T: Texture2d> UtilSprites<T> {
    pub fn new(
        glyph_cache: &mut GlyphCache<T>,
        metrics: &RenderMetrics,
    ) -> Result<Self, OutOfTextureSpace> {
        let mut buffer = Image::new(
            metrics.cell_size.width as usize,
            metrics.cell_size.height as usize,
        );

        let black = SrgbaPixel::rgba(0, 0, 0, 0);
        let white = SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);

        buffer.clear_rect(cell_rect, white);
        let filled_box = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        let white_space = glyph_cache.atlas.allocate(&buffer)?;

        Ok(Self {
            white_space,
            filled_box,
        })
    }
}
