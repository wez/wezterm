//! Systems using rust native rasterizer

#[cfg(unix)]
use super::hbwrap as harfbuzz;
use config::{Config, TextStyle};
use failure::Error;
use font::{FallbackIdx, Font, FontMetrics, FontSystem, GlyphInfo, NamedFont, RasterizedGlyph};
use font_loader::system_fonts;
use rusttype::{
    point, Codepoint, Font as RTFont, FontCollection, PositionedGlyph, Rect, Scale, ScaledGlyph,
    VMetrics,
};
use unicode_normalization::UnicodeNormalization;

struct NamedFontImpl<'a> {
    fonts: Vec<FontImpl<'a>>,
}

struct FontImpl<'a> {
    _collection: FontCollection<'a>,
    font: RTFont<'a>,
    scale: Scale,
    vmetrics: VMetrics,
    cell_height: f64,
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
        let mut fonts = Vec::new();
        for font_attr in &style.font {
            let mut font_props = system_fonts::FontPropertyBuilder::new()
                .family(&font_attr.family)
                .monospace();
            font_props = if *font_attr.bold.as_ref().unwrap_or(&false) {
                font_props.bold()
            } else {
                font_props
            };
            font_props = if *font_attr.italic.as_ref().unwrap_or(&false) {
                font_props.italic()
            } else {
                font_props
            };
            let font_props = font_props.build();

            let (data, idx) = system_fonts::get(&font_props)
                .ok_or_else(|| format_err!("no font matching {:?}", font_attr))?;
            eprintln!("want idx {} in bytes of len {}", idx, data.len());
            let collection = FontCollection::from_bytes(data)?;
            // Most likely problem is that we matched an OpenType font and rusttype can't
            // load it today.
            let font = collection.font_at(idx as usize).map_err(|e| {
                format_err!(
                    "{:?}: {} (Note that rusttype only supports TrueType font files!)",
                    font_attr,
                    e
                )
            })?;
            eprintln!("made a font for {:?}", font_attr);
            let scale = Scale::uniform(config.font_size as f32 * config.dpi as f32 / 72.0);
            let vmetrics = font.v_metrics(scale);
            eprintln!("vmetrics {:?}", vmetrics);
            let cell_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
            fonts.push(FontImpl {
                _collection: collection,
                cell_height,
                font,
                scale,
                vmetrics,
            });
        }
        Ok(Box::new(NamedFontImpl { fonts }))
    }
}

fn bounds(g: &PositionedGlyph) -> Rect<i32> {
    match g.pixel_bounding_box() {
        Some(bounds) => bounds,
        None => rusttype::Rect {
            min: point(0, 0),
            max: point(0, 0),
        },
    }
}

impl<'a> NamedFontImpl<'a> {
    fn glyph(&mut self, c: char) -> Result<(ScaledGlyph, usize), Error> {
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

        for (cluster, c) in s.nfc().enumerate() {
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
            })
        }
        Ok(shaped)
    }
}

impl<'a> Font for FontImpl<'a> {
    #[cfg(unix)]
    fn harfbuzz_shape(
        &self,
        _buf: &mut harfbuzz::Buffer,
        _features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        unimplemented!();
    }

    fn has_color(&self) -> bool {
        false
    }

    fn metrics(&self) -> FontMetrics {
        let hmetrics = self
            .font
            .glyph(Codepoint(33))
            .scaled(self.scale)
            .h_metrics();
        FontMetrics {
            cell_height: self.cell_height,
            cell_width: hmetrics.advance_width.into(),
            descender: self.vmetrics.descent as i16,
        }
    }

    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error> {
        let g = self
            .font
            .glyph(rusttype::GlyphId(glyph_pos))
            .scaled(self.scale)
            .positioned(point(0.0, 0.0));
        let bounds = bounds(&g);
        let width = bounds.width() as usize;
        let height = bounds.height() as usize;
        let mut data = Vec::with_capacity(width * height * 4);
        g.draw(|_x, _y, value| {
            let v = (value * 255.0) as u8;
            data.push(v); // alpha
            data.push(v); // red
            data.push(v); // green
            data.push(v); // blue
        });
        /*
        eprintln!(
            "rasterize_glyph {} {}x{} {} bounds {:?}",
            glyph_pos, width, height, self.cell_height, bounds
        );
        */
        // FIXME: there's something funky about either the bearing
        // calculation here or the y_offset calculation in the
        // shape function that causes the baseline to vary and
        // the text look crazy.
        Ok(RasterizedGlyph {
            data,
            height,
            width,
            bearing_x: bounds.min.x,
            bearing_y: -bounds.min.y,
        })
    }
}
