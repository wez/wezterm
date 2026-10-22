#![cfg(target_os = "macos")]

use crate::locator::{FontDataSource, FontLocator, FontOrigin};
use crate::parser::ParsedFont;
use config::{FontAttributes, FontStretch, FontStyle, FontWeight};
use objc2_core_foundation::{
    kCFPreferencesCurrentApplication, CFArray, CFDictionary, CFPreferencesCopyAppValue, CFRange,
    CFRetained, CFString, CFURL,
};
use objc2_core_text::{
    kCTFontFamilyNameAttribute, kCTFontURLAttribute, CTFont, CTFontCollection, CTFontDescriptor,
};
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::collections::HashSet;

lazy_static::lazy_static! {
    static ref FALLBACK: Vec<ParsedFont> = build_fallback_list();
}

/// A FontLocator implemented using the system font loading
/// functions provided by core text.
pub struct CoreTextFontLocator {}

fn descriptor_from_attr(
    attr: &FontAttributes,
) -> anyhow::Result<CFRetained<CFArray<CTFontDescriptor>>> {
    let family_name = CFString::from_str(&attr.family);

    let family_attr = unsafe { kCTFontFamilyNameAttribute };

    let attributes = CFDictionary::from_slices(&[family_attr], &[&*family_name]);
    let desc = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };

    let array = unsafe { desc.matching_font_descriptors(None) };
    match array {
        Some(array) => Ok(unsafe { CFRetained::cast_unchecked(array) }),
        None => anyhow::bail!("no font matches {:?}", attr),
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
    let path = unsafe { descriptor.attribute(kCTFontURLAttribute) }
        .and_then(|url| url.downcast::<CFURL>().ok())
        .and_then(|url| url.to_file_path());
    if let Some(path) = path {
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

        let menlo =
            unsafe { CTFont::with_name(&CFString::from_str("Menlo"), 0.0, std::ptr::null()) };

        for &c in codepoints {
            let mut wanted = RangeSet::new();
            wanted.add(c as u32);
            let text = CFString::from_str(&c.to_string());

            let font = unsafe { menlo.for_string(&text, CFRange::new(0, 1)) };

            let font_desc = unsafe { font.font_descriptor() };
            let candidates = handles_from_descriptor(&font_desc);

            let mut matched_any = false;

            for font in candidates {
                if font.names().family == ".LastResort"
                    || font.names().postscript_name.as_deref() == Some("LastResort")
                {
                    // Always exclude a last resort font, as it has
                    // placeholder glyphs for everything
                    continue;
                }

                let is_normal = font.weight() == FontWeight::REGULAR
                    && font.stretch() == FontStretch::Normal
                    && font.style() == FontStyle::Normal;
                if !is_normal {
                    // Only use normal attributed text for fallbacks,
                    // otherwise we'll end up picking something with
                    // undefined and undesirable attributes
                    // <https://github.com/wezterm/wezterm/issues/4808>
                    continue;
                }

                if let Ok(cov) = font.coverage_intersection(&wanted) {
                    // Explicitly check coverage because the list may not
                    // actually match the text we asked about(!)
                    if !cov.is_empty() {
                        matches.push((cov.len(), font));
                        matched_any = true;
                    }
                }
            }

            if !matched_any {
                // Consult our global, more general list of fallbacks
                for font in FALLBACK.iter() {
                    if let Ok(cov) = font.coverage_intersection(&wanted) {
                        if !cov.is_empty() {
                            matches.push((cov.len(), font.clone()));
                        }
                    }
                }
            }
        }

        // Add the handles in order of descending coverage; the idea being
        // that if a font has a large coverage then it is probably a better
        // candidate and more likely to result in other glyphs matching
        // in future shaping calls.
        let mut wanted = RangeSet::new();
        for &c in codepoints {
            wanted.add(c as u32);
        }
        for (cov_len, font) in &mut matches {
            if let Ok(cov) = font.coverage_intersection(&wanted) {
                *cov_len = cov.len();
            }
        }

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

        let collection = unsafe { CTFontCollection::from_available_fonts(None) };
        if let Some(descriptors) = unsafe { collection.matching_font_descriptors() } {
            let descriptors =
                unsafe { CFRetained::cast_unchecked::<CFArray<CTFontDescriptor>>(descriptors) };
            for descriptor in descriptors.iter() {
                fonts.append(&mut handles_from_descriptor(&descriptor));
            }
        }

        fonts.sort();
        fonts.dedup();
        Ok(fonts)
    }
}

fn build_fallback_list() -> Vec<ParsedFont> {
    build_fallback_list_impl().unwrap_or_else(|err| {
        log::error!("Error getting system fallback fonts: {:#}", err);
        Vec::new()
    })
}

fn build_fallback_list_impl() -> anyhow::Result<Vec<ParsedFont>> {
    let menlo = unsafe { CTFont::with_name(&CFString::from_str("Menlo"), 0.0, std::ptr::null()) };

    let key = CFString::from_str("AppleLanguages");
    let langs = CFPreferencesCopyAppValue(&key, unsafe { kCFPreferencesCurrentApplication })
        .and_then(|langs| langs.downcast::<CFArray>().ok());

    let cascade = unsafe { menlo.default_cascade_list_for_languages(langs.as_deref()) };
    let mut fonts = vec![];
    // Explicitly include Menlo itself, as it appears to be the only
    // font on macOS that contains U+2718.
    // <https://github.com/wezterm/wezterm/issues/849>
    let menlo_desc = unsafe { menlo.font_descriptor() };
    fonts.append(&mut handles_from_descriptor(&menlo_desc));
    if let Some(cascade) = cascade {
        let cascade = unsafe { CFRetained::cast_unchecked::<CFArray<CTFontDescriptor>>(cascade) };
        for descriptor in cascade.iter() {
            fonts.append(&mut handles_from_descriptor(&descriptor));
        }
    }
    // Some of the fallback fonts are special fonts that don't exist on
    // disk, and that we can't open.
    // In particular, `.AppleSymbolsFB` is one such font.  Let's try
    // a nearby approximation.
    let symbols = FontAttributes {
        family: "Apple Symbols".to_string(),
        weight: FontWeight::REGULAR,
        stretch: FontStretch::Normal,
        style: FontStyle::Normal,
        is_fallback: true,
        is_synthetic: true,
        harfbuzz_features: None,
        freetype_load_target: None,
        freetype_render_target: None,
        freetype_load_flags: None,
        scale: None,
        assume_emoji_presentation: None,
    };
    if let Ok(descriptors) = descriptor_from_attr(&symbols) {
        for descriptor in descriptors.iter() {
            fonts.append(&mut handles_from_descriptor(&descriptor));
        }
    }

    // Constrain to default weight/stretch/style
    fonts.retain(|f| {
        f.weight() == FontWeight::REGULAR
            && f.stretch() == FontStretch::Normal
            && f.style() == FontStyle::Normal
    });

    let mut seen = HashSet::new();
    let fonts: Vec<ParsedFont> = fonts
        .into_iter()
        .filter_map(|f| {
            if seen.contains(&f.handle) {
                None
            } else {
                seen.insert(f.handle.clone());
                Some(f)
            }
        })
        .collect();

    // Pre-compute coverage
    let empty = RangeSet::new();
    for font in &fonts {
        if let Err(err) = font.coverage_intersection(&empty) {
            log::error!("Error computing coverage for {:?}: {:#}", font, err);
        }
    }

    Ok(fonts)
}
