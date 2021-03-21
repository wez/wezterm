#![cfg(windows)]

use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use dwrote::{FontDescriptor, FontStretch, FontStyle, FontWeight};
use std::borrow::Cow;
use std::collections::HashSet;
use ttf_parser::fonts_in_collection;
use winapi::shared::windef::HFONT;
use winapi::um::dwrite::*;
use winapi::um::wingdi::{
    CreateCompatibleDC, CreateFontIndirectW, DeleteDC, DeleteObject, GetFontData, SelectObject,
    FIXED_PITCH, GDI_ERROR, LF_FACESIZE, LOGFONTW, OUT_TT_ONLY_PRECIS,
};

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
pub struct GdiFontLocator {}

fn extract_font_data(font: HFONT, attr: &FontAttributes) -> anyhow::Result<FontDataHandle> {
    unsafe {
        let hdc = CreateCompatibleDC(std::ptr::null_mut());
        SelectObject(hdc, font as *mut _);

        // GetFontData can retrieve different parts of the font data.
        // We want to fetch the entire font file, but things are made
        // more complicated because the file may be a TTC file.
        // In that case, the full file data isn't full parsable
        // as a TTF so we need to ask specifically for the TTC file,
        // and then try to reverse engineer which element of the TTC
        // is the one we were looking for.

        // See if we can retrieve the ttc data as a first try
        let ttc_table = 0x66637474; // 'ttcf'

        let ttc_size = GetFontData(hdc, ttc_table, 0, std::ptr::null_mut(), 0);

        let result = if ttc_size > 0 && ttc_size != GDI_ERROR {
            let mut data = vec![0u8; ttc_size as usize];
            GetFontData(hdc, ttc_table, 0, data.as_mut_ptr() as *mut _, ttc_size);

            // Determine which of the contained fonts is the one
            // that we asked for.
            let index =
                crate::parser::resolve_font_from_ttc_data(&attr, &data)?.unwrap_or(0) as u32;
            Ok(FontDataHandle::Memory {
                data: Cow::Owned(data),
                index,
                name: attr.family.clone(),
            })
        } else {
            // Otherwise: presumably a regular ttf

            let size = GetFontData(hdc, 0, 0, std::ptr::null_mut(), 0);
            match size {
                _ if size > 0 && size != GDI_ERROR => {
                    let mut data = vec![0u8; size as usize];
                    GetFontData(hdc, 0, 0, data.as_mut_ptr() as *mut _, size);
                    Ok(FontDataHandle::Memory {
                        data: Cow::Owned(data),
                        index: 0,
                        name: attr.family.clone(),
                    })
                }
                _ => Err(anyhow::anyhow!("Failed to get font data")),
            }
        };

        DeleteDC(hdc);
        result
    }
}

/// Convert a rust string to a windows wide string
fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn load_font(font_attr: &FontAttributes) -> anyhow::Result<FontDataHandle> {
    let mut log_font = LOGFONTW {
        lfHeight: 0,
        lfWidth: 0,
        lfEscapement: 0,
        lfOrientation: 0,
        lfWeight: if font_attr.bold { 700 } else { 0 },
        lfItalic: if font_attr.italic { 1 } else { 0 },
        lfUnderline: 0,
        lfStrikeOut: 0,
        lfCharSet: 0,
        lfOutPrecision: OUT_TT_ONLY_PRECIS as u8,
        lfClipPrecision: 0,
        lfQuality: 0,
        lfPitchAndFamily: FIXED_PITCH as u8,
        lfFaceName: [0u16; 32],
    };

    let name = wide_string(&font_attr.family);
    if name.len() > LF_FACESIZE {
        anyhow::bail!(
            "family name {:?} is too large for LOGFONTW",
            font_attr.family
        );
    }
    for (i, &c) in name.iter().enumerate() {
        log_font.lfFaceName[i] = c;
    }

    unsafe {
        let font = CreateFontIndirectW(&log_font);
        let result = extract_font_data(font, font_attr);
        DeleteObject(font as *mut _);
        result
    }
}

fn attributes_to_descriptor(font_attr: &FontAttributes) -> FontDescriptor {
    FontDescriptor {
        family_name: font_attr.family.to_string(),
        weight: if font_attr.bold {
            FontWeight::Bold
        } else {
            FontWeight::Regular
        },
        stretch: FontStretch::Normal,
        style: if font_attr.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

fn handle_from_descriptor(
    collection: &dwrote::FontCollection,
    descriptor: &FontDescriptor,
) -> Option<FontDataHandle> {
    let font = collection.get_font_from_descriptor(&descriptor)?;
    let face = font.create_font_face();
    for file in face.get_files() {
        if let Some(path) = file.get_font_file_path() {
            let family_name = font.family_name();
            log::debug!("{} -> {}", family_name, path.display());
            if let Ok(data) = std::fs::read(&path) {
                let size = fonts_in_collection(&data).unwrap_or(1);

                let mut handle = FontDataHandle::Memory {
                    data: Cow::Owned(data),
                    name: family_name.clone(),
                    index: 0,
                };

                for index in 0..size {
                    if let FontDataHandle::Memory { index: idx, .. } = &mut handle {
                        *idx = index;
                    }
                    let parsed = crate::parser::ParsedFont::from_locator(&handle).ok()?;
                    let names = parsed.names();
                    if names.full_name == family_name || names.family.as_ref() == Some(&family_name)
                    {
                        // Switch to an OnDisk handle so that we don't hold
                        // all of the fallback fonts in memory
                        return Some(FontDataHandle::OnDisk { path, index });
                    }
                }
            }
        }
    }
    None
}

impl FontLocator for GdiFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = Vec::new();
        let collection = dwrote::FontCollection::system();

        for font_attr in fonts_selection {
            let descriptor = attributes_to_descriptor(font_attr);

            if let Some(handle) = handle_from_descriptor(&collection, &descriptor) {
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    if crate::parser::font_info_matches(font_attr, parsed.names()) {
                        log::debug!("Got {:?} from dwrote", handle);
                        fonts.push(handle);
                        loaded.insert(font_attr.clone());
                        continue;
                    }
                }
            }

            if let Ok(handle) = load_font(font_attr) {
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    if crate::parser::font_info_matches(font_attr, parsed.names()) {
                        fonts.push(handle);
                        loaded.insert(font_attr.clone());
                    }
                }
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let text: Vec<u16> = codepoints
            .iter()
            .map(|&c| c as u16)
            .chain(std::iter::once(0))
            .collect();

        let collection = dwrote::FontCollection::system();
        struct Source {
            locale: String,
            len: u32,
        };
        impl dwrote::TextAnalysisSourceMethods for Source {
            fn get_locale_name<'a>(&'a self, _: u32) -> (Cow<'a, str>, u32) {
                (Cow::Borrowed(&self.locale), self.len)
            }
            fn get_paragraph_reading_direction(&self) -> u32 {
                DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
            }
        }

        let source = dwrote::TextAnalysisSource::from_text(
            Box::new(Source {
                locale: "".to_string(),
                len: codepoints.len() as u32,
            }),
            Cow::Borrowed(&text),
        );

        let mut handles = vec![];
        let mut resolved = HashSet::new();

        if let Some(fallback) = dwrote::FontFallback::get_system_fallback() {
            let mut start = 0usize;
            let mut len = codepoints.len();
            loop {
                let result = fallback.map_characters(
                    &source,
                    start as u32,
                    len as u32,
                    &collection,
                    None,
                    FontWeight::Regular,
                    FontStyle::Normal,
                    FontStretch::Normal,
                );

                if let Some(font) = result.mapped_font {
                    log::trace!(
                        "DirectWrite Suggested fallback: {} {}",
                        font.family_name(),
                        font.face_name()
                    );

                    let attr = FontAttributes {
                        bold: match font.weight() {
                            FontWeight::Thin
                            | FontWeight::ExtraLight
                            | FontWeight::Light
                            | FontWeight::SemiLight
                            | FontWeight::Regular
                            | FontWeight::Medium => false,
                            FontWeight::SemiBold
                            | FontWeight::Bold
                            | FontWeight::ExtraBold
                            | FontWeight::Black
                            | FontWeight::ExtraBlack => true,
                            FontWeight::Unknown(n) => n > 80,
                        },
                        italic: false,
                        family: font.family_name(),
                        is_fallback: true,
                    };

                    if !resolved.contains(&attr) {
                        resolved.insert(attr.clone());

                        let descriptor = attributes_to_descriptor(&attr);
                        if let Some(handle) = handle_from_descriptor(&collection, &descriptor) {
                            handles.push(handle);
                        }
                    }
                }
                if result.mapped_length > 0 {
                    start += result.mapped_length
                } else {
                    break;
                }
                if start == codepoints.len() {
                    break;
                }
                len = codepoints.len() - start;
            }
        } else {
            log::error!("Unable to get system fallback from dwrote");
        }

        Ok(handles)
    }
}
