
use cairo_sys;
use fontconfig::fontconfig::FcPattern;
use freetype::freetype::FT_Face;

pub use cairo::*;

extern "C" {
    pub fn cairo_ft_font_face_create_for_ft_face(
        face: FT_Face,
        load_flags: i32,
    ) -> *mut cairo_sys::cairo_font_face_t;
    pub fn cairo_ft_font_face_create_for_pattern(
        pattern: *mut FcPattern,
    ) -> *mut cairo_sys::cairo_font_face_t;
}
