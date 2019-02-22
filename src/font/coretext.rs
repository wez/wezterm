//! Render Glyphs on macOS using Core Text
use crate::config::{Config, TextStyle};
use crate::font::hbwrap as harfbuzz;
use crate::font::system::{FallbackIdx, Font, FontMetrics, GlyphInfo, RasterizedGlyph};
use crate::font::{shape_with_harfbuzz, FontSystem, NamedFont};
use core::cell::RefCell;
use core_graphics::base::{kCGBitmapByteOrder32Big, kCGImageAlphaPremultipliedLast};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::font::CGGlyph;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_text::font::{new_from_descriptor, CTFont};
use core_text::font_collection::create_for_family;
use core_text::font_descriptor::{
    kCTFontDefaultOrientation, SymbolicTraitAccessors, TraitAccessors,
};
use failure::Error;
use std::ptr;

#[allow(non_upper_case_globals)]
const kCTFontTraitColorGlyphs: u32 = (1 << 13);

pub type FontSystemImpl = CoreTextSystem;

pub struct CoreTextSystem {}

impl CoreTextSystem {
    pub fn new() -> Self {
        Self {}
    }
}

struct CoreTextFontImpl {
    ct_font: CTFont,
    hb_font: RefCell<harfbuzz::Font>,
    metrics: Metrics,
    has_color: bool,
}

struct NamedFontImpl {
    fonts: Vec<CoreTextFontImpl>,
}

impl FontSystem for CoreTextSystem {
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<NamedFont>, Error> {
        let mut fonts = Vec::new();
        for font_attr in style.font_with_fallback() {
            let col = match create_for_family(&font_attr.family) {
                Some(col) => col,
                None => continue,
            };
            if let Some(desc) = col.get_descriptors() {
                let want_bold = *font_attr.bold.as_ref().unwrap_or(&false);
                let want_italic = *font_attr.italic.as_ref().unwrap_or(&false);
                for d in desc.iter() {
                    let traits = d.traits().symbolic_traits();
                    if want_bold != traits.is_bold() {
                        continue;
                    }
                    if want_italic != traits.is_italic() {
                        continue;
                    }
                    let has_color = (traits & kCTFontTraitColorGlyphs) == kCTFontTraitColorGlyphs;

                    let d = d.clone();
                    let ct_font =
                        new_from_descriptor(&d, font_scale * config.font_size * config.dpi / 72.0);
                    fonts.push(CoreTextFontImpl::new(ct_font, has_color));
                }
            }
        }
        Ok(Box::new(NamedFontImpl { fonts }))
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

/// Resolve a codepoint into a glyph index for subsequent metric lookup
fn glyph_index(ct_font: &CTFont, codepoint: char) -> Option<CGGlyph> {
    // "a buffer of length 2 is large enough to encode any char" says char::encode_utf16().
    let mut buf = [0; 2];
    let encoded = codepoint.encode_utf16(&mut buf);
    let mut glyph: CGGlyph = 0;
    let res = unsafe { ct_font.get_glyphs_for_characters(encoded.as_ptr(), &mut glyph, 1) };
    if res {
        Some(glyph)
    } else {
        None
    }
}

#[derive(Debug)]
struct Metrics {
    font_metrics: FontMetrics,
    ascent: f64,
    descent: f64,
    leading: f64,
}

fn metrics(codepoint: char, ct_font: &CTFont) -> Option<Metrics> {
    let glyph_pos = glyph_index(ct_font, codepoint)?;
    let cell_width = unsafe {
        ct_font.get_advances_for_glyphs(kCTFontDefaultOrientation, &glyph_pos, ptr::null_mut(), 1)
    };

    // ascent - distance from baseline to top of text
    let ascent = ct_font.ascent() as f64;
    // descent - distance from baseline to bottom of text
    let descent = ct_font.descent();
    // leading - additional space between lines of text
    let leading = ct_font.leading() as f64;
    let cell_height = ascent + descent + leading;
    Some(Metrics {
        font_metrics: FontMetrics {
            cell_height,
            cell_width,
            // render.rs divides this value by 64 because freetype returns
            // a scaled integer value, so compensate here
            descender: -64 * descent as i16,
        },
        ascent,
        descent,
        leading,
    })
}

impl CoreTextFontImpl {
    fn new(ct_font: CTFont, has_color: bool) -> Self {
        let hb_font = RefCell::new(harfbuzz::Font::new_coretext(&ct_font));

        let w_metrics = metrics('W', &ct_font);
        let m_metrics = metrics('M', &ct_font);
        let zero_metrics = metrics('0', &ct_font);
        let metrics =
            w_metrics.unwrap_or_else(|| m_metrics.unwrap_or_else(|| zero_metrics.unwrap()));
        Self {
            ct_font,
            hb_font,
            metrics,
            has_color,
        }
    }
}

impl Font for CoreTextFontImpl {
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        self.hb_font.borrow_mut().shape(buf, features)
    }

    fn has_color(&self) -> bool {
        self.has_color
    }

    fn metrics(&self) -> FontMetrics {
        self.metrics.font_metrics
    }

    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error> {
        let rect = self
            .ct_font
            .get_bounding_rects_for_glyphs(kCTFontDefaultOrientation, &[glyph_pos as CGGlyph]);

        let left = rect.origin.x.floor();
        let descent = (-rect.origin.y).ceil();
        let ascent = (rect.size.height + rect.origin.y).ceil();

        let width = (rect.origin.x - left + rect.size.width).ceil() as usize;
        let height = (descent + ascent) as usize;

        if width == 0 || height == 0 {
            return Ok(RasterizedGlyph {
                data: Vec::new(),
                height: 0,
                width: 0,
                bearing_x: 0,
                bearing_y: 0,
            });
        }

        let mut context = CGContext::create_bitmap_context(
            None,
            width,
            height,
            8,
            width * 4,
            &CGColorSpace::create_device_rgb(),
            // Big-endian RGBA
            kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big,
        );

        context.set_rgb_fill_color(0.0, 0.0, 0.0, 0.0);
        context.fill_rect(CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(width as f64, height as f64),
        ));
        context.set_allows_font_smoothing(true);
        context.set_should_smooth_fonts(true);
        context.set_allows_font_subpixel_quantization(true);
        context.set_should_subpixel_quantize_fonts(true);
        context.set_allows_font_subpixel_positioning(true);
        context.set_should_subpixel_position_fonts(true);
        context.set_allows_antialiasing(true);
        context.set_should_antialias(true);
        context.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);

        self.ct_font.draw_glyphs(
            &[glyph_pos as CGGlyph],
            &[CGPoint {
                x: -left,
                y: descent,
            }],
            context.clone(),
        );

        let data = context.data().to_vec();

        let bearing_y = (rect.origin.y + rect.size.height).ceil() as i32;
        Ok(RasterizedGlyph {
            data,
            height,
            width,
            bearing_x: left as i32,
            bearing_y,
        })
    }
}
