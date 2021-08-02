use super::utilsprites::RenderMetrics;
use crate::cache::LruCache;
use crate::customglyph::*;
use ::window::bitmaps::atlas::{Atlas, OutOfTextureSpace, Sprite};
#[cfg(test)]
use ::window::bitmaps::ImageTexture;
use ::window::bitmaps::{BitmapImage, Image, Texture2d};
use ::window::color::SrgbaPixel;
use ::window::glium;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::CapabilitiesSource;
use ::window::{Point, Rect};
use anyhow::Context;
use config::{AllowSquareGlyphOverflow, TextStyle};
use euclid::num::Zero;
use std::collections::HashMap;
use std::convert::TryInto;
use std::rc::Rc;
use std::sync::{Arc, MutexGuard};
use std::time::Instant;
use termwiz::image::{ImageData, ImageDataType};
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
    pub brightness_adjust: f32,
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

/// A helper struct to implement BitmapImage for ImageDataType while
/// holding the mutex for the sake of safety.
struct DecodedImageHandle<'a> {
    current_frame: usize,
    h: MutexGuard<'a, ImageDataType>,
}

impl<'a> BitmapImage for DecodedImageHandle<'a> {
    unsafe fn pixel_data(&self) -> *const u8 {
        match &*self.h {
            ImageDataType::Rgba8 { data, .. } => data.as_ptr(),
            ImageDataType::AnimRgba8 { frames, .. } => frames[self.current_frame].as_ptr(),
            ImageDataType::EncodedFile(_) => unreachable!(),
        }
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        panic!("cannot mutate DecodedImage");
    }

    fn image_dimensions(&self) -> (usize, usize) {
        match &*self.h {
            ImageDataType::Rgba8 { width, height, .. }
            | ImageDataType::AnimRgba8 { width, height, .. } => (*width as usize, *height as usize),
            ImageDataType::EncodedFile(_) => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct DecodedImage {
    frame_start: Instant,
    current_frame: usize,
    image: Arc<ImageData>,
}

impl DecodedImage {
    fn placeholder() -> Self {
        let image = ImageData::with_data(ImageDataType::Rgba8 {
            // A single black pixel
            data: vec![0, 0, 0, 0],
            width: 1,
            height: 1,
        });
        Self {
            frame_start: Instant::now(),
            current_frame: 0,
            image: Arc::new(image),
        }
    }

    fn load(image_data: &Arc<ImageData>) -> Self {
        match &*image_data.data() {
            ImageDataType::EncodedFile(_) => {
                log::warn!("Unexpected ImageDataType::EncodedFile; either file is unreadable or we missed a .decode call somewhere");
                Self::placeholder()
            }
            _ => Self {
                frame_start: Instant::now(),
                current_frame: 0,
                image: Arc::clone(image_data),
            },
        }
    }
}

pub struct GlyphCache<T: Texture2d> {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph<T>>>,
    pub atlas: Atlas<T>,
    fonts: Rc<FontConfiguration>,
    pub image_cache: LruCache<usize, DecodedImage>,
    frame_cache: HashMap<(usize, usize), Sprite<T>>,
    line_glyphs: HashMap<LineKey, Sprite<T>>,
    pub block_glyphs: HashMap<BlockKey, Sprite<T>>,
    pub metrics: RenderMetrics,
}

#[cfg(test)]
impl GlyphCache<ImageTexture> {
    pub fn new_in_memory(
        fonts: &Rc<FontConfiguration>,
        size: usize,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Self> {
        let surface = Rc::new(ImageTexture::new(size, size));
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Ok(Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            image_cache: LruCache::new(
                "glyph_cache.image_cache.hit.rate",
                "glyph_cache.image_cache.miss.rate",
                16,
            ),
            frame_cache: HashMap::new(),
            atlas,
            metrics: metrics.clone(),
            line_glyphs: HashMap::new(),
            block_glyphs: HashMap::new(),
        })
    }
}

impl GlyphCache<SrgbTexture2d> {
    pub fn new_gl(
        backend: &Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        size: usize,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Self> {
        let caps = backend.get_capabilities();
        // You'd hope that allocating a texture would automatically
        // include this check, but it doesn't, and instead, the texture
        // silently fails to bind when attempting to render into it later.
        // So! We check and raise here for ourselves!
        if size
            > caps
                .max_texture_size
                .try_into()
                .context("represent Capabilities.max_texture_size as usize")?
        {
            anyhow::bail!(
                "Cannot use a texture of size {} as it is larger \
                 than the max {} supported by your GPU",
                size,
                caps.max_texture_size
            );
        }
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
            image_cache: LruCache::new(
                "glyph_cache.image_cache.hit.rate",
                "glyph_cache.image_cache.miss.rate",
                64, // FIXME: make configurable
            ),
            frame_cache: HashMap::new(),
            atlas,
            metrics: metrics.clone(),
            line_glyphs: HashMap::new(),
            block_glyphs: HashMap::new(),
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
        followed_by_space: bool,
    ) -> anyhow::Result<Rc<CachedGlyph<T>>> {
        let key = BorrowedGlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            style,
            followed_by_space,
        };

        if let Some(entry) = self.glyph_cache.get(&key as &dyn GlyphKeyTrait) {
            metrics::histogram!("glyph_cache.glyph_cache.hit.rate", 1.);
            return Ok(Rc::clone(entry));
        }
        metrics::histogram!("glyph_cache.glyph_cache.miss.rate", 1.);

        let glyph = match self.load_glyph(info, style, followed_by_space) {
            Ok(g) => g,
            Err(err) => {
                if err
                    .root_cause()
                    .downcast_ref::<OutOfTextureSpace>()
                    .is_some()
                {
                    // Ensure that we propagate this signal to expand
                    // our available teexture space
                    return Err(err);
                }

                // But otherwise: don't allow glyph loading errors to propagate,
                // as that will result in incomplete window painting.
                // Log the error and substitute instead.
                log::error!(
                    "load_glyph failed; using blank instead. Error: {:#}. {:?} {:?}",
                    err,
                    info,
                    style
                );
                Rc::new(CachedGlyph {
                    brightness_adjust: 1.0,
                    has_color: false,
                    texture: None,
                    x_offset: PixelLength::zero(),
                    y_offset: PixelLength::zero(),
                    bearing_x: PixelLength::zero(),
                    bearing_y: PixelLength::zero(),
                    scale: 1.0,
                })
            }
        };
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
        let brightness_adjust;
        let glyph;

        {
            let font = self.fonts.resolve_font(style)?;
            base_metrics = font.metrics();
            glyph = font.rasterize_glyph(info.glyph_pos, info.font_idx)?;

            idx_metrics = font.metrics_for_idx(info.font_idx)?;
            brightness_adjust = font.brightness_adjust(info.font_idx);
        }

        let aspect = (idx_metrics.cell_width / idx_metrics.cell_height).get();

        // 0.7 is used for this as that is ~ the threshold for \u24e9 on a mac,
        // which is looks squareish and for which it is desirable to allow to
        // overflow.  0.5 is the typical monospace font aspect ratio.
        let is_square_or_wide = aspect >= 0.7;

        let allow_width_overflow = if is_square_or_wide {
            match self.fonts.config().allow_square_glyphs_to_overflow_width {
                AllowSquareGlyphOverflow::Never => false,
                AllowSquareGlyphOverflow::Always => true,
                AllowSquareGlyphOverflow::WhenFollowedBySpace => followed_by_space,
            }
        } else {
            false
        };

        // Maximum width allowed for this glyph based on its unicode width and
        // the dimensions of a cell
        let max_pixel_width = base_metrics.cell_width.get() * (info.num_cells as f64 + 0.25);

        let scale;
        if info.font_idx == 0 {
            // We are the base font
            scale = if allow_width_overflow || glyph.width as f64 <= max_pixel_width {
                1.0
            } else {
                // Scale the glyph to fit in its number of cells
                1.0 / info.num_cells as f64
            };
        } else if !idx_metrics.is_scaled {
            // A bitmap font that isn't scaled to the requested height.
            let y_scale = base_metrics.cell_height.get() / idx_metrics.cell_height.get();
            let y_scaled_width = y_scale * glyph.width as f64;

            if allow_width_overflow || y_scaled_width <= max_pixel_width {
                // prefer height-wise scaling
                scale = y_scale;
            } else {
                // otherwise just make it fit the width
                scale = max_pixel_width / glyph.width as f64;
            }
        } else {
            // a scalable fallback font
            let y_scale = match (
                self.fonts.config().use_cap_height_to_scale_fallback_fonts,
                base_metrics.cap_height_ratio,
                idx_metrics.cap_height_ratio,
            ) {
                (true, Some(base_cap), Some(cap)) => {
                    // both fonts have cap-height metrics and we're in
                    // use_cap_height_to_scale_fallback_fonts mode, so
                    // scale based on their respective cap heights
                    base_cap / cap
                }
                _ => {
                    // Assume that the size we requested doesn't need
                    // any additional scaling
                    1.0
                }
            };

            // How wide the glyph would be using the y_scale we produced
            let y_scaled_width = y_scale * glyph.width as f64;

            if allow_width_overflow || y_scaled_width <= max_pixel_width {
                scale = y_scale;
            } else {
                scale = max_pixel_width / glyph.width as f64;
            }

            #[cfg(debug_assertions)]
            {
                log::debug!(
                    "{} allow_width_overflow={} is_square_or_wide={} aspect={} \
                       y_scaled_width={} max_pixel_width={} glyph.width={} -> scale={}",
                    info.text,
                    allow_width_overflow,
                    is_square_or_wide,
                    aspect,
                    y_scaled_width,
                    max_pixel_width,
                    glyph.width,
                    scale
                );
            }
        };

        let (cell_width, cell_height) = (base_metrics.cell_width, base_metrics.cell_height);

        let glyph = if glyph.width == 0 || glyph.height == 0 {
            // a whitespace glyph
            CachedGlyph {
                brightness_adjust: 1.0,
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
                    "physically scaling {:?} by {} bcos {}x{} > {:?}x{:?}. aspect={}",
                    info,
                    scale,
                    glyph.width,
                    glyph.height,
                    cell_width,
                    cell_height,
                    aspect,
                );
                (1.0, raw_im.scale_by(scale))
            } else {
                (scale, raw_im)
            };

            let tex = self.atlas.allocate(&raw_im)?;

            let g = CachedGlyph {
                brightness_adjust,
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

    fn cached_image_impl(
        frame_cache: &mut HashMap<(usize, usize), Sprite<T>>,
        atlas: &mut Atlas<T>,
        decoded: &mut DecodedImage,
        padding: Option<usize>,
    ) -> anyhow::Result<(Sprite<T>, Option<Instant>)> {
        let id = decoded.image.id();
        let mut handle = DecodedImageHandle {
            h: decoded.image.data(),
            current_frame: decoded.current_frame,
        };
        match &*handle.h {
            ImageDataType::Rgba8 { .. } => {
                if let Some(sprite) = frame_cache.get(&(id, 0)) {
                    return Ok((sprite.clone(), None));
                }
                let sprite = atlas.allocate_with_padding(&handle, padding)?;
                frame_cache.insert((id, 0), sprite.clone());

                return Ok((sprite, None));
            }
            ImageDataType::AnimRgba8 {
                frames, durations, ..
            } => {
                let mut next = None;
                if frames.len() > 1 {
                    let now = Instant::now();
                    let mut next_due = decoded.frame_start + durations[decoded.current_frame];
                    if now >= next_due {
                        // Advance to next frame
                        decoded.current_frame += 1;
                        if decoded.current_frame >= frames.len() {
                            decoded.current_frame = 0;
                        }
                        decoded.frame_start = now;
                        next_due = decoded.frame_start + durations[decoded.current_frame];
                        handle.current_frame = decoded.current_frame;
                    }

                    next.replace(next_due);
                }

                if let Some(sprite) = frame_cache.get(&(id, decoded.current_frame)) {
                    return Ok((sprite.clone(), next));
                }

                let sprite = atlas.allocate_with_padding(&handle, padding)?;

                frame_cache.insert((id, decoded.current_frame), sprite.clone());

                return Ok((
                    sprite,
                    Some(decoded.frame_start + durations[decoded.current_frame]),
                ));
            }
            ImageDataType::EncodedFile(_) => unreachable!(),
        }
    }

    pub fn cached_image(
        &mut self,
        image_data: &Arc<ImageData>,
        padding: Option<usize>,
    ) -> anyhow::Result<(Sprite<T>, Option<Instant>)> {
        let id = image_data.id();

        if let Some(decoded) = self.image_cache.get_mut(&id) {
            Self::cached_image_impl(&mut self.frame_cache, &mut self.atlas, decoded, padding)
        } else {
            let mut decoded = DecodedImage::load(image_data);
            let res = Self::cached_image_impl(
                &mut self.frame_cache,
                &mut self.atlas,
                &mut decoded,
                padding,
            )?;
            self.image_cache.put(id, decoded);
            Ok(res)
        }
    }

    pub fn cached_block(&mut self, block: BlockKey) -> anyhow::Result<Sprite<T>> {
        if let Some(s) = self.block_glyphs.get(&block) {
            return Ok(s.clone());
        }
        self.block_sprite(block)
    }

    fn line_sprite(&mut self, key: LineKey) -> anyhow::Result<Sprite<T>> {
        let mut buffer = Image::new(
            self.metrics.cell_size.width as usize,
            self.metrics.cell_size.height as usize,
        );
        let black = SrgbaPixel::rgba(0, 0, 0, 0);
        let white = SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff);

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
                );
            }
        };

        let draw_dotted = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                let y = (cell_rect.origin.y + self.metrics.descender_row + row) as usize;
                if y >= self.metrics.cell_size.height as usize {
                    break;
                }

                let mut color = white;
                let segment_length = (self.metrics.cell_size.width / 4) as usize;
                let mut count = segment_length;
                let range =
                    buffer.horizontal_pixel_range_mut(0, self.metrics.cell_size.width as usize, y);
                for c in range.iter_mut() {
                    *c = color.as_srgba32();
                    count -= 1;
                    if count == 0 {
                        color = if color == white { black } else { white };
                        count = segment_length;
                    }
                }
            }
        };

        let draw_dashed = |buffer: &mut Image| {
            for row in 0..self.metrics.underline_height {
                let y = (cell_rect.origin.y + self.metrics.descender_row + row) as usize;
                if y >= self.metrics.cell_size.height as usize {
                    break;
                }
                let mut color = white;
                let third = (self.metrics.cell_size.width / 3) as usize + 1;
                let mut count = third;
                let range =
                    buffer.horizontal_pixel_range_mut(0, self.metrics.cell_size.width as usize, y);
                for c in range.iter_mut() {
                    *c = color.as_srgba32();
                    count -= 1;
                    if count == 0 {
                        color = if color == white { black } else { white };
                        count = third;
                    }
                }
            }
        };

        let draw_curly = |buffer: &mut Image| {
            let max_y = self.metrics.cell_size.height as usize - 1;
            let x_factor = (2. * std::f32::consts::PI) / self.metrics.cell_size.width as f32;

            // Have the wave go from the descender to the bottom of the cell
            let wave_height =
                self.metrics.cell_size.height - (cell_rect.origin.y + self.metrics.descender_row);

            let half_height = (wave_height as f32 / 4.).max(1.);
            let y =
                (cell_rect.origin.y + self.metrics.descender_row) as usize - half_height as usize;

            fn add(x: usize, y: usize, val: u8, max_y: usize, buffer: &mut Image) {
                let y = y.min(max_y);
                let pixel = buffer.pixel_mut(x, y);
                let (current, _, _, _) = SrgbaPixel::with_srgba_u32(*pixel).as_rgba();
                let value = current.saturating_add(val);
                *pixel = SrgbaPixel::rgba(value, value, value, value).as_srgba32();
            }

            for x in 0..self.metrics.cell_size.width as usize {
                let vertical = -half_height * (x as f32 * x_factor).sin() + half_height;
                let v1 = vertical.floor();
                let v2 = vertical.ceil();

                for row in 0..self.metrics.underline_height as usize {
                    let value = (255. * (vertical - v1).abs()) as u8;
                    add(x, row + y + v1 as usize, 255 - value, max_y, buffer);
                    add(x, row + y + v2 as usize, value, max_y, buffer);
                }
            }
        };

        let draw_double = |buffer: &mut Image| {
            let first_line = self
                .metrics
                .descender_row
                .min(self.metrics.descender_plus_two - 2 * self.metrics.underline_height);

            for row in 0..self.metrics.underline_height {
                buffer.draw_line(
                    Point::new(cell_rect.origin.x, cell_rect.origin.y + first_line + row),
                    Point::new(
                        cell_rect.origin.x + self.metrics.cell_size.width,
                        cell_rect.origin.y + first_line + row,
                    ),
                    white,
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
                );
            }
        };

        buffer.clear_rect(cell_rect, black);
        if key.overline {
            draw_overline(&mut buffer);
        }
        match key.underline {
            Underline::None => {}
            Underline::Single => draw_single(&mut buffer),
            Underline::Curly => draw_curly(&mut buffer),
            Underline::Dashed => draw_dashed(&mut buffer),
            Underline::Dotted => draw_dotted(&mut buffer),
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
