//! Higher level harfbuzz bindings
use freetype;

pub use harfbuzz::*;

use crate::locator::{FontDataHandle, FontDataSource};
use anyhow::{ensure, Context, Error};
use memmap2::{Mmap, MmapOptions};
use std::ffi::CStr;
use std::io::Read;
use std::ops::Range;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::Arc;
use std::{mem, slice};

extern "C" {
    fn hb_ft_font_set_load_flags(font: *mut hb_font_t, load_flags: i32);
}

pub fn language_from_string(s: &str) -> Result<hb_language_t, Error> {
    unsafe {
        let lang = hb_language_from_string(s.as_ptr() as *const c_char, s.len() as i32);
        ensure!(!lang.is_null(), "failed to convert {} to language", s);
        Ok(lang)
    }
}

pub fn feature_from_string(s: &str) -> Result<hb_feature_t, Error> {
    unsafe {
        let mut feature = mem::zeroed();
        ensure!(
            hb_feature_from_string(
                s.as_ptr() as *const c_char,
                s.len() as i32,
                &mut feature as *mut _,
            ) != 0,
            "failed to create feature from {}",
            s
        );
        Ok(feature)
    }
}

pub struct Blob {
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
    pub fn from_source(source: &FontDataSource) -> anyhow::Result<Self> {
        let blob = match source {
            FontDataSource::OnDisk(p) => {
                let mut file = std::fs::File::open(p)
                    .with_context(|| format!("opening file {}", p.display()))?;

                let meta = file
                    .metadata()
                    .with_context(|| format!("querying metadata for {}", p.display()))?;

                if !meta.is_file() {
                    anyhow::bail!("{} is not a file", p.display());
                }

                let len = meta.len();
                if len as usize > c_uint::MAX as usize {
                    anyhow::bail!(
                        "{} is too large to pass to harfbuzz! (len={})",
                        p.display(),
                        len
                    );
                }

                match unsafe { MmapOptions::new().map(&file) } {
                    Ok(map) => {
                        let data_ptr = map.as_ptr();
                        let data_len = map.len() as u32;
                        let user_data = Arc::new(map);

                        let user_data: *const Mmap = Arc::into_raw(user_data);

                        extern "C" fn release_arc_mmap(user_data: *mut c_void) {
                            let user_data = user_data as *mut Mmap;
                            let user_data: Arc<Mmap> = unsafe { Arc::from_raw(user_data) };
                            drop(user_data);
                        }

                        let blob = unsafe {
                            hb_blob_create_or_fail(
                                data_ptr as *const _,
                                data_len,
                                hb_memory_mode_t::HB_MEMORY_MODE_READONLY,
                                user_data as *mut _,
                                Some(release_arc_mmap),
                            )
                        };

                        if blob.is_null() {
                            release_arc_mmap(user_data as *mut _);
                        }

                        blob
                    }
                    Err(err) => {
                        log::warn!(
                            "Unable to memory map {}: {}, will use regular file IO instead",
                            p.display(),
                            err
                        );
                        let mut data = vec![];
                        file.read_to_end(&mut data)
                            .with_context(|| format!("reading font file {}", p.display()))?;
                        let data = Arc::new(data);

                        let data_ptr = data.as_ptr();
                        let data_len = data.len() as u32;
                        let user_data: *const Vec<u8> = Arc::into_raw(data);

                        extern "C" fn release_arc_vec(user_data: *mut c_void) {
                            let user_data = user_data as *mut Vec<u8>;
                            let user_data: Arc<Vec<u8>> = unsafe { Arc::from_raw(user_data) };
                            drop(user_data);
                        }

                        let blob = unsafe {
                            hb_blob_create_or_fail(
                                data_ptr as *const _,
                                data_len,
                                hb_memory_mode_t::HB_MEMORY_MODE_READONLY,
                                user_data as *mut _,
                                Some(release_arc_vec),
                            )
                        };

                        if blob.is_null() {
                            release_arc_vec(user_data as *mut _);
                        }

                        blob
                    }
                }
            }
            FontDataSource::BuiltIn { data, .. } => unsafe {
                hb_blob_create_or_fail(
                    data.as_ptr() as *const _,
                    data.len() as u32,
                    hb_memory_mode_t::HB_MEMORY_MODE_READONLY,
                    std::ptr::null_mut(),
                    None,
                )
            },
            FontDataSource::Memory { data, .. } => {
                let data_ptr = data.as_ptr();
                let data_len = data.len() as u32;
                let user_data: *const Box<[u8]> = Arc::into_raw(Arc::clone(data));

                extern "C" fn release_arc(user_data: *mut c_void) {
                    let user_data = user_data as *const Box<[u8]>;
                    let user_data: Arc<Box<[u8]>> = unsafe { Arc::from_raw(user_data) };
                    drop(user_data);
                }

                let blob = unsafe {
                    hb_blob_create_or_fail(
                        data_ptr as *const _,
                        data_len,
                        hb_memory_mode_t::HB_MEMORY_MODE_READONLY,
                        user_data as *mut _,
                        Some(release_arc),
                    )
                };

                if blob.is_null() {
                    release_arc(user_data as *mut _);
                }

                blob
            }
        };

        if blob.is_null() {
            anyhow::bail!("failed to wrap font as blob");
        }

        Ok(Self { blob })
    }
}

pub struct Face {
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
    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let blob = Blob::from_source(&handle.source)?;
        let mut index = handle.index;
        if handle.variation != 0 {
            index |= handle.variation << 16;
        }

        let face = unsafe { hb_face_create(blob.blob, index) };
        if face.is_null() {
            anyhow::bail!("failed to create harfbuzz Face");
        }

        Ok(Self { face })
    }

    #[allow(dead_code)]
    pub fn get_upem(&self) -> c_uint {
        unsafe { hb_face_get_upem(self.face) }
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

impl Font {
    /// Create a harfbuzz face from a freetype font
    pub fn new(face: freetype::FT_Face) -> Font {
        // hb_ft_font_create_referenced always returns a
        // pointer to something, or derefs a nullptr internally
        // if everything fails, so there's nothing for us to
        // test here.
        Font {
            font: unsafe { hb_ft_font_create_referenced(face as _) },
        }
    }

    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let face = Face::from_locator(handle)?;
        let font = unsafe { hb_font_create(face.face) };
        if font.is_null() {
            anyhow::bail!("failed to create harfbuzz Font");
        }
        Ok(Self { font })
    }

    #[allow(dead_code)]
    pub fn get_face(&self) -> Face {
        let face = unsafe { hb_font_get_face(self.font) };
        unsafe {
            hb_face_reference(face);
        }
        Face { face }
    }

    pub fn set_ot_funcs(&mut self) {
        unsafe {
            hb_ot_font_set_funcs(self.font);
        }
    }

    #[allow(dead_code)]
    pub fn set_ft_funcs(&mut self) {
        unsafe {
            hb_ft_font_set_funcs(self.font);
        }
    }

    pub fn set_font_scale(&mut self, x_scale: c_int, y_scale: c_int) {
        log::info!("setting x_scale={x_scale}, y_scale={y_scale}");
        unsafe {
            hb_font_set_scale(self.font, x_scale, y_scale);
        }
    }

    pub fn set_ppem(&mut self, x_ppem: u32, y_ppem: u32) {
        unsafe {
            hb_font_set_ppem(self.font, x_ppem, y_ppem);
        }
    }

    pub fn set_ptem(&mut self, ptem: f32) {
        unsafe {
            hb_font_set_ptem(self.font, ptem);
        }
    }

    pub fn font_changed(&mut self) {
        unsafe {
            hb_ft_font_changed(self.font);
        }
    }

    pub fn set_load_flags(&mut self, load_flags: freetype::FT_Int32) {
        unsafe {
            hb_ft_font_set_load_flags(self.font, load_flags);
        }
    }

    /// Perform shaping.  On entry, Buffer holds the text to shape.
    /// Once done, Buffer holds the output glyph and position info
    pub fn shape(&mut self, buf: &mut Buffer, features: &[hb_feature_t]) {
        unsafe { hb_shape(self.font, buf.buf, features.as_ptr(), features.len() as u32) }
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

#[allow(dead_code)]
extern "C" fn log_buffer_message(
    _buf: *mut hb_buffer_t,
    _font: *mut hb_font_t,
    message: *const c_char,
    _user_data: *mut c_void,
) -> i32 {
    unsafe {
        if !message.is_null() {
            let message = CStr::from_ptr(message);
            log::info!("{message:?}");
        }
    }

    1
}

impl Buffer {
    /// Create a new buffer
    pub fn new() -> Result<Buffer, Error> {
        let buf = unsafe { hb_buffer_create() };
        ensure!(
            unsafe { hb_buffer_allocation_successful(buf) } != 0,
            "hb_buffer_create failed"
        );
        unsafe {
            hb_buffer_set_content_type(
                buf,
                harfbuzz::hb_buffer_content_type_t::HB_BUFFER_CONTENT_TYPE_UNICODE,
            );

            // hb_buffer_set_message_func(buf, Some(log_buffer_message), std::ptr::null_mut(), None);
        };
        Ok(Buffer { buf })
    }

    /// Reset the buffer back to its initial post-creation state
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        unsafe {
            hb_buffer_reset(self.buf);
        }
    }

    pub fn set_cluster_level(&mut self, level: hb_buffer_cluster_level_t) {
        unsafe {
            hb_buffer_set_cluster_level(self.buf, level);
        }
    }

    pub fn set_direction(&mut self, direction: hb_direction_t) {
        unsafe {
            hb_buffer_set_direction(self.buf, direction);
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn reverse(&mut self) {
        unsafe {
            hb_buffer_reverse_clusters(self.buf);
        }
    }

    pub fn add_str(&mut self, paragraph: &str, range: Range<usize>) {
        let bytes = paragraph.as_bytes();
        unsafe {
            hb_buffer_add_utf8(
                self.buf,
                bytes.as_ptr() as *const c_char,
                bytes.len() as i32,
                range.start as u32,
                (range.end - range.start) as i32,
            );
        }
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

    #[allow(dead_code)]
    pub fn serialize(&self, font: Option<&Font>) -> String {
        unsafe {
            let len = hb_buffer_get_length(self.buf);
            let mut text = vec![0u8; len as usize * 16];
            let buf_len = text.len();
            let mut text_len = 0;
            hb_buffer_serialize(
                self.buf,
                0,
                len,
                text.as_mut_ptr() as *mut _,
                buf_len as _,
                &mut text_len,
                match font {
                    Some(f) => f.font,
                    None => std::ptr::null_mut(),
                },
                harfbuzz::hb_buffer_serialize_format_t::HB_BUFFER_SERIALIZE_FORMAT_TEXT,
                harfbuzz::hb_buffer_serialize_flags_t::HB_BUFFER_SERIALIZE_FLAG_DEFAULT,
            );
            String::from_utf8_lossy(&text[0..text_len as usize]).into()
        }
    }

    pub fn guess_segment_properties(&mut self) {
        unsafe {
            hb_buffer_guess_segment_properties(self.buf);
        }
    }
}
