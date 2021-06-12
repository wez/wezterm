use super::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::bitmaps::atlas::{Atlas, Sprite};
#[cfg(test)]
use ::window::bitmaps::ImageTexture;
use ::window::bitmaps::{BitmapImage, Image, Texture2d};
use ::window::color::{LinearRgba, SrgbaPixel};
use ::window::glium;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::{Point, Rect};
use config::{AllowSquareGlyphOverflow, TextStyle};
use euclid::num::Zero;
use lru::LruCache;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

bitflags::bitflags! {
    pub struct Quadrant: u8{
        const UPPER_LEFT = 1<<1;
        const UPPER_RIGHT = 1<<2;
        const LOWER_LEFT = 1<<3;
        const LOWER_RIGHT = 1<<4;
    }
}

bitflags::bitflags! {
    pub struct Sextant: u8{
        /// Upper-left
        const ONE = 1<<1;
        /// Upper-right
        const TWO = 1<<2;
        /// Middle left
        const THREE = 1<<3;
        /// Middle Right
        const FOUR = 1<<4;
        /// Lower left
        const FIVE = 1<<5;
        /// Lower right
        const SIX = 1<<6;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockAlpha {
    /// 100%
    Full,
    /// 75%
    Dark,
    /// 50%
    Medium,
    /// 25%
    Light,
}

/// Represents a Block Element glyph, decoded from
/// <https://en.wikipedia.org/wiki/Block_Elements>
/// <https://www.unicode.org/charts/PDF/U2580.pdf>
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockKey {
    /// Number of 1/8ths in the upper half
    Upper(u8),
    /// Number of 1/8ths in the lower half
    Lower(u8),
    /// Number of 1/8ths in the left half
    Left(u8),
    /// Number of 1/8ths in the right half
    Right(u8),
    /// Full block with alpha level
    Full(BlockAlpha),
    /// A combination of quadrants
    Quadrants(Quadrant),
    /// A combination of sextants
    Sextants(Sextant),
}

impl BlockKey {
    pub fn filter_out_synthetic(glyphs: &mut Vec<char>) {
        let config = config::configuration();
        if config.custom_block_glyphs {
            glyphs.retain(|&c| Self::from_char(c).is_none());
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        let c = c as u32;
        Some(match c {
            // Upper half block
            0x2580 => Self::Upper(4),
            // Lower 1..7 eighths
            0x2581..=0x2587 => Self::Lower((c - 0x2580) as u8),
            0x2588 => Self::Full(BlockAlpha::Full),
            // Left 7..1 eighths
            0x2589..=0x258f => Self::Left((0x2590 - c) as u8),
            // Right half
            0x2590 => Self::Right(4),
            0x2591 => Self::Full(BlockAlpha::Light),
            0x2592 => Self::Full(BlockAlpha::Medium),
            0x2593 => Self::Full(BlockAlpha::Dark),
            0x2594 => Self::Upper(1),
            0x2595 => Self::Right(1),
            0x2596 => Self::Quadrants(Quadrant::LOWER_LEFT),
            0x2597 => Self::Quadrants(Quadrant::LOWER_RIGHT),
            0x2598 => Self::Quadrants(Quadrant::UPPER_LEFT),
            0x2599 => {
                Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::LOWER_LEFT | Quadrant::LOWER_RIGHT)
            }
            0x259a => Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::LOWER_RIGHT),
            0x259b => {
                Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT)
            }
            0x259c => Self::Quadrants(
                Quadrant::UPPER_LEFT | Quadrant::UPPER_RIGHT | Quadrant::LOWER_RIGHT,
            ),
            0x259d => Self::Quadrants(Quadrant::UPPER_RIGHT),
            0x259e => Self::Quadrants(Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT),
            0x259f => Self::Quadrants(
                Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT | Quadrant::LOWER_RIGHT,
            ),
            0x1fb00 => Self::Sextants(Sextant::ONE),
            0x1fb01 => Self::Sextants(Sextant::TWO),
            0x1fb02 => Self::Sextants(Sextant::ONE | Sextant::TWO),
            0x1fb03 => Self::Sextants(Sextant::THREE),
            0x1fb04 => Self::Sextants(Sextant::ONE | Sextant::THREE),
            0x1fb05 => Self::Sextants(Sextant::TWO | Sextant::THREE),
            0x1fb06 => Self::Sextants(Sextant::TWO | Sextant::TWO | Sextant::THREE),
            0x1fb07 => Self::Sextants(Sextant::FOUR),
            0x1fb08 => Self::Sextants(Sextant::ONE | Sextant::FOUR),
            0x1fb09 => Self::Sextants(Sextant::TWO | Sextant::FOUR),
            0x1fb0a => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR),
            0x1fb0b => Self::Sextants(Sextant::THREE | Sextant::FOUR),
            0x1fb0c => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR),
            0x1fb0d => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR),
            0x1fb0e => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR),
            0x1fb0f => Self::Sextants(Sextant::FIVE),
            0x1fb10 => Self::Sextants(Sextant::ONE | Sextant::FIVE),
            0x1fb11 => Self::Sextants(Sextant::TWO | Sextant::FIVE),
            0x1fb12 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FIVE),
            0x1fb13 => Self::Sextants(Sextant::THREE | Sextant::FIVE),
            0x1fb14 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FIVE),
            0x1fb15 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FIVE),
            0x1fb16 => Self::Sextants(Sextant::FOUR | Sextant::FIVE),
            0x1fb17 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::FIVE),
            0x1fb18 => Self::Sextants(Sextant::TWO | Sextant::FOUR | Sextant::FIVE),
            0x1fb19 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::FIVE),
            0x1fb1a => Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::FIVE),
            0x1fb1b => {
                Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::FIVE)
            }
            0x1fb1c => {
                Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE)
            }
            0x1fb1d => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE,
            ),
            0x1fb1e => Self::Sextants(Sextant::SIX),
            0x1fb1f => Self::Sextants(Sextant::ONE | Sextant::SIX),
            0x1fb20 => Self::Sextants(Sextant::TWO | Sextant::SIX),
            0x1fb21 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::SIX),
            0x1fb22 => Self::Sextants(Sextant::THREE | Sextant::SIX),
            0x1fb23 => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::SIX),
            0x1fb24 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::SIX),
            0x1fb25 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::SIX),
            0x1fb26 => Self::Sextants(Sextant::FOUR | Sextant::SIX),
            0x1fb27 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::SIX),
            0x1fb28 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::SIX),
            0x1fb29 => Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            0x1fb2a => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            0x1fb2b => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            0x1fb2c => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::SIX,
            ),
            0x1fb2d => Self::Sextants(Sextant::FIVE | Sextant::SIX),
            0x1fb2e => Self::Sextants(Sextant::ONE | Sextant::FIVE | Sextant::SIX),
            0x1fb2f => Self::Sextants(Sextant::TWO | Sextant::FIVE | Sextant::SIX),
            0x1fb30 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FIVE | Sextant::SIX),
            0x1fb31 => Self::Sextants(Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            0x1fb32 => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            0x1fb33 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            0x1fb34 => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FIVE | Sextant::SIX,
            ),
            0x1fb35 => Self::Sextants(Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            0x1fb36 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            0x1fb37 => Self::Sextants(Sextant::TWO | Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            0x1fb38 => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
            0x1fb39 => {
                Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX)
            }
            0x1fb3a => Self::Sextants(
                Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
            0x1fb3b => Self::Sextants(
                Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
            _ => return None,
        })
    }

    pub fn from_cell(cell: &termwiz::cell::Cell) -> Option<Self> {
        let mut chars = cell.str().chars();
        let first_char = chars.next()?;
        if chars.next().is_some() {
            None
        } else {
            Self::from_char(first_char)
        }
    }
}

#[derive(Debug)]
pub struct ImageFrame {
    duration: Duration,
    image: ::window::bitmaps::Image,
}

#[derive(Debug)]
pub enum CachedImage {
    Animation(DecodedImage),
    SingleFrame,
}

#[derive(Debug)]
pub struct DecodedImage {
    frame_start: Instant,
    current_frame: usize,
    frames: Vec<ImageFrame>,
}

impl DecodedImage {
    fn placeholder() -> Self {
        let image = ::window::bitmaps::Image::new(1, 1);
        let frame = ImageFrame {
            duration: Duration::default(),
            image,
        };
        Self {
            frame_start: Instant::now(),
            current_frame: 0,
            frames: vec![frame],
        }
    }

    fn with_frames(frames: Vec<image::Frame>) -> Self {
        let frames = frames
            .into_iter()
            .map(|frame| {
                let duration: Duration = frame.delay().into();
                let image = image::DynamicImage::ImageRgba8(frame.into_buffer()).to_rgba8();
                let (w, h) = image.dimensions();
                let width = w as usize;
                let height = h as usize;
                let image = ::window::bitmaps::Image::from_raw(width, height, image.into_vec());
                ImageFrame { duration, image }
            })
            .collect();
        Self {
            frame_start: Instant::now(),
            current_frame: 0,
            frames,
        }
    }

    fn with_single(image_data: &Arc<ImageData>) -> anyhow::Result<Self> {
        let image = image::load_from_memory(image_data.data())?.to_rgba8();
        let (width, height) = image.dimensions();
        let width = width as usize;
        let height = height as usize;
        let image = ::window::bitmaps::Image::from_raw(width, height, image.into_vec());
        Ok(Self {
            frame_start: Instant::now(),
            current_frame: 0,
            frames: vec![ImageFrame {
                duration: Default::default(),
                image,
            }],
        })
    }

    fn load(image_data: &Arc<ImageData>) -> anyhow::Result<Self> {
        use image::{AnimationDecoder, ImageFormat};
        let format = image::guess_format(image_data.data())?;
        match format {
            ImageFormat::Gif => image::gif::GifDecoder::new(image_data.data())
                .and_then(|decoder| decoder.into_frames().collect_frames())
                .and_then(|frames| Ok(Self::with_frames(frames)))
                .or_else(|err| {
                    log::error!(
                        "Unable to parse animated gif: {:#}, trying as single frame",
                        err
                    );
                    Self::with_single(image_data)
                }),
            ImageFormat::Png => {
                let decoder = image::png::PngDecoder::new(image_data.data())?;
                if decoder.is_apng() {
                    let frames = decoder.apng().into_frames().collect_frames()?;
                    Ok(Self::with_frames(frames))
                } else {
                    Self::with_single(image_data)
                }
            }
            _ => Self::with_single(image_data),
        }
    }
}

pub struct GlyphCache<T: Texture2d> {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph<T>>>,
    pub atlas: Atlas<T>,
    fonts: Rc<FontConfiguration>,
    pub image_cache: LruCache<usize, CachedImage>,
    frame_cache: HashMap<(usize, usize), Sprite<T>>,
    line_glyphs: HashMap<LineKey, Sprite<T>>,
    block_glyphs: HashMap<BlockKey, Sprite<T>>,
    metrics: RenderMetrics,
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
            image_cache: LruCache::new(16),
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
            image_cache: LruCache::new(16),
            frame_cache: HashMap::new(),
            atlas,
            metrics: metrics.clone(),
            line_glyphs: HashMap::new(),
            block_glyphs: HashMap::new(),
        })
    }

    pub fn clear(&mut self) {
        self.atlas.clear();
        // self.image_cache.clear(); - relatively expensive to re-populate
        self.frame_cache.clear();
        self.glyph_cache.clear();
        self.line_glyphs.clear();
        self.block_glyphs.clear();
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

    pub fn cached_image(
        &mut self,
        image_data: &Arc<ImageData>,
        padding: Option<usize>,
    ) -> anyhow::Result<(Sprite<T>, Option<Instant>)> {
        let id = image_data.id();
        if let Some(cached) = self.image_cache.get_mut(&id) {
            match cached {
                CachedImage::SingleFrame => {
                    // We can simply use the frame cache to manage
                    // the texture space; the frame is always 0 for
                    // a single frame
                    if let Some(sprite) = self.frame_cache.get(&(id, 0)) {
                        return Ok((sprite.clone(), None));
                    }
                }
                CachedImage::Animation(decoded) => {
                    let mut next = None;
                    if decoded.frames.len() > 1 {
                        let now = Instant::now();
                        let mut next_due =
                            decoded.frame_start + decoded.frames[decoded.current_frame].duration;
                        if now >= next_due {
                            // Advance to next frame
                            decoded.current_frame += 1;
                            if decoded.current_frame >= decoded.frames.len() {
                                decoded.current_frame = 0;
                            }
                            decoded.frame_start = now;
                            next_due = decoded.frame_start
                                + decoded.frames[decoded.current_frame].duration;
                        }

                        next.replace(next_due);
                    }

                    if let Some(sprite) = self.frame_cache.get(&(id, decoded.current_frame)) {
                        return Ok((sprite.clone(), next));
                    }

                    let sprite = self.atlas.allocate_with_padding(
                        &decoded.frames[decoded.current_frame].image,
                        padding,
                    )?;

                    self.frame_cache
                        .insert((id, decoded.current_frame), sprite.clone());

                    return Ok((
                        sprite,
                        Some(decoded.frame_start + decoded.frames[decoded.current_frame].duration),
                    ));
                }
            }
        }

        let decoded =
            DecodedImage::load(image_data).or_else(|e| -> anyhow::Result<DecodedImage> {
                log::debug!("Failed to decode image: {:#}", e);
                // Use a placeholder instead
                Ok(DecodedImage::placeholder())
            })?;
        let sprite = self
            .atlas
            .allocate_with_padding(&decoded.frames[0].image, padding)?;
        self.frame_cache.insert((id, 0), sprite.clone());
        if decoded.frames.len() > 1 {
            let next = Some(decoded.frame_start + decoded.frames[0].duration);
            self.image_cache.put(id, CachedImage::Animation(decoded));
            Ok((sprite, next))
        } else {
            self.image_cache.put(id, CachedImage::SingleFrame);
            Ok((sprite, None))
        }
    }

    fn block_sprite(&mut self, block: BlockKey) -> anyhow::Result<Sprite<T>> {
        let mut buffer = Image::new(
            self.metrics.cell_size.width as usize,
            self.metrics.cell_size.height as usize,
        );
        let black = SrgbaPixel::rgba(0, 0, 0, 0);
        let white = SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), self.metrics.cell_size);

        fn scale(f: f32) -> usize {
            f.ceil().max(1.) as usize
        }

        buffer.clear_rect(cell_rect, black);

        /// Fill a rectangular region described by the x and y ranges
        /// TODO: this could probably be a call to buffer.clear_rect?
        let fill_rect = |buffer: &mut Image, x: Range<usize>, y: Range<usize>| {
            for y in y {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x + x.start as isize,
                        cell_rect.origin.y + y as isize,
                    ),
                    Point::new(
                        // Note: draw_line uses inclusive coordinates, but our
                        // range is exclusive coordinates, so compensate here!
                        // We don't need to do this for `y` since we are already
                        // iterating over the correct set of `y` values in our loop.
                        cell_rect.origin.x + x.end.saturating_sub(1) as isize,
                        cell_rect.origin.y + y as isize,
                    ),
                    white,
                );
            }
        };

        match block {
            BlockKey::Upper(num) => {
                let lower = self.metrics.cell_size.height as f32 * (num as f32) / 8.;
                let width = self.metrics.cell_size.width as usize;
                fill_rect(&mut buffer, 0..width, 0..scale(lower));
            }
            BlockKey::Lower(num) => {
                let upper = self.metrics.cell_size.height as f32 * ((8 - num) as f32) / 8.;
                let width = self.metrics.cell_size.width as usize;
                let height = self.metrics.cell_size.height as usize;
                fill_rect(&mut buffer, 0..width, scale(upper)..height);
            }
            BlockKey::Left(num) => {
                let width = self.metrics.cell_size.width as f32 * (num as f32) / 8.;
                let height = self.metrics.cell_size.height as usize;
                fill_rect(&mut buffer, 0..scale(width), 0..height);
            }
            BlockKey::Right(num) => {
                let left = self.metrics.cell_size.width as f32 * ((8 - num) as f32) / 8.;
                let width = self.metrics.cell_size.width as usize;
                let height = self.metrics.cell_size.height as usize;
                fill_rect(&mut buffer, scale(left)..width, 0..height);
            }
            BlockKey::Full(alpha) => {
                let alpha = match alpha {
                    BlockAlpha::Full => 1.0,
                    BlockAlpha::Dark => 0.75,
                    BlockAlpha::Medium => 0.5,
                    BlockAlpha::Light => 0.25,
                };
                let fill = LinearRgba::with_components(alpha, alpha, alpha, alpha);

                buffer.clear_rect(cell_rect, fill.srgba_pixel());
            }
            BlockKey::Quadrants(quads) => {
                let y_half = self.metrics.cell_size.height as f32 / 2.;
                let x_half = self.metrics.cell_size.width as f32 / 2.;
                let width = self.metrics.cell_size.width as usize;
                let height = self.metrics.cell_size.height as usize;
                if quads.contains(Quadrant::UPPER_LEFT) {
                    fill_rect(&mut buffer, 0..scale(x_half), 0..scale(y_half));
                }
                if quads.contains(Quadrant::UPPER_RIGHT) {
                    fill_rect(&mut buffer, scale(x_half)..width, 0..scale(y_half));
                }
                if quads.contains(Quadrant::LOWER_LEFT) {
                    fill_rect(&mut buffer, 0..scale(x_half), scale(y_half)..height);
                }
                if quads.contains(Quadrant::LOWER_RIGHT) {
                    fill_rect(&mut buffer, scale(x_half)..width, scale(y_half)..height);
                }
            }
            BlockKey::Sextants(s) => {
                let y_third = self.metrics.cell_size.height as f32 / 3.;
                let x_half = self.metrics.cell_size.width as f32 / 2.;
                let width = self.metrics.cell_size.width as usize;
                let height = self.metrics.cell_size.height as usize;

                if s.contains(Sextant::ONE) {
                    fill_rect(&mut buffer, 0..scale(x_half), 0..scale(y_third));
                }
                if s.contains(Sextant::TWO) {
                    fill_rect(&mut buffer, scale(x_half)..width, 0..scale(y_third));
                }
                if s.contains(Sextant::THREE) {
                    fill_rect(
                        &mut buffer,
                        0..scale(x_half),
                        scale(y_third)..scale(y_third * 2.),
                    );
                }
                if s.contains(Sextant::FOUR) {
                    fill_rect(
                        &mut buffer,
                        scale(x_half)..width,
                        scale(y_third)..scale(y_third * 2.),
                    );
                }
                if s.contains(Sextant::FIVE) {
                    fill_rect(&mut buffer, 0..scale(x_half), scale(y_third * 2.)..height);
                }
                if s.contains(Sextant::SIX) {
                    fill_rect(
                        &mut buffer,
                        scale(x_half)..width,
                        scale(y_third * 2.)..height,
                    );
                }
            }
        }

        /*
        log::info!("{:?}", block);
        buffer.log_bits();
        */

        let sprite = self.atlas.allocate(&buffer)?;
        self.block_glyphs.insert(block, sprite.clone());
        Ok(sprite)
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
