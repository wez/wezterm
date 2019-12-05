//! Systems that use fontconfig and freetype

pub use self::fcwrap::Pattern as FontPattern;
use crate::config::{Config, TextStyle};
use crate::font::ftfont::FreeTypeFontImpl;
use crate::font::{fcwrap, ftwrap};
use crate::font::{shape_with_harfbuzz, FallbackIdx, Font, FontSystem, GlyphInfo, NamedFont};
use failure::{bail, ensure, err_msg, Error};
use log::{debug, warn};

pub type FontSystemImpl = FontConfigAndFreeType;

pub struct FontConfigAndFreeType {}

impl FontConfigAndFreeType {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for FontConfigAndFreeType {
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<dyn NamedFont>, Error> {
        let mut fonts = vec![];
        for attr in style.font_with_fallback() {
            let mut pattern = FontPattern::new()?;
            pattern.family(&attr.family)?;
            if attr.bold {
                pattern.add_integer("weight", 200)?;
            }
            if attr.italic {
                pattern.add_integer("slant", 100)?;
            }
            pattern.add_double("size", config.font_size * font_scale)?;
            pattern.add_double("dpi", config.dpi)?;
            fonts.push(NamedFontImpl::new(pattern)?);
        }

        if fonts.is_empty() {
            bail!("no fonts specified!?");
        }

        Ok(Box::new(NamedFontListImpl::new(fonts)))
    }
}

pub struct NamedFontListImpl {
    fallback: Vec<NamedFontImpl>,
    fonts: Vec<FreeTypeFontImpl>,
}

impl NamedFontListImpl {
    fn new(fallback: Vec<NamedFontImpl>) -> Self {
        Self {
            fallback,
            fonts: vec![],
        }
    }

    /// We prefer the termwiz config specified set of fallbacks,
    /// so if the user specified two fonts then idx=0 and idx=1
    /// map to those explicit names.  indices idx=2..N are the first
    /// fontconfig provided fallback for idx=0, through Nth fallback.
    /// Index=N+1 is the first fontconfig provided fallback for idx=1
    /// and so on.
    /// This function decodes the idx into the pair of user specified
    /// font and the index into its set of fallbacks
    fn idx_to_fallback(&mut self, idx: usize) -> Option<(&mut NamedFontImpl, usize)> {
        if idx < self.fallback.len() {
            return Some((&mut self.fallback[idx], 0));
        }
        let mut candidate = idx - self.fallback.len();

        for f in &mut self.fallback {
            if candidate < f.font_list_size {
                return Some((f, candidate));
            }
            candidate -= f.font_list_size;
        }
        None
    }

    fn load_next_fallback(&mut self) -> Result<(), Error> {
        let idx = self.fonts.len();
        let (f, idx) = self
            .idx_to_fallback(idx)
            .ok_or_else(|| err_msg("no more fallbacks"))?;
        let pat = f
            .font_list
            .iter()
            .nth(idx)
            .ok_or_else(|| err_msg("no more fallbacks"))?;
        let pat = f.pattern.render_prepare(&pat)?;
        let file = pat.get_file()?;

        debug!("load_next_fallback: file={}", file);
        debug!("{}", pat.format("%{=unparse}")?);

        let size = pat.get_double("size")?;
        let dpi = pat.get_double("dpi")? as u32;
        let face = f.lib.new_face(file, 0)?;
        self.fonts
            .push(FreeTypeFontImpl::with_face_size_and_dpi(face, size, dpi)?);
        Ok(())
    }

    fn get_font(&mut self, idx: usize) -> Result<&mut FreeTypeFontImpl, Error> {
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

impl NamedFont for NamedFontListImpl {
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&dyn Font, Error> {
        Ok(self.get_font(idx)?)
    }
    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        shape_with_harfbuzz(self, 0, s)
    }
}

impl Drop for NamedFontListImpl {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

/// Holds "the" font selected by the user.  In actuality, it
/// holds the set of fallback fonts that match their criteria
pub struct NamedFontImpl {
    lib: ftwrap::Library,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    font_list_size: usize,
}

impl NamedFontImpl {
    /// Construct a new Font from the user supplied pattern
    fn new(mut pattern: FontPattern) -> Result<Self, Error> {
        let mut lib = ftwrap::Library::new()?;

        // Some systems don't support this mode, so if it fails, we don't
        // care to abort the rest of what we're doing
        if let Err(err) = lib.set_lcd_filter(ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT) {
            warn!("Ignoring: FT_LcdFilter failed: {:?}", err);
        };

        // Enable some filtering options and pull in the standard
        // fallback font selection from the user configuration
        pattern.monospace()?;
        debug!("Base pattern {:?}", pattern);

        pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
        pattern.default_substitute();

        // and obtain the selection with the best preference
        // at index 0.
        let font_list = pattern.sort(true)?;
        let font_list_size = font_list.iter().count();

        Ok(Self {
            lib,
            font_list,
            font_list_size,
            pattern,
        })
    }
}
