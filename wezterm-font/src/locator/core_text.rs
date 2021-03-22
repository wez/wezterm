#![cfg(target_os = "macos")]

use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_text::font::*;
use core_text::font_descriptor::*;
use std::borrow::Cow;
use std::collections::HashSet;
use ttf_parser::fonts_in_collection;

lazy_static::lazy_static! {
    static ref FALLBACK: Vec<FontDataHandle> = build_fallback_list();
}

/// A FontLocator implemented using the system font loading
/// functions provided by core text.
pub struct CoreTextFontLocator {}

fn descriptor_from_attr(attr: &FontAttributes) -> anyhow::Result<CTFontDescriptor> {
    let family_name = attr
        .family
        .parse::<CFString>()
        .map_err(|_| anyhow::anyhow!("failed to parse family name {} as CFString", attr.family))?;

    let symbolic_traits: CTFontSymbolicTraits = kCTFontMonoSpaceTrait
        | if attr.bold { kCTFontBoldTrait } else { 0 }
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
    Ok(core_text::font_descriptor::new_from_attributes(&attributes))
}

/// Given a descriptor, return a handle that can be used to open it.
/// The descriptor may not refer to an on-disk font and thus may
/// not have a path.
/// In addition, it may point to a ttc; so we'll need to reference
/// each contained font to figure out which one is the one that
/// the descriptor is referencing.
fn handle_from_descriptor(descriptor: &CTFontDescriptor) -> Option<FontDataHandle> {
    let path = descriptor.font_path()?;
    let name = descriptor.display_name();
    let family_name = descriptor.family_name();

    let data = std::fs::read(&path).ok()?;
    let size = fonts_in_collection(&data).unwrap_or(1);

    let mut handle = FontDataHandle::Memory {
        data: Cow::Owned(data),
        name,
        index: 0,
    };

    for index in 0..size {
        if let FontDataHandle::Memory { index: idx, .. } = &mut handle {
            *idx = index;
        }
        let parsed = crate::parser::ParsedFont::from_locator(&handle).ok()?;
        let names = parsed.names();
        if names.full_name == family_name || names.family.as_ref() == Some(&family_name) {
            // Switch to an OnDisk handle so that we don't hold
            // all of the fallback fonts in memory
            return Some(FontDataHandle::OnDisk { path, index });
        }
    }

    None
}

impl FontLocator for CoreTextFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = vec![];

        for attr in fonts_selection {
            if let Ok(descriptor) = descriptor_from_attr(attr) {
                if let Some(handle) = handle_from_descriptor(&descriptor) {
                    if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                        // The system may have returned a fallback font rather than the
                        // font that we requested, so verify that the name matches.
                        if crate::parser::font_info_matches(attr, parsed.names()) {
                            fonts.push(handle);
                            loaded.insert(attr.clone());
                        }
                    }
                }
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        _codepoints: &[char],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        // We don't have an API to resolve a font for the codepoints, so instead we
        // just get the system fallback list and add the whole thing to the fallback.
        Ok(FALLBACK.clone())
    }
}

fn build_fallback_list() -> Vec<FontDataHandle> {
    build_fallback_list_impl().unwrap_or_else(|err| {
        log::error!("Error getting system fallback fonts: {:#}", err);
        Vec::new()
    })
}

fn build_fallback_list_impl() -> anyhow::Result<Vec<FontDataHandle>> {
    let font =
        new_from_name("Menlo", 0.0).map_err(|_| anyhow::anyhow!("failed to get Menlo font"))?;
    let lang = "en"
        .parse::<CFString>()
        .map_err(|_| anyhow::anyhow!("failed to parse lang name en as CFString"))?;
    let langs = CFArray::from_CFTypes(&[lang]);
    let cascade = cascade_list_for_languages(&font, &langs);
    let mut fonts = vec![];
    for descriptor in &cascade {
        if let Some(handle) = handle_from_descriptor(&descriptor) {
            fonts.push(handle);
        }
    }

    // Some of the fallback fonts are special fonts that don't exist on
    // disk, and that we can't open.
    // In particular, `.AppleSymbolsFB` is one such font.  Let's try
    // a nearby approximation.
    let symbols = FontAttributes {
        family: "Apple Symbols".to_string(),
        bold: false,
        italic: false,
        is_fallback: true,
    };
    if let Ok(descriptor) = descriptor_from_attr(&symbols) {
        if let Some(handle) = handle_from_descriptor(&descriptor) {
            fonts.push(handle);
        }
    }

    Ok(fonts)
}
