//! Systems using rust native loader and rasterizer
use crate::config::{Config, TextStyle};
use crate::font::fontloader;
use crate::font::rtype::RustTypeFontImpl;
use crate::font::{FallbackIdx, Font, FontSystem, GlyphInfo, NamedFont};
use failure::Error;
use rusttype::{point, Codepoint, ScaledGlyph};

struct NamedFontImpl<'a> {
    fonts: Vec<RustTypeFontImpl<'a>>,
}

pub type FontSystemImpl = RustTypeFonts;
pub struct RustTypeFonts {}
impl RustTypeFonts {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for RustTypeFonts {
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<NamedFont>, Error> {
        let mut fonts = Vec::new();
        for (data, idx) in fontloader::load_system_fonts(config, style)? {
            eprintln!("want idx {} in bytes of len {}", idx, data.len());
            fonts.push(RustTypeFontImpl::from_bytes(
                data,
                idx,
                font_scale as f32 * config.font_size as f32 * config.dpi as f32 / 72.0,
            )?);
        }
        Ok(Box::new(NamedFontImpl { fonts }))
    }
}

impl<'a> NamedFontImpl<'a> {
    pub fn glyph(&mut self, c: char) -> Result<(ScaledGlyph, usize), Error> {
        let codepoint = Codepoint(c as u32);
        for (idx, font) in self.fonts.iter().enumerate() {
            let g = font.font.glyph(codepoint);
            if g.id().0 == 0 {
                // notdef; continue looking in the fallbacks
                continue;
            }
            return Ok((g.scaled(font.scale), idx));
        }
        if c != '?' {
            return match self.glyph('?') {
                Ok((g, idx)) => Ok((g, idx)),
                Err(_) => bail!("unable to resolve glyph for char={} or the fallback ?", c),
            };
        }
        bail!("unable to resolve glyph for ?");
    }
}

impl<'a> NamedFont for NamedFontImpl<'a> {
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
        let mut shaped = Vec::new();

        let mut cluster = 0;
        for c in s.chars() {
            let (glyph, font_idx) = self.glyph(c)?;
            let hmetrics = glyph.h_metrics();
            let glyph = glyph.positioned(point(0.0, 0.0));

            shaped.push(GlyphInfo {
                #[cfg(debug_assertions)]
                text: c.to_string(),
                cluster: cluster as u32,
                num_cells: 1,
                font_idx,
                glyph_pos: glyph.id().0,
                x_advance: hmetrics.advance_width.into(),
                x_offset: (-hmetrics.left_side_bearing).into(),
                y_advance: 0.0,
                y_offset: 0.0, //(-bounds.max.y).into(),
                               // vmetrics.descent.into(),
            });
            cluster += c.len_utf8();
        }
        Ok(shaped)
    }
}
