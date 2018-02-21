//! Systems that use fontconfig and freetype

pub use self::fcwrap::Pattern as FontPattern;
use config::{Config, TextStyle};
use failure::{self, Error};
use font::{FallbackIdx, Font, FontMetrics, FontSystem, GlyphInfo, NamedFont, shape_with_harfbuzz};
use font::{fcwrap, ftwrap};
use harfbuzz;
use std::cell::RefCell;
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
        let mut pattern = FontPattern::parse(&style.fontconfig_pattern)?;
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
        unsafe { ((*face.face).face_flags & ftwrap::FT_FACE_FLAG_COLOR as i64) != 0 }
    }

    fn metrics(&self) -> FontMetrics {
        let face = self.face.borrow();
        FontMetrics {
            cell_height: self.cell_height,
            cell_width: self.cell_width,
            descender: unsafe { (*face.face).descender },
        }
    }

    fn load_glyph(&self, glyph_pos: u32) -> Result<ftwrap::FT_GlyphSlotRec_, Error> {
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
        self.face
            .borrow_mut()
            .load_and_render_glyph(glyph_pos, load_flags, render_mode)
            .map(|g| g.clone())
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
        lib.set_lcd_filter(
            ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT,
        )?;

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
        let pat = self.font_list.iter().nth(idx).ok_or(failure::err_msg(
            "no more fallbacks",
        ))?;
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

        match face.set_char_size(size, size, dpi, dpi) {
            Err(err) => {
                let sizes = unsafe {
                    let rec = &(*face.face);
                    slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.len() == 0 {
                    return Err(err);
                } else {
                    // Find the best matching size.
                    // We just take the biggest.
                    let mut size = 0i16;
                    for info in sizes.iter() {
                        size = size.max(info.height);
                    }
                    face.set_pixel_sizes(size as u32, size as u32)?;
                    debug!("fall back to set_pixel_sizes {}", size);
                }
            }
            Ok(_) => {}
        }
        let font = harfbuzz::Font::new(face.face);

        // Compute metrics for the nominal monospace cell
        let (cell_width, cell_height) = face.cell_metrics();
        debug!("metrics: width={} height={}", cell_width, cell_height);

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
