use super::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::{Atlas, Sprite};
use ::window::bitmaps::{Image, Texture2d};
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::*;
use anyhow::{anyhow, Context};
use config::{configuration, AllowSquareGlyphOverflow, TextStyle};
use euclid::num::Zero;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::image::ImageData;
use wezterm_font::units::*;
use wezterm_font::{FontConfiguration, GlyphInfo};
use wezterm_term::Underline;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_idx: usize,
    pub glyph_pos: u32,
    pub style: TextStyle,
    pub followed_by_space: bool,
}

/// We'd like to avoid allocating when resolving from the cache
/// so this is the borrowed version of GlyphKey.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BorrowedGlyphKey<'a> {
    pub font_idx: usize,
    pub glyph_pos: u32,
    pub style: &'a TextStyle,
    pub followed_by_space: bool,
}

impl<'a> BorrowedGlyphKey<'a> {
    fn to_owned(&self) -> GlyphKey {
        GlyphKey {
            font_idx: self.font_idx,
            glyph_pos: self.glyph_pos,
            style: self.style.clone(),
            followed_by_space: self.followed_by_space,
        }
    }
}

trait GlyphKeyTrait {
    fn key<'k>(&'k self) -> BorrowedGlyphKey<'k>;
}

impl GlyphKeyTrait for GlyphKey {
    fn key<'k>(&'k self) -> BorrowedGlyphKey<'k> {
        BorrowedGlyphKey {
            font_idx: self.font_idx,
            glyph_pos: self.glyph_pos,
            style: &self.style,
            followed_by_space: self.followed_by_space,
        }
    }
}

impl<'a> GlyphKeyTrait for BorrowedGlyphKey<'a> {
    fn key<'k>(&'k self) -> BorrowedGlyphKey<'k> {
        *self
    }
}

impl<'a> std::borrow::Borrow<dyn GlyphKeyTrait + 'a> for GlyphKey {
    fn borrow(&self) -> &(dyn GlyphKeyTrait + 'a) {
        self
    }
}

impl<'a> PartialEq for (dyn GlyphKeyTrait + 'a) {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(&other.key())
    }
}

impl<'a> Eq for (dyn GlyphKeyTrait + 'a) {}

impl<'a> std::hash::Hash for (dyn GlyphKeyTrait + 'a) {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}

/// Caches a rendered glyph.
/// The image data may be None for whitespace glyphs.
pub struct CachedGlyph<T: Texture2d> {
    pub has_color: bool,
    pub x_offset: PixelLength,
    pub y_offset: PixelLength,
    pub bearing_x: PixelLength,
    pub bearing_y: PixelLength,
    pub texture: Option<Sprite<T>>,
    pub scale: f64,
}

impl<T: Texture2d> std::fmt::Debug for CachedGlyph<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("CachedGlyph")
            .field("has_color", &self.has_color)
            .field("x_offset", &self.x_offset)
            .field("y_offset", &self.y_offset)
            .field("bearing_x", &self.bearing_x)
            .field("bearing_y", &self.bearing_y)
            .field("scale", &self.scale)
            .field("texture", &self.texture)
            .finish()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct LineKey {
    strike_through: bool,
    underline: Underline,
    overline: bool,
}

pub struct GlyphCache<T: Texture2d> {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph<T>>>,
    pub atlas: Atlas<T>,
    fonts: Rc<FontConfiguration>,
    image_cache: HashMap<usize, Sprite<T>>,
    line_glyphs: HashMap<LineKey, Sprite<T>>,
    metrics: RenderMetrics,
}

impl GlyphCache<SrgbTexture2d> {
    pub fn new_gl(
        backend: &Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        size: usize,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Self> {
        let surface = Rc::new(SrgbTexture2d::empty_with_format(
            backend,
            glium::texture::SrgbFormat::U8U8U8U8,
            glium::texture::MipmapsOption::NoMipmap,
            size as u32,
            size as u32,
        )?);
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Ok(Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            image_cache: HashMap::new(),
            atlas,
            metrics: metrics.clone(),
            line_glyphs: HashMap::new(),
        })
    }

    pub fn clear(&mut self) {
        self.atlas.clear();
        self.image_cache.clear();
        self.glyph_cache.clear();
        self.line_glyphs.clear();
    }
}

impl<T: Texture2d> GlyphCache<T> {
    /// Resolve a glyph from the cache, rendering the glyph on-demand if
    /// the cache doesn't already hold the desired glyph.
    pub fn cached_glyph(
        &mut self,
        info: &GlyphInfo,
        style: &TextStyle,
        followed_by_space: bool,
    ) -> anyhow::Result<Rc<CachedGlyph<T>>> {
        let key = BorrowedGlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            style,
            followed_by_space,
        };

        if let Some(entry) = self.glyph_cache.get(&key as &dyn GlyphKeyTrait) {
            return Ok(Rc::clone(entry));
        }

        let glyph = self
            .load_glyph(info, style, followed_by_space)
            .with_context(|| anyhow!("load_glyph {:?} {:?}", info, style))?;
        self.glyph_cache.insert(key.to_owned(), Rc::clone(&glyph));
        Ok(glyph)
    }

    /// Perform the load and render of a glyph
    #[allow(clippy::float_cmp)]
    fn load_glyph(
        &mut self,
        info: &GlyphInfo,
        style: &TextStyle,
        followed_by_space: bool,
    ) -> anyhow::Result<Rc<CachedGlyph<T>>> {
        let base_metrics;
        let idx_metrics;
        let glyph;

        {
            let font = self.fonts.resolve_font(style)?;
            base_metrics = font.metrics();
            glyph = font.rasterize_glyph(info.glyph_pos, info.font_idx)?;

            idx_metrics = font.metrics_for_idx(info.font_idx)?;
        }

        let y_scale = base_metrics.cell_height.get() / idx_metrics.cell_height.get();
        let x_scale =
            base_metrics.cell_width.get() / (idx_metrics.cell_width.get() / info.num_cells as f64);

        let aspect = (idx_metrics.cell_height / idx_metrics.cell_width).get();
        let is_square = aspect >= 0.9 && aspect <= 1.1;

        let allow_width_overflow = if is_square {
            match configuration().allow_square_glyphs_to_overflow_width {
                AllowSquareGlyphOverflow::Never => false,
                AllowSquareGlyphOverflow::Always => true,
                AllowSquareGlyphOverflow::WhenFollowedBySpace => followed_by_space,
            }
        } else {
            false
        };

        let scale = if !allow_width_overflow
            && y_scale * glyph.width as f64 > base_metrics.cell_width.get() * info.num_cells as f64
        {
            // y-scaling would make us too wide, so use the x-scale
            x_scale
        } else {
            y_scale
        };

        let (cell_width, cell_height) = (base_metrics.cell_width, base_metrics.cell_height);

        let glyph = if glyph.width == 0 || glyph.height == 0 {
            // a whitespace glyph
            CachedGlyph {
                has_color: glyph.has_color,
                texture: None,
                x_offset: info.x_offset * scale,
                y_offset: info.y_offset * scale,
                bearing_x: PixelLength::zero(),
                bearing_y: PixelLength::zero(),
                scale,
            }
        } else {
            let raw_im = Image::with_rgba32(
                glyph.width as usize,
                glyph.height as usize,
                4 * glyph.width as usize,
                &glyph.data,
            );

            let bearing_x = glyph.bearing_x * scale;
            let bearing_y = glyph.bearing_y * scale;
            let x_offset = info.x_offset * scale;
            let y_offset = info.y_offset * scale;

            let (scale, raw_im) = if scale != 1.0 {
                log::trace!(
                    "physically scaling {:?} by {} bcos {}x{} > {:?}x{:?}",
                    info,
                    scale,
                    glyph.width,
                    glyph.height,
                    cell_width,
                    cell_height
                );
                (1.0, raw_im.scale_by(scale))
            } else {
                (scale, raw_im)
            };

            let tex = self.atlas.allocate(&raw_im)?;

            let g = CachedGlyph {
                has_color: glyph.has_color,
                texture: Some(tex),
                x_offset,
                y_offset,
                bearing_x,
                bearing_y,
                scale,
            };

            if info.font_idx != 0 {
                // It's generally interesting to examine eg: emoji or ligatures
                // that we might have fallen back to
                log::trace!("{:?} {:?}", info, g);
            }

            g
        };

        Ok(Rc::new(glyph))
    }

    pub fn cached_image(
        &mut self,
        image_data: &Arc<ImageData>,
        padding: Option<usize>,
    ) -> anyhow::Result<Sprite<T>> {
        if let Some(sprite) = self.image_cache.get(&image_data.id()) {
            return Ok(sprite.clone());
        }

        let decoded_image = image::load_from_memory(image_data.data())?.to_bgra8();
        let (width, height) = decoded_image.dimensions();
        let image = ::window::bitmaps::Image::from_raw(
            width as usize,
            height as usize,
            decoded_image.to_vec(),
        );

        let sprite = self.atlas.allocate_with_padding(&image, padding)?;

        self.image_cache.insert(image_data.id(), sprite.clone());

        Ok(sprite)
    }

    fn line_sprite(&mut self, key: LineKey) -> anyhow::Result<Sprite<T>> {
        let mut buffer = Image::new(
            self.metrics.cell_size.width as usize,
            self.metrics.cell_size.height as usize,
        );
        let black = ::window::color::Color::rgba(0, 0, 0, 0);
        let white = ::window::color::Color::rgb(0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), self.metrics.cell_size);

        let draw_single = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + self.metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + self.metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_double = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + self.metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + self.metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + self.metrics.descender_plus_two + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + self.metrics.descender_plus_two + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_strike = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + self.metrics.strike_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + self.metrics.strike_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_overline = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                buffer.draw_line(
                    Point::new(cell_rect.origin.x, cell_rect.origin.y + row),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        buffer.clear_rect(cell_rect, black);
        if key.overline {
            draw_overline(&mut buffer);
        }
        match key.underline {
            Underline::None => {}
            Underline::Single |
                // FIXME: these extra styles need to be rendered separately!
                Underline::Curly | Underline::Dotted | Underline::Dashed => {
                draw_single(&mut buffer)
            }
            Underline::Double => draw_double(&mut buffer),
        }
        if key.strike_through {
            draw_strike(&mut buffer);
        }
        let sprite = self.atlas.allocate(&buffer)?;
        self.line_glyphs.insert(key, sprite.clone());
        Ok(sprite)
    }

    /// Figure out what we're going to draw for the underline.
    /// If the current cell is part of the current URL highlight
    /// then we want to show the underline.
    pub fn cached_line_sprite(
        &mut self,
        is_highlited_hyperlink: bool,
        is_strike_through: bool,
        underline: Underline,
        overline: bool,
    ) -> anyhow::Result<Sprite<T>> {
        let effective_underline = match (is_highlited_hyperlink, underline) {
            (true, Underline::None) => Underline::Single,
            (true, Underline::Single) => Underline::Double,
            (true, _) => Underline::Single,
            (false, u) => u,
        };

        let key = LineKey {
            strike_through: is_strike_through,
            overline,
            underline: effective_underline,
        };

        if let Some(s) = self.line_glyphs.get(&key) {
            return Ok(s.clone());
        }

        self.line_sprite(key)
    }
}
