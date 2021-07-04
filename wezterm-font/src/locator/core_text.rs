#![cfg(target_os = "macos")]

use crate::locator::{FontDataSource, FontLocator, FontOrigin};
use crate::parser::ParsedFont;
use config::{FontAttributes, FontStretch, FontWeight};
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_text::font::*;
use core_text::font_descriptor::*;
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::collections::HashSet;

lazy_static::lazy_static! {
    static ref FALLBACK: Vec<ParsedFont> = build_fallback_list();
}

/// A FontLocator implemented using the system font loading
/// functions provided by core text.
pub struct CoreTextFontLocator {}

fn descriptor_from_attr(attr: &FontAttributes) -> anyhow::Result<CFArray<CTFontDescriptor>> {
    let family_name = attr
        .family
        .parse::<CFString>()
        .map_err(|_| anyhow::anyhow!("failed to parse family name {} as CFString", attr.family))?;

    let symbolic_traits: CTFontSymbolicTraits = kCTFontMonoSpaceTrait
        | if attr.weight >= FontWeight::BOLD {
            kCTFontBoldTrait
        } else {
            0
        }
        | if attr.stretch < FontStretch::Normal {
            kCTFontCondensedTrait
        } else if attr.stretch > FontStretch::Normal {
            kCTFontExpandedTrait
        } else {
            0
        }
        | if attr.italic { kCTFontItalicTrait } else { 0 };

    let family_attr: CFString = unsafe { TCFType::wrap_under_get_rule(kCTFontFamilyNameAttribute) };
    let traits_attr: CFString = unsafe { TCFType::wrap_under_get_rule(kCTFontTraitsAttribute) };
    let symbolic_traits_attr: CFString =
        unsafe { TCFType::wrap_under_get_rule(kCTFontSymbolicTrait) };

    let traits = CFDictionary::from_CFType_pairs(&[(
        symbolic_traits_attr.as_CFType(),
        CFNumber::from(symbolic_traits as i32).as_CFType(),
    )]);

    let attributes = CFDictionary::from_CFType_pairs(&[
        (traits_attr, traits.as_CFType()),
        (family_attr, family_name.as_CFType()),
    ]);
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
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut fonts = vec![];

        for attr in fonts_selection {
            if let Ok(descriptors) = descriptor_from_attr(attr) {
                let mut handles = vec![];
                for descriptor in descriptors.iter() {
                    handles.append(&mut handles_from_descriptor(&descriptor));
                }
                log::trace!("core text matched {:?} to {:#?}", attr, handles);
                if let Some(parsed) = ParsedFont::best_match(attr, handles) {
                    log::trace!("best match from core text is {:?}", parsed);
                    fonts.push(parsed);
                    loaded.insert(attr.clone());
                }
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut wanted = RangeSet::new();
        for &c in codepoints {
            wanted.add(c as u32);
        }
        let mut matches = vec![];
        for font in FALLBACK.iter() {
            if let Ok(cov) = font.coverage_intersection(&wanted) {
                if !cov.is_empty() {
                    matches.push((cov.len(), font.clone()));
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

fn build_fallback_list() -> Vec<ParsedFont> {
    build_fallback_list_impl().unwrap_or_else(|err| {
        log::error!("Error getting system fallback fonts: {:#}", err);
        Vec::new()
    })
}

fn build_fallback_list_impl() -> anyhow::Result<Vec<ParsedFont>> {
    let font =
        new_from_name("Menlo", 0.0).map_err(|_| anyhow::anyhow!("failed to get Menlo font"))?;
    let lang = "en"
        .parse::<CFString>()
        .map_err(|_| anyhow::anyhow!("failed to parse lang name en as CFString"))?;
    let langs = CFArray::from_CFTypes(&[lang]);
    let cascade = cascade_list_for_languages(&font, &langs);
    let mut fonts = vec![];
    for descriptor in &cascade {
        fonts.append(&mut handles_from_descriptor(&descriptor));
    }

    // Some of the fallback fonts are special fonts that don't exist on
    // disk, and that we can't open.
    // In particular, `.AppleSymbolsFB` is one such font.  Let's try
    // a nearby approximation.
    let symbols = FontAttributes {
        family: "Apple Symbols".to_string(),
        weight: FontWeight::REGULAR,
        stretch: FontStretch::Normal,
        italic: false,
        is_fallback: true,
        is_synthetic: true,
    };
    if let Ok(descriptors) = descriptor_from_attr(&symbols) {
        for descriptor in descriptors.iter() {
            fonts.append(&mut handles_from_descriptor(&descriptor));
        }
    }

    // Constrain to default weight/stretch/style
    fonts.retain(|f| {
        f.weight() == FontWeight::REGULAR && f.stretch() == FontStretch::Normal && !f.italic()
    });

    // Pre-compute coverage
    let empty = RangeSet::new();
    for font in &fonts {
        if let Err(err) = font.coverage_intersection(&empty) {
            log::error!("Error computing coverage for {:?}: {:#}", font, err);
        }
    }

    Ok(fonts)
}
