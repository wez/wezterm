use super::glyphcache::GlyphCache;
use ::window::bitmaps::atlas::{OutOfTextureSpace, Sprite};
use ::window::bitmaps::{BitmapImage, Image, Texture2d};
use ::window::color::SrgbaPixel;
use ::window::{Point, Rect, Size};
use anyhow::Context;
use config::configuration;
use std::rc::Rc;
use termwiz::surface::CursorShape;
use wezterm_font::units::*;
use wezterm_font::FontConfiguration;

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
    pub fn new(fonts: &Rc<FontConfiguration>) -> anyhow::Result<Self> {
        let metrics = fonts
            .default_font_metrics()
            .context("failed to get font metrics!?")?;

        let line_height = configuration().line_height;

        let (cell_height, cell_width) = (
            (metrics.cell_height.get() * line_height).ceil() as usize,
            metrics.cell_width.get().ceil() as usize,
        );

        // When line_height != 1.0, we want to adjust the baseline position
        // such that we are horizontally centered.
        let line_height_y_adjust = (cell_height as f64 - metrics.cell_height.get()) / 2.;

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
    pub cursor_box: Sprite<T>,
    pub cursor_i_beam: Sprite<T>,
    pub cursor_underline: Sprite<T>,
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

        buffer.clear_rect(cell_rect, black);
        let white_space = glyph_cache.atlas.allocate(&buffer)?;

        // Derive a width for the border box from the underline height,
        // but aspect ratio adjusted for width.
        let border_width = (metrics.underline_height as f64 * metrics.cell_size.width as f64
            / metrics.cell_size.height as f64)
            .ceil() as usize;

        buffer.clear_rect(cell_rect, black);
        for i in 0..metrics.underline_height {
            // Top border
            buffer.draw_line(
                Point::new(cell_rect.origin.x, cell_rect.origin.y + i),
                Point::new(
                    cell_rect.origin.x + metrics.cell_size.width,
                    cell_rect.origin.y + i,
                ),
                white,
            );
            // Bottom border
            buffer.draw_line(
                Point::new(
                    cell_rect.origin.x,
                    cell_rect.origin.y + metrics.cell_size.height.saturating_sub(1 + i),
                ),
                Point::new(
                    cell_rect.origin.x + metrics.cell_size.width,
                    cell_rect.origin.y + metrics.cell_size.height.saturating_sub(1 + i),
                ),
                white,
            );
        }
        for i in 0..border_width {
            // Left border
            buffer.draw_line(
                Point::new(cell_rect.origin.x + i as isize, cell_rect.origin.y),
                Point::new(
                    cell_rect.origin.x + i as isize,
                    cell_rect.origin.y + metrics.cell_size.height,
                ),
                white,
            );
            // Right border
            buffer.draw_line(
                Point::new(
                    cell_rect.origin.x + metrics.cell_size.width.saturating_sub(1 + i as isize),
                    cell_rect.origin.y,
                ),
                Point::new(
                    cell_rect.origin.x + metrics.cell_size.width.saturating_sub(1 + i as isize),
                    cell_rect.origin.y + metrics.cell_size.height,
                ),
                white,
            );
        }
        let cursor_box = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        for i in 0..border_width * 2 {
            // Left border
            buffer.draw_line(
                Point::new(cell_rect.origin.x + i as isize, cell_rect.origin.y),
                Point::new(
                    cell_rect.origin.x + i as isize,
                    cell_rect.origin.y + metrics.cell_size.height,
                ),
                white,
            );
        }
        let cursor_i_beam = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        for i in 0..metrics.underline_height {
            // Bottom border
            buffer.draw_line(
                Point::new(
                    cell_rect.origin.x,
                    cell_rect.origin.y + metrics.cell_size.height.saturating_sub(1 + i),
                ),
                Point::new(
                    cell_rect.origin.x + metrics.cell_size.width,
                    cell_rect.origin.y + metrics.cell_size.height.saturating_sub(1 + i),
                ),
                white,
            );
        }
        let cursor_underline = glyph_cache.atlas.allocate(&buffer)?;

        Ok(Self {
            white_space,
            cursor_box,
            cursor_i_beam,
            cursor_underline,
        })
    }

    pub fn cursor_sprite(&self, shape: Option<CursorShape>) -> &Sprite<T> {
        match shape {
            None => &self.white_space,
            Some(shape) => match shape {
                CursorShape::Default => &self.white_space,
                CursorShape::BlinkingBlock | CursorShape::SteadyBlock => &self.cursor_box,
                CursorShape::BlinkingBar | CursorShape::SteadyBar => &self.cursor_i_beam,
                CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline => {
                    &self.cursor_underline
                }
            },
        }
    }
}
