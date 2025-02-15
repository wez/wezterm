use super::utilsprites::RenderMetrics;
use crate::customglyph::*;
use crate::renderstate::RenderContext;
use crate::termwindow::render::paint::AllowImage;
use ::window::bitmaps::atlas::{Atlas, OutOfTextureSpace, Sprite};
use ::window::bitmaps::{BitmapImage, Image, ImageTexture, Texture2d};
use ::window::color::SrgbaPixel;
use ::window::{Point, Rect};
use anyhow::Context;
use config::{AllowSquareGlyphOverflow, TextStyle};
use euclid::num::Zero;
use image::{
    AnimationDecoder, DynamicImage, Frame, Frames, ImageDecoder, ImageFormat, ImageResult, Limits,
};
use lfucache::LfuCache;
use ordered_float::NotNan;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Seek;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, LazyLock, MutexGuard};
use std::time::{Duration, Instant};
use termwiz::color::RgbColor;
use termwiz::image::{ImageData, ImageDataType};
use termwiz::surface::CursorShape;
use wezterm_blob_leases::{BlobLease, BlobManager, BoxedReader};
use wezterm_font::units::*;
use wezterm_font::{FontConfiguration, GlyphInfo, LoadedFont, LoadedFontId};
use wezterm_term::Underline;

static FRAME_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

/// We only want to report a frame error once at error level, because
/// if it is triggering it is likely in a animated image and will continue
/// to trigger multiple times per second as the frames are cycled.
fn report_frame_error<S: Into<String>>(message: S) {
    if FRAME_ERROR_REPORTED.load(Ordering::Relaxed) {
        log::debug!("{}", message.into());
    } else {
        log::error!("{}", message.into());
        FRAME_ERROR_REPORTED.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Loaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellMetricKey {
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl From<&RenderMetrics> for CellMetricKey {
    fn from(metrics: &RenderMetrics) -> CellMetricKey {
        CellMetricKey {
            pixel_width: metrics.cell_size.width as u16,
            pixel_height: metrics.cell_size.height as u16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizedBlockKey {
    pub block: BlockKey,
    pub size: CellMetricKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_idx: usize,
    pub glyph_pos: u32,
    pub num_cells: u8,
    pub style: TextStyle,
    pub followed_by_space: bool,
    pub metric: CellMetricKey,
    pub id: LoadedFontId,
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
    pub num_cells: u8,
    pub style: &'a TextStyle,
    pub followed_by_space: bool,
    pub metric: CellMetricKey,
    pub id: LoadedFontId,
}

impl<'a> BorrowedGlyphKey<'a> {
    fn to_owned(&self) -> GlyphKey {
        GlyphKey {
            font_idx: self.font_idx,
            glyph_pos: self.glyph_pos,
            num_cells: self.num_cells,
            style: self.style.clone(),
            followed_by_space: self.followed_by_space,
            metric: self.metric,
            id: self.id,
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
            num_cells: self.num_cells,
            style: &self.style,
            followed_by_space: self.followed_by_space,
            metric: self.metric,
            id: self.id,
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
pub struct CachedGlyph {
    pub has_color: bool,
    pub brightness_adjust: f32,
    pub x_offset: PixelLength,
    pub y_offset: PixelLength,
    pub x_advance: PixelLength,
    pub bearing_x: PixelLength,
    pub bearing_y: PixelLength,
    pub texture: Option<Sprite>,
    pub scale: f64,
}

impl std::fmt::Debug for CachedGlyph {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("CachedGlyph")
            .field("has_color", &self.has_color)
            .field("x_advance", &self.x_advance)
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
    size: CellMetricKey,
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
            ImageDataType::EncodedLease(_) | ImageDataType::EncodedFile(_) => unreachable!(),
        }
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        panic!("cannot mutate DecodedImage");
    }

    fn image_dimensions(&self) -> (usize, usize) {
        match &*self.h {
            ImageDataType::Rgba8 { width, height, .. }
            | ImageDataType::AnimRgba8 { width, height, .. } => (*width as usize, *height as usize),
            ImageDataType::EncodedLease(_) | ImageDataType::EncodedFile(_) => unreachable!(),
        }
    }
}

#[derive(Clone)]
struct DecodedFrame {
    lease: BlobLease,
    duration: Duration,
    width: usize,
    height: usize,
}

struct FrameDecoder {}

impl FrameDecoder {
    pub fn start(lease: BlobLease) -> anyhow::Result<Receiver<DecodedFrame>> {
        let (tx, rx) = sync_channel(2);

        let buf_reader = lease.get_reader().context("lease.get_reader()")?;
        let reader = image::ImageReader::new(buf_reader)
            .with_guessed_format()
            .context("guess format from lease")?;
        let format = reader
            .format()
            .ok_or_else(|| anyhow::anyhow!("cannot determine image format"))?;

        std::thread::spawn(move || {
            if let Err(err) = Self::run_decoder_thread(reader, format, tx) {
                if err
                    .downcast_ref::<std::sync::mpsc::SendError<DecodedFrame>>()
                    .is_none()
                {
                    log::error!("Error decoding image: {err:#}");
                }
            }
        });

        Ok(rx)
    }

    fn run_decoder_thread(
        reader: image::ImageReader<BoxedReader>,
        format: ImageFormat,
        tx: SyncSender<DecodedFrame>,
    ) -> anyhow::Result<()> {
        let start = Instant::now();
        let limits = Limits::default();
        let mut frames = match format {
            ImageFormat::Gif => {
                let mut reader = reader.into_inner();
                reader.rewind().context("rewinding reader for gif")?;
                let mut decoder =
                    image::codecs::gif::GifDecoder::new(reader).context("GifDecoder::new")?;
                decoder
                    .set_limits(limits)
                    .context("GifDecoder::set_limits")?;
                decoder.into_frames()
            }
            ImageFormat::Png => {
                let mut reader = reader.into_inner();
                reader.rewind().context("rewinding reader for png")?;
                let decoder = image::codecs::png::PngDecoder::with_limits(reader, limits.clone())
                    .context("PngDecoder::with_limits")?;
                if decoder.is_apng().unwrap_or(false) {
                    decoder.apng()?.into_frames()
                } else {
                    let buf = DynamicImage::from_decoder(decoder)?.into_rgba8();
                    let delay = image::Delay::from_numer_denom_ms(u32::MAX, 1);
                    let frame = Frame::from_parts(buf, 0, 0, delay);
                    Frames::new(Box::new(std::iter::once(ImageResult::Ok(frame))))
                }
            }
            ImageFormat::WebP => {
                let mut reader = reader.into_inner();
                reader.rewind().context("rewinding reader for WebP")?;
                let mut decoder =
                    image::codecs::webp::WebPDecoder::new(reader).context("WebPDecoder")?;
                decoder
                    .set_limits(limits)
                    .context("WebPDecoder::set_limits")?;
                decoder.into_frames()
            }
            _ => {
                let buf = reader.decode().context("decode image")?;
                let delay = image::Delay::from_numer_denom_ms(u32::MAX, 1);
                let frame = Frame::from_parts(buf.into_rgba8(), 0, 0, delay);
                Frames::new(Box::new(std::iter::once(ImageResult::Ok(frame))))
            }
        };

        let frame = frames
            .next()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unable to decode image data. Either it is corrupt, or \
                    the Image format is not fully supported by \
                    https://github.com/image-rs/image/blob/master/README.md#supported-image-formats")
            })?;
        let frame = frame.context("first frame result")?;

        let mut decoded_frames = vec![];
        let (width, height) = frame.buffer().dimensions();
        let width = width as usize;
        let height = height as usize;

        let duration: Duration = frame.delay().into();
        log::debug!("first frame took {:?} to decode.", start.elapsed());

        let data = frame.into_buffer().into_raw();
        let lease = BlobManager::store(&data).context("BlobManager::store")?;
        let decoded_frame = DecodedFrame {
            lease,
            duration,
            width,
            height,
        };
        tx.send(decoded_frame.clone())
            .context("sending first frame")?;
        decoded_frames.push(decoded_frame);

        while let Some(frame) = frames.next() {
            let frame = frame?;

            let duration: Duration = frame.delay().into();
            let data = frame.into_buffer().into_raw();
            let lease = BlobManager::store(&data).context("BlobManager::store")?;

            let decoded_frame = DecodedFrame {
                lease,
                duration,
                width,
                height,
            };
            tx.send(decoded_frame.clone()).context("sending a frame")?;
            decoded_frames.push(decoded_frame);
        }

        drop(frames);

        let elapsed = start.elapsed();
        let fps = decoded_frames.len() as f32 / elapsed.as_secs_f32();

        log::debug!(
            "decoded {} frames, {} bytes in {elapsed:?}, {fps} fps",
            decoded_frames.len(),
            decoded_frames.len() * width * height * 4
        );
        Ok(())
    }
}

enum FrameSource {
    Decoder(Receiver<DecodedFrame>),
    FrameIndex(usize),
}

struct FrameState {
    source: FrameSource,
    current_frame: DecodedFrame,
    frames: Vec<DecodedFrame>,
    load_state: LoadState,
}

impl FrameState {
    fn new(rx: Receiver<DecodedFrame>) -> Self {
        const BLACK_SIZE: usize = 8;
        static BLACK: LazyLock<BlobLease> = LazyLock::new(|| {
            let mut data = vec![];
            for _ in 0..BLACK_SIZE * BLACK_SIZE {
                data.extend_from_slice(&[0, 0, 0, 0xff]);
            }
            BlobManager::store(&data).unwrap()
        });

        Self {
            source: FrameSource::Decoder(rx),
            frames: vec![],
            current_frame: DecodedFrame {
                lease: BLACK.clone(),
                width: BLACK_SIZE,
                height: BLACK_SIZE,
                duration: Duration::from_millis(0),
            },
            load_state: LoadState::Loading,
        }
    }

    fn wait_for_first_frame(&mut self, duration: Duration) {
        if !self.frames.is_empty() {
            // Already decoded the first frame
            return;
        }

        match &mut self.source {
            FrameSource::Decoder(rx) => match rx.recv_timeout(duration) {
                Ok(frame) => {
                    self.frames.push(frame.clone());
                    self.current_frame = frame;
                    self.load_state = LoadState::Loaded;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.source = FrameSource::FrameIndex(0);
                    log::warn!("image decoder thread terminated");
                    self.current_frame.duration = Duration::from_secs(86400);
                    self.frames.push(self.current_frame.clone());
                }
            },
            FrameSource::FrameIndex(_) => {}
        }
    }

    fn load_next_frame(&mut self) -> bool {
        match &mut self.source {
            FrameSource::Decoder(rx) => match rx.try_recv() {
                Ok(frame) => {
                    self.frames.push(frame.clone());
                    self.current_frame = frame;
                    self.load_state = LoadState::Loaded;
                    true
                }
                Err(TryRecvError::Empty) => false,
                Err(TryRecvError::Disconnected) => {
                    self.source = FrameSource::FrameIndex(0);
                    if self.frames.is_empty() {
                        log::warn!("image decoder thread terminated");
                        self.current_frame.duration = Duration::from_secs(86400);
                        self.frames.push(self.current_frame.clone());
                        false
                    } else if self.frames.len() == 1 {
                        // If there's only a single frame, we may as well ensure
                        // that it has a long duration so that we don't waste
                        // resources ticking to the same frame over and over
                        self.frames[0].duration = Duration::from_secs(86400);
                        true
                    } else {
                        true
                    }
                }
            },
            FrameSource::FrameIndex(idx) => {
                *idx = *idx + 1;
                if *idx >= self.frames.len() {
                    *idx = 0;
                }
                self.current_frame = self.frames[*idx].clone();
                true
            }
        }
    }

    fn frame_duration(&self) -> Duration {
        self.current_frame.duration
    }

    fn frame_hash(&self) -> [u8; 32] {
        self.current_frame.lease.content_id().as_hash_bytes()
    }
}

impl std::fmt::Debug for FrameState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("FrameState").finish()
    }
}

#[derive(Debug)]
pub struct DecodedImage {
    frame_start: RefCell<Instant>,
    current_frame: RefCell<usize>,
    image: Arc<ImageData>,
    frames: RefCell<Option<FrameState>>,
}

impl DecodedImage {
    fn placeholder() -> Self {
        let image = ImageData::with_data(ImageDataType::placeholder());
        Self {
            frame_start: RefCell::new(Instant::now()),
            current_frame: RefCell::new(0),
            image: Arc::new(image),
            frames: RefCell::new(None),
        }
    }

    fn start_frame_decoder(lease: BlobLease, image_data: &Arc<ImageData>) -> Self {
        match FrameDecoder::start(lease.clone()) {
            Ok(rx) => Self {
                frame_start: RefCell::new(Instant::now()),
                current_frame: RefCell::new(0),
                image: Arc::clone(image_data),
                frames: RefCell::new(Some(FrameState::new(rx))),
            },
            Err(err) => {
                log::error!("failed to start FrameDecoder: {err:#}");
                Self::placeholder()
            }
        }
    }

    fn load(image_data: &Arc<ImageData>) -> Self {
        match &*image_data.data() {
            ImageDataType::EncodedLease(lease) => {
                Self::start_frame_decoder(lease.clone(), image_data)
            }
            ImageDataType::EncodedFile(data) => match BlobManager::store(&data) {
                Ok(lease) => Self::start_frame_decoder(lease, image_data),
                Err(err) => {
                    log::error!("Unable to move file data to blob manager: {err:#}");
                    Self::placeholder()
                }
            },
            ImageDataType::AnimRgba8 { durations, .. } => {
                let current_frame = if durations.len() > 1 && durations[0].as_millis() == 0 {
                    // Skip possible 0-duration root frame
                    1
                } else {
                    0
                };
                Self {
                    frame_start: RefCell::new(Instant::now()),
                    current_frame: RefCell::new(current_frame),
                    image: Arc::clone(image_data),
                    frames: RefCell::new(None),
                }
            }

            _ => Self {
                frame_start: RefCell::new(Instant::now()),
                current_frame: RefCell::new(0),
                image: Arc::clone(image_data),
                frames: RefCell::new(None),
            },
        }
    }
}

/// A number of items here are HashMaps rather than LfuCaches;
/// eviction is managed by recreating Self when the Atlas is filled
pub struct GlyphCache {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph>>,
    pub atlas: Atlas,
    pub fonts: Rc<FontConfiguration>,
    pub image_cache: LfuCache<[u8; 32], DecodedImage>,
    frame_cache: HashMap<[u8; 32], Sprite>,
    line_glyphs: HashMap<LineKey, Sprite>,
    pub block_glyphs: HashMap<SizedBlockKey, Sprite>,
    pub cursor_glyphs: HashMap<(Option<CursorShape>, u8), Sprite>,
    pub color: HashMap<(RgbColor, NotNan<f32>), Sprite>,
    min_frame_duration: Duration,
}

impl GlyphCache {
    pub fn new_in_memory(fonts: &Rc<FontConfiguration>, size: usize) -> anyhow::Result<Self> {
        let surface: Rc<dyn Texture2d> = Rc::new(ImageTexture::new(size, size));
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Ok(Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            image_cache: LfuCache::new(
                "glyph_cache.image_cache.hit.rate",
                "glyph_cache.image_cache.miss.rate",
                |config| config.glyph_cache_image_cache_size,
                &fonts.config(),
            ),
            frame_cache: HashMap::new(),
            atlas,
            line_glyphs: HashMap::new(),
            block_glyphs: HashMap::new(),
            cursor_glyphs: HashMap::new(),
            color: HashMap::new(),
            min_frame_duration: Duration::from_millis(1000 / fonts.config().max_fps as u64),
        })
    }
}

impl GlyphCache {
    pub fn new_gl(
        backend: &RenderContext,
        fonts: &Rc<FontConfiguration>,
        size: usize,
    ) -> anyhow::Result<Self> {
        let surface = backend.allocate_texture_atlas(size)?;
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Ok(Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            image_cache: LfuCache::new(
                "glyph_cache.image_cache.hit.rate",
                "glyph_cache.image_cache.miss.rate",
                |config| config.glyph_cache_image_cache_size,
                &fonts.config(),
            ),
            frame_cache: HashMap::new(),
            atlas,
            line_glyphs: HashMap::new(),
            block_glyphs: HashMap::new(),
            cursor_glyphs: HashMap::new(),
            color: HashMap::new(),
            min_frame_duration: Duration::from_millis(1000 / fonts.config().max_fps as u64),
        })
    }
}

impl GlyphCache {
    /// Resolve a glyph from the cache, rendering the glyph on-demand if
    /// the cache doesn't already hold the desired glyph.
    pub fn cached_glyph(
        &mut self,
        info: &GlyphInfo,
        style: &TextStyle,
        followed_by_space: bool,
        font: &Rc<LoadedFont>,
        metrics: &RenderMetrics,
        num_cells: u8,
    ) -> anyhow::Result<Rc<CachedGlyph>> {
        let key = BorrowedGlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            num_cells: num_cells,
            style,
            followed_by_space,
            metric: metrics.into(),
            id: font.id(),
        };

        if let Some(entry) = self.glyph_cache.get(&key as &dyn GlyphKeyTrait) {
            metrics::histogram!("glyph_cache.glyph_cache.hit.rate").record(1.);
            return Ok(Rc::clone(entry));
        }
        metrics::histogram!("glyph_cache.glyph_cache.miss.rate").record(1.);

        let glyph = match self.load_glyph(info, font, followed_by_space, num_cells) {
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
                    x_advance: PixelLength::zero(),
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

    pub fn config_changed(&mut self) {
        let config = self.fonts.config();
        self.image_cache.update_config(&config);
        self.cursor_glyphs.clear();
    }

    /// Perform the load and render of a glyph
    #[allow(clippy::float_cmp)]
    fn load_glyph(
        &mut self,
        info: &GlyphInfo,
        font: &Rc<LoadedFont>,
        followed_by_space: bool,
        num_cells: u8,
    ) -> anyhow::Result<Rc<CachedGlyph>> {
        let base_metrics;
        let idx_metrics;
        let brightness_adjust;
        let glyph;

        {
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

        // We shouldn't need to render a glyph that occupies zero cells, but that
        // can happen somehow; see <https://github.com/wezterm/wezterm/issues/1042>
        // so let's treat 0 cells as 1 cell so that we don't try to divide by
        // zero below.
        let num_cells = num_cells.max(1) as f64;

        // Maximum width allowed for this glyph based on its unicode width and
        // the dimensions of a cell
        let max_pixel_width = base_metrics.cell_width.get() * (num_cells + 0.25);

        let scale;

        // This helps to compensate for the !idx_metrics.is_scaled && glyph.is_scaled
        // case which happens when using the harfbuzz rasterizer with a bitmap font.
        // The default value is no compensation.
        let mut metrics_only_scale = 1.0;

        if info.font_idx == 0 {
            // We are the base font
            scale = if allow_width_overflow || glyph.width as f64 <= max_pixel_width {
                1.0
            } else {
                // Scale the glyph to fit in its number of cells
                1.0 / num_cells
            };
        } else if !glyph.is_scaled {
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

            let f_width = glyph.width as f64;

            if allow_width_overflow || f_width <= max_pixel_width {
                scale = 1.0;
            } else {
                scale = max_pixel_width / f_width;
            }

            if !idx_metrics.is_scaled {
                // A special case: the shaper (eg: harfbuzz) processed
                // a bitmap font (eg: older versions of Noto Color Emoji)
                // to produce shaping info at the bitmap strike size,
                // which is 128 for that font.  The advance is expressed
                // at that size and not at the size of the font.
                // If we get to this condition, the rasterizer used a mode
                // where it has already scaled the glyph, so the dimensions
                // in the bitmap are correct, but the shaper metrics need
                // to be adjusted.
                let y_scale = base_metrics.cell_height.get() / idx_metrics.cell_height.get();
                metrics_only_scale = y_scale;
            }

            #[cfg(debug_assertions)]
            {
                log::debug!(
                    "{text} allow_width_overflow={allow_width_overflow} \
                     is_square_or_wide={is_square_or_wide} aspect={aspect} \
                     max_pixel_width={max_pixel_width} glyph.width={glyph_width} \
                     -> scale={scale} metrics_only_scale={metrics_only_scale}",
                    text = info.text,
                    glyph_width = glyph.width,
                );
            }
        };

        let descender_adjust = if info.font_idx == 0 {
            PixelLength::new(0.0)
        } else {
            idx_metrics.force_y_adjust
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
                x_advance: info.x_advance * scale,
                bearing_x: PixelLength::zero(),
                bearing_y: descender_adjust,
                scale,
            }
        } else {
            let raw_im = Image::with_rgba32(
                glyph.width as usize,
                glyph.height as usize,
                4 * glyph.width as usize,
                &glyph.data,
            );

            let bearing_x = glyph.bearing_x * scale * metrics_only_scale;
            // No metrics_only_scale adjustment to bearing_y is needed because
            // the value comes from the rasterized glyph and not from the
            // shaper stage.
            let bearing_y = descender_adjust + (glyph.bearing_y * scale);
            let x_offset = info.x_offset * scale * metrics_only_scale;
            let y_offset = info.y_offset * scale * metrics_only_scale;
            let x_advance = info.x_advance * scale * metrics_only_scale;

            log::trace!(
                "bearing_x={bearing_x:?} bearing_y={bearing_y:?} \
                 x_offset={x_offset:?} y_offset={y_offset:?} x_advance={x_advance:?}"
            );

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
                x_advance,
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
        frame_cache: &mut HashMap<[u8; 32], Sprite>,
        atlas: &mut Atlas,
        decoded: &DecodedImage,
        padding: Option<usize>,
        min_frame_duration: Duration,
        allow_image: AllowImage,
    ) -> anyhow::Result<(Sprite, Option<Instant>, LoadState)> {
        let mut handle = DecodedImageHandle {
            h: decoded.image.data(),
            current_frame: *decoded.current_frame.borrow(),
        };

        let scale_down = match allow_image {
            AllowImage::Scale(n) => Some(n),
            _ => None,
        };

        match &*handle.h {
            ImageDataType::Rgba8 { hash, .. } => {
                if let Some(sprite) = frame_cache.get(hash) {
                    return Ok((sprite.clone(), None, LoadState::Loaded));
                }
                let sprite = atlas
                    .allocate_with_padding(&handle, padding, scale_down)
                    .context("atlas.allocate_with_padding")?;
                frame_cache.insert(*hash, sprite.clone());

                return Ok((sprite, None, LoadState::Loaded));
            }
            ImageDataType::AnimRgba8 {
                hashes,
                frames,
                durations,
                ..
            } => {
                let mut next = None;
                let mut decoded_frame_start = decoded.frame_start.borrow_mut();
                let mut decoded_current_frame = decoded.current_frame.borrow_mut();
                if frames.len() > 1 {
                    let now = Instant::now();

                    // We round up the frame duration to at least the minimum
                    // frame duration that wezterm can use when rendering.
                    // There's no point trying to deal with smaller intervals
                    // because we simply cannot render them without dropping
                    // frames.
                    // In addition, with a 1ms frame delay, there's a good chance
                    // that any given cell may switch to a different frame from
                    // its neighbor while we are rendering the entire terminal
                    // frame, so we want to avoid that.
                    // <https://github.com/wezterm/wezterm/issues/3260>
                    let mut next_due = *decoded_frame_start
                        + durations[*decoded_current_frame].max(min_frame_duration);
                    if now >= next_due {
                        // Advance to next frame
                        *decoded_current_frame = *decoded_current_frame + 1;
                        if *decoded_current_frame >= frames.len() {
                            *decoded_current_frame = 0;
                            // Skip potential 0-duration root frame
                            if durations[0].as_millis() == 0 && frames.len() > 1 {
                                *decoded_current_frame = *decoded_current_frame + 1;
                            }
                        }
                        *decoded_frame_start = now;
                        next_due = *decoded_frame_start
                            + durations[*decoded_current_frame].max(min_frame_duration);
                        handle.current_frame = *decoded_current_frame;
                    }

                    next.replace(next_due);
                }

                let hash = hashes[*decoded_current_frame];

                if let Some(sprite) = frame_cache.get(&hash) {
                    return Ok((sprite.clone(), next, LoadState::Loaded));
                }

                let sprite = atlas
                    .allocate_with_padding(&handle, padding, scale_down)
                    .context("atlas.allocate_with_padding")?;

                frame_cache.insert(hash, sprite.clone());

                return Ok((
                    sprite,
                    Some(
                        *decoded_frame_start
                            + durations[*decoded_current_frame].max(min_frame_duration),
                    ),
                    LoadState::Loaded,
                ));
            }
            ImageDataType::EncodedLease(_) | ImageDataType::EncodedFile(_) => {
                let mut frames = decoded.frames.borrow_mut();
                let frames = frames.as_mut().expect("to have frames");

                let mut next = None;
                let mut decoded_frame_start = decoded.frame_start.borrow_mut();
                let mut decoded_current_frame = decoded.current_frame.borrow_mut();

                // Wait up to the approx limit of human tolerable delay for
                // the first frame to be decoded, so that we can avoid showing
                // a flash of the black frame in the common case
                let max_duration = Duration::from_millis(125).max(min_frame_duration);
                if let Some(remain) = max_duration.checked_sub(decoded_frame_start.elapsed()) {
                    frames.wait_for_first_frame(remain);
                }

                let now = Instant::now();
                // We round up the frame duration to at least the minimum
                // frame duration that wezterm can use when rendering.
                // There's no point trying to deal with smaller intervals
                // because we simply cannot render them without dropping
                // frames.
                // In addition, with a 1ms frame delay, there's a good chance
                // that any given cell may switch to a different frame from
                // its neighbor while we are rendering the entire terminal
                // frame, so we want to avoid that.
                // <https://github.com/wezterm/wezterm/issues/3260>
                let mut next_due =
                    *decoded_frame_start + frames.frame_duration().max(min_frame_duration);
                if now >= next_due {
                    // Advance to next frame
                    if frames.load_next_frame() {
                        *decoded_current_frame = *decoded_current_frame + 1;
                        *decoded_frame_start = now;
                        next_due =
                            *decoded_frame_start + frames.frame_duration().max(min_frame_duration);
                        handle.current_frame = *decoded_current_frame;
                    }
                }

                next.replace(next_due);

                let hash = frames.frame_hash();

                if let Some(sprite) = frame_cache.get(&hash) {
                    return Ok((sprite.clone(), next, frames.load_state));
                }

                let expected_byte_size =
                    frames.current_frame.width * frames.current_frame.height * 4;

                let frame_data = match frames.current_frame.lease.get_data() {
                    Ok(data) => {
                        // If the size isn't right, ignore this frame and replace
                        // it with a blank one instead. This might happen if
                        // some process is truncating the files, or perhaps if
                        // the disk is full.
                        // We need to check for this because the consequence of
                        // a mismatched size is a panic in a layer where we
                        // cannot handle the error case.
                        if data.len() != expected_byte_size {
                            report_frame_error(format!("frame data is corrupted: expected size {expected_byte_size} but have {}", data.len()));
                            vec![0u8; expected_byte_size]
                        } else {
                            data
                        }
                    }
                    Err(err) => {
                        report_frame_error(format!("frame data error: {err:#}"));
                        vec![0u8; expected_byte_size]
                    }
                };

                let frame = Image::from_raw(
                    frames.current_frame.width,
                    frames.current_frame.height,
                    frame_data,
                );
                let sprite = atlas.allocate_with_padding(&frame, padding, scale_down)?;

                frame_cache.insert(hash, sprite.clone());

                Ok((
                    sprite,
                    Some(*decoded_frame_start + frames.frame_duration().max(min_frame_duration)),
                    frames.load_state,
                ))
            }
        }
    }

    pub fn cached_image(
        &mut self,
        image_data: &Arc<ImageData>,
        padding: Option<usize>,
        allow_image: AllowImage,
    ) -> anyhow::Result<(Sprite, Option<Instant>, LoadState)> {
        let hash = image_data.hash();

        if let Some(decoded) = self.image_cache.get(&hash) {
            Self::cached_image_impl(
                &mut self.frame_cache,
                &mut self.atlas,
                decoded,
                padding,
                self.min_frame_duration,
                allow_image,
            )
        } else {
            let decoded = DecodedImage::load(image_data);
            let res = Self::cached_image_impl(
                &mut self.frame_cache,
                &mut self.atlas,
                &decoded,
                padding,
                self.min_frame_duration,
                allow_image,
            )?;
            self.image_cache.put(hash, decoded);
            Ok(res)
        }
    }

    pub fn cached_color(&mut self, color: RgbColor, alpha: f32) -> anyhow::Result<Sprite> {
        let key = (color, NotNan::new(alpha).unwrap());

        if let Some(s) = self.color.get(&key) {
            return Ok(s.clone());
        }

        let (red, green, blue) = color.to_tuple_rgb8();
        let alpha = (alpha * 255.0) as u8;

        let data = vec![
            red, green, blue, alpha, red, green, blue, alpha, red, green, blue, alpha, red, green,
            blue, alpha,
        ];
        let image = Image::from_raw(2, 2, data);

        let sprite = self.atlas.allocate(&image)?;
        self.color.insert(key, sprite.clone());
        Ok(sprite)
    }

    pub fn cached_block(
        &mut self,
        block: BlockKey,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Sprite> {
        let key = SizedBlockKey {
            block,
            size: metrics.into(),
        };
        if let Some(s) = self.block_glyphs.get(&key) {
            return Ok(s.clone());
        }
        self.block_sprite(metrics, key)
    }

    fn line_sprite(&mut self, key: LineKey, metrics: &RenderMetrics) -> anyhow::Result<Sprite> {
        let mut buffer = Image::new(
            metrics.cell_size.width as usize,
            metrics.cell_size.height as usize,
        );
        let black = SrgbaPixel::rgba(0, 0, 0, 0);
        let white = SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);

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
                );
            }
        };

        let draw_dotted = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                let y = (cell_rect.origin.y + metrics.descender_row + row) as usize;
                if y >= metrics.cell_size.height as usize {
                    break;
                }

                let mut color = white;
                let segment_length = (metrics.cell_size.width / 4) as usize;
                let mut count = segment_length;
                let range =
                    buffer.horizontal_pixel_range_mut(0, metrics.cell_size.width as usize, y);
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
            for row in 0..metrics.underline_height {
                let y = (cell_rect.origin.y + metrics.descender_row + row) as usize;
                if y >= metrics.cell_size.height as usize {
                    break;
                }
                let mut color = white;
                let third = (metrics.cell_size.width / 3) as usize + 1;
                let mut count = third;
                let range =
                    buffer.horizontal_pixel_range_mut(0, metrics.cell_size.width as usize, y);
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
            let max_y = metrics.cell_size.height as usize - 1;
            let x_factor = (2. * std::f32::consts::PI) / metrics.cell_size.width as f32;

            // Have the wave go from the descender to the bottom of the cell
            let wave_height =
                metrics.cell_size.height - (cell_rect.origin.y + metrics.descender_row);

            let half_height = (wave_height as f32 / 4.).max(1.);
            let y = ((cell_rect.origin.y + metrics.descender_row) as usize)
                .saturating_sub(half_height as usize);

            fn add(x: usize, y: usize, val: u8, max_y: usize, buffer: &mut Image) {
                let y = y.min(max_y);
                let pixel = buffer.pixel_mut(x, y);
                let (current, _, _, _) = SrgbaPixel::with_srgba_u32(*pixel).as_rgba();
                let value = current.saturating_add(val);
                *pixel = SrgbaPixel::rgba(value, value, value, value).as_srgba32();
            }

            for x in 0..metrics.cell_size.width as usize {
                let vertical = -half_height * (x as f32 * x_factor).sin() + half_height;
                let v1 = vertical.floor();
                let v2 = vertical.ceil();

                for row in 0..metrics.underline_height as usize {
                    let value = (255. * (vertical - v1).abs()) as u8;
                    add(
                        x,
                        row.saturating_add(y).saturating_add(v1 as usize),
                        255u8.saturating_sub(value),
                        max_y,
                        buffer,
                    );
                    add(
                        x,
                        row.saturating_add(y).saturating_add(v2 as usize),
                        value,
                        max_y,
                        buffer,
                    );
                }
            }
        };

        let draw_double = |buffer: &mut Image| {
            let first_line = metrics
                .descender_row
                .min(metrics.descender_plus_two - 2 * metrics.underline_height);

            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(cell_rect.origin.x, cell_rect.origin.y + first_line + row),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + first_line + row,
                    ),
                    white,
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
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Sprite> {
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
            size: metrics.into(),
        };

        if let Some(s) = self.line_glyphs.get(&key) {
            return Ok(s.clone());
        }

        self.line_sprite(key, metrics)
    }
}
