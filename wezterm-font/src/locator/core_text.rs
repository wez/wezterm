#![cfg(target_os="macos")]

use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_text::font_descriptor::*;
use std::collections::HashSet;
use std::path::PathBuf;

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
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

fn font_path_from_descriptor(descriptor: &CTFontDescriptor) -> anyhow::Result<PathBuf> {
    let url: CFURL;
    unsafe {
        let value =
            CTFontDescriptorCopyAttribute(descriptor.as_concrete_TypeRef(), kCTFontURLAttribute);

        if value.is_null() {
            return Err(anyhow::anyhow!("font descriptor has no URL"));
        }

        let value: CFType = TCFType::wrap_under_get_rule(value);
        if !value.instance_of::<CFURL>() {
            return Err(anyhow::anyhow!("font descriptor URL is not a CFURL"));
        }
        url = TCFType::wrap_under_get_rule(std::mem::transmute(value.as_CFTypeRef()));
    }
    if let Some(path) = url.to_path() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!("font descriptor URL is not a path"))
    }
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
                if let Ok(path) = font_path_from_descriptor(&descriptor) {
                    let handle = FontDataHandle::OnDisk { path, index: 0 };

                    if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
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
}
