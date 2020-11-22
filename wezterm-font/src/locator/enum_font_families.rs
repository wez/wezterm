#![cfg(windows)]

use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use winapi::ctypes::c_int;
use winapi::shared::minwindef::{DWORD, LPARAM};
use winapi::um::wingdi::{
    CreateCompatibleDC, CreateFontIndirectW, DeleteDC, EnumFontFamiliesExW, GetFontData,
    SelectObject, ENUMLOGFONTEXW, FIXED_PITCH, GDI_ERROR, LF_FULLFACESIZE, LOGFONTW,
    OUT_TT_ONLY_PRECIS, TEXTMETRICW, TRUETYPE_FONTTYPE,
};

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
pub struct EnumFontFamiliesFontLocator {}

struct Entry {
    log_font: ENUMLOGFONTEXW,
    name: String,
}

impl Entry {
    fn locator(&self) -> anyhow::Result<FontDataHandle> {
        unsafe {
            let hdc = CreateCompatibleDC(std::ptr::null_mut());
            let font = CreateFontIndirectW(&self.log_font.elfLogFont);
            SelectObject(hdc, font as *mut _);
            let size = GetFontData(hdc, 0, 0, std::ptr::null_mut(), 0);
            let result = match size {
                _ if size > 0 && size != GDI_ERROR => {
                    let mut data = vec![0u8; size as usize];
                    GetFontData(hdc, 0, 0, data.as_mut_ptr() as *mut _, size);
                    Ok(FontDataHandle::Memory {
                        data,
                        index: 0,
                        name: self.name.clone(),
                    })
                }
                _ => Err(anyhow::anyhow!("Failed to get font data")),
            };
            DeleteDC(hdc);
            result
        }
    }
}

#[allow(non_snake_case)]
unsafe extern "system" fn callback(
    lpelfe: *const LOGFONTW,
    _: *const TEXTMETRICW,
    fonttype: DWORD,
    lparam: LPARAM,
) -> c_int {
    let log_font: &ENUMLOGFONTEXW = &*(lpelfe as *const ENUMLOGFONTEXW);
    if fonttype == TRUETYPE_FONTTYPE && log_font.elfFullName[0] != b'@' as u16 {
        let fonts = lparam as *mut Vec<Entry>;

        let len = log_font
            .elfFullName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(LF_FULLFACESIZE);
        if let Ok(name) = OsString::from_wide(&log_font.elfFullName[0..len]).into_string() {
            (*fonts).push(Entry {
                log_font: *log_font,
                name,
            });
        }
    }
    1 // continue enumeration
}

impl FontLocator for EnumFontFamiliesFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut log_font = LOGFONTW {
            lfHeight: 0,
            lfWidth: 0,
            lfEscapement: 0,
            lfOrientation: 0,
            lfWeight: 0,
            lfItalic: 0,
            lfUnderline: 0,
            lfStrikeOut: 0,
            lfCharSet: 0,
            lfOutPrecision: OUT_TT_ONLY_PRECIS as u8,
            lfClipPrecision: 0,
            lfQuality: 0,
            lfPitchAndFamily: FIXED_PITCH as u8,
            lfFaceName: [0u16; 32],
        };

        let mut sys_fonts: Vec<Entry> = vec![];
        unsafe {
            let hdc = CreateCompatibleDC(std::ptr::null_mut());
            EnumFontFamiliesExW(
                hdc,
                &mut log_font,
                Some(callback),
                &mut sys_fonts as *mut _ as LPARAM,
                0,
            );
            DeleteDC(hdc);
        }

        let mut handles = vec![];
        for font in sys_fonts {
            if let Ok(handle) = font.locator() {
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    handles.push((handle, parsed));
                }
            }
        }

        let mut fonts = Vec::new();
        for font_attr in fonts_selection {
            for (handle, parsed) in &handles {
                if crate::parser::font_info_matches(font_attr, parsed.names()) {
                    fonts.push(handle.clone());
                    loaded.insert(font_attr.clone());
                }
            }
        }

        Ok(fonts)
    }
}
