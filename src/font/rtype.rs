//! Systems using rust native rasterizer

use super::hbwrap as harfbuzz;
use config::{Config, TextStyle};
use failure::{self, Error};
use font::{
    shape_with_harfbuzz, FallbackIdx, Font, FontMetrics, FontSystem, GlyphInfo, NamedFont,
    RasterizedGlyph,
};
use font_loader::system_fonts;
use rusttype::{Font as RTFont, FontCollection, Scale};

struct NamedFontImpl<'a> {
    collection: FontCollection<'a>,
    font: RTFont<'a>,
    scale: Scale,
}

pub type FontSystemImpl = RustTypeFonts;
pub struct RustTypeFonts {}
impl RustTypeFonts {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for RustTypeFonts {
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        let font_props = system_fonts::FontPropertyBuilder::new()
            .family(&style.fontconfig_pattern)
            .monospace()
            .build();
        let (data, idx) = system_fonts::get(&font_props)
            .ok_or_else(|| format_err!("no font matching {:?}", style))?;
        let collection = FontCollection::from_bytes(data)?;
        let font = collection.font_at(idx as usize)?;
        eprintln!("made a font for {:?}", style);
        let scale = Scale::uniform(config.font_size as f32 * 96.0 / 72.0);
        Ok(Box::new(NamedFontImpl {
            collection,
            font,
            scale,
        }))
    }
}

impl<'a> NamedFont for NamedFontImpl<'a> {
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&Font, Error> {
        ensure!(idx == 0, "no fallback fonts available");
        Ok(self)
    }
    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        shape_with_harfbuzz(self, 0, s)
    }
}

impl<'a> Font for NamedFontImpl<'a> {
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        unimplemented!();
    }

    fn has_color(&self) -> bool {
        false
    }

    fn metrics(&self) -> FontMetrics {
        let vmetrics = self.font.v_metrics(self.scale);
        let hmetrics = self
            .font
            .glyph(rusttype::Codepoint(33))
            .scaled(self.scale)
            .h_metrics();
        FontMetrics {
            cell_height: f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap),
            cell_width: hmetrics.advance_width.into(),
            descender: vmetrics.descent as i16,
        }
    }

    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error> {
        unimplemented!();
    }
}
