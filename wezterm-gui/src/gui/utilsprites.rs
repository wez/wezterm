use super::glyphcache::GlyphCache;
use ::window::bitmaps::atlas::{OutOfTextureSpace, Sprite};
use ::window::bitmaps::{BitmapImage, Image, Texture2d};
use ::window::*;
use config::configuration;
use std::rc::Rc;
use termwiz::surface::CursorShape;
use wezterm_font::units::*;
use wezterm_font::FontConfiguration;
use wezterm_term::Underline;

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
    pub fn new(fonts: &Rc<FontConfiguration>) -> Self {
        let metrics = fonts
            .default_font_metrics()
            .expect("failed to get font metrics!?");

        let line_height = configuration().line_height;

        let (cell_height, cell_width) = (
            (metrics.cell_height.get() * line_height).ceil() as usize,
            metrics.cell_width.get().ceil() as usize,
        );

        let underline_height = metrics.underline_thickness.get().round().max(1.) as isize;

        let descender_row =
            (cell_height as f64 + (metrics.descender - metrics.underline_position).get()) as isize;
        let descender_plus_two =
            (2 * underline_height + descender_row).min(cell_height as isize - 1);
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
}

pub struct UtilSprites<T: Texture2d> {
    pub white_space: Sprite<T>,
    pub single_underline: Sprite<T>,
    pub double_underline: Sprite<T>,
    pub strike_through: Sprite<T>,
    pub single_and_strike: Sprite<T>,
    pub double_and_strike: Sprite<T>,
    pub cursor_box: Sprite<T>,
    pub cursor_i_beam: Sprite<T>,
    pub cursor_underline: Sprite<T>,
    pub overline: Sprite<T>,
    pub single_under_over: Sprite<T>,
    pub double_under_over: Sprite<T>,
    pub strike_over: Sprite<T>,
    pub single_strike_over: Sprite<T>,
    pub double_strike_over: Sprite<T>,
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

        let black = ::window::color::Color::rgba(0, 0, 0, 0);
        let white = ::window::color::Color::rgb(0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);

        buffer.clear_rect(cell_rect, black);
        let white_space = glyph_cache.atlas.allocate(&buffer)?;

        let draw_single = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_double = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_plus_two + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_plus_two + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_strike = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.strike_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.strike_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_overline = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(cell_rect.origin.x, cell_rect.origin.y + row),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        let overline = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_single(&mut buffer);
        let single_underline = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        draw_single(&mut buffer);
        let single_under_over = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_double(&mut buffer);
        let double_underline = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        draw_double(&mut buffer);
        let double_under_over = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_strike(&mut buffer);
        let strike_through = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        draw_strike(&mut buffer);
        let strike_over = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_single(&mut buffer);
        draw_strike(&mut buffer);
        let single_and_strike = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        draw_single(&mut buffer);
        draw_strike(&mut buffer);
        let single_strike_over = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_double(&mut buffer);
        draw_strike(&mut buffer);
        let double_and_strike = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_overline(&mut buffer);
        draw_double(&mut buffer);
        draw_strike(&mut buffer);
        let double_strike_over = glyph_cache.atlas.allocate(&buffer)?;

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
                Operator::Source,
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
                Operator::Source,
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
                Operator::Source,
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
                Operator::Source,
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
                Operator::Source,
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
                Operator::Source,
            );
        }
        let cursor_underline = glyph_cache.atlas.allocate(&buffer)?;

        Ok(Self {
            white_space,
            single_underline,
            double_underline,
            strike_through,
            single_and_strike,
            double_and_strike,
            cursor_box,
            cursor_i_beam,
            cursor_underline,
            overline,
            single_under_over,
            double_under_over,
            strike_over,
            single_strike_over,
            double_strike_over,
        })
    }

    /// Figure out what we're going to draw for the underline.
    /// If the current cell is part of the current URL highlight
    /// then we want to show the underline.
    pub fn select_sprite(
        &self,
        is_highlited_hyperlink: bool,
        is_strike_through: bool,
        underline: Underline,
        overline: bool,
    ) -> &Sprite<T> {
        match (
            is_highlited_hyperlink,
            is_strike_through,
            underline,
            overline,
        ) {
            (true, false, Underline::None, false) => &self.single_underline,
            (true, false, Underline::Single, false) => &self.double_underline,
            (true, false, Underline::Double, false) => &self.single_underline,
            (true, true, Underline::None, false) => &self.strike_through,
            (true, true, Underline::Single, false) => &self.single_and_strike,
            (true, true, Underline::Double, false) => &self.double_and_strike,
            (false, false, Underline::None, false) => &self.white_space,
            (false, false, Underline::Single, false) => &self.single_underline,
            (false, false, Underline::Double, false) => &self.double_underline,
            (false, true, Underline::None, false) => &self.strike_through,
            (false, true, Underline::Single, false) => &self.single_and_strike,
            (false, true, Underline::Double, false) => &self.double_and_strike,

            (true, false, Underline::None, true) => &self.single_under_over,
            (true, false, Underline::Single, true) => &self.double_under_over,
            (true, false, Underline::Double, true) => &self.single_under_over,
            (true, true, Underline::None, true) => &self.strike_over,
            (true, true, Underline::Single, true) => &self.single_strike_over,
            (true, true, Underline::Double, true) => &self.double_strike_over,
            (false, false, Underline::None, true) => &self.overline,
            (false, false, Underline::Single, true) => &self.single_under_over,
            (false, false, Underline::Double, true) => &self.double_under_over,
            (false, true, Underline::None, true) => &self.strike_over,
            (false, true, Underline::Single, true) => &self.single_strike_over,
            (false, true, Underline::Double, true) => &self.double_strike_over,

            // FIXME: these are just placeholders under we render
            // these things properly
            (_, _, Underline::Curly, _) => &self.double_underline,
            (_, _, Underline::Dotted, _) => &self.double_underline,
            (_, _, Underline::Dashed, _) => &self.double_underline,
        }
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
