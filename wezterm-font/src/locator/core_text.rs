#![cfg(target_os = "macos")]

use crate::locator::{FontDataSource, FontLocator, FontOrigin};
use crate::parser::ParsedFont;
use config::FontAttributes;
use core_foundation::array::CFArray;
use core_foundation::base::{CFRange, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use core_text::font::*;
use core_text::font_descriptor::*;
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::collections::HashSet;

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateForString(
        currentFont: CTFontRef,
        string: CFStringRef,
        range: CFRange,
    ) -> CTFontRef;
}

/// A FontLocator implemented using the system font loading
/// functions provided by core text.
pub struct CoreTextFontLocator {}

fn descriptor_from_attr(attr: &FontAttributes) -> anyhow::Result<CFArray<CTFontDescriptor>> {
    let family_name = attr
        .family
        .parse::<CFString>()
        .map_err(|_| anyhow::anyhow!("failed to parse family name {} as CFString", attr.family))?;

    let family_attr: CFString = unsafe { TCFType::wrap_under_get_rule(kCTFontFamilyNameAttribute) };

    let attributes = CFDictionary::from_CFType_pairs(&[(family_attr, family_name.as_CFType())]);
    let desc = core_text::font_descriptor::new_from_attributes(&attributes);

    let array = unsafe {
        core_text::font_descriptor::CTFontDescriptorCreateMatchingFontDescriptors(
            desc.as_concrete_TypeRef(),
            std::ptr::null(),
        )
    };
    if array.is_null() {
        anyhow::bail!("no font matches {:?}", attr);
    } else {
        Ok(unsafe { CFArray::wrap_under_get_rule(array) })
    }
}

/// Given a descriptor, return a handle that can be used to open it.
/// The descriptor may not refer to an on-disk font and thus may
/// not have a path.
/// In addition, it may point to a ttc; so we'll need to reference
/// each contained font to figure out which one is the one that
/// the descriptor is referencing.
fn handles_from_descriptor(descriptor: &CTFontDescriptor) -> Vec<ParsedFont> {
    let mut result = vec![];
    if let Some(path) = descriptor.font_path() {
        let source = FontDataSource::OnDisk(path);
        let _ =
            crate::parser::parse_and_collect_font_info(&source, &mut result, FontOrigin::CoreText);
    }

    result
}

impl FontLocator for CoreTextFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
        pixel_size: u16,
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut fonts = vec![];

        for attr in fonts_selection {
            match descriptor_from_attr(attr) {
                Ok(descriptors) => {
                    let mut handles = vec![];
                    for descriptor in descriptors.iter() {
                        handles.append(&mut handles_from_descriptor(&descriptor));
                    }
                    log::trace!("core text matched {:?} to {:#?}", attr, handles);

                    // If we got a series of .ttc files, we may have a selection of
                    // different font families.  Let's make a first pass a limit
                    // ourselves to name matches
                    let name_matches: Vec<_> = handles
                        .iter()
                        .filter_map(|p| {
                            if p.matches_name(attr) {
                                Some(p.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !name_matches.is_empty() {
                        handles = name_matches;
                    }

                    if let Some(parsed) = ParsedFont::best_match(attr, pixel_size, handles) {
                        log::trace!("best match from core text is {:?}", parsed);
                        fonts.push(parsed);
                        loaded.insert(attr.clone());
                    }
                }
                Err(err) => log::trace!("load_fonts: descriptor_from_attr: {:#}", err),
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut matches = vec![];
        let mut wanted = RangeSet::new();

        let menlo =
            new_from_name("Menlo", 0.0).map_err(|_| anyhow::anyhow!("failed to get Menlo font"))?;

        for &c in codepoints {
            wanted.add(c as u32);
            let text = CFString::new(&c.to_string());

            let font = unsafe {
                CTFontCreateForString(
                    menlo.as_concrete_TypeRef(),
                    text.as_concrete_TypeRef(),
                    CFRange::init(0, 1),
                )
            };

            if font.is_null() {
                continue;
            }

            let font = unsafe { CTFont::wrap_under_create_rule(font) };

            let candidates = handles_from_descriptor(&font.copy_descriptor());

            for font in candidates {
                if let Ok(cov) = font.coverage_intersection(&wanted) {
                    if !cov.is_empty() {
                        matches.push((cov.len(), font));
                    }
                }
            }
        }

        // Add the handles in order of descending coverage; the idea being
        // that if a font has a large coverage then it is probably a better
        // candidate and more likely to result in other glyphs matching
        // in future shaping calls.
        matches.sort_by(|(a_len, a), (b_len, b)| {
            let primary = a_len.cmp(&b_len).reverse();
            if primary == Ordering::Equal {
                a.cmp(b)
            } else {
                primary
            }
        });
        matches.dedup();

        log::trace!("fallback candidates for {codepoints:?} is {matches:#?}");

        Ok(matches.into_iter().map(|(_len, handle)| handle).collect())
    }

    fn enumerate_all_fonts(&self) -> anyhow::Result<Vec<ParsedFont>> {
        let mut fonts = vec![];

        let collection = core_text::font_collection::create_for_all_families();
        if let Some(descriptors) = collection.get_descriptors() {
            for descriptor in descriptors.iter() {
                fonts.append(&mut handles_from_descriptor(&descriptor));
            }
        }

        fonts.sort();
        fonts.dedup();
        Ok(fonts)
    }
}
