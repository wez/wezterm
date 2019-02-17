//! Higher level harfbuzz bindings
#![allow(dead_code)]
#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
use freetype;

pub use self::harfbuzz::*;
use harfbuzz_sys as harfbuzz;

use failure::Error;

use std::mem;
use std::ptr;
use std::slice;

#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
extern "C" {
    fn hb_ft_font_set_load_flags(font: *mut hb_font_t, load_flags: i32);
}

pub fn language_from_string(s: &str) -> Result<hb_language_t, Error> {
    unsafe {
        let lang = hb_language_from_string(s.as_ptr() as *const i8, s.len() as i32);
        ensure!(!lang.is_null(), "failed to convert {} to language");
        Ok(lang)
    }
}

pub fn feature_from_string(s: &str) -> Result<hb_feature_t, Error> {
    unsafe {
        let mut feature = mem::zeroed();
        ensure!(
            hb_feature_from_string(
                s.as_ptr() as *const i8,
                s.len() as i32,
                &mut feature as *mut _,
            ) != 0,
            "failed to create feature from {}",
            s
        );
        Ok(feature)
    }
}

pub struct Font {
    font: *mut hb_font_t,
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            hb_font_destroy(self.font);
        }
    }
}

struct Blob {
    blob: *mut hb_blob_t,
}

impl Drop for Blob {
    fn drop(&mut self) {
        unsafe {
            hb_blob_destroy(self.blob);
        }
    }
}

impl Blob {
    fn from_slice(data: &[u8]) -> Result<Self, Error> {
        let blob = unsafe {
            hb_blob_create(
                data.as_ptr() as *const i8,
                data.len() as u32,
                HB_MEMORY_MODE_READONLY,
                ptr::null_mut(),
                None,
            )
        };
        ensure!(!blob.is_null(), "failed to create harfbuzz blob for slice");
        Ok(Self { blob })
    }
}

struct Face {
    face: *mut hb_face_t,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            hb_face_destroy(self.face);
        }
    }
}

impl Face {
    fn from_blob(blob: &Blob, idx: u32) -> Result<Face, Error> {
        let face = unsafe { hb_face_create(blob.blob, idx) };
        ensure!(
            !face.is_null(),
            "failed to create face from blob data at idx {}",
            idx
        );
        Ok(Self { face })
    }
}

impl Font {
    #[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
    /// Create a harfbuzz face from a freetype font
    pub fn new(face: freetype::freetype::FT_Face) -> Font {
        // hb_ft_font_create_referenced always returns a
        // pointer to something, or derefs a nullptr internally
        // if everything fails, so there's nothing for us to
        // test here.
        Font {
            font: unsafe { hb_ft_font_create_referenced(face) },
        }
    }

    /// Create a font from raw data
    /// Harfbuzz doesn't know how to interpret this without registering
    /// some callbacks
    /// FIXME: need to specialize this for rusttype
    pub fn new_from_slice(data: &[u8], idx: u32) -> Result<Font, Error> {
        let blob = Blob::from_slice(data)?;
        let face = Face::from_blob(&blob, idx)?;
        let font = unsafe { hb_font_create(face.face) };
        ensure!(!font.is_null(), "failed to convert face to font");
        Ok(Self { font })
    }

    #[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
    pub fn set_load_flags(&mut self, load_flags: freetype::freetype::FT_Int32) {
        unsafe {
            hb_ft_font_set_load_flags(self.font, load_flags);
        }
    }

    /// Perform shaping.  On entry, Buffer holds the text to shape.
    /// Once done, Buffer holds the output glyph and position info
    pub fn shape(&mut self, buf: &mut Buffer, features: Option<&[hb_feature_t]>) {
        unsafe {
            if let Some(features) = features {
                hb_shape(self.font, buf.buf, features.as_ptr(), features.len() as u32)
            } else {
                hb_shape(self.font, buf.buf, ptr::null(), 0)
            }
        }
    }
}

pub struct Buffer {
    buf: *mut hb_buffer_t,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            hb_buffer_destroy(self.buf);
        }
    }
}

impl Buffer {
    /// Create a new buffer
    pub fn new() -> Result<Buffer, Error> {
        let buf = unsafe { hb_buffer_create() };
        ensure!(
            unsafe { hb_buffer_allocation_successful(buf) } != 0,
            "hb_buffer_create failed"
        );
        Ok(Buffer { buf })
    }

    /// Reset the buffer back to its initial post-creation state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        unsafe {
            hb_buffer_reset(self.buf);
        }
    }

    pub fn set_direction(&mut self, direction: hb_direction_t) {
        unsafe {
            hb_buffer_set_direction(self.buf, direction);
        }
    }

    pub fn set_script(&mut self, script: hb_script_t) {
        unsafe {
            hb_buffer_set_script(self.buf, script);
        }
    }

    pub fn set_language(&mut self, lang: hb_language_t) {
        unsafe {
            hb_buffer_set_language(self.buf, lang);
        }
    }

    #[allow(dead_code)]
    pub fn add(&mut self, codepoint: hb_codepoint_t, cluster: u32) {
        unsafe {
            hb_buffer_add(self.buf, codepoint, cluster);
        }
    }

    pub fn add_utf8(&mut self, buf: &[u8]) {
        unsafe {
            hb_buffer_add_utf8(
                self.buf,
                buf.as_ptr() as *const i8,
                buf.len() as i32,
                0,
                buf.len() as i32,
            );
        }
    }

    pub fn add_str(&mut self, s: &str) {
        self.add_utf8(s.as_bytes())
    }

    /// Returns glyph information.  This is only valid after calling
    /// font->shape() on this buffer instance.
    pub fn glyph_infos(&self) -> &[hb_glyph_info_t] {
        unsafe {
            let mut len: u32 = 0;
            let info = hb_buffer_get_glyph_infos(self.buf, &mut len as *mut _);
            slice::from_raw_parts(info, len as usize)
        }
    }

    /// Returns glyph positions.  This is only valid after calling
    /// font->shape() on this buffer instance.
    pub fn glyph_positions(&self) -> &[hb_glyph_position_t] {
        unsafe {
            let mut len: u32 = 0;
            let pos = hb_buffer_get_glyph_positions(self.buf, &mut len as *mut _);
            slice::from_raw_parts(pos, len as usize)
        }
    }
}
