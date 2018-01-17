//! Higher level freetype bindings

use failure::Error;
pub use freetype::freetype::*;
use std::ffi::CString;
use std::ptr;

/// Translate an error and value into a result
fn ft_result<T>(err: FT_Error, t: T) -> Result<T, Error> {
    if err.succeeded() {
        Ok(t)
    } else {
        Err(format_err!("FreeType error {:?}", err))
    }
}

pub struct Face {
    pub face: FT_Face,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
        }
    }
}

impl Face {
    pub fn set_char_size(
        &mut self,
        char_width: FT_F26Dot6,
        char_height: FT_F26Dot6,
        horz_resolution: FT_UInt,
        vert_resolution: FT_UInt,
    ) -> Result<(), Error> {
        ft_result(
            unsafe {
                FT_Set_Char_Size(
                    self.face,
                    char_width,
                    char_height,
                    horz_resolution,
                    vert_resolution,
                )
            },
            (),
        )
    }

    pub fn set_pixel_sizes(&mut self, char_width: u32, char_height: u32) -> Result<(), Error> {
        ft_result(
            unsafe {
                FT_Set_Pixel_Sizes(self.face, char_width , char_height )
            },
            (),
        )
    }

    pub fn load_and_render_glyph(
        &mut self,
        glyph_index: FT_UInt,
        load_flags: FT_Int32,
        render_mode: FT_Render_Mode,
    ) -> Result<&FT_GlyphSlotRec_, Error> {
        unsafe {
            let res = FT_Load_Glyph(self.face, glyph_index, load_flags);
            if res.succeeded() {
                FT_Render_Glyph((*self.face).glyph, render_mode);
            }
            ft_result(res, &*(*self.face).glyph)
        }
    }

    pub fn cell_metrics(&mut self) -> (i64, i64) {
        unsafe {
            let metrics = &(*(*self.face).size).metrics;
            let height = FT_MulFix(metrics.y_scale, (*self.face).height as i64) / 64;

            let mut width = 0;
            for i in 32..128 {
                let glyph_pos = FT_Get_Char_Index(self.face, i);
                let res = FT_Load_Glyph(self.face, glyph_pos, FT_LOAD_COLOR as i32);
                if res.succeeded() {
                    let glyph = &(*(*self.face).glyph);
                    width = width.max(((glyph.metrics.horiAdvance as f64) / 64f64).ceil() as i64);
                }
            }
            (width, height)
        }
    }
}

pub struct Library {
    lib: FT_Library,
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FT_Done_FreeType(self.lib);
        }
    }
}

impl Library {
    pub fn new() -> Result<Library, Error> {
        let mut lib = ptr::null_mut();
        let res = unsafe { FT_Init_FreeType(&mut lib as *mut _) };
        let lib = ft_result(res, lib)?;
        Ok(Library { lib })
    }

    pub fn new_face<P>(&self, path: P, face_index: FT_Long) -> Result<Face, Error>
    where
        P: Into<Vec<u8>>,
    {
        let mut face = ptr::null_mut();
        let path = CString::new(path.into())?;

        let res = unsafe { FT_New_Face(self.lib, path.as_ptr(), face_index, &mut face as *mut _) };
        Ok(Face { face: ft_result(res, face)? })
    }

    pub fn set_lcd_filter(&mut self, filter: FT_LcdFilter) -> Result<(), Error> {
        unsafe { ft_result(FT_Library_SetLcdFilter(self.lib, filter), ()) }
    }
}
