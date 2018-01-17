//! Slightly higher level helper for fontconfig

use failure::{self, Error};
pub use fontconfig::fontconfig::*;
use std::ffi::{CStr, CString};
use std::mem;
use std::ptr;

static FC_MONO: i32 = 100;

pub struct FontSet {
    fonts: *mut FcFontSet,
}

impl Drop for FontSet {
    fn drop(&mut self) {
        unsafe {
            FcFontSetDestroy(self.fonts);
        }
    }
}

pub struct FontSetIter<'a> {
    set: &'a FontSet,
    position: isize,
}

impl<'a> Iterator for FontSetIter<'a> {
    type Item = Pattern;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.position == (*self.set.fonts).nfont as isize {
                None
            } else {
                let pat = *(*self.set.fonts)
                    .fonts
                    .offset(self.position)
                    .as_mut()
                    .unwrap();
                FcPatternReference(pat);
                self.position += 1;
                Some(Pattern { pat })
            }
        }
    }
}

impl FontSet {
    pub fn iter(&self) -> FontSetIter {
        FontSetIter {
            set: &self,
            position: 0,
        }
    }

    pub fn add(&mut self, pat: &Pattern) {
        unsafe {
            FcFontSetAdd(self.fonts, pat.pat);
        }
    }
}

#[repr(C)]
pub enum MatchKind {
    Pattern = FcMatchPattern as isize,
    Font = FcMatchFont as isize,
    Scan = FcMatchScan as isize,
}

pub struct FcResultWrap(FcResult);

impl FcResultWrap {
    pub fn succeeded(&self) -> bool {
        self.0 == FcResultMatch
    }

    pub fn as_err(&self) -> Error {
        // the compiler thinks we defined these globals, when all
        // we did was import them from elsewhere
        #[allow(non_upper_case_globals)]
        match self.0 {
            FcResultMatch => failure::err_msg("FcResultMatch"),
            FcResultNoMatch => failure::err_msg("FcResultNoMatch"),
            FcResultTypeMismatch => failure::err_msg("FcResultTypeMismatch"),
            FcResultNoId => failure::err_msg("FcResultNoId"),
            FcResultOutOfMemory => failure::err_msg("FcResultOutOfMemory"),
            _ => format_err!("FcResult holds invalid value {}", self.0),
        }
    }

    pub fn result<T>(&self, t: T) -> Result<T, Error> {
        match self.0 {
            FcResultMatch => Ok(t),
            _ => Err(self.as_err()),
        }
    }
}

pub struct Pattern {
    pat: *mut FcPattern,
}

impl Pattern {
    pub fn new() -> Result<Pattern, Error> {
        unsafe {
            let p = FcPatternCreate();
            ensure!(!p.is_null(), "FcPatternCreate failed");
            Ok(Pattern { pat: p })
        }
    }

    pub fn add_string(&mut self, key: &str, value: &str) -> Result<(), Error> {
        let key = CString::new(key)?;
        let value = CString::new(value)?;
        unsafe {
            ensure!(
                FcPatternAddString(self.pat, key.as_ptr(), value.as_ptr() as *const u8) != 0,
                "failed to add string property {:?} -> {:?}",
                key,
                value
            );
            Ok(())
        }
    }

    pub fn add_integer(&mut self, key: &str, value: i32) -> Result<(), Error> {
        let key = CString::new(key)?;
        unsafe {
            ensure!(
                FcPatternAddInteger(self.pat, key.as_ptr(), value) != 0,
                "failed to set integer property {:?} -> {}",
                key,
                value
            );
            Ok(())
        }
    }


    pub fn family(&mut self, family: &str) -> Result<(), Error> {
        self.add_string("family", family)
    }

    pub fn monospace(&mut self) -> Result<(), Error> {
        self.add_integer("spacing", FC_MONO)
    }

    pub fn print(&self) {
        unsafe {
            FcPatternPrint(self.pat);
        }
    }

    pub fn format(&self, fmt: &str) -> Result<String, Error> {
        let fmt = CString::new(fmt)?;
        unsafe {
            let s = FcPatternFormat(self.pat, fmt.as_ptr() as *const u8);
            ensure!(!s.is_null(), "failed to format pattern");

            let res = CStr::from_ptr(s as *const i8)
                .to_string_lossy()
                .into_owned();
            FcStrFree(s);
            Ok(res)
        }
    }

    pub fn render_prepare(&self, pat: &Pattern) -> Result<Pattern, Error> {
        unsafe {
            let pat = FcFontRenderPrepare(ptr::null_mut(), self.pat, pat.pat);
            ensure!(!pat.is_null(), "failed to prepare pattern");
            Ok(Pattern { pat })
        }
    }

    pub fn parse(pattern: &str) -> Result<Pattern, Error> {
        let pattern = CString::new(pattern)?;
        unsafe {
            let p = FcNameParse(pattern.as_ptr() as *const u8);
            ensure!(!p.is_null(), "failed to parse {:?}", pattern);
            Ok(Pattern { pat: p })
        }
    }

    pub fn config_substitute(&mut self, match_kind: MatchKind) -> Result<(), Error> {
        unsafe {
            ensure!(
                FcConfigSubstitute(ptr::null_mut(), self.pat, mem::transmute(match_kind)) != 0,
                "FcConfigSubstitute failed"
            );
            Ok(())
        }
    }

    pub fn default_substitute(&mut self) {
        unsafe {
            FcDefaultSubstitute(self.pat);
        }
    }

    pub fn find_match(&self) -> Result<Pattern, Error> {
        unsafe {
            let mut res = FcResultWrap(0);
            let pat = FcFontMatch(ptr::null_mut(), self.pat, &mut res.0 as *mut _);
            res.result(Pattern { pat })
        }
    }

    pub fn sort(&self, trim: bool) -> Result<FontSet, Error> {
        unsafe {
            let mut res = FcResultWrap(0);
            let fonts = FcFontSort(
                ptr::null_mut(),
                self.pat,
                if trim { 1 } else { 0 },
                ptr::null_mut(),
                &mut res.0 as *mut _,
            );

            res.result(FontSet { fonts })
        }
    }

    pub fn get_file(&self) -> Result<String, Error> {
        self.get_string("file")
    }

    pub fn get_string(&self, key: &str) -> Result<String, Error> {
        unsafe {
            let key = CString::new(key)?;
            let mut ptr: *mut u8 = ptr::null_mut();
            let res = FcResultWrap(FcPatternGetString(
                self.pat,
                key.as_ptr(),
                0,
                &mut ptr as *mut *mut u8,
            ));
            if !res.succeeded() {
                Err(res.as_err())
            } else {
                Ok(
                    CStr::from_ptr(ptr as *const i8)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        }
    }
}

impl Drop for Pattern {
    fn drop(&mut self) {
        unsafe {
            FcPatternDestroy(self.pat);
        }
    }
}
