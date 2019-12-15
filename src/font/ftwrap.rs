//! Higher level freetype bindings

use crate::font::locator::FontDataHandle;
use anyhow::{anyhow, bail, Context};
pub use freetype::*;
use std::ptr;

#[inline]
pub fn succeeded(error: FT_Error) -> bool {
    error == freetype::FT_Err_Ok as FT_Error
}

/// Translate an error and value into a result
fn ft_result<T>(err: FT_Error, t: T) -> anyhow::Result<T> {
    if succeeded(err) {
        Ok(t)
    } else {
        Err(anyhow!("FreeType error {:?} 0x{:x}", err, err))
    }
}

pub fn compute_load_flags_for_mode(render_mode: FT_Render_Mode) -> i32 {
    FT_LOAD_COLOR as i32 |
            // enable FT_LOAD_TARGET bits.  There are no flags defined
            // for these in the bindings so we do some bit magic for
            // ourselves.  This is how the FT_LOAD_TARGET_() macro
            // assembles these bits.
            (render_mode as i32) << 16
}

pub struct Face {
    pub face: FT_Face,
    _bytes: Vec<u8>,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
        }
    }
}

impl Face {
    /// This is a wrapper around set_char_size and select_size
    /// that accounts for some weirdness with eg: color emoji
    pub fn set_font_size(&mut self, size: f64, dpi: u32) -> anyhow::Result<(f64, f64)> {
        log::debug!("set_char_size {} dpi={}", size, dpi);
        // Scaling before truncating to integer minimizes the chances of hitting
        // the fallback code for set_pixel_sizes below.
        let size = (size * 64.0) as FT_F26Dot6;

        let (cell_width, cell_height) = match self.set_char_size(size, size, dpi, dpi) {
            Ok(_) => {
                // Compute metrics for the nominal monospace cell
                self.cell_metrics()
            }
            Err(err) => {
                let sizes = unsafe {
                    let rec = &(*self.face);
                    std::slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.is_empty() {
                    return Err(err);
                }
                // Find the best matching size.
                // We just take the biggest.
                let mut best = 0;
                let mut best_size = 0;
                let mut cell_width = 0;
                let mut cell_height = 0;

                for (idx, info) in sizes.iter().enumerate() {
                    let size = best_size.max(info.height);
                    if size > best_size {
                        best = idx;
                        best_size = size;
                        cell_width = info.width;
                        cell_height = info.height;
                    }
                }
                self.select_size(best)?;
                (f64::from(cell_width), f64::from(cell_height))
            }
        };

        Ok((cell_width, cell_height))
    }

    pub fn set_char_size(
        &mut self,
        char_width: FT_F26Dot6,
        char_height: FT_F26Dot6,
        horz_resolution: FT_UInt,
        vert_resolution: FT_UInt,
    ) -> anyhow::Result<()> {
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

    #[allow(unused)]
    pub fn set_pixel_sizes(&mut self, char_width: u32, char_height: u32) -> anyhow::Result<()> {
        ft_result(
            unsafe { FT_Set_Pixel_Sizes(self.face, char_width, char_height) },
            (),
        )
        .with_context(|| format!("set_pixel_sizes {}x{}", char_width, char_height))
    }

    pub fn select_size(&mut self, idx: usize) -> anyhow::Result<()> {
        ft_result(unsafe { FT_Select_Size(self.face, idx as i32) }, ())
    }

    pub fn load_and_render_glyph(
        &mut self,
        glyph_index: FT_UInt,
        load_flags: FT_Int32,
        render_mode: FT_Render_Mode,
    ) -> anyhow::Result<&FT_GlyphSlotRec_> {
        unsafe {
            let res = FT_Load_Glyph(self.face, glyph_index, load_flags);
            if succeeded(res) {
                let render = FT_Render_Glyph((*self.face).glyph, render_mode);
                if !succeeded(render) {
                    bail!("FT_Render_Glyph failed: {:?}", render);
                }
            }
            ft_result(res, &*(*self.face).glyph)
        }
    }

    pub fn cell_metrics(&mut self) -> (f64, f64) {
        unsafe {
            let metrics = &(*(*self.face).size).metrics;
            let height = (metrics.y_scale as f64 * f64::from((*self.face).height))
                / (f64::from(0x1_0000) * 64.0);

            let mut width = 0.0;
            for i in 32..128 {
                let glyph_pos = FT_Get_Char_Index(self.face, i);
                if glyph_pos == 0 {
                    continue;
                }
                let res = FT_Load_Glyph(self.face, glyph_pos, FT_LOAD_COLOR as i32);
                if succeeded(res) {
                    let glyph = &(*(*self.face).glyph);
                    if glyph.metrics.horiAdvance as f64 > width {
                        width = glyph.metrics.horiAdvance as f64;
                    }
                }
            }
            (width / 64.0, height)
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
    pub fn new() -> anyhow::Result<Library> {
        let mut lib = ptr::null_mut();
        let res = unsafe { FT_Init_FreeType(&mut lib as *mut _) };
        let lib = ft_result(res, lib).context("FT_Init_FreeType")?;
        let mut lib = Library { lib };

        // Some systems don't support this mode, so if it fails, we don't
        // care to abort the rest of what we're doing
        lib.set_lcd_filter(FT_LcdFilter::FT_LCD_FILTER_DEFAULT).ok();

        Ok(lib)
    }

    pub fn face_from_locator(&self, handle: &FontDataHandle) -> anyhow::Result<Face> {
        match handle {
            FontDataHandle::OnDisk { path, index } => {
                self.new_face(path.to_str().unwrap(), *index as _)
            }
            FontDataHandle::Memory { data, index } => self.new_face_from_slice(&data, *index as _),
        }
    }

    #[allow(dead_code)]
    pub fn new_face<P>(&self, path: P, face_index: FT_Long) -> anyhow::Result<Face>
    where
        P: AsRef<std::path::Path>,
    {
        let mut face = ptr::null_mut();
        let path = path.as_ref();

        // We open the file for ourselves and treat it as a memory based
        // face because freetype doesn't use O_CLOEXEC and keeps the fd
        // floating around for a long time!
        let data = std::fs::read(path)?;
        log::error!("Loading {} for freetype!", path.display());

        let res = unsafe {
            FT_New_Memory_Face(
                self.lib,
                data.as_ptr(),
                data.len() as _,
                face_index,
                &mut face as *mut _,
            )
        };
        Ok(Face {
            face: ft_result(res, face).with_context(|| {
                format!(
                    "FT_New_Memory_Face for {} index {}",
                    path.display(),
                    face_index
                )
            })?,
            _bytes: data,
        })
    }

    #[allow(dead_code)]
    pub fn new_face_from_slice(&self, data: &[u8], face_index: FT_Long) -> anyhow::Result<Face> {
        let data = data.to_vec();
        let mut face = ptr::null_mut();

        let res = unsafe {
            FT_New_Memory_Face(
                self.lib,
                data.as_ptr(),
                data.len() as _,
                face_index,
                &mut face as *mut _,
            )
        };
        Ok(Face {
            face: ft_result(res, face)
                .with_context(|| format!("FT_New_Memory_Face for index {}", face_index))?,
            _bytes: data,
        })
    }

    pub fn set_lcd_filter(&mut self, filter: FT_LcdFilter) -> anyhow::Result<()> {
        unsafe { ft_result(FT_Library_SetLcdFilter(self.lib, filter), ()) }
    }
}
