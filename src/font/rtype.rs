//! Systems using rust native rasterizer

#[cfg(unix)]
use super::hbwrap as harfbuzz;
use failure::Error;
use font::{Font, FontMetrics, RasterizedGlyph};
use rusttype::{
    point, Codepoint, Font as RTFont, FontCollection, PositionedGlyph, Rect, Scale, VMetrics,
};

pub struct RustTypeFontImpl<'a> {
    _collection: FontCollection<'a>,
    pub(crate) font: RTFont<'a>,
    pub(crate) scale: Scale,
    vmetrics: VMetrics,
    cell_height: f64,
}

impl<'a> RustTypeFontImpl<'a> {
    pub fn from_bytes(data: Vec<u8>, idx: i32, size: f32) -> Result<Self, Error> {
        let collection = FontCollection::from_bytes(data)?;
        // Most likely problem is that we matched an OpenType font and rusttype can't
        // load it today.
        let font = collection.font_at(idx as usize).map_err(|e| {
            format_err!(
                "{} (Note that rusttype only supports TrueType font files!)",
                e
            )
        })?;
        let scale = Scale::uniform(size);
        let vmetrics = font.v_metrics(scale);
        eprintln!("vmetrics {:?}", vmetrics);
        let cell_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
        Ok(RustTypeFontImpl {
            _collection: collection,
            cell_height,
            font,
            scale,
            vmetrics,
        })
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

impl<'a> Font for RustTypeFontImpl<'a> {
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
