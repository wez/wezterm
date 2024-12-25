//! Higher level harfbuzz bindings
use freetype;

pub use harfbuzz::*;

use crate::locator::{FontDataHandle, FontDataSource};
use crate::rasterizer::colr::{ColorLine, ColorStop, DrawOp};
use anyhow::{ensure, Context, Error};
use cairo::Extend;
use memmap2::{Mmap, MmapOptions};
use std::ffi::CStr;
use std::io::Read;
use std::mem;
use std::ops::Range;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::Arc;
use wezterm_color_types::SrgbaPixel;

extern "C" {
    fn hb_ft_font_set_load_flags(font: *mut hb_font_t, load_flags: i32);
}

pub const IS_PNG: hb_tag_t = hb_tag(b'p', b'n', b'g', b' ');
#[allow(unused)]
pub const IS_SVG: hb_tag_t = hb_tag(b's', b'v', b'g', b' ');
pub const IS_BGRA: hb_tag_t = hb_tag(b'B', b'G', b'R', b'A');

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

#[derive(Debug)]
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

impl Clone for Blob {
    fn clone(&self) -> Self {
        unsafe { hb_blob_reference(self.blob) };
        Self { blob: self.blob }
    }
}

impl Blob {
    pub fn with_reference(blob: *mut hb_blob_t) -> Self {
        unsafe { hb_blob_reference(blob) };
        Self { blob }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            let mut len = 0;
            let ptr = hb_blob_get_data(self.blob, &mut len);
            from_raw_parts(ptr as *const u8, len as usize)
        }
    }

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

    pub fn set_synthetic_slant(&mut self, slant: f32) {
        unsafe {
            hb_font_set_synthetic_slant(self.font, slant);
        }
    }

    pub fn set_synthetic_bold(&mut self, x_embolden: f32, y_embolden: f32, in_place: bool) {
        unsafe {
            hb_font_set_synthetic_bold(
                self.font,
                x_embolden,
                y_embolden,
                if in_place { 1 } else { 0 },
            );
        }
    }

    pub fn set_font_scale(&self, x_scale: c_int, y_scale: c_int) {
        unsafe {
            hb_font_set_scale(self.font, x_scale, y_scale);
        }
    }

    pub fn set_ppem(&self, x_ppem: u32, y_ppem: u32) {
        unsafe {
            hb_font_set_ppem(self.font, x_ppem, y_ppem);
        }
    }

    pub fn set_ptem(&self, ptem: f32) {
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

    /// Fetches a list of the caret positions defined for a ligature glyph in the GDEF table of the
    /// font. The list returned will begin at the offset provided.
    /// Note that a ligature that is formed from n characters will have n-1 caret positions. The
    /// first character is not represented in the array, since its caret position is the glyph
    /// position.
    /// The positions returned by this function are 'unshaped', and will have to be fixed up for
    /// kerning that may be applied to the ligature glyp
    #[allow(dead_code)]
    pub fn get_ligature_carets(
        &self,
        direction: hb_direction_t,
        glyph_pos: u32,
    ) -> Vec<hb_position_t> {
        let mut positions = [0 as hb_position_t; 8];

        unsafe {
            let mut array_size = positions.len() as c_uint;
            let n_carets = hb_ot_layout_get_ligature_carets(
                self.font,
                direction,
                glyph_pos,
                0,
                &mut array_size,
                positions.as_mut_ptr(),
            ) as usize;

            if n_carets > positions.len() {
                let mut positions = vec![0 as hb_position_t; n_carets];
                array_size = positions.len() as c_uint;
                hb_ot_layout_get_ligature_carets(
                    self.font,
                    direction,
                    glyph_pos,
                    0,
                    &mut array_size,
                    positions.as_mut_ptr(),
                );

                return positions;
            }

            positions[..n_carets].to_vec()
        }
    }

    #[allow(unused)]
    pub fn draw_glyph(&self, glyph_pos: u32, funcs: &DrawFuncs, draw_data: *mut c_void) {
        unsafe { hb_font_draw_glyph(self.font, glyph_pos, funcs.funcs, draw_data) }
    }

    #[allow(unused)]
    pub fn paint_glyph(
        &self,
        glyph_pos: u32,
        funcs: &FontFuncs,
        paint_data: *mut c_void,
        palette_index: ::std::os::raw::c_uint,
        foreground: hb_color_t,
    ) {
        unsafe {
            hb_font_paint_glyph(
                self.font,
                glyph_pos,
                funcs.funcs,
                paint_data,
                palette_index,
                foreground,
            )
        }
    }

    pub fn get_paint_ops_for_glyph(
        &self,
        glyph_pos: u32,
        palette_index: ::std::os::raw::c_uint,
        foreground: hb_color_t,
        // TODO: pass a callback for querying custom palette colors
        // from the application
    ) -> anyhow::Result<Vec<PaintOp>> {
        let mut ops = vec![];

        let funcs = FontFuncs::new()?;

        macro_rules! func {
            ($hbfunc:ident, $method:ident) => {
                $hbfunc(
                    funcs.funcs,
                    Some(PaintOp::$method),
                    std::ptr::null_mut(),
                    None,
                );
            };
        }

        unsafe {
            func!(hb_paint_funcs_set_push_transform_func, push_transform);
            func!(hb_paint_funcs_set_pop_transform_func, pop_transform);
            func!(hb_paint_funcs_set_push_clip_glyph_func, push_clip_glyph);
            func!(hb_paint_funcs_set_push_clip_rectangle_func, push_clip_rect);
            func!(hb_paint_funcs_set_pop_clip_func, pop_clip);
            func!(hb_paint_funcs_set_color_func, paint_solid);
            func!(
                hb_paint_funcs_set_linear_gradient_func,
                paint_linear_gradient
            );
            func!(
                hb_paint_funcs_set_radial_gradient_func,
                paint_radial_gradient
            );
            func!(hb_paint_funcs_set_sweep_gradient_func, paint_sweep_gradient);
            func!(hb_paint_funcs_set_image_func, paint_image);
            func!(hb_paint_funcs_set_push_group_func, push_group);
            func!(hb_paint_funcs_set_pop_group_func, pop_group);

            // TODO: hb_paint_funcs_set_custom_palette_color_func
        }

        unsafe {
            hb_font_paint_glyph(
                self.font,
                glyph_pos,
                funcs.funcs,
                &mut ops as *mut Vec<PaintOp> as *mut _,
                palette_index,
                foreground,
            )
        }

        Ok(ops)
    }
}

#[derive(Debug, Clone)]
pub enum PaintOp {
    PushTransform {
        xx: f32,
        yx: f32,
        xy: f32,
        yy: f32,
        dx: f32,
        dy: f32,
    },
    PopTransform,
    PushGlyphClip {
        #[allow(unused)]
        glyph: hb_codepoint_t,
        draw: Vec<DrawOp>,
    },
    PushRectClip {
        xmin: f32,
        ymin: f32,
        xmax: f32,
        ymax: f32,
    },
    PopClip,
    PaintSolid {
        #[allow(unused)]
        is_foreground: bool,
        color: hb_color_t,
    },
    PaintImage {
        image: Blob,
        #[allow(unused)]
        width: u32,
        #[allow(unused)]
        height: u32,
        format: hb_tag_t,
        slant: f32,
        extents: Option<hb_glyph_extents_t>,
    },
    PaintLinearGradient {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color_line: ColorLine,
    },
    PaintRadialGradient {
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
        color_line: ColorLine,
    },
    PaintSweepGradient {
        x0: f32,
        y0: f32,
        start_angle: f32,
        end_angle: f32,
        color_line: ColorLine,
    },
    PushGroup,
    PopGroup {
        mode: hb_paint_composite_mode_t,
    },
}

impl PaintOp {
    unsafe fn paint_data(data: *mut ::std::os::raw::c_void) -> &'static mut Vec<PaintOp> {
        &mut *(data as *mut Vec<PaintOp>)
    }

    unsafe extern "C" fn push_transform(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        xx: f32,
        yx: f32,
        xy: f32,
        yy: f32,
        dx: f32,
        dy: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PushTransform {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        });
    }

    unsafe extern "C" fn pop_transform(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PopTransform);
    }

    unsafe extern "C" fn push_clip_rect(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        xmin: f32,
        ymin: f32,
        xmax: f32,
        ymax: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PushRectClip {
            xmin,
            ymin,
            xmax,
            ymax,
        });
    }

    unsafe extern "C" fn pop_clip(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PopClip);
    }

    unsafe extern "C" fn paint_solid(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        is_foreground: hb_bool_t,
        color: hb_color_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PaintSolid {
            is_foreground: is_foreground != 0,
            color,
        });
    }

    unsafe extern "C" fn paint_linear_gradient(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        color_line: *mut hb_color_line_t,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        let color_line = ColorLine::new_from_hb(color_line);
        ops.push(Self::PaintLinearGradient {
            color_line,
            x0,
            y0,
            x1,
            y1,
            x2,
            y2,
        });
    }

    unsafe extern "C" fn paint_radial_gradient(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        color_line: *mut hb_color_line_t,
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        let color_line = ColorLine::new_from_hb(color_line);
        ops.push(Self::PaintRadialGradient {
            color_line,
            x0,
            y0,
            r0,
            x1,
            y1,
            r1,
        });
    }

    unsafe extern "C" fn paint_sweep_gradient(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        color_line: *mut hb_color_line_t,
        x0: f32,
        y0: f32,
        start_angle: f32,
        end_angle: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        let color_line = ColorLine::new_from_hb(color_line);
        ops.push(Self::PaintSweepGradient {
            color_line,
            x0,
            y0,
            start_angle,
            end_angle,
        });
    }

    unsafe extern "C" fn paint_image(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        image: *mut hb_blob_t,
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        format: hb_tag_t,
        slant: f32,
        extents: *mut hb_glyph_extents_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) -> hb_bool_t {
        if format != IS_PNG && format != IS_BGRA {
            // We only support PNG and BGRA
            return 0;
        }

        let ops = Self::paint_data(paint_data);
        let image = Blob::with_reference(image);
        let extents = if extents.is_null() {
            None
        } else {
            Some(*extents)
        };
        ops.push(Self::PaintImage {
            image,
            extents,
            width,
            height,
            format,
            slant,
        });

        1
    }

    unsafe extern "C" fn push_group(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PushGroup);
    }

    unsafe extern "C" fn pop_group(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        mode: hb_paint_composite_mode_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);
        ops.push(Self::PopGroup { mode });
    }

    unsafe extern "C" fn push_clip_glyph(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        glyph: hb_codepoint_t,
        font: *mut hb_font_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::paint_data(paint_data);

        let mut draw = vec![];

        let funcs = DrawFuncs::new().unwrap();
        macro_rules! func {
            ($hbfunc:ident, $method:ident) => {
                $hbfunc(
                    funcs.funcs,
                    Some(DrawOp::$method),
                    std::ptr::null_mut(),
                    None,
                );
            };
        }
        func!(hb_draw_funcs_set_move_to_func, move_to);
        func!(hb_draw_funcs_set_line_to_func, line_to);
        func!(hb_draw_funcs_set_quadratic_to_func, quad_to);
        func!(hb_draw_funcs_set_cubic_to_func, cubic_to);
        func!(hb_draw_funcs_set_close_path_func, close_path);

        hb_font_draw_glyph(
            font,
            glyph,
            funcs.funcs,
            &mut draw as *mut Vec<DrawOp> as *mut _,
        );

        ops.push(Self::PushGlyphClip { glyph, draw });
    }
}

impl DrawOp {
    unsafe fn draw_data(data: *mut ::std::os::raw::c_void) -> &'static mut Vec<DrawOp> {
        &mut *(data as *mut Vec<DrawOp>)
    }

    unsafe extern "C" fn move_to(
        _dfuncs: *mut hb_draw_funcs_t,
        draw_data: *mut ::std::os::raw::c_void,
        _st: *mut hb_draw_state_t,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::draw_data(draw_data);
        ops.push(Self::MoveTo { to_x, to_y });
    }

    unsafe extern "C" fn line_to(
        _dfuncs: *mut hb_draw_funcs_t,
        draw_data: *mut ::std::os::raw::c_void,
        _st: *mut hb_draw_state_t,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::draw_data(draw_data);
        ops.push(Self::LineTo { to_x, to_y });
    }

    unsafe extern "C" fn quad_to(
        _dfuncs: *mut hb_draw_funcs_t,
        draw_data: *mut ::std::os::raw::c_void,
        _st: *mut hb_draw_state_t,
        control_x: f32,
        control_y: f32,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::draw_data(draw_data);
        ops.push(Self::QuadTo {
            control_x,
            control_y,
            to_x,
            to_y,
        });
    }

    unsafe extern "C" fn cubic_to(
        _dfuncs: *mut hb_draw_funcs_t,
        draw_data: *mut ::std::os::raw::c_void,
        _st: *mut hb_draw_state_t,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::draw_data(draw_data);
        ops.push(Self::CubicTo {
            control1_x,
            control1_y,
            control2_x,
            control2_y,
            to_x,
            to_y,
        });
    }

    unsafe extern "C" fn close_path(
        _dfuncs: *mut hb_draw_funcs_t,
        draw_data: *mut ::std::os::raw::c_void,
        _st: *mut hb_draw_state_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let ops = Self::draw_data(draw_data);
        ops.push(Self::ClosePath);
    }
}

impl ColorLine {
    pub fn new_from_hb(line: *mut hb_color_line_t) -> Self {
        let num_stops = unsafe {
            hb_color_line_get_color_stops(line, 0, std::ptr::null_mut(), std::ptr::null_mut())
        };
        let mut color_stops = Vec::with_capacity(num_stops as usize);
        color_stops.resize(
            num_stops as usize,
            hb_color_stop_t {
                offset: 0.,
                is_foreground: 0,
                color: 0,
            },
        );

        unsafe {
            let mut count = num_stops;
            hb_color_line_get_color_stops(line, 0, &mut count, color_stops.as_mut_ptr());
        }

        let extend = unsafe { hb_color_line_get_extend(line) };

        Self {
            color_stops: color_stops
                .into_iter()
                .map(|stop| ColorStop {
                    offset: stop.offset.into(),
                    color: if stop.is_foreground != 0 {
                        SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff)
                    } else {
                        hb_color_to_srgba_pixel(stop.color)
                    },
                })
                .collect(),
            extend: hb_extend_to_cairo(extend),
        }
    }
}

fn hb_color_to_srgba_pixel(color: hb_color_t) -> SrgbaPixel {
    let red = unsafe { hb_color_get_red(color) };
    let green = unsafe { hb_color_get_green(color) };
    let blue = unsafe { hb_color_get_blue(color) };
    let alpha = unsafe { hb_color_get_alpha(color) };
    SrgbaPixel::rgba(red, green, blue, alpha)
}

fn hb_extend_to_cairo(extend: hb_paint_extend_t) -> Extend {
    match extend {
        hb_paint_extend_t::HB_PAINT_EXTEND_PAD => Extend::Pad,
        hb_paint_extend_t::HB_PAINT_EXTEND_REPEAT => Extend::Repeat,
        hb_paint_extend_t::HB_PAINT_EXTEND_REFLECT => Extend::Reflect,
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
            from_raw_parts(info, len as usize)
        }
    }

    /// Returns glyph positions.  This is only valid after calling
    /// font->shape() on this buffer instance.
    pub fn glyph_positions(&self) -> &[hb_glyph_position_t] {
        unsafe {
            let mut len: u32 = 0;
            let pos = hb_buffer_get_glyph_positions(self.buf, &mut len as *mut _);
            from_raw_parts(pos, len as usize)
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

pub struct FontFuncs {
    funcs: *mut hb_paint_funcs_t,
}

impl Drop for FontFuncs {
    fn drop(&mut self) {
        unsafe {
            hb_paint_funcs_destroy(self.funcs);
        }
    }
}

impl FontFuncs {
    pub fn new() -> anyhow::Result<Self> {
        let funcs = unsafe { hb_paint_funcs_create() };
        anyhow::ensure!(!funcs.is_null(), "hb_paint_funcs_create failed");
        Ok(Self { funcs })
    }
}

pub struct DrawFuncs {
    funcs: *mut hb_draw_funcs_t,
}

impl Drop for DrawFuncs {
    fn drop(&mut self) {
        unsafe {
            hb_draw_funcs_destroy(self.funcs);
        }
    }
}

impl DrawFuncs {
    pub fn new() -> anyhow::Result<Self> {
        let funcs = unsafe { hb_draw_funcs_create() };
        anyhow::ensure!(!funcs.is_null(), "hb_draw_funcs_create failed");
        Ok(Self { funcs })
    }
}

pub struct TagString([u8; 4]);

impl std::convert::AsRef<str> for TagString {
    fn as_ref(&self) -> &str {
        std::str::from_utf8(&self.0).expect("tag to be valid ascii")
    }
}

impl std::ops::Deref for TagString {
    type Target = str;
    fn deref(&self) -> &str {
        std::str::from_utf8(&self.0).expect("tag to be valid ascii")
    }
}

impl std::fmt::Display for TagString {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.as_ref().fmt(fmt)
    }
}

pub const fn hb_tag(c1: u8, c2: u8, c3: u8, c4: u8) -> hb_tag_t {
    ((c1 as u32) << 24) | ((c2 as u32) << 16) | ((c3 as u32) << 8) | (c4 as u32)
}

pub fn hb_color(b: u8, g: u8, r: u8, a: u8) -> hb_tag_t {
    hb_tag(b, g, r, a)
}

pub fn hb_tag_to_string(tag: hb_tag_t) -> TagString {
    let mut buf = [0u8; 4];

    // safety: hb_tag_to_string stores 4 bytes to the provided buffer
    unsafe {
        harfbuzz::hb_tag_to_string(tag, &mut buf as *mut u8 as *mut c_char);
    }
    TagString(buf)
}

/// Wrapper around std::slice::from_raw_parts that allows for ptr to be
/// null. In the null ptr case, an empty slice is returned.
/// This is necessary because harfbuzz may sometimes encode
/// empty arrays in that way, and rust 1.78 will panic if a null
/// ptr is passed in.
pub(crate) unsafe fn from_raw_parts<'a, T>(ptr: *const T, size: usize) -> &'a [T] {
    if ptr.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, size)
    }
}
