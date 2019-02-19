//! Systems that use fontconfig and freetype

pub use self::fcwrap::Pattern as FontPattern;
use crate::config::{Config, TextStyle};
use crate::font::ftfont::FreeTypeFontImpl;
use crate::font::{fcwrap, ftwrap};
use crate::font::{shape_with_harfbuzz, FallbackIdx, Font, FontSystem, GlyphInfo, NamedFont};
use failure::{self, Error};

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

/// Holds "the" font selected by the user.  In actuality, it
/// holds the set of fallback fonts that match their criteria
pub struct NamedFontImpl {
    lib: ftwrap::Library,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FreeTypeFontImpl>,
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
        let face = self.lib.new_face(file, 0)?;
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
