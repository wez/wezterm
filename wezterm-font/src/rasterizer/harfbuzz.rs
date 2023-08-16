use crate::hbwrap::{
    hb_blob_get_data, hb_blob_t, hb_bool_t, hb_codepoint_t, hb_color_line_t, hb_color_t,
    hb_draw_funcs_t, hb_draw_state_t, hb_font_draw_glyph, hb_font_t, hb_glyph_extents_t,
    hb_paint_composite_mode_t, hb_paint_funcs_t, hb_tag_t, hb_tag_to_string, DrawFuncs, Face, Font,
    FontFuncs,
};
use crate::rasterizer::FAKE_ITALIC_SKEW;
use crate::units::PixelLength;
use crate::{FontRasterizer, ParsedFont, RasterizedGlyph};
use anyhow::Context;
use image::DynamicImage::{ImageLuma8, ImageLumaA8};
use resvg::tiny_skia::{
    BlendMode, Color, FillRule, Paint, PathBuilder, Pixmap, Shader, Stroke, Transform,
};

pub struct HarfbuzzRasterizer {
    face: Face,
    font: Font,
    funcs: FontFuncs,
}

impl HarfbuzzRasterizer {
    pub fn from_locator(parsed: &ParsedFont) -> anyhow::Result<Self> {
        let mut font = Font::from_locator(&parsed.handle)?;
        font.set_ot_funcs();
        let face = font.get_face();

        if parsed.synthesize_italic {
            font.set_synthetic_slant(FAKE_ITALIC_SKEW as f32);
        }
        if parsed.synthesize_bold {
            font.set_synthetic_bold(0.02, 0.02, false);
        }

        let mut funcs = FontFuncs::new()?;
        funcs.set_push_transform_func(Some(PaintData::push_transform_trampoline));
        funcs.set_pop_transform_func(Some(PaintData::pop_transform_trampoline));
        funcs.set_image_func(Some(PaintData::image_trampoline));
        funcs.set_linear_gradient(Some(PaintData::linear_gradient_trampoline));
        funcs.set_push_clip_glyph_func(Some(PaintData::push_clip_glyph_trampoline));
        funcs.set_push_clip_rectangle_func(Some(PaintData::push_clip_rectangle_trampoline));
        funcs.set_pop_clip_func(Some(PaintData::pop_clip_trampoline));
        funcs.set_color_func(Some(PaintData::set_color_trampoline));
        funcs.set_radial_gradient(Some(PaintData::set_radial_gradient_trampoline));
        funcs.set_sweep_gradient(Some(PaintData::set_sweep_gradient_trampline));
        funcs.set_push_group(Some(PaintData::push_group_trampoline));
        funcs.set_pop_group(Some(PaintData::pop_group_trampoline));
        funcs.set_custom_palette_color(Some(PaintData::custom_palette_color_trampoline));

        Ok(Self { face, font, funcs })
    }
}

impl FontRasterizer for HarfbuzzRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        let pixel_size = (size * dpi as f64 / 72.) as u32;
        let upem = self.face.get_upem();

        let scale = pixel_size as i32 * 64;
        let ppem = pixel_size;
        log::info!("computed scale={scale}, ppem={ppem}, upem={upem}");

        let mut data = PaintData {
            rasterizer: self,
            glyph_pos,
            size,
            dpi,
            upem,
            ppem,
            result: RasterizedGlyph {
                data: vec![],
                height: 0,
                width: 0,
                bearing_x: PixelLength::new(0.),
                bearing_y: PixelLength::new(0.),
                has_color: false,
            },
            path_builder: PathBuilder::new(),
        };

        self.font.set_ppem(ppem, ppem);
        self.font.set_ptem(size as f32);
        self.font.set_font_scale(scale, scale);
        self.font.paint_glyph(
            glyph_pos,
            &self.funcs,
            &mut data as *mut _ as _,
            0,          // palette index 0
            0xffffffff, // 100% white
        );

        if let Some(path) = data
            .path_builder
            .finish()
            .and_then(|path| path.transform(Transform::from_scale(1.0 / 64., -1.0 / 64.)))
        {
            let bounds = path.bounds();
            log::info!("got a path with bounds {bounds:?}");

            let width = (bounds.right() + (bounds.left().min(0.) * -1.0)).ceil();
            let height = (bounds.bottom() + (bounds.top().min(0.) * -1.0)).ceil();

            if let Some(mut pixmap) = Pixmap::new(width as u32, height as u32) {
                log::info!("using pixmap {}x{}", pixmap.width(), pixmap.height());
                pixmap.fill_path(
                    &path,
                    &Paint {
                        shader: Shader::SolidColor(Color::WHITE),
                        blend_mode: BlendMode::SourceOver,
                        anti_alias: true,
                        force_hq_pipeline: true,
                    },
                    FillRule::default(),
                    Transform::from_translate(
                        bounds.left().min(0.) * -1.,
                        bounds.top().min(0.) * -1.,
                    ),
                    /*
                    Transform::from_scale(1.0, -1.0)
                        .post_translate(bounds.left().min(0.) * -1., bounds.top().min(0.) * -1.)
                        .post_translate(0.0, bounds.bottom()),
                        */
                    None, // mask
                );

                data.result.height = pixmap.height() as usize;
                data.result.width = pixmap.width() as usize;
                data.result.data = pixmap.take();

                data.result.bearing_x = PixelLength::new(bounds.left().min(0.) as f64);
                data.result.bearing_y = PixelLength::new(bounds.top() as f64 * -1.);
            }
        }

        Ok(data.result)
    }
}

struct PaintData<'a> {
    rasterizer: &'a HarfbuzzRasterizer,
    glyph_pos: u32,
    size: f64,
    dpi: u32,
    upem: u32,
    ppem: u32,
    result: RasterizedGlyph,
    path_builder: PathBuilder,
}

impl<'a> PaintData<'a> {
    extern "C" fn push_transform_trampoline(
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
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        this.push_transform(xx, yx, xy, yy, dx, dy);
    }

    extern "C" fn pop_transform_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        this.pop_transform();
    }

    extern "C" fn image_trampoline(
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
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };

        let mut image_len = 0;
        let mut image_ptr = unsafe { hb_blob_get_data(image, &mut image_len) };
        let image =
            unsafe { std::slice::from_raw_parts(image_ptr as *const u8, image_len as usize) };

        let result = this.image(image, width, height, format, slant, unsafe {
            if extents.is_null() {
                None
            } else {
                Some(&*extents)
            }
        });
        match result {
            Ok(()) => 1,
            Err(err) => {
                log::error!("image: {err:#}");
                0
            }
        }
    }

    extern "C" fn linear_gradient_trampoline(
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
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let color_line: &hb_color_line_t = unsafe { &*(color_line as *const hb_color_line_t) };
        this.linear_gradient(color_line, x0, y0, x1, y1, x2, y2);
    }

    extern "C" fn push_clip_glyph_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        glyph: hb_codepoint_t,
        font: *mut hb_font_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!("push_clip_glyph_trampoline: glyph={glyph}");

        match DrawFuncs::new() {
            Ok(mut draw_funcs) => {
                draw_funcs.set_move_to_func(Some(PaintData::move_to_trampoline));
                draw_funcs.set_line_to_func(Some(PaintData::line_to_trampoline));
                draw_funcs.set_quadratic_to_func(Some(PaintData::quadratic_to_trampoline));
                draw_funcs.set_cubic_to(Some(PaintData::cubic_to_trampoline));
                draw_funcs.set_close_path(Some(PaintData::close_path_trampoline));

                unsafe { hb_font_draw_glyph(font, glyph, draw_funcs.as_ptr(), paint_data) }
            }
            Err(err) => {
                log::error!("DrawFuncs::new: {err:#}");
            }
        }
    }

    extern "C" fn move_to_trampoline(
        _funcs: *mut hb_draw_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        st: *mut hb_draw_state_t,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let st: &mut hb_draw_state_t = unsafe { &mut *st };

        let st = DebuggableDrawState(st);
        this.move_to(st, to_x, to_y);
    }

    extern "C" fn line_to_trampoline(
        _funcs: *mut hb_draw_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        st: *mut hb_draw_state_t,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let st: &mut hb_draw_state_t = unsafe { &mut *st };
        let st = DebuggableDrawState(st);

        this.line_to(st, to_x, to_y);
    }

    extern "C" fn quadratic_to_trampoline(
        _funcs: *mut hb_draw_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        st: *mut hb_draw_state_t,
        control_x: f32,
        control_y: f32,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let st: &mut hb_draw_state_t = unsafe { &mut *st };
        let st = DebuggableDrawState(st);

        this.quadratic_to(st, control_x, control_y, to_x, to_y);
    }

    extern "C" fn cubic_to_trampoline(
        _funcs: *mut hb_draw_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        st: *mut hb_draw_state_t,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        to_x: f32,
        to_y: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let st: &mut hb_draw_state_t = unsafe { &mut *st };
        let st = DebuggableDrawState(st);

        this.cubic_to(
            st, control1_x, control1_y, control2_x, control2_y, to_x, to_y,
        );
    }

    extern "C" fn close_path_trampoline(
        _funcs: *mut hb_draw_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        st: *mut hb_draw_state_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let st: &mut hb_draw_state_t = unsafe { &mut *st };
        let st = DebuggableDrawState(st);

        this.close_path(st);
    }

    extern "C" fn push_clip_rectangle_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        xmin: f32,
        ymin: f32,
        xmax: f32,
        ymax: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!(
            "push_clip_rectangle_trampoline: xmin={xmin} ymin={ymin} xmax={xmax} ymax={ymax}"
        );
    }

    extern "C" fn pop_clip_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!("pop_clip_trampoline");
    }

    extern "C" fn set_color_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        is_foreground: hb_bool_t,
        color: hb_color_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!("set_color_trampoline is_foreground={is_foreground} color={color:x?}");
    }

    extern "C" fn set_radial_gradient_trampoline(
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
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let color_line: &hb_color_line_t = unsafe { &*(color_line as *const hb_color_line_t) };
        log::info!("set_radial_gradient_trampoline {color_line:?} x0={x0} y0={y0} r0={r0} x1={x1} y1={y1} r1={r1}");
    }

    extern "C" fn set_sweep_gradient_trampline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        color_line: *mut hb_color_line_t,
        x0: f32,
        y0: f32,
        start_angle: f32,
        end_angle: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let color_line: &hb_color_line_t = unsafe { &*(color_line as *const hb_color_line_t) };
        log::info!("set_sweep_gradient_trampline {color_line:?} x0={x0} y0={y0} start_angle={start_angle} end_angle={end_angle}");
    }

    extern "C" fn push_group_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!("push_group");
    }

    extern "C" fn pop_group_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        mode: hb_paint_composite_mode_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        log::info!("pop_group {mode:?}");
    }

    extern "C" fn custom_palette_color_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        color_index: ::std::os::raw::c_uint,
        color: *mut hb_color_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) -> hb_bool_t {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        let color: &hb_color_t = unsafe { &*color };
        log::info!("custom_palette_color_trampoline color_index={color_index} = {color:x?}");
        1
    }

    fn push_transform(&mut self, xx: f32, yx: f32, xy: f32, yy: f32, dx: f32, dy: f32) {
        log::info!("push_transform: xx={xx} yx={yx} xy={xy} yy={yy} dx={dx} dy={dy}");
    }
    fn pop_transform(&mut self) {
        log::info!("pop_transform");
    }
    fn image(
        &mut self,
        image: &[u8],
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        format: hb_tag_t,
        slant: f32,
        extents: Option<&hb_glyph_extents_t>,
    ) -> anyhow::Result<()> {
        let format = hb_tag_to_string(format);
        log::info!("image {width}x{height} format={format} slant={slant} {extents:?}");

        let decoded = image::io::Reader::new(std::io::Cursor::new(image))
            .with_guessed_format()?
            .decode()?;

        match &decoded {
            ImageLuma8(_) | ImageLumaA8(_) => self.result.has_color = false,
            _ => self.result.has_color = true,
        }

        let mut decoded = decoded.into_rgba8();

        // Convert to premultiplied form
        fn multiply_alpha(alpha: u8, color: u8) -> u8 {
            let temp: u32 = alpha as u32 * (color as u32 + 0x80);

            ((temp + (temp >> 8)) >> 8) as u8
        }

        for (_x, _y, pixel) in decoded.enumerate_pixels_mut() {
            let alpha = pixel[3];
            if alpha == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            } else {
                if alpha != 0xff {
                    for n in 0..3 {
                        pixel[n] = multiply_alpha(alpha, pixel[n]);
                    }
                }
            }
        }

        // Crop to the non-transparent portions of the image
        let cropped = crate::rasterizer::crop_to_non_transparent(&mut decoded).to_image();
        self.result.height = cropped.height() as usize;
        self.result.width = cropped.width() as usize;

        log::info!("cropped -> {}x{}", self.result.width, self.result.height);
        // FIXME: compensate for glypcache scaling here.
        // However, we don't know the base_metrics to scale against,
        // so perhaps we should fix the reported metrics from harfbuzz
        // for images, and then fixup the freetype rasterizer to scale
        // the image that it produces?

        self.result.data = cropped.into_vec();

        let (bearing_x, bearing_y) = extents
            .map(|ext| {
                (
                    ext.x_bearing as f64 / 64.,
                    (ext.y_bearing - ext.height.min(0)) as f64 / 64.,
                )
            })
            .unwrap_or((0., 0.));
        self.result.bearing_x = PixelLength::new(bearing_x);
        self.result.bearing_y = PixelLength::new(bearing_y);

        Ok(())
    }

    fn linear_gradient(
        &mut self,
        color_line: &hb_color_line_t,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) {
        log::trace!(
            "linear_gradient {color_line:?} x0={x0}, y0={y0}, x1={x1}, y1={y1}, x2={x2},y2={y2}"
        );
    }

    fn move_to(&mut self, st: DebuggableDrawState, to_x: f32, to_y: f32) {
        log::trace!("move_to: st={st:?} to={to_x},{to_y}");
        self.path_builder.move_to(to_x, to_y);
    }

    fn line_to(&mut self, st: DebuggableDrawState, to_x: f32, to_y: f32) {
        log::trace!("line_to: st={st:?} to={to_x},{to_y}");
        self.path_builder.line_to(to_x, to_y);
    }

    fn quadratic_to(
        &mut self,
        st: DebuggableDrawState,
        control_x: f32,
        control_y: f32,
        to_x: f32,
        to_y: f32,
    ) {
        log::trace!(
            "quadratic_to: st={st:?} \
            control={control_x},{control_y} to={to_x},{to_y}"
        );

        // Express quadratic as a cubic
        // <https://stackoverflow.com/a/55034115/149111>

        self.path_builder.cubic_to(
            st.0.current_x + (2. / 3.) * (control_x - st.0.current_x),
            st.0.current_y + (2. / 3.) * (control_y - st.0.current_y),
            to_x + (2. / 3.) * (control_x - to_x),
            to_y + (2. / 3.) * (control_y - to_y),
            to_x,
            to_y,
        );
    }

    fn cubic_to(
        &mut self,
        st: DebuggableDrawState,
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        to_x: f32,
        to_y: f32,
    ) {
        log::trace!(
            "cubic_to: st={st:?} \
            control1={control1_x},{control1_y} control2={control2_x},{control2_y} to={to_x},{to_y}"
        );

        self.path_builder
            .cubic_to(control1_x, control1_y, control2_x, control2_y, to_x, to_y);
    }

    fn close_path(&mut self, st: DebuggableDrawState) {
        log::trace!("close_path {st:?}");
        self.path_builder.close();
    }
}

struct DebuggableDrawState<'a>(&'a hb_draw_state_t);

impl<'a> std::fmt::Debug for DebuggableDrawState<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("hb_draw_state_t")
            .field("path_open", &self.0.path_open)
            .field("path_start_x", &self.0.path_start_x)
            .field("path_start_y", &self.0.path_start_y)
            .field("current_x", &self.0.current_x)
            .field("current_y", &self.0.current_y)
            .finish()
    }
}
