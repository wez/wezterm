//! Systems using rust native loader and freetype for rasterizing
use crate::config::{Config, TextStyle};
use crate::font::fontloader;
use crate::font::ftfont::FreeTypeFontImpl;
use crate::font::{ftwrap, FallbackIdx, Font, FontSystem, GlyphInfo, NamedFont};
use failure::Error;

struct NamedFontImpl {
    _lib: ftwrap::Library,
    fonts: Vec<FreeTypeFontImpl>,
    _fontdata: Vec<Vec<u8>>,
}

impl Drop for NamedFontImpl {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

pub type FontSystemImpl = FontLoaderAndFreeType;
pub struct FontLoaderAndFreeType {}
impl FontLoaderAndFreeType {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for FontLoaderAndFreeType {
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        let mut lib = ftwrap::Library::new()?;
        // Some systems don't support this mode, so if it fails, we don't
        // care to abort the rest of what we're doing
        match lib.set_lcd_filter(ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT) {
            Ok(_) => (),
            Err(err) => eprintln!("Ignoring: FT_LcdFilter failed: {:?}", err),
        };

        let mut fonts = Vec::new();
        let mut fontdata = Vec::new();
        for (data, idx) in fontloader::load_system_fonts(config, style)? {
            eprintln!("want idx {} in bytes of len {}", idx, data.len());

            let face = lib.new_face_from_slice(&data, idx.into())?;
            fontdata.push(data);

            fonts.push(FreeTypeFontImpl::with_face_size_and_dpi(
                face,
                config.font_size,
                config.dpi as u32,
            )?);
        }
        Ok(Box::new(NamedFontImpl {
            fonts,
            _lib: lib,
            _fontdata: fontdata,
        }))
    }
}
impl NamedFontImpl {
    fn shape_codepoint(&mut self, c: char, cluster: usize) -> Result<GlyphInfo, Error> {
        for (font_idx, font) in self.fonts.iter().enumerate() {
            let mut info = font.single_glyph_info(c)?;
            if info.glyph_pos == 0 {
                continue;
            }
            info.cluster = cluster as u32;
            info.font_idx = font_idx;
            return Ok(info);
        }
        if c == '?' {
            bail!("no glyph for ?");
        }
        match self.shape_codepoint('?', cluster) {
            Ok(info) => Ok(info),
            Err(_) => bail!("no glyph for {}, and no glyph for fallback ?", c),
        }
    }
}
impl NamedFont for NamedFontImpl {
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&Font, Error> {
        self.fonts
            .get(idx)
            .map(|f| {
                let f: &Font = f;
                f
            })
            .ok_or_else(|| format_err!("no fallback fonts available (idx={})", idx))
    }

    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        let mut shaped = Vec::with_capacity(s.len());

        let mut cluster = 0;
        for c in s.chars() {
            let info = self.shape_codepoint(c, cluster)?;
            cluster += c.len_utf8();
            shaped.push(info);
        }
        Ok(shaped)
    }
}
