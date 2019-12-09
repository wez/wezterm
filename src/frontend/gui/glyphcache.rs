use crate::config::TextStyle;
use crate::font::{FontConfiguration, GlyphInfo};
use ::window::bitmaps::atlas::{Atlas, Sprite};
use ::window::bitmaps::{Image, ImageTexture, Texture2d};
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::*;
use failure::Fallible;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::image::ImageData;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_idx: usize,
    pub glyph_pos: u32,
    pub style: TextStyle,
}

/// Caches a rendered glyph.
/// The image data may be None for whitespace glyphs.
pub struct CachedGlyph<T: Texture2d> {
    pub has_color: bool,
    pub x_offset: f64,
    pub y_offset: f64,
    pub bearing_x: f64,
    pub bearing_y: f64,
    pub texture: Option<Sprite<T>>,
    pub scale: f64,
}

pub struct GlyphCache<T: Texture2d> {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph<T>>>,
    pub atlas: Atlas<T>,
    fonts: Rc<FontConfiguration>,
    image_cache: HashMap<usize, Sprite<T>>,
}

impl GlyphCache<ImageTexture> {
    pub fn new(fonts: &Rc<FontConfiguration>, size: usize) -> Self {
        let surface = Rc::new(ImageTexture::new(size, size));
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            image_cache: HashMap::new(),
            atlas,
        }
    }
}

impl GlyphCache<SrgbTexture2d> {
    pub fn new_gl(
        backend: &Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        size: usize,
    ) -> Fallible<Self> {
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
        })
    }
}

impl<T: Texture2d> GlyphCache<T> {
    /// Resolve a glyph from the cache, rendering the glyph on-demand if
    /// the cache doesn't already hold the desired glyph.
    pub fn cached_glyph(
        &mut self,
        info: &GlyphInfo,
        style: &TextStyle,
    ) -> Fallible<Rc<CachedGlyph<T>>> {
        let key = GlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            style: style.clone(),
        };

        if let Some(entry) = self.glyph_cache.get(&key) {
            return Ok(Rc::clone(entry));
        }

        let glyph = self.load_glyph(info, style)?;
        self.glyph_cache.insert(key, Rc::clone(&glyph));
        Ok(glyph)
    }

    /// Perform the load and render of a glyph
    #[allow(clippy::float_cmp)]
    fn load_glyph(&mut self, info: &GlyphInfo, style: &TextStyle) -> Fallible<Rc<CachedGlyph<T>>> {
        let metrics;
        let glyph;

        {
            let font = self.fonts.resolve_font(style)?;
            metrics = font.metrics();
            glyph = font.rasterize_glyph(info.glyph_pos, info.font_idx)?;
        }
        let (cell_width, cell_height) = (metrics.cell_width, metrics.cell_height);

        let scale = if (info.x_advance / f64::from(info.num_cells)).floor() > cell_width {
            f64::from(info.num_cells) * (cell_width / info.x_advance)
        } else if glyph.height as f64 > cell_height {
            cell_height / glyph.height as f64
        } else {
            1.0f64
        };
        let glyph = if glyph.width == 0 || glyph.height == 0 {
            // a whitespace glyph
            CachedGlyph {
                has_color: glyph.has_color,
                texture: None,
                x_offset: info.x_offset * scale,
                y_offset: info.y_offset * scale,
                bearing_x: 0.0,
                bearing_y: 0.0,
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
                (1.0, raw_im.scale_by(scale))
            } else {
                (scale, raw_im)
            };

            let tex = self.atlas.allocate(&raw_im)?;

            CachedGlyph {
                has_color: glyph.has_color,
                texture: Some(tex),
                x_offset,
                y_offset,
                bearing_x,
                bearing_y,
                scale,
            }
        };

        Ok(Rc::new(glyph))
    }

    pub fn cached_image(&mut self, image_data: &Arc<ImageData>) -> Fallible<Sprite<T>> {
        if let Some(sprite) = self.image_cache.get(&image_data.id()) {
            return Ok(sprite.clone());
        }

        let decoded_image = image::load_from_memory(image_data.data())?.to_bgra();
        let (width, height) = decoded_image.dimensions();
        let image = ::window::bitmaps::Image::from_raw(
            width as usize,
            height as usize,
            decoded_image.to_vec(),
        );

        let sprite = self.atlas.allocate(&image)?;

        self.image_cache.insert(image_data.id(), sprite.clone());

        Ok(sprite)
    }
}
