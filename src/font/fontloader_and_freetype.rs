//! Systems using rust native loader and freetype for rasterizing
use crate::config::{Config, TextStyle};
use crate::font::fontloader;
use crate::font::ftfont::FreeTypeFontImpl;
use crate::font::{
    ftwrap, shape_with_harfbuzz, FallbackIdx, Font, FontSystem, GlyphInfo, NamedFont,
};
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
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<NamedFont>, Error> {
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
                config.font_size * font_scale,
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
        shape_with_harfbuzz(self, 0, s)
    }
}
