#![cfg(target_os = "macos")]

use crate::locator::FontDataSource;
use crate::parser::ParsedFont;
use crate::rasterizer::{FontRasterizer, RasterizedGlyph, FAKE_ITALIC_SKEW};
use crate::units::*;
use anyhow::anyhow;
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_graphics::base::{
    kCGBitmapByteOrder32Big, kCGImageAlphaPremultipliedLast, CGFloat,
};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::geometry::{CGAffineTransform, CGPoint};
use core_text::font::{self, CTFont, CTFontRef};
use core_text::font_descriptor::{
    kCTFontBoldTrait, kCTFontColorGlyphsTrait, kCTFontOrientationDefault, CTFontDescriptor,
    CTFontDescriptorRef,
};
use core_text::font_manager::CTFontManagerCreateFontDescriptorsFromURL;
use std::os::raw::c_void;
use std::ptr;

pub struct CoreTextRasterizer {
    ct_font: CTFont,
    has_color: bool,
    synthesize_italic: bool,
    scale: f64,
}

impl CoreTextRasterizer {
    pub fn from_locator(parsed: &ParsedFont) -> anyhow::Result<Self> {
        let ct_font = create_ct_font(
            &parsed.handle.source,
            parsed.handle.index,
            parsed.handle.variation,
        )?;

        let ct_font = if parsed.synthesize_bold {
            ct_font
                .clone_with_symbolic_traits(kCTFontBoldTrait, kCTFontBoldTrait)
                .unwrap_or(ct_font)
        } else {
            ct_font
        };

        let has_color = (ct_font.symbolic_traits() & kCTFontColorGlyphsTrait) != 0;

        Ok(Self {
            ct_font,
            has_color,
            synthesize_italic: parsed.synthesize_italic,
            scale: parsed.scale.unwrap_or(1.0),
        })
    }
}

fn clone_with_transform(font: &CTFont, size: f64, matrix: &CGAffineTransform) -> CTFont {
    unsafe {
        let font_ref = CTFontCreateCopyWithAttributes(
            font.as_concrete_TypeRef(),
            size as CGFloat,
            matrix,
            ptr::null(),
        );
        CTFont::wrap_under_create_rule(font_ref)
    }
}

fn create_ct_font(
    source: &FontDataSource,
    index: u32,
    variation: u32,
) -> anyhow::Result<CTFont> {
    let ct_font = match source {
        FontDataSource::OnDisk(path) => {
            let url = CFURL::from_path(path, false)
                .ok_or_else(|| anyhow!("Failed to create CFURL from path {:?}", path))?;
            let descriptors = unsafe {
                let array_ref = CTFontManagerCreateFontDescriptorsFromURL(
                    url.as_concrete_TypeRef().cast(),
                );
                if array_ref.is_null() {
                    anyhow::bail!(
                        "CTFontManagerCreateFontDescriptorsFromURL returned null for {:?}",
                        path
                    );
                }
                CFArray::<CTFontDescriptor>::wrap_under_create_rule(array_ref)
            };
            let desc = descriptors.get(index as isize).ok_or_else(|| {
                anyhow!(
                    "Font index {} out of range (font has {} faces) in {:?}",
                    index,
                    descriptors.len(),
                    path
                )
            })?;
            font::new_from_descriptor(&desc, 0.0)
        }
        FontDataSource::BuiltIn { data, .. } => ct_font_from_bytes(data, index)?,
        FontDataSource::Memory { data, .. } => ct_font_from_bytes(data, index)?,
    };

    if variation == 0 {
        return Ok(ct_font);
    }

    apply_named_instance(&ct_font, variation)
}

fn apply_named_instance(ct_font: &CTFont, variation: u32) -> anyhow::Result<CTFont> {
    let cg_font = ct_font.copy_to_CGFont();
    let fvar_tag = u32::from_be_bytes(*b"fvar");
    let fvar_data = cg_font
        .copy_table_for_tag(fvar_tag)
        .ok_or_else(|| anyhow!("Font has no fvar table but variation {} requested", variation))?;
    let fvar = fvar_data.bytes();

    if fvar.len() < 16 {
        anyhow::bail!("fvar table too short");
    }

    let axis_count = u16::from_be_bytes([fvar[8], fvar[9]]) as usize;
    let axis_size = u16::from_be_bytes([fvar[10], fvar[11]]) as usize;
    let instance_count = u16::from_be_bytes([fvar[12], fvar[13]]) as usize;
    let instance_size = u16::from_be_bytes([fvar[14], fvar[15]]) as usize;

    let instance_idx = (variation as usize).checked_sub(1).ok_or_else(|| {
        anyhow!("Invalid variation index {}", variation)
    })?;
    if instance_idx >= instance_count {
        anyhow::bail!(
            "Variation {} out of range (font has {} named instances)",
            variation,
            instance_count
        );
    }

    let axes_end = 16 + axis_count * axis_size;
    let instance_offset = axes_end + instance_idx * instance_size;

    if instance_offset + 4 + axis_count * 4 > fvar.len() {
        anyhow::bail!("fvar table truncated");
    }

    let mut axis_tags = Vec::with_capacity(axis_count);
    for i in 0..axis_count {
        let offset = 16 + i * axis_size;
        let tag = u32::from_be_bytes([
            fvar[offset],
            fvar[offset + 1],
            fvar[offset + 2],
            fvar[offset + 3],
        ]);
        axis_tags.push(tag);
    }

    // Instance record: u16 subfamilyNameID, u16 flags, then Fixed[axisCount] coordinates
    let coords_offset = instance_offset + 4;
    let mut pairs: Vec<(CFString, CFNumber)> = Vec::with_capacity(axis_count);
    for (i, &tag) in axis_tags.iter().enumerate() {
        let off = coords_offset + i * 4;
        let raw = i32::from_be_bytes([
            fvar[off],
            fvar[off + 1],
            fvar[off + 2],
            fvar[off + 3],
        ]);
        let value = raw as f64 / 65536.0; // Fixed 16.16 → f64
        let tag_bytes = tag.to_be_bytes();
        let tag_str =
            CFString::new(&String::from_utf8_lossy(&tag_bytes));
        pairs.push((tag_str, CFNumber::from(value)));
    }

    let variations = CFDictionary::from_CFType_pairs(&pairs);
    Ok(font::new_from_CGFont_with_variations(&cg_font, 0.0, &variations))
}

fn ct_font_from_bytes(data: &[u8], index: u32) -> anyhow::Result<CTFont> {
    if index == 0 {
        let descriptor = core_text::font_manager::create_font_descriptor(data)
            .map_err(|_| anyhow!("Failed to create font descriptor from in-memory data"))?;
        return Ok(font::new_from_descriptor(&descriptor, 0.0));
    }

    let cf_data = CFData::from_buffer(data);
    let descriptors = unsafe {
        let array_ref = CTFontManagerCreateFontDescriptorsFromData(cf_data.as_concrete_TypeRef());
        if array_ref.is_null() {
            anyhow::bail!("CTFontManagerCreateFontDescriptorsFromData returned null");
        }
        CFArray::<CTFontDescriptor>::wrap_under_create_rule(array_ref)
    };
    let desc = descriptors.get(index as isize).ok_or_else(|| {
        anyhow!(
            "Font index {} out of range (collection has {} faces)",
            index,
            descriptors.len()
        )
    })?;
    Ok(font::new_from_descriptor(&desc, 0.0))
}

extern "C" {
    fn CTFontManagerCreateFontDescriptorsFromData(
        data: core_foundation::data::CFDataRef,
    ) -> core_foundation::array::CFArrayRef;

    fn CTFontCreateCopyWithAttributes(
        font: CTFontRef,
        size: CGFloat,
        matrix: *const CGAffineTransform,
        attributes: CTFontDescriptorRef,
    ) -> CTFontRef;
}

impl FontRasterizer for CoreTextRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        let pixel_size = size * self.scale * dpi as f64 / 72.0;

        let ct_font = if self.synthesize_italic {
            let skew = CGAffineTransform::new(1.0, 0.0, FAKE_ITALIC_SKEW, 1.0, 0.0, 0.0);
            clone_with_transform(&self.ct_font, pixel_size, &skew)
        } else {
            self.ct_font.clone_with_font_size(pixel_size)
        };

        let glyph: u16 = glyph_pos
            .try_into()
            .map_err(|_| anyhow!("Glyph index {} exceeds CGGlyph (u16) range", glyph_pos))?;
        let bounds =
            ct_font.get_bounding_rects_for_glyphs(kCTFontOrientationDefault, &[glyph]);

        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return Ok(RasterizedGlyph {
                data: vec![],
                height: 0,
                width: 0,
                bearing_x: PixelLength::new(0.0),
                bearing_y: PixelLength::new(0.0),
                has_color: self.has_color,
                is_scaled: false,
            });
        }

        let padding = 2.0;

        let y_min = (bounds.origin.y - padding).floor();
        let y_max = (bounds.origin.y + bounds.size.height + padding).ceil();
        let x_min = (bounds.origin.x - padding).floor();
        let x_max = (bounds.origin.x + bounds.size.width + padding).ceil();

        let width = (x_max - x_min) as usize;
        let height = (y_max - y_min) as usize;

        if width == 0 || height == 0 {
            return Ok(RasterizedGlyph {
                data: vec![],
                height: 0,
                width: 0,
                bearing_x: PixelLength::new(0.0),
                bearing_y: PixelLength::new(0.0),
                has_color: self.has_color,
                is_scaled: false,
            });
        }

        let bytes_per_row = width * 4;
        let mut buffer = vec![0u8; height * bytes_per_row];

        let color_space = CGColorSpace::create_device_rgb();
        let bitmap_info = kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big;

        let ctx = CGContext::create_bitmap_context(
            Some(buffer.as_mut_ptr() as *mut c_void),
            width,
            height,
            8,
            bytes_per_row,
            &color_space,
            bitmap_info,
        );

        ctx.set_allows_font_smoothing(true);
        ctx.set_should_smooth_fonts(true);
        ctx.set_allows_antialiasing(true);
        ctx.set_should_antialias(true);
        ctx.set_allows_font_subpixel_positioning(true);
        ctx.set_should_subpixel_position_fonts(true);

        if !self.has_color {
            ctx.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);
        }

        let origin_x = -x_min;
        let origin_y = -y_min;
        let position = CGPoint::new(origin_x, origin_y);

        ct_font.draw_glyphs(&[glyph], &[position], ctx);

        let bearing_x = x_min;
        let bearing_y = y_max;

        Ok(RasterizedGlyph {
            data: buffer,
            height,
            width,
            bearing_x: PixelLength::new(bearing_x),
            bearing_y: PixelLength::new(bearing_y),
            has_color: self.has_color,
            is_scaled: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::FontOrigin;
    use crate::parser::ParsedFont;
    use std::path::PathBuf;

    fn find_menlo() -> Option<PathBuf> {
        let path = PathBuf::from("/System/Library/Fonts/Menlo.ttc");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    fn make_parsed_font(path: PathBuf) -> ParsedFont {
        let source = FontDataSource::OnDisk(path);
        let mut fonts = vec![];
        crate::parser::parse_and_collect_font_info(&source, &mut fonts, FontOrigin::CoreText);
        fonts.into_iter().next().expect("No fonts parsed from file")
    }

    #[test]
    fn rasterize_glyph_produces_nonempty_output() {
        let path = match find_menlo() {
            Some(p) => p,
            None => {
                eprintln!("Menlo.ttc not found, skipping test");
                return;
            }
        };

        let parsed = make_parsed_font(path);
        let rasterizer =
            CoreTextRasterizer::from_locator(&parsed).expect("Failed to create rasterizer");

        // Rasterize 'A' (glyph index 36 in most fonts, but use cmap-resolved index)
        // Just use glyph index 1 which is typically .notdef or a real glyph
        // Try a few indices to find a renderable glyph
        let mut found_nonempty = false;
        for glyph_idx in 1..70u32 {
            if let Ok(glyph) = rasterizer.rasterize_glyph(glyph_idx, 14.0, 144) {
                if glyph.width > 0 && glyph.height > 0 && !glyph.data.is_empty() {
                    assert_eq!(
                        glyph.data.len(),
                        glyph.width * glyph.height * 4,
                        "Buffer size should match width * height * 4 (RGBA)"
                    );
                    found_nonempty = true;
                    break;
                }
            }
        }
        assert!(found_nonempty, "Should produce at least one non-empty glyph");
    }


    #[test]
    fn rasterizer_does_not_crash_on_zero_glyph() {
        let path = match find_menlo() {
            Some(p) => p,
            None => return,
        };

        let parsed = make_parsed_font(path);
        let rasterizer =
            CoreTextRasterizer::from_locator(&parsed).expect("Failed to create rasterizer");

        // Glyph 0 is typically .notdef — should not crash
        let _ = rasterizer.rasterize_glyph(0, 14.0, 144);
    }

    #[test]
    fn glyphs_with_same_top_edge_have_same_bearing_y() {
        use std::collections::HashMap;

        let path = match find_menlo() {
            Some(p) => p,
            None => {
                eprintln!("Menlo.ttc not found, skipping test");
                return;
            }
        };

        let parsed = make_parsed_font(path);
        let rasterizer =
            CoreTextRasterizer::from_locator(&parsed).expect("Failed to create rasterizer");

        let pixel_size = 14.0 * rasterizer.scale * 144.0 / 72.0;
        let ct_font = rasterizer.ct_font.clone_with_font_size(pixel_size);
        let glyph_count = ct_font.glyph_count() as u32;

        // Group glyphs by their typographic top edge (origin.y + height),
        // rounded to centipixels to absorb float noise.
        // Key: top edge in centipixels, Value: vec of (glyph_id, bearing_y)
        let mut top_edge_groups: HashMap<i64, Vec<(u32, f64)>> = HashMap::new();
        let mut checked = 0u32;

        for gid in 1..glyph_count.min(2000) {
            let glyph = gid as u16;
            let bounds =
                ct_font.get_bounding_rects_for_glyphs(kCTFontOrientationDefault, &[glyph]);

            if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
                continue;
            }

            let g = match rasterizer.rasterize_glyph(gid, 14.0, 144) {
                Ok(g) if g.width > 0 && g.height > 0 => g,
                _ => continue,
            };

            let bearing_y = g.bearing_y.get();

            // bearing_y can exceed bitmap height (glyph sits high above
            // baseline) and can be negative (glyph is entirely below baseline,
            // e.g. combining marks), so no range assertion here.

            let top_edge = bounds.origin.y + bounds.size.height;
            let key = (top_edge * 100.0).round() as i64;
            top_edge_groups
                .entry(key)
                .or_default()
                .push((gid, bearing_y));
            checked += 1;
        }

        let mut failures = Vec::new();
        let mut groups_checked = 0;

        for (key, members) in &top_edge_groups {
            if members.len() < 2 {
                continue;
            }
            groups_checked += 1;
            let expected = members[0].1;
            for &(gid, by) in &members[1..] {
                if (by - expected).abs() > 0.01 {
                    failures.push(format!(
                        "  top_edge={:.2}: glyph {} has bearing_y={:.2}, \
                         expected {:.2} (like glyph {})",
                        *key as f64 / 100.0,
                        gid,
                        by,
                        expected,
                        members[0].0
                    ));
                }
            }
        }

        eprintln!(
            "Checked {checked} glyphs across {groups_checked} alignment groups \
             ({} total groups, {} singleton)",
            top_edge_groups.len(),
            top_edge_groups.values().filter(|v| v.len() == 1).count()
        );

        assert!(
            failures.is_empty(),
            "Glyphs with same typographic top have different bearing_y:\n{}",
            failures.join("\n")
        );
    }
}
