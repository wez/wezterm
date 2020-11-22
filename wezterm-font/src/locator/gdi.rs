#![cfg(windows)]

use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use std::collections::HashSet;
use winapi::shared::windef::HFONT;
use winapi::um::wingdi::{
    CreateCompatibleDC, CreateFontIndirectW, DeleteDC, DeleteObject, GetFontData, SelectObject,
    FIXED_PITCH, GDI_ERROR, LF_FACESIZE, LOGFONTW, OUT_TT_ONLY_PRECIS,
};

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
pub struct GdiFontLocator {}

fn extract_font_data(font: HFONT, name: &str) -> anyhow::Result<FontDataHandle> {
    unsafe {
        let hdc = CreateCompatibleDC(std::ptr::null_mut());
        SelectObject(hdc, font as *mut _);

        let size = GetFontData(hdc, 0, 0, std::ptr::null_mut(), 0);
        let result = match size {
            _ if size > 0 && size != GDI_ERROR => {
                let mut data = vec![0u8; size as usize];
                GetFontData(hdc, 0, 0, data.as_mut_ptr() as *mut _, size);
                Ok(FontDataHandle::Memory {
                    data,
                    index: 0,
                    name: name.to_string(),
                })
            }
            _ => Err(anyhow::anyhow!("Failed to get font data")),
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

impl FontLocator for GdiFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = Vec::new();
        for font_attr in fonts_selection {
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
                log::error!(
                    "family name {:?} is too large for LOGFONTW",
                    font_attr.family
                );
                continue;
            }
            for (i, &c) in name.iter().enumerate() {
                log_font.lfFaceName[i] = c;
            }

            let handle = unsafe {
                let font = CreateFontIndirectW(&log_font);
                let result = extract_font_data(font, &font_attr.family);
                DeleteObject(font as *mut _);
                result
            };

            if let Ok(handle) = handle {
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
}
