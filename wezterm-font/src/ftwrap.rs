//! Higher level freetype bindings

use crate::locator::FontDataHandle;
use anyhow::{anyhow, Context};
use config::{configuration, FreeTypeLoadTarget};
pub use freetype::*;
use std::borrow::Cow;
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
        unsafe {
            let reason = FT_Error_String(err);
            if reason.is_null() {
                Err(anyhow!("FreeType error {:?} 0x{:x}", err, err))
            } else {
                let reason = std::ffi::CStr::from_ptr(reason);
                Err(anyhow!(
                    "FreeType error {:?} 0x{:x}: {}",
                    err,
                    err,
                    reason.to_string_lossy()
                ))
            }
        }
    }
}

fn render_mode_to_load_target(render_mode: FT_Render_Mode) -> u32 {
    // enable FT_LOAD_TARGET bits.  There are no flags defined
    // for these in the bindings so we do some bit magic for
    // ourselves.  This is how the FT_LOAD_TARGET_() macro
    // assembles these bits.
    (render_mode as u32) & 15 << 16
}

pub fn compute_load_flags_from_config() -> (i32, FT_Render_Mode) {
    let config = configuration();

    let load_flags = config.freetype_load_flags.bits() | FT_LOAD_COLOR;
    let render = match config.freetype_load_target {
        FreeTypeLoadTarget::Mono => FT_Render_Mode::FT_RENDER_MODE_MONO,
        FreeTypeLoadTarget::Normal => FT_Render_Mode::FT_RENDER_MODE_NORMAL,
        FreeTypeLoadTarget::Light => FT_Render_Mode::FT_RENDER_MODE_LIGHT,
        FreeTypeLoadTarget::HorizontalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD,
        FreeTypeLoadTarget::VerticalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD_V,
    };

    let load_flags = load_flags | render_mode_to_load_target(render);

    (load_flags as i32, render)
}

type CowVecU8 = Cow<'static, [u8]>;

pub struct Face {
    pub face: FT_Face,
    _bytes: CowVecU8,
    size: Option<FaceSize>,
}

impl Drop for Face {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
        }
    }
}

struct FaceSize {
    size: f64,
    dpi: u32,
    cell_width: f64,
    cell_height: f64,
}

impl Face {
    /// This is a wrapper around set_char_size and select_size
    /// that accounts for some weirdness with eg: color emoji
    pub fn set_font_size(&mut self, point_size: f64, dpi: u32) -> anyhow::Result<(f64, f64)> {
        if let Some(face_size) = self.size.as_ref() {
            if face_size.size == point_size && face_size.dpi == dpi {
                return Ok((face_size.cell_width, face_size.cell_height));
            }
        }

        let pixel_height = point_size * dpi as f64 / 72.0;
        log::debug!(
            "set_char_size computing {} dpi={} (pixel height={})",
            point_size,
            dpi,
            pixel_height
        );

        // Scaling before truncating to integer minimizes the chances of hitting
        // the fallback code for set_pixel_sizes below.
        let size = (point_size * 64.0) as FT_F26Dot6;

        let (cell_width, cell_height) = match self.set_char_size(size, size, dpi, dpi) {
            Ok(_) => {
                // Compute metrics for the nominal monospace cell
                self.cell_metrics()
            }
            Err(err) => {
                log::debug!("set_char_size: {:?}, will inspect strikes", err);

                let sizes = unsafe {
                    let rec = &(*self.face);
                    std::slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.is_empty() {
                    return Err(err);
                }
                // Find the best matching size; we look for the strike whose height
                // is closest to the desired size.
                struct Best {
                    idx: usize,
                    distance: usize,
                    height: i16,
                    width: i16,
                }
                let mut best: Option<Best> = None;

                for (idx, info) in sizes.iter().enumerate() {
                    log::debug!("idx={} info={:?}", idx, info);
                    let distance = (info.height - (pixel_height as i16)).abs() as usize;
                    let candidate = Best {
                        idx,
                        distance,
                        height: info.height,
                        width: info.width,
                    };

                    match best.take() {
                        Some(existing) => {
                            best.replace(if candidate.distance < existing.distance {
                                candidate
                            } else {
                                existing
                            });
                        }
                        None => {
                            best.replace(candidate);
                        }
                    }
                }
                let best = best.unwrap();
                self.select_size(best.idx)?;
                (f64::from(best.width), f64::from(best.height))
            }
        };

        self.size.replace(FaceSize {
            size: point_size,
            dpi,
            cell_width,
            cell_height,
        });

        Ok((cell_width, cell_height))
    }

    fn set_char_size(
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
        .context("FT_Set_Char_Size")?;

        unsafe {
            if (*self.face).height == 0 {
                anyhow::bail!("font has 0 height, fallback to bitmaps");
            }
        }

        Ok(())
    }

    fn select_size(&mut self, idx: usize) -> anyhow::Result<()> {
        ft_result(unsafe { FT_Select_Size(self.face, idx as i32) }, ()).context("FT_Select_Size")
    }

    pub fn load_and_render_glyph(
        &mut self,
        glyph_index: FT_UInt,
        load_flags: FT_Int32,
        render_mode: FT_Render_Mode,
    ) -> anyhow::Result<&FT_GlyphSlotRec_> {
        unsafe {
            ft_result(FT_Load_Glyph(self.face, glyph_index, load_flags), ()).with_context(
                || {
                    anyhow!(
                        "load_and_render_glyph: FT_Load_Glyph glyph_index:{}",
                        glyph_index
                    )
                },
            )?;
            let slot = &mut *(*self.face).glyph;
            ft_result(FT_Render_Glyph(slot, render_mode), ())
                .context("load_and_render_glyph: FT_Render_Glyph")?;
            Ok(slot)
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
            if width == 0.0 {
                // Most likely we're looking at a symbol font with no latin
                // glyphs at all. Let's just pick a selection of glyphs
                for glyph_pos in 1..8 {
                    let res = FT_Load_Glyph(self.face, glyph_pos, FT_LOAD_COLOR as i32);
                    if succeeded(res) {
                        let glyph = &(*(*self.face).glyph);
                        if glyph.metrics.horiAdvance as f64 > width {
                            width = glyph.metrics.horiAdvance as f64;
                        }
                    }
                }
                if width == 0.0 {
                    log::error!(
                        "Couldn't find any glyphs for metrics, so guessing width == height"
                    );
                    width = height * 64.;
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

        let config = configuration();
        if let Some(vers) = config.freetype_interpreter_version {
            let interpreter_version: FT_UInt = vers;
            unsafe {
                FT_Property_Set(
                    lib.lib,
                    b"truetype\0" as *const u8 as *const FT_String,
                    b"interpreter-version\0" as *const u8 as *const FT_String,
                    &interpreter_version as *const FT_UInt as *const _,
                );
            }
        }

        // Due to patent concerns, the freetype library disables the LCD
        // filtering feature by default, and since we always build our
        // own copy of freetype, it is likewise disabled by default for
        // us too.  As a result, this call will generally fail.
        // Freetype is still able to render a decent result without it!
        lib.set_lcd_filter(FT_LcdFilter::FT_LCD_FILTER_DEFAULT).ok();

        Ok(lib)
    }

    pub fn face_from_locator(&self, handle: &FontDataHandle) -> anyhow::Result<Face> {
        match handle {
            FontDataHandle::OnDisk { path, index } => {
                self.new_face(path.to_str().unwrap(), *index as _)
            }
            FontDataHandle::Memory { data, index, .. } => {
                self.new_face_from_slice(data.clone(), *index as _)
            }
        }
    }

    pub fn new_face<P>(&self, path: P, face_index: FT_Long) -> anyhow::Result<Face>
    where
        P: AsRef<std::path::Path>,
    {
        let mut face = ptr::null_mut();

        if let Some(path_str) = path.as_ref().to_str() {
            if let Ok(path_cstr) = std::ffi::CString::new(path_str) {
                let res = unsafe {
                    FT_New_Face(
                        self.lib,
                        path_cstr.as_ptr(),
                        face_index,
                        &mut face as *mut _,
                    )
                };
                return Ok(Face {
                    face: ft_result(res, face).with_context(|| {
                        format!(
                            "FT_New_Face for {} index {}",
                            path.as_ref().display(),
                            face_index
                        )
                    })?,
                    _bytes: CowVecU8::Borrowed(b""),
                    size: None,
                });
            }
        }

        let path = path.as_ref();

        let data = std::fs::read(path)?;
        log::trace!(
            "Loading {} ({} bytes) for freetype!",
            path.display(),
            data.len()
        );
        let data = CowVecU8::Owned(data);

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
            size: None,
        })
    }

    pub fn new_face_from_slice(&self, data: CowVecU8, face_index: FT_Long) -> anyhow::Result<Face> {
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
            size: None,
        })
    }

    pub fn set_lcd_filter(&mut self, filter: FT_LcdFilter) -> anyhow::Result<()> {
        unsafe {
            ft_result(FT_Library_SetLcdFilter(self.lib, filter), ())
                .context("FT_Library_SetLcdFilter")
        }
    }
}
