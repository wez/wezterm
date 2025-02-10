//! Higher level freetype bindings

use crate::locator::{FontDataHandle, FontDataSource};
use crate::parser::ParsedFont;
use crate::rasterizer::colr::DrawOp;
use anyhow::{anyhow, Context};
use config::{configuration, FreeTypeLoadFlags, FreeTypeLoadTarget};
pub use freetype::*;
use memmap2::{Mmap, MmapOptions};
use rangeset::RangeSet;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::{c_int, c_void, CStr};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::mem::MaybeUninit;
use std::os::raw::{c_uchar, c_ulong};
use std::path::Path;
use std::ptr;
use std::sync::Arc;

#[inline]
pub fn succeeded(error: FT_Error) -> bool {
    error == freetype::FT_Err_Ok as FT_Error
}

/// Translate an error and value into a result
pub fn ft_result<T>(err: FT_Error, t: T) -> anyhow::Result<T> {
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
    ((render_mode as u32) & 15) << 16
}

pub fn compute_load_flags_from_config(
    freetype_load_flags: Option<FreeTypeLoadFlags>,
    freetype_load_target: Option<FreeTypeLoadTarget>,
    freetype_render_target: Option<FreeTypeLoadTarget>,
    dpi: Option<u32>,
) -> (i32, FT_Render_Mode) {
    let config = configuration();

    let load_flags = freetype_load_flags
        .or(config.freetype_load_flags)
        .unwrap_or_else(|| match dpi {
            Some(dpi) if dpi >= 100 => FreeTypeLoadFlags::default_hidpi(),
            _ => FreeTypeLoadFlags::default(),
        })
        .bits()
        | FT_LOAD_COLOR;

    fn target_to_render(t: FreeTypeLoadTarget) -> FT_Render_Mode {
        match t {
            FreeTypeLoadTarget::Mono => FT_Render_Mode::FT_RENDER_MODE_MONO,
            FreeTypeLoadTarget::Normal => FT_Render_Mode::FT_RENDER_MODE_NORMAL,
            FreeTypeLoadTarget::Light => FT_Render_Mode::FT_RENDER_MODE_LIGHT,
            FreeTypeLoadTarget::HorizontalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD,
            FreeTypeLoadTarget::VerticalLcd => FT_Render_Mode::FT_RENDER_MODE_LCD_V,
        }
    }

    let load_target = target_to_render(freetype_load_target.unwrap_or(config.freetype_load_target));
    let render = target_to_render(
        freetype_render_target.unwrap_or(
            config
                .freetype_render_target
                .unwrap_or(config.freetype_load_target),
        ),
    );

    let load_flags = load_flags | render_mode_to_load_target(load_target);

    (load_flags as i32, render)
}

pub struct Face {
    pub face: FT_Face,
    source: FontDataHandle,
    size: Option<FaceSize>,
    lib: FT_Library,
    palette: Option<&'static mut [FT_Color]>,
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
    cap_height: Option<f64>,
    cap_height_to_height_ratio: Option<f64>,
    is_scaled: bool,
}

#[derive(Debug)]
struct ComputedCellMetrics {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct SelectedFontSize {
    pub width: f64,
    pub height: f64,
    pub cap_height: Option<f64>,
    pub cap_height_to_height_ratio: Option<f64>,
    pub is_scaled: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("Glyph is SVG")]
pub struct IsSvg;

#[derive(Debug, thiserror::Error)]
#[error("Glyph is COLR1 or later")]
pub struct IsColr1OrLater;

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
                let source = FontDataHandle {
                    source: self.source.source.clone(),
                    index: self.source.index,
                    variation: i,
                    origin: self.source.origin.clone(),
                    coverage: self.source.coverage.clone(),
                };
                res.push(ParsedFont::from_face(&self, source)?);
            }

            FT_Done_MM_Var(self.lib, mm);
            FT_Set_Named_Instance(self.face, 0);

            log::debug!("Variations: {:#?}", res);

            Ok(res)
        }
    }

    pub fn get_glyph_name(&self, glyph_index: u32) -> Option<String> {
        let mut buf = [0u8; 128];
        let res = unsafe {
            FT_Get_Glyph_Name(
                self.face,
                glyph_index,
                buf.as_mut_ptr() as *mut _,
                buf.len() as _,
            )
        };
        if res != 0 {
            None
        } else {
            Some(String::from_utf8_lossy(&buf).into_owned())
        }
    }

    pub fn get_sfnt_names(&self) -> HashMap<u32, Vec<NameRecord>> {
        let num_names = unsafe { FT_Get_Sfnt_Name_Count(self.face) };

        let mut names = HashMap::new();

        let mut sfnt_name = FT_SfntName {
            platform_id: 0,
            encoding_id: 0,
            language_id: 0,
            name_id: 0,
            string: std::ptr::null_mut(),
            string_len: 0,
        };

        for i in 0..num_names {
            if unsafe { FT_Get_Sfnt_Name(self.face, i, &mut sfnt_name) } != 0 {
                continue;
            }

            if sfnt_name.string.is_null() {
                continue;
            }

            if !matches!(
                sfnt_name.name_id as u32,
                TT_NAME_ID_TYPOGRAPHIC_FAMILY
                    | TT_NAME_ID_TYPOGRAPHIC_SUBFAMILY
                    | TT_NAME_ID_FONT_FAMILY
                    | TT_NAME_ID_FONT_SUBFAMILY
                    | TT_NAME_ID_PS_NAME
            ) {
                continue;
            }

            let bytes = unsafe {
                from_raw_parts(sfnt_name.string as *const u8, sfnt_name.string_len as usize)
            };

            let encoding = match (sfnt_name.platform_id as u32, sfnt_name.encoding_id as u32) {
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_JAPANESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_SJIS) => encoding_rs::SHIFT_JIS,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_SIMPLIFIED_CHINESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_PRC) => encoding_rs::GBK,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_TRADITIONAL_CHINESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_BIG_5) => encoding_rs::BIG5,
                (
                    TT_PLATFORM_MICROSOFT,
                    TT_MS_ID_UCS_4 | TT_MS_ID_UNICODE_CS | TT_MS_ID_SYMBOL_CS,
                ) => encoding_rs::UTF_16BE,
                (TT_PLATFORM_MICROSOFT, TT_MS_ID_WANSUNG) => encoding_rs::EUC_KR,
                (TT_PLATFORM_APPLE_UNICODE | TT_PLATFORM_ISO, _) => encoding_rs::UTF_16BE,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_ROMAN) => encoding_rs::MACINTOSH,
                _ => {
                    log::trace!(
                        "Skipping name_id={} because platform_id={} encoding_id={}",
                        sfnt_name.name_id,
                        sfnt_name.platform_id,
                        sfnt_name.encoding_id
                    );
                    continue;
                }
            };

            let (name, _) = encoding.decode_with_bom_removal(bytes);

            names
                .entry(sfnt_name.name_id as u32)
                .or_insert_with(Vec::new)
                .push(NameRecord {
                    platform_id: sfnt_name.platform_id,
                    encoding_id: sfnt_name.encoding_id,
                    name_id: sfnt_name.name_id,
                    language_id: sfnt_name.language_id,
                    name: name.to_string(),
                });
        }
        names
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

    /// Returns the cap_height/units_per_EM ratio if known
    pub fn cap_height(&self) -> Option<f64> {
        unsafe {
            let os2 = self.get_os2_table()?;
            let units_per_em = (*self.face).units_per_EM;
            if units_per_em == 0 || os2.sCapHeight == 0 {
                return None;
            }
            Some(os2.sCapHeight as f64 / units_per_em as f64)
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

                    let styles = from_raw_parts(mm.namedstyle, mm.num_namedstyles as usize);
                    let instance = &styles[vidx];
                    let axes = from_raw_parts(mm.axis, mm.num_axis as usize);

                    for (i, axis) in axes.iter().enumerate() {
                        let coords = from_raw_parts(instance.coords, mm.num_axis as usize);
                        let value = coords[i].to_num::<f64>();
                        let default_value = axis.def.to_num::<f64>();
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
        if let Some(coverage) = self.source.coverage.as_ref() {
            return coverage.clone();
        }
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
            if glyph == 0 {
                break;
            }
            let mut ucs4_range_start = ucs4;
            loop {
                let ucs4_new = unsafe { FT_Get_Next_Char(self.face, ucs4, &mut glyph) };
                if glyph == 0 {
                    break;
                }
                if ucs4_new - ucs4 != 1 {
                    coverage.add_range_unchecked(ucs4_range_start as u32..(ucs4 + 1) as u32);
                    ucs4_range_start = ucs4_new;
                }
                ucs4 = ucs4_new;
            }
            coverage.add_range_unchecked(ucs4_range_start as u32..(ucs4 + 1) as u32);

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

    /// Returns the bitmap strike sizes in this font
    pub fn pixel_sizes(&self) -> Vec<u16> {
        let sizes = unsafe {
            let rec = &(*self.face);
            from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
        };
        sizes
            .iter()
            .filter_map(|info| {
                if info.height > 0 {
                    Some(info.height as u16)
                } else {
                    None
                }
            })
            .collect()
    }

    /// This is a wrapper around set_char_size and select_size
    /// that accounts for some weirdness with eg: color emoji
    pub fn set_font_size(&mut self, point_size: f64, dpi: u32) -> anyhow::Result<SelectedFontSize> {
        if let Some(face_size) = self.size.as_ref() {
            if face_size.size == point_size && face_size.dpi == dpi {
                return Ok(SelectedFontSize {
                    width: face_size.cell_width,
                    height: face_size.cell_height,
                    is_scaled: face_size.is_scaled,
                    cap_height: face_size.cap_height,
                    cap_height_to_height_ratio: face_size.cap_height_to_height_ratio,
                });
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
        let size = FT_F26Dot6::from_num(point_size);

        let selected_size = match self.set_char_size(size, size, dpi, dpi) {
            Ok(_) => {
                // Compute metrics for the nominal monospace cell
                let ComputedCellMetrics { width, height } = self.cell_metrics();
                SelectedFontSize {
                    width,
                    height,
                    cap_height: None,
                    cap_height_to_height_ratio: None,
                    is_scaled: true,
                }
            }
            Err(err) => {
                log::debug!("set_char_size: {:?}, will inspect strikes", err);

                let sizes = unsafe {
                    let rec = &(*self.face);
                    from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
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
                // Compute the cell metrics at this size.
                // This stuff is a bit weird; for GohuFont.otb, cell_metrics()
                // returns (8.0, 0.0) when the selected bitmap strike is (4, 14).
                // 4 pixels is too thin for this font, so we take the max of the
                // known dimensions to produce the size.
                // <https://github.com/wezterm/wezterm/issues/1165>
                let m = self.cell_metrics();
                let height = f64::from(best.height).max(m.height);
                SelectedFontSize {
                    width: f64::from(best.width).max(m.width),
                    height,
                    is_scaled: false,
                    cap_height: None,
                    cap_height_to_height_ratio: None,
                }
            }
        };

        self.size.replace(FaceSize {
            size: point_size,
            dpi,
            cell_width: selected_size.width,
            cap_height: None,
            cap_height_to_height_ratio: None,
            cell_height: selected_size.height,
            is_scaled: selected_size.is_scaled,
        });

        // Can't compute cap height until after we've assigned self.size
        if let Ok(cap_height) = self.compute_cap_height() {
            let cap_height_to_height_ratio = cap_height / selected_size.height;

            self.size.replace(FaceSize {
                size: point_size,
                dpi,
                cell_width: selected_size.width,
                cap_height: Some(cap_height),
                cap_height_to_height_ratio: Some(cap_height_to_height_ratio),
                cell_height: selected_size.height,
                is_scaled: selected_size.is_scaled,
            });

            Ok(SelectedFontSize {
                cap_height: Some(cap_height),
                cap_height_to_height_ratio: Some(cap_height_to_height_ratio),
                ..selected_size
            })
        } else {
            Ok(selected_size)
        }
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

    pub fn set_transform(&mut self, matrix: Option<FT_Matrix>) {
        let mut matrix = matrix;
        unsafe {
            FT_Set_Transform(
                self.face,
                match &mut matrix {
                    Some(m) => m as *mut _,
                    None => std::ptr::null_mut(),
                },
                std::ptr::null_mut(),
            )
        }
    }

    pub fn get_color_glyph_paint(
        &mut self,
        glyph_index: FT_UInt,
        root_transform: FT_Color_Root_Transform,
    ) -> anyhow::Result<FT_Opaque_Paint_> {
        unsafe {
            let mut result = MaybeUninit::<FT_Opaque_Paint_>::zeroed();
            let status = FT_Get_Color_Glyph_Paint(
                self.face,
                glyph_index,
                root_transform,
                result.as_mut_ptr(),
            );
            if status == 1 {
                Ok(result.assume_init())
            } else {
                anyhow::bail!("FT_Get_Color_Glyph_Paint for glyph {glyph_index} failed");
            }
        }
    }

    pub fn get_color_glyph_clip_box(
        &mut self,
        glyph_index: FT_UInt,
    ) -> anyhow::Result<FT_ClipBox_> {
        unsafe {
            let mut result = MaybeUninit::<FT_ClipBox_>::zeroed();
            let status = FT_Get_Color_Glyph_ClipBox(self.face, glyph_index, result.as_mut_ptr());
            if status == 1 {
                Ok(result.assume_init())
            } else {
                anyhow::bail!("FT_Get_Color_Glyph_ClipBox for glyph {glyph_index} failed");
            }
        }
    }

    pub fn get_paint(&mut self, paint: FT_Opaque_Paint_) -> anyhow::Result<FT_COLR_Paint_> {
        unsafe {
            let mut result = MaybeUninit::<FT_COLR_Paint_>::zeroed();
            let status = FT_Get_Paint(self.face, paint, result.as_mut_ptr());
            if status == 1 {
                Ok(result.assume_init())
            } else {
                anyhow::bail!("FT_Get_Paint failed");
            }
        }
    }

    /// Replace any palette entry overrides and select the specified palette
    pub fn select_palette(&mut self, index: FT_UShort) -> anyhow::Result<()> {
        unsafe {
            self.palette.take();

            let mut pdata = MaybeUninit::<FT_Palette_Data>::zeroed();
            ft_result(FT_Palette_Data_Get(self.face, pdata.as_mut_ptr()), ())
                .context("FT_Palette_Data_Get")?;
            let pdata = pdata.assume_init();

            let mut palette_ptr = std::ptr::null_mut();

            ft_result(FT_Palette_Select(self.face, index, &mut palette_ptr), ())
                .with_context(|| format!("FT_Palette_Select for index={index}. Note: {pdata:?}"))?;

            let palette =
                std::slice::from_raw_parts_mut(palette_ptr, pdata.num_palette_entries as usize);

            self.palette.replace(palette);

            Ok(())
        }
    }

    pub fn get_palette_entry(&self, index: u32) -> anyhow::Result<FT_Color> {
        self.palette
            .as_ref()
            .and_then(|slice| slice.get(index as usize))
            .copied()
            .ok_or_else(|| anyhow::anyhow!("invalid palette entry {index}"))
    }

    pub fn get_palette_data(&self) -> anyhow::Result<PaletteInfo> {
        unsafe {
            let mut result = MaybeUninit::<FT_Palette_Data>::zeroed();
            ft_result(FT_Palette_Data_Get(self.face, result.as_mut_ptr()), ())
                .context("FT_Palette_Data_Get")?;

            let data = result.assume_init();
            let mut palettes = vec![];

            let name_ids = from_raw_parts(data.palette_name_ids, data.num_palettes as usize);
            let flagses = from_raw_parts(data.palette_flags, data.num_palettes as usize);
            let entry_name_ids = from_raw_parts(
                data.palette_entry_name_ids,
                data.num_palette_entries as usize,
            );

            let entry_names: Vec<String> = entry_name_ids
                .iter()
                .map(|&id| {
                    self.get_sfnt_name(id as _)
                        .map(|rec| rec.name)
                        .unwrap_or_else(|_| String::new())
                })
                .collect();

            for (palette_index, (&name_id, &flags)) in
                name_ids.iter().zip(flagses.iter()).enumerate()
            {
                palettes.push(Palette {
                    palette_index,
                    flags,
                    name: self
                        .get_sfnt_name(name_id as _)
                        .map(|rec| rec.name)
                        .unwrap_or_else(|_| String::new()),
                    entry_names: entry_names.clone(),
                });
            }
            Ok(PaletteInfo {
                num_palettes: data.num_palettes as usize,
                palettes,
            })
        }
    }

    pub fn get_sfnt_name(&self, i: FT_UInt) -> anyhow::Result<NameRecord> {
        unsafe {
            let mut sfnt_name = MaybeUninit::<FT_SfntName>::zeroed();
            ft_result(FT_Get_Sfnt_Name(self.face, i, sfnt_name.as_mut_ptr()), ())
                .context("FT_Get_Sfnt_Name")?;
            let sfnt_name = sfnt_name.assume_init();
            let bytes =
                from_raw_parts(sfnt_name.string as *const u8, sfnt_name.string_len as usize);

            let encoding = match (sfnt_name.platform_id as u32, sfnt_name.encoding_id as u32) {
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_JAPANESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_SJIS) => encoding_rs::SHIFT_JIS,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_SIMPLIFIED_CHINESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_PRC) => encoding_rs::GBK,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_TRADITIONAL_CHINESE)
                | (TT_PLATFORM_MICROSOFT, TT_MS_ID_BIG_5) => encoding_rs::BIG5,
                (
                    TT_PLATFORM_MICROSOFT,
                    TT_MS_ID_UCS_4 | TT_MS_ID_UNICODE_CS | TT_MS_ID_SYMBOL_CS,
                ) => encoding_rs::UTF_16BE,
                (TT_PLATFORM_MICROSOFT, TT_MS_ID_WANSUNG) => encoding_rs::EUC_KR,
                (TT_PLATFORM_APPLE_UNICODE | TT_PLATFORM_ISO, _) => encoding_rs::UTF_16BE,
                (TT_PLATFORM_MACINTOSH, TT_MAC_ID_ROMAN) => encoding_rs::MACINTOSH,
                _ => {
                    anyhow::bail!(
                        "Skipping name_id={} because platform_id={} encoding_id={}",
                        sfnt_name.name_id,
                        sfnt_name.platform_id,
                        sfnt_name.encoding_id
                    );
                }
            };

            let (name, _) = encoding.decode_with_bom_removal(bytes);

            Ok(NameRecord {
                platform_id: sfnt_name.platform_id,
                encoding_id: sfnt_name.encoding_id,
                name_id: sfnt_name.name_id,
                language_id: sfnt_name.language_id,
                name: name.to_string(),
            })
        }
    }

    pub fn get_paint_layers(
        &mut self,
        iter: &mut FT_LayerIterator_,
    ) -> anyhow::Result<FT_Opaque_Paint_> {
        unsafe {
            let mut result = MaybeUninit::<FT_Opaque_Paint_>::zeroed();
            let status = FT_Get_Paint_Layers(self.face, iter, result.as_mut_ptr());
            if status == 1 {
                Ok(result.assume_init())
            } else {
                anyhow::bail!("FT_Get_Paint_Layers failed");
            }
        }
    }

    pub fn load_glyph_outlines(
        &mut self,
        glyph_index: FT_UInt,
        load_flags: FT_Int32,
    ) -> anyhow::Result<Vec<DrawOp>> {
        unsafe {
            ft_result(FT_Load_Glyph(self.face, glyph_index, load_flags), ())
                .with_context(|| format!("FT_Load_Glyph {glyph_index}"))?;
            let slot = &mut *(*self.face).glyph;
            if slot.format != FT_Glyph_Format_::FT_GLYPH_FORMAT_OUTLINE {
                anyhow::bail!(
                    "Expected FT_COLR_PAINTFORMAT_GLYPH to be an outline, got {:?}",
                    slot.format
                );
            }

            let funcs = FT_Outline_Funcs_ {
                move_to: Some(move_to),
                line_to: Some(line_to),
                conic_to: Some(conic_to),
                cubic_to: Some(cubic_to),
                shift: 16, // match the same coordinate space as transforms
                delta: FT_Pos::from_font_units(0),
            };

            let mut ops = vec![];

            unsafe extern "C" fn move_to(to: *const FT_Vector, user: *mut c_void) -> c_int {
                let ops = user as *mut Vec<DrawOp>;
                let (to_x, to_y) = vector_x_y(&*to);
                (*ops).push(DrawOp::MoveTo { to_x, to_y });
                0
            }
            unsafe extern "C" fn line_to(to: *const FT_Vector, user: *mut c_void) -> c_int {
                let ops = user as *mut Vec<DrawOp>;
                let (to_x, to_y) = vector_x_y(&*to);
                (*ops).push(DrawOp::LineTo { to_x, to_y });
                0
            }
            unsafe extern "C" fn conic_to(
                control: *const FT_Vector,
                to: *const FT_Vector,
                user: *mut c_void,
            ) -> c_int {
                let ops = user as *mut Vec<DrawOp>;
                let (control_x, control_y) = vector_x_y(&*control);
                let (to_x, to_y) = vector_x_y(&*to);
                (*ops).push(DrawOp::QuadTo {
                    control_x,
                    control_y,
                    to_x,
                    to_y,
                });
                0
            }
            unsafe extern "C" fn cubic_to(
                control1: *const FT_Vector,
                control2: *const FT_Vector,
                to: *const FT_Vector,
                user: *mut c_void,
            ) -> c_int {
                let ops = user as *mut Vec<DrawOp>;
                let (control1_x, control1_y) = vector_x_y(&*control1);
                let (control2_x, control2_y) = vector_x_y(&*control2);
                let (to_x, to_y) = vector_x_y(&*to);
                (*ops).push(DrawOp::CubicTo {
                    control1_x,
                    control1_y,
                    control2_x,
                    control2_y,
                    to_x,
                    to_y,
                });
                0
            }

            ft_result(
                FT_Outline_Decompose(
                    &mut slot.outline,
                    &funcs,
                    &mut ops as *mut Vec<DrawOp> as *mut c_void,
                ),
                (),
            )
            .with_context(|| format!("FT_Outline_Decompose. ops so far: {ops:?}"))?;

            if !ops.is_empty() {
                ops.push(DrawOp::ClosePath);
            }

            Ok(ops)
        }
    }

    pub fn load_and_render_glyph(
        &mut self,
        glyph_index: FT_UInt,
        load_flags: FT_Int32,
        render_mode: FT_Render_Mode,
        synthesize_bold: bool,
    ) -> anyhow::Result<&FT_GlyphSlotRec_> {
        unsafe {
            ft_result(
                FT_Load_Glyph(self.face, glyph_index, load_flags | FT_LOAD_NO_SVG as i32),
                (),
            )
            .with_context(|| {
                format!(
                    "load_and_render_glyph: FT_Load_Glyph glyph_index:{}",
                    glyph_index
                )
            })?;
            let slot = &mut *(*self.face).glyph;

            if slot.format == FT_Glyph_Format_::FT_GLYPH_FORMAT_SVG {
                return Err(IsSvg.into());
            }

            if synthesize_bold {
                FT_GlyphSlot_Embolden(slot as *mut _);
            }

            // Current versions of freetype overload the operation of FT_LOAD_COLOR
            // and the resulting glyph format such that we cannot determine solely
            // from the flags whether we got a regular set of outlines,
            // or its COLR v0 synthesized glyphs, or whether it's COLR v1 or later
            // and it can't render the result.
            // So, we probe here to look for color layer information: if we find it,
            // we don't call freetype's renderer and instead bubble up an error
            // that the embedding application can trap and decide what to do.
            if slot.format == FT_Glyph_Format_::FT_GLYPH_FORMAT_OUTLINE {
                if self
                    .get_color_glyph_paint(
                        glyph_index,
                        FT_Color_Root_Transform::FT_COLOR_NO_ROOT_TRANSFORM,
                    )
                    .is_ok()
                {
                    return Err(IsColr1OrLater.into());
                }
            }

            ft_result(FT_Render_Glyph(slot, render_mode), ())
                .context("load_and_render_glyph: FT_Render_Glyph")?;

            Ok(slot)
        }
    }

    /// Compute the cap-height metric in pixels.
    /// This is pixel-perfect based on the rendered glyph data for `I`,
    /// which is a technique that works for any font regardless
    /// of the integrity of its internal cap-height metric or
    /// whether the font is a bitmap font.
    /// `I` is chosen rather than `O` as `O` glyphs are often optically
    /// compensated and overshoot a little.
    fn compute_cap_height(&mut self) -> anyhow::Result<f64> {
        let glyph_pos = unsafe { FT_Get_Char_Index(self.face, b'I' as _) };
        if glyph_pos == 0 {
            anyhow::bail!("no I from which to compute cap height");
        }
        let (load_flags, render_mode) = compute_load_flags_from_config(None, None, None, None);
        let ft_glyph = self.load_and_render_glyph(glyph_pos, load_flags, render_mode, false)?;

        let mode: FT_Pixel_Mode =
            unsafe { std::mem::transmute(u32::from(ft_glyph.bitmap.pixel_mode)) };

        // pitch is the number of bytes per source row
        let pitch = ft_glyph.bitmap.pitch.abs() as usize;
        let data = unsafe {
            std::slice::from_raw_parts_mut(
                ft_glyph.bitmap.buffer,
                ft_glyph.bitmap.rows as usize * pitch,
            )
        };

        let mut first_row = None;
        let mut last_row = None;

        match mode {
            FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                let width = ft_glyph.bitmap.width as usize / 3;
                let height = ft_glyph.bitmap.rows as usize;

                'next_line_lcd: for y in 0..height {
                    let src_offset = y * pitch as usize;
                    for x in 0..width {
                        if data[src_offset + (x * 3)] != 0
                            || data[src_offset + (x * 3) + 1] != 0
                            || data[src_offset + (x * 3) + 2] != 0
                        {
                            if first_row.is_none() {
                                first_row.replace(y);
                            }
                            last_row.replace(y);
                            continue 'next_line_lcd;
                        }
                    }
                }
            }

            FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;
                'next_line_bgra: for y in 0..height {
                    let src_offset = y * pitch as usize;
                    for x in 0..width {
                        let alpha = data[src_offset + (x * 4) + 3];
                        if alpha != 0 {
                            if first_row.is_none() {
                                first_row.replace(y);
                            }
                            last_row.replace(y);
                            continue 'next_line_bgra;
                        }
                    }
                }
            }
            FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;
                'next_line_gray: for y in 0..height {
                    let src_offset = y * pitch;
                    for x in 0..width {
                        if data[src_offset + x] != 0 {
                            if first_row.is_none() {
                                first_row.replace(y);
                            }
                            last_row.replace(y);
                            continue 'next_line_gray;
                        }
                    }
                }
            }
            FT_Pixel_Mode::FT_PIXEL_MODE_MONO => {
                let width = ft_glyph.bitmap.width as usize;
                let height = ft_glyph.bitmap.rows as usize;
                'next_line_mono: for y in 0..height {
                    let src_offset = y * pitch;
                    let mut x = 0;
                    for i in 0..pitch {
                        if x >= width {
                            break;
                        }
                        let mut b = data[src_offset + i];
                        for _ in 0..8 {
                            if x >= width {
                                break;
                            }
                            if b & 0x80 == 0x80 {
                                if first_row.is_none() {
                                    first_row.replace(y);
                                }
                                last_row.replace(y);
                                continue 'next_line_mono;
                            }
                            b <<= 1;
                            x += 1;
                        }
                    }
                }
            }
            _ => anyhow::bail!("unhandled pixel mode {:?}", mode),
        }

        match (first_row, last_row) {
            (Some(first), Some(last)) => Ok((last - first) as f64),
            _ => anyhow::bail!("didn't find any rasterized rows?"),
        }
    }

    fn cell_metrics(&mut self) -> ComputedCellMetrics {
        unsafe {
            let metrics = &(*(*self.face).size).metrics;
            let height = metrics.y_scale.to_num::<f64>() * f64::from((*self.face).height) / 64.0;

            let mut width = 0.0;
            let mut num_examined = 0;
            for i in 32..128 {
                let glyph_pos = FT_Get_Char_Index(self.face, i);
                if glyph_pos == 0 {
                    continue;
                }
                let res = FT_Load_Glyph(self.face, glyph_pos, FT_LOAD_COLOR as i32);
                if succeeded(res) {
                    num_examined += 1;
                    let glyph = &(*(*self.face).glyph);
                    if glyph.metrics.horiAdvance.font_units() as f64 > width {
                        width = glyph.metrics.horiAdvance.font_units() as f64;
                    }
                }
            }
            if width == 0.0 {
                // Most likely we're looking at a symbol font with no latin
                // glyphs at all. Let's just pick a selection of glyphs
                for glyph_pos in 1..8 {
                    let res = FT_Load_Glyph(self.face, glyph_pos, FT_LOAD_COLOR as i32);
                    if succeeded(res) {
                        num_examined += 1;
                        let glyph = &(*(*self.face).glyph);
                        if glyph.metrics.horiAdvance.font_units() as f64 > width {
                            width = glyph.metrics.horiAdvance.font_units() as f64;
                        }
                    }
                }
                if width == 0.0 {
                    log::error!(
                        "Couldn't find usable advance metrics out of {} glyphs \
                        sampled from the font, so guessing width == height",
                        num_examined,
                    );
                    width = height * 64.;
                }
            }

            ComputedCellMetrics {
                width: width / 64.0,
                height,
            }
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

        {
            let no_long_names: FT_Bool = if config.freetype_pcf_long_family_names {
                0
            } else {
                1
            };
            unsafe {
                FT_Property_Set(
                    lib.lib,
                    b"pcf\0" as *const u8 as *const FT_String,
                    b"no-long-family-names\0" as *const u8 as *const FT_String,
                    &no_long_names as *const FT_Bool as *const _,
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
    pub fn query_num_faces(&self, source: &FontDataSource) -> anyhow::Result<u32> {
        let face = self.new_face(source, -1).context("query_num_faces")?;
        let num_faces = unsafe { (*face).num_faces }.try_into();

        unsafe {
            FT_Done_Face(face);
        }

        Ok(num_faces?)
    }

    pub fn face_from_locator(&self, handle: &FontDataHandle) -> anyhow::Result<Face> {
        let source = handle.clone();

        let mut index = handle.index;
        if handle.variation != 0 {
            index |= handle.variation << 16;
        }

        let face = self
            .new_face(&source.source, index as _)
            .with_context(|| format!("face_from_locator({:?})", handle))?;

        Ok(Face {
            face,
            lib: self.lib,
            source,
            size: None,
            palette: None,
        })
    }

    fn new_face(&self, source: &FontDataSource, face_index: FT_Long) -> anyhow::Result<FT_Face> {
        let mut face = ptr::null_mut();

        // FT_Open_Face will take ownership of this and closes it in both
        // the error case and the success case (although the latter is when
        // the face is dropped).
        let stream = FreeTypeStream::from_source(source)?;

        let args = FT_Open_Args {
            flags: FT_OPEN_STREAM,
            memory_base: ptr::null(),
            memory_size: 0,
            pathname: ptr::null_mut(),
            stream,
            driver: ptr::null_mut(),
            num_params: 0,
            params: ptr::null_mut(),
        };

        let res = unsafe { FT_Open_Face(self.lib, &args, face_index, &mut face as *mut _) };

        ft_result(res, face)
            .with_context(|| format!("FT_Open_Face(\"{:?}\", face_index={})", source, face_index))
    }

    pub fn set_lcd_filter(&mut self, filter: FT_LcdFilter) -> anyhow::Result<()> {
        unsafe {
            ft_result(FT_Library_SetLcdFilter(self.lib, filter), ())
                .context("FT_Library_SetLcdFilter")
        }
    }
}

/// Our own stream implementation.
/// This is present because we cannot guarantee to be able to convert
/// Path -> c-string on Windows systems, but also because we've seen
/// mysterious errors about not being able to open a resource.
/// The intent is to avoid a potential problem and to help reveal
/// more context on problems opening files as/when that happens.
struct FreeTypeStream {
    stream: FT_StreamRec_,
    backing: StreamBacking,
    name: String,
}

#[allow(dead_code)]
enum StreamBacking {
    File(BufReader<File>),
    Map(Mmap),
    Static(&'static [u8]),
    Memory(Arc<Box<[u8]>>),
}

impl FreeTypeStream {
    pub fn from_source(source: &FontDataSource) -> anyhow::Result<FT_Stream> {
        let (backing, base, len) = match source {
            FontDataSource::OnDisk(path) => return Self::open_path(path),
            FontDataSource::BuiltIn { data, .. } => {
                let base = data.as_ptr();
                let len = data.len();
                (StreamBacking::Static(data), base, len)
            }
            FontDataSource::Memory { data, .. } => {
                let base = data.as_ptr();
                let len = data.len();
                (StreamBacking::Memory(Arc::clone(data)), base, len)
            }
        };

        let name = source.name_or_path_str().to_string();

        if len > c_ulong::MAX as usize {
            anyhow::bail!("{} is too large to pass to freetype! (len={})", name, len);
        }

        let stream = Box::new(Self {
            stream: FT_StreamRec_ {
                base: base as *mut _,
                size: len as c_ulong,
                pos: 0,
                descriptor: FT_StreamDesc_ {
                    pointer: ptr::null_mut(),
                },
                pathname: FT_StreamDesc_ {
                    pointer: ptr::null_mut(),
                },
                read: None,
                close: Some(Self::close),
                memory: ptr::null_mut(),
                cursor: ptr::null_mut(),
                limit: ptr::null_mut(),
            },
            backing,
            name,
        });
        let stream = Box::into_raw(stream);
        unsafe {
            (*stream).stream.descriptor.pointer = stream as *mut _;
            Ok(&mut (*stream).stream)
        }
    }

    fn open_path(p: &Path) -> anyhow::Result<FT_Stream> {
        let file = File::open(p).with_context(|| format!("opening file {}", p.display()))?;

        let meta = file
            .metadata()
            .with_context(|| format!("querying metadata for {}", p.display()))?;

        if !meta.is_file() {
            anyhow::bail!("{} is not a file", p.display());
        }

        let len = meta.len();
        if len as usize > c_ulong::MAX as usize {
            anyhow::bail!(
                "{} is too large to pass to freetype! (len={})",
                p.display(),
                len
            );
        }

        let (backing, base) = match unsafe { MmapOptions::new().map(&file) } {
            Ok(map) => {
                let base = map.as_ptr() as *mut _;
                (StreamBacking::Map(map), base)
            }
            Err(err) => {
                log::warn!(
                    "Unable to memory map {}: {}, will use regular file IO instead",
                    p.display(),
                    err
                );
                (StreamBacking::File(BufReader::new(file)), ptr::null_mut())
            }
        };

        let stream = Box::new(Self {
            stream: FT_StreamRec_ {
                base,
                size: len as c_ulong,
                pos: 0,
                descriptor: FT_StreamDesc_ {
                    pointer: ptr::null_mut(),
                },
                pathname: FT_StreamDesc_ {
                    pointer: ptr::null_mut(),
                },
                read: if base.is_null() {
                    Some(Self::read)
                } else {
                    // when backing is mmap, a null read routine causes
                    // freetype to simply resolve data from `base`
                    None
                },
                close: Some(Self::close),
                memory: ptr::null_mut(),
                cursor: ptr::null_mut(),
                limit: ptr::null_mut(),
            },
            backing,
            name: p.to_string_lossy().to_string(),
        });
        let stream = Box::into_raw(stream);
        unsafe {
            (*stream).stream.descriptor.pointer = stream as *mut _;
            Ok(&mut (*stream).stream)
        }
    }

    /// Called by freetype when it wants to read data from the file
    unsafe extern "C" fn read(
        stream: FT_Stream,
        offset: c_ulong,
        buffer: *mut c_uchar,
        count: c_ulong,
    ) -> c_ulong {
        if count == 0 {
            return 0;
        }

        let myself = &mut *((*stream).descriptor.pointer as *mut Self);
        match &mut myself.backing {
            StreamBacking::Map(_) | StreamBacking::Static(_) | StreamBacking::Memory(_) => {
                log::error!("read called on memory data {} !?", myself.name);
                0
            }
            StreamBacking::File(file) => {
                if let Err(err) = file.seek(SeekFrom::Start(offset.into())) {
                    log::error!(
                        "failed to seek {} to offset {}: {:#}",
                        myself.name,
                        offset,
                        err
                    );
                    return 0;
                }

                let buf = std::slice::from_raw_parts_mut(buffer, count as usize);
                match file.read(buf) {
                    Ok(len) => len as c_ulong,
                    Err(err) => {
                        log::error!(
                            "failed to read {} bytes @ offset {} of {}: {:#}",
                            count,
                            offset,
                            myself.name,
                            err
                        );
                        0
                    }
                }
            }
        }
    }

    /// Called by freetype when the stream is closed
    unsafe extern "C" fn close(stream: FT_Stream) {
        let myself = Box::from_raw((*stream).descriptor.pointer as *mut Self);
        drop(myself);
    }
}

/// Wrapper around std::slice::from_raw_parts that allows for ptr to be
/// null. In the null ptr case, an empty slice is returned.
/// This is necessary because it is common for freetype to encode
/// empty arrays in that way, and rust 1.78 will panic if a null
/// ptr is passed in.
pub(crate) unsafe fn from_raw_parts<'a, T>(ptr: *const T, size: usize) -> &'a [T] {
    if ptr.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, size)
    }
}

#[derive(Debug)]
pub struct PaletteInfo {
    pub num_palettes: usize,
    /// Note that this may be empty even when num_palettes is non-zero
    pub palettes: Vec<Palette>,
}

#[derive(Debug)]
pub struct Palette {
    pub palette_index: usize,
    pub name: String,
    pub flags: u16,
    pub entry_names: Vec<String>,
}

#[derive(Debug)]
pub struct NameRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: u16,
    pub name: String,
}

pub fn vector_x_y(vector: &FT_Vector) -> (f32, f32) {
    (vector.x.f16d16().to_num(), vector.y.f16d16().to_num())
}

pub fn composite_mode_to_operator(mode: FT_Composite_Mode) -> cairo::Operator {
    use cairo::Operator;
    use FT_Composite_Mode::*;
    match mode {
        FT_COLR_COMPOSITE_CLEAR => Operator::Clear,
        FT_COLR_COMPOSITE_SRC => Operator::Source,
        FT_COLR_COMPOSITE_DEST => Operator::Dest,
        FT_COLR_COMPOSITE_SRC_OVER => Operator::Over,
        FT_COLR_COMPOSITE_DEST_OVER => Operator::DestOver,
        FT_COLR_COMPOSITE_SRC_IN => Operator::In,
        FT_COLR_COMPOSITE_DEST_IN => Operator::DestIn,
        FT_COLR_COMPOSITE_SRC_OUT => Operator::Out,
        FT_COLR_COMPOSITE_DEST_OUT => Operator::DestOut,
        FT_COLR_COMPOSITE_SRC_ATOP => Operator::Atop,
        FT_COLR_COMPOSITE_DEST_ATOP => Operator::DestAtop,
        FT_COLR_COMPOSITE_XOR => Operator::Xor,
        FT_COLR_COMPOSITE_PLUS => Operator::Add,
        FT_COLR_COMPOSITE_SCREEN => Operator::Screen,
        FT_COLR_COMPOSITE_OVERLAY => Operator::Overlay,
        FT_COLR_COMPOSITE_DARKEN => Operator::Darken,
        FT_COLR_COMPOSITE_LIGHTEN => Operator::Lighten,
        FT_COLR_COMPOSITE_COLOR_DODGE => Operator::ColorDodge,
        FT_COLR_COMPOSITE_COLOR_BURN => Operator::ColorBurn,
        FT_COLR_COMPOSITE_HARD_LIGHT => Operator::HardLight,
        FT_COLR_COMPOSITE_SOFT_LIGHT => Operator::SoftLight,
        FT_COLR_COMPOSITE_DIFFERENCE => Operator::Difference,
        FT_COLR_COMPOSITE_EXCLUSION => Operator::Exclusion,
        FT_COLR_COMPOSITE_MULTIPLY => Operator::Multiply,
        FT_COLR_COMPOSITE_HSL_HUE => Operator::HslHue,
        FT_COLR_COMPOSITE_HSL_SATURATION => Operator::HslSaturation,
        FT_COLR_COMPOSITE_HSL_COLOR => Operator::HslColor,
        FT_COLR_COMPOSITE_HSL_LUMINOSITY => Operator::HslLuminosity,
        _ => unreachable!(),
    }
}

fn ft_make_tag(a: u8, b: u8, c: u8, d: u8) -> FT_ULong {
    (a as FT_ULong) << 24 | (b as FT_ULong) << 16 | (c as FT_ULong) << 8 | (d as FT_ULong)
}
