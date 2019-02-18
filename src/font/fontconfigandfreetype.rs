//! Systems that use fontconfig and freetype

pub use self::fcwrap::Pattern as FontPattern;
use super::hbwrap as harfbuzz;
use config::{Config, TextStyle};
use failure::{self, Error};
use font::{fcwrap, ftwrap};
use font::{
    shape_with_harfbuzz, FallbackIdx, Font, FontMetrics, FontSystem, GlyphInfo, NamedFont,
    RasterizedGlyph,
};
use std::cell::RefCell;
use std::mem;
use std::slice;

pub type FontSystemImpl = FontConfigAndFreeType;

pub struct FontConfigAndFreeType {}

impl FontConfigAndFreeType {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for FontConfigAndFreeType {
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        let mut pattern = if style.font.len() >= 1 {
            let mut pattern = FontPattern::new()?;
            if style.font.len() > 1 {
                eprintln!(
                    "FIXME: fontconfig loader currently only processes
                      the first in your set of fonts for {:?}",
                    style
                );
            }
            let attr = &style.font[0];
            pattern.family(&attr.family)?;
            if *attr.bold.as_ref().unwrap_or(&false) {
                pattern.add_integer("weight", 200)?;
            }
            if *attr.italic.as_ref().unwrap_or(&false) {
                pattern.add_integer("slant", 100)?;
            }
            pattern
        } else {
            FontPattern::parse(&style.fontconfig_pattern)?
        };
        pattern.add_double("size", config.font_size)?;
        pattern.add_double("dpi", config.dpi)?;

        Ok(Box::new(NamedFontImpl::new(pattern)?))
    }
}

/// Holds a loaded font alternative
struct FontImpl {
    face: RefCell<ftwrap::Face>,
    font: RefCell<harfbuzz::Font>,
    /// nominal monospace cell height
    cell_height: f64,
    /// nominal monospace cell width
    cell_width: f64,
}

impl Font for FontImpl {
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        self.font.borrow_mut().shape(buf, features)
    }
    fn has_color(&self) -> bool {
        let face = self.face.borrow();
        unsafe { ((*face.face).face_flags & i64::from(ftwrap::FT_FACE_FLAG_COLOR)) != 0 }
    }

    fn metrics(&self) -> FontMetrics {
        let face = self.face.borrow();
        FontMetrics {
            cell_height: self.cell_height,
            cell_width: self.cell_width,
            // Note: face.face.descender is useless, we have to go through
            // face.face.size.metrics to get to the real descender!
            descender: unsafe { (*(*face.face).size).metrics.descender as i16 },
        }
    }

    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error> {
        let render_mode = //ftwrap::FT_Render_Mode::FT_RENDER_MODE_NORMAL;
 //       ftwrap::FT_Render_Mode::FT_RENDER_MODE_LCD;
        ftwrap::FT_Render_Mode::FT_RENDER_MODE_LIGHT;

        // when changing the load flags, we also need
        // to change them for harfbuzz otherwise it won't
        // hint correctly
        let load_flags = (ftwrap::FT_LOAD_COLOR) as i32 |
            // enable FT_LOAD_TARGET bits.  There are no flags defined
            // for these in the bindings so we do some bit magic for
            // ourselves.  This is how the FT_LOAD_TARGET_() macro
            // assembles these bits.
            (render_mode as i32) << 16;

        self.font.borrow_mut().set_load_flags(load_flags);
        // This clone is conceptually unsafe, but ok in practice as we are
        // single threaded and don't load any other glyphs in the body of
        // this load_glyph() function.
        let mut face = self.face.borrow_mut();
        let ft_glyph = face.load_and_render_glyph(glyph_pos, load_flags, render_mode)?;

        let mode: ftwrap::FT_Pixel_Mode =
            unsafe { mem::transmute(u32::from(ft_glyph.bitmap.pixel_mode)) };

        // pitch is the number of bytes per source row
        let pitch = ft_glyph.bitmap.pitch.abs() as usize;
        let data = unsafe {
            slice::from_raw_parts_mut(
                ft_glyph.bitmap.buffer,
                ft_glyph.bitmap.rows as usize * pitch,
            )
        };

        let glyph = match mode {
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                let width = ft_glyph.bitmap.width as usize / 3;
                let height = ft_glyph.bitmap.rows as usize;
                let size = (width * height * 4) as usize;
                let mut rgba = Vec::with_capacity(size);
                rgba.resize(size, 0u8);
                for y in 0..height {
                    let src_offset = y * pitch as usize;
                    let dest_offset = y * width * 4;
                    for x in 0..width {
                        let blue = data[src_offset + (x * 3)];
                        let green = data[src_offset + (x * 3) + 1];
                        let red = data[src_offset + (x * 3) + 2];
                        let alpha = red | green | blue;
                        rgba[dest_offset + (x * 4)] = red;
                        rgba[dest_offset + (x * 4) + 1] = green;
                        rgba[dest_offset + (x * 4) + 2] = blue;
                        rgba[dest_offset + (x * 4) + 3] = alpha;
                    }
                }

                RasterizedGlyph {
                    data: rgba,
                    height,
                    width,
                    bearing_x: ft_glyph.bitmap_left,
                    bearing_y: ft_glyph.bitmap_top,
                }
            }
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;

                // emoji glyphs don't always fill the bitmap size, so we compute
                // the non-transparent bounds here with this simplistic code.
                // This can likely be improved!

                let mut first_line = None;
                let mut first_col = None;
                let mut last_col = None;
                let mut last_line = None;

                for y in 0..height {
                    let src_offset = y * pitch as usize;

                    for x in 0..width {
                        let alpha = data[src_offset + (x * 4) + 3];
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
                for y in (0..height).rev() {
                    let src_offset = y * pitch as usize;

                    for x in (0..width).rev() {
                        let alpha = data[src_offset + (x * 4) + 3];
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

                let first_line = first_line.unwrap_or(0);
                let last_line = last_line.unwrap_or(0);
                let first_col = first_col.unwrap_or(0);
                let last_col = last_col.unwrap_or(0);

                let dest_width = 1 + last_col - first_col;
                let dest_height = 1 + last_line - first_line;

                let size = (dest_width * dest_height * 4) as usize;
                let mut rgba = Vec::with_capacity(size);
                rgba.resize(size, 0u8);

                for y in first_line..=last_line {
                    let src_offset = y * pitch as usize;
                    let dest_offset = (y - first_line) * dest_width * 4;
                    for x in first_col..=last_col {
                        let blue = data[src_offset + (x * 4)];
                        let green = data[src_offset + (x * 4) + 1];
                        let red = data[src_offset + (x * 4) + 2];
                        let alpha = data[src_offset + (x * 4) + 3];

                        let dest_x = x - first_col;

                        rgba[dest_offset + (dest_x * 4)] = red;
                        rgba[dest_offset + (dest_x * 4) + 1] = green;
                        rgba[dest_offset + (dest_x * 4) + 2] = blue;
                        rgba[dest_offset + (dest_x * 4) + 3] = alpha;
                    }
                }
                RasterizedGlyph {
                    data: rgba,
                    height: dest_height,
                    width: dest_width,
                    bearing_x: (ft_glyph.bitmap_left as f64 * (dest_width as f64 / width as f64))
                        as i32,
                    bearing_y: (ft_glyph.bitmap_top as f64 * (dest_height as f64 / height as f64))
                        as i32,
                }
            }
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;
                let size = (width * height * 4) as usize;
                let mut rgba = Vec::with_capacity(size);
                rgba.resize(size, 0u8);
                for y in 0..height {
                    let src_offset = y * pitch;
                    let dest_offset = y * width * 4;
                    for x in 0..width {
                        let gray = data[src_offset + x];

                        rgba[dest_offset + (x * 4)] = gray;
                        rgba[dest_offset + (x * 4) + 1] = gray;
                        rgba[dest_offset + (x * 4) + 2] = gray;
                        rgba[dest_offset + (x * 4) + 3] = gray;
                    }
                }
                RasterizedGlyph {
                    data: rgba,
                    height,
                    width,
                    bearing_x: ft_glyph.bitmap_left,
                    bearing_y: ft_glyph.bitmap_top,
                }
            }
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_MONO => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;
                let size = (width * height * 4) as usize;
                let mut rgba = Vec::with_capacity(size);
                rgba.resize(size, 0u8);
                for y in 0..height {
                    let src_offset = y * pitch;
                    let dest_offset = y * width * 4;
                    let mut x = 0;
                    for i in 0..pitch {
                        if x >= width {
                            break;
                        }
                        let mut b = data[src_offset + i];
                        for _ in 0..8 {
                            if x >= width {
                                break;
                            }
                            if b & 0x80 == 0x80 {
                                for j in 0..4 {
                                    rgba[dest_offset + (x * 4) + j] = 0xff;
                                }
                            }
                            b <<= 1;
                            x += 1;
                        }
                    }
                }
                RasterizedGlyph {
                    data: rgba,
                    height,
                    width,
                    bearing_x: ft_glyph.bitmap_left,
                    bearing_y: ft_glyph.bitmap_top,
                }
            }
            mode => bail!("unhandled pixel mode: {:?}", mode),
        };
        Ok(glyph)
    }
}

/// Holds "the" font selected by the user.  In actuality, it
/// holds the set of fallback fonts that match their criteria
pub struct NamedFontImpl {
    lib: ftwrap::Library,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FontImpl>,
}

impl Drop for NamedFontImpl {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

impl NamedFont for NamedFontImpl {
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&Font, Error> {
        Ok(self.get_font(idx)?)
    }
    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        shape_with_harfbuzz(self, 0, s)
    }
}

impl NamedFontImpl {
    /// Construct a new Font from the user supplied pattern
    pub fn new(mut pattern: FontPattern) -> Result<Self, Error> {
        let mut lib = ftwrap::Library::new()?;

        // Some systems don't support this mode, so if it fails, we don't
        // care to abort the rest of what we're doing
        match lib.set_lcd_filter(ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT) {
            Ok(_) => (),
            Err(err) => eprintln!("Ignoring: FT_LcdFilter failed: {:?}", err),
        };

        // Enable some filtering options and pull in the standard
        // fallback font selection from the user configuration
        pattern.monospace()?;
        pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
        pattern.default_substitute();

        // and obtain the selection with the best preference
        // at index 0.
        let font_list = pattern.sort(true)?;

        Ok(Self {
            lib,
            font_list,
            pattern,
            fonts: Vec::new(),
        })
    }

    fn load_next_fallback(&mut self) -> Result<(), Error> {
        let idx = self.fonts.len();
        let pat = self
            .font_list
            .iter()
            .nth(idx)
            .ok_or_else(|| failure::err_msg("no more fallbacks"))?;
        let pat = self.pattern.render_prepare(&pat)?;
        let file = pat.get_file()?;

        debug!("load_next_fallback: file={}", file);
        debug!("{}", pat.format("%{=unparse}")?);

        let size = pat.get_double("size")?;
        let dpi = pat.get_double("dpi")? as u32;
        debug!("set_char_size {} dpi={}", size, dpi);
        // Scaling before truncating to integer minimizes the chances of hitting
        // the fallback code for set_pixel_sizes below.
        let size = (size * 64.0) as i64;

        let mut face = self.lib.new_face(file, 0)?;

        let (cell_width, cell_height) = match face.set_char_size(size, size, dpi, dpi) {
            Ok(_) => {
                // Compute metrics for the nominal monospace cell
                face.cell_metrics()
            }
            Err(err) => {
                let sizes = unsafe {
                    let rec = &(*face.face);
                    slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.is_empty() {
                    return Err(err);
                }
                // Find the best matching size.
                // We just take the biggest.
                let mut best = 0;
                let mut best_size = 0;
                let mut cell_width = 0;
                let mut cell_height = 0;

                for (idx, info) in sizes.iter().enumerate() {
                    let size = best_size.max(info.height);
                    if size > best_size {
                        best = idx;
                        best_size = size;
                        cell_width = info.width;
                        cell_height = info.height;
                    }
                }
                face.select_size(best)?;
                (cell_width as f64, cell_height as f64)
            }
        };

        debug!("metrics: width={} height={}", cell_width, cell_height);
        let font = harfbuzz::Font::new(face.face);

        self.fonts.push(FontImpl {
            face: RefCell::new(face),
            font: RefCell::new(font),
            cell_height,
            cell_width,
        });
        Ok(())
    }

    fn get_font(&mut self, idx: usize) -> Result<&mut FontImpl, Error> {
        if idx >= self.fonts.len() {
            self.load_next_fallback()?;
            ensure!(
                idx < self.fonts.len(),
                "should not ask for a font later than the next prepared font"
            );
        }

        Ok(&mut self.fonts[idx])
    }
}
