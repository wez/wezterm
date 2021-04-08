//! Higher level freetype bindings

use crate::locator::FontDataHandle;
use crate::parser::ParsedFont;
use anyhow::{anyhow, Context};
use config::{configuration, FreeTypeLoadTarget};
pub use freetype::*;
use rangeset::RangeSet;
use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::CStr;
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

    fn target_to_render(t: FreeTypeLoadTarget) -> FT_Render_Mode {
        match t {
            FreeTypeLoadTarget::Mono => FT_Render_Mode::FT_RENDER_MODE_MONO,
            FreeTypeLoadTarget::Normal => FT_Render_Mode::FT_RENDER_MODE_NORMAL,
            FreeTypeLoadTarget::Light => FT_Render_Mode::FT_RENDER_MODE_LIGHT,
            FreeTypeLoadTarget::HorizontalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD,
            FreeTypeLoadTarget::VerticalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD_V,
        }
    }

    let load_target = target_to_render(config.freetype_load_target);
    let render = target_to_render(
        config
            .freetype_render_target
            .unwrap_or(config.freetype_load_target),
    );

    let load_flags = load_flags | render_mode_to_load_target(load_target);

    (load_flags as i32, render)
}

type CowVecU8 = Cow<'static, [u8]>;

pub struct Face {
    pub face: FT_Face,
    _bytes: CowVecU8,
    size: Option<FaceSize>,
    lib: FT_Library,
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
    pub fn family_name(&self) -> String {
        unsafe {
            if (*self.face).family_name.is_null() {
                "".to_string()
            } else {
                let c = CStr::from_ptr((*self.face).family_name);
                c.to_string_lossy().to_string()
            }
        }
    }

    pub fn style_name(&self) -> String {
        unsafe {
            if (*self.face).style_name.is_null() {
                "".to_string()
            } else {
                let c = CStr::from_ptr((*self.face).style_name);
                c.to_string_lossy().to_string()
            }
        }
    }

    pub fn postscript_name(&self) -> String {
        unsafe {
            let c = FT_Get_Postscript_Name(self.face);
            if c.is_null() {
                "".to_string()
            } else {
                let c = CStr::from_ptr(c);
                c.to_string_lossy().to_string()
            }
        }
    }

    pub fn variations(&self) -> anyhow::Result<Vec<ParsedFont>> {
        let mut mm = std::ptr::null_mut();

        unsafe {
            ft_result(FT_Get_MM_Var(self.face, &mut mm), ()).context("FT_Get_MM_Var")?;

            let mut res = vec![];
            let num_styles = (*mm).num_namedstyles;
            for i in 1..=num_styles {
                FT_Set_Named_Instance(self.face, i);
                res.push(ParsedFont::from_face(&self)?);
            }

            FT_Done_MM_Var(self.lib, mm);
            FT_Set_Named_Instance(self.face, 0);

            log::debug!("Variations: {:#?}", res);

            Ok(res)
        }
    }

    pub fn get_os2_table(&self) -> Option<&TT_OS2> {
        unsafe {
            let os2: *const TT_OS2 = FT_Get_Sfnt_Table(self.face, FT_Sfnt_Tag::FT_SFNT_OS2) as _;
            if os2.is_null() {
                None
            } else {
                Some(&*os2)
            }
        }
    }

    pub fn weight_and_width(&self) -> (u16, u16) {
        let (mut weight, mut width) = self
            .get_os2_table()
            .map(|os2| (os2.usWeightClass as f64, os2.usWidthClass as f64))
            .unwrap_or((400., 5.));

        unsafe {
            let index = (*self.face).face_index;
            let variation = index >> 16;
            if variation > 0 {
                let vidx = (variation - 1) as usize;

                let mut mm = std::ptr::null_mut();

                ft_result(FT_Get_MM_Var(self.face, &mut mm), ())
                    .context("FT_Get_MM_Var")
                    .unwrap();
                {
                    let mm = &*mm;

                    let styles =
                        std::slice::from_raw_parts(mm.namedstyle, mm.num_namedstyles as usize);
                    let instance = &styles[vidx];
                    let axes = std::slice::from_raw_parts(mm.axis, mm.num_axis as usize);

                    fn ft_make_tag(a: u8, b: u8, c: u8, d: u8) -> FT_ULong {
                        (a as FT_ULong) << 24
                            | (b as FT_ULong) << 16
                            | (c as FT_ULong) << 8
                            | (d as FT_ULong)
                    }

                    for (i, axis) in axes.iter().enumerate() {
                        let coords =
                            std::slice::from_raw_parts(instance.coords, mm.num_axis as usize);
                        let value = coords[i] as f64 / (1 << 16) as f64;
                        let default_value = axis.def as f64 / (1 << 16) as f64;
                        let scale = if default_value != 0. {
                            value / default_value
                        } else {
                            1.
                        };

                        if axis.tag == ft_make_tag(b'w', b'g', b'h', b't') {
                            weight = weight * scale;
                        }

                        if axis.tag == ft_make_tag(b'w', b'd', b't', b'h') {
                            width = width * scale;
                        }
                    }
                }

                FT_Done_MM_Var(self.lib, mm);
            }
        }

        (weight.round() as u16, width.round() as u16)
    }

    pub fn italic(&self) -> bool {
        unsafe { ((*self.face).style_flags & FT_STYLE_FLAG_ITALIC as FT_Long) != 0 }
    }

    pub fn compute_coverage(&self) -> RangeSet<u32> {
        let mut coverage = RangeSet::new();

        for encoding in &[
            FT_Encoding::FT_ENCODING_UNICODE,
            FT_Encoding::FT_ENCODING_MS_SYMBOL,
        ] {
            if unsafe { FT_Select_Charmap(self.face, *encoding) } != 0 {
                continue;
            }

            let mut glyph = 0;
            let mut ucs4 = unsafe { FT_Get_First_Char(self.face, &mut glyph) };
            while glyph != 0 {
                coverage.add(ucs4 as u32);
                ucs4 = unsafe { FT_Get_Next_Char(self.face, ucs4, &mut glyph) };
            }

            if *encoding == FT_Encoding::FT_ENCODING_MS_SYMBOL {
                // Fontconfig duplicates F000..F0FF to 0000..00FF
                for ucs4 in 0xf00..0xf100 {
                    if coverage.contains(ucs4) {
                        coverage.add(ucs4 as u32 - 0xf000);
                    }
                }
            }
        }

        coverage
    }

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

    /// Returns the number of faces in a given font.
    /// For a TTF this will be 1.
    /// For a TTC, it will be the number of contained fonts
    pub fn query_num_faces(&self, handle: &FontDataHandle) -> anyhow::Result<u32> {
        let face = match handle {
            FontDataHandle::OnDisk { path, .. } => self
                .new_face(path.to_str().unwrap(), -1)
                .context("query_num_faces")?,
            FontDataHandle::Memory { data, .. } => self
                .new_face_from_slice(data.clone(), -1)
                .context("query_num_faces")?,
        };
        Ok(unsafe { (*face.face).num_faces }.try_into()?)
    }

    pub fn face_from_locator(&self, handle: &FontDataHandle) -> anyhow::Result<Face> {
        let (face, variation) = match handle {
            FontDataHandle::OnDisk {
                path,
                index,
                variation,
            } => (
                self.new_face(path.to_str().unwrap(), *index as _)?,
                *variation,
            ),
            FontDataHandle::Memory {
                data,
                index,
                variation,
                ..
            } => (
                self.new_face_from_slice(data.clone(), *index as _)?,
                *variation,
            ),
        };

        if variation != 0 {
            unsafe {
                ft_result(FT_Set_Named_Instance(face.face, variation), ())
                    .context("FT_Set_Named_Instance")?;
            }
        }

        Ok(face)
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
                    lib: self.lib,
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
            lib: self.lib,
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
            lib: self.lib,
        })
    }

    pub fn set_lcd_filter(&mut self, filter: FT_LcdFilter) -> anyhow::Result<()> {
        unsafe {
            ft_result(FT_Library_SetLcdFilter(self.lib, filter), ())
                .context("FT_Library_SetLcdFilter")
        }
    }
}
