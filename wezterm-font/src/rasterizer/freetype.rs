use crate::ftwrap::{
    composite_mode_to_operator, fixed_to_f32, fixed_to_f64, two_dot_14_to_f64, vector_x_y,
    FT_Affine23, FT_ColorIndex, FT_ColorLine, FT_ColorStop, FT_Get_Colorline_Stops, FT_Int32,
    FT_PaintExtend, IsSvg, FT_LOAD_NO_HINTING,
};
use crate::hbwrap::DrawOp;
use crate::parser::ParsedFont;
use crate::rasterizer::harfbuzz::argb_to_rgba;
use crate::rasterizer::{FontRasterizer, FAKE_ITALIC_SKEW};
use crate::units::*;
use crate::{ftwrap, RasterizedGlyph};
use ::freetype::{
    FT_Color_Root_Transform, FT_GlyphSlotRec_, FT_Matrix, FT_Opaque_Paint_, FT_PaintFormat_,
};
use anyhow::bail;
use cairo::{
    Content, Context, Extend, Format, ImageSurface, LinearGradient, Matrix, Operator,
    RadialGradient, RecordingSurface,
};
use config::{DisplayPixelGeometry, FreeTypeLoadFlags, FreeTypeLoadTarget};
use std::cell::RefCell;
use std::f64::consts::PI;
use std::mem::MaybeUninit;
use std::{mem, slice};
use wezterm_color_types::{linear_u8_to_srgb8, SrgbaPixel};

pub struct FreeTypeRasterizer {
    has_color: bool,
    face: RefCell<ftwrap::Face>,
    _lib: ftwrap::Library,
    synthesize_bold: bool,
    freetype_load_target: Option<FreeTypeLoadTarget>,
    freetype_render_target: Option<FreeTypeLoadTarget>,
    freetype_load_flags: Option<FreeTypeLoadFlags>,
    display_pixel_geometry: DisplayPixelGeometry,
    scale: f64,
}

impl FontRasterizer for FreeTypeRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        self.face
            .borrow_mut()
            .set_font_size(size * self.scale, dpi)?;

        let (load_flags, render_mode) = ftwrap::compute_load_flags_from_config(
            self.freetype_load_flags,
            self.freetype_load_target,
            self.freetype_render_target,
        );

        let mut face = self.face.borrow_mut();
        let ft_glyph = match face.load_and_render_glyph(
            glyph_pos,
            load_flags,
            render_mode,
            self.synthesize_bold,
        ) {
            Ok(g) => g,
            Err(err) => {
                if err.root_cause().downcast_ref::<IsSvg>().is_some() {
                    drop(face);
                    return self
                        .rasterize_outlines(glyph_pos, load_flags | FT_LOAD_NO_HINTING as i32);
                }
                return Err(err);
            }
        };

        let mode: ftwrap::FT_Pixel_Mode =
            unsafe { mem::transmute(u32::from(ft_glyph.bitmap.pixel_mode)) };

        // pitch is the number of bytes per source row
        let pitch = ft_glyph.bitmap.pitch.abs() as usize;
        let data = unsafe {
            slice::from_raw_parts_mut(
                ft_glyph.bitmap.buffer,
                ft_glyph.bitmap.rows as usize * pitch,
            )
        };

        let glyph = match mode {
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => self.rasterize_lcd(pitch, ft_glyph, data),
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => self.rasterize_bgra(pitch, ft_glyph, data),
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => self.rasterize_gray(pitch, ft_glyph, data),
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_MONO => self.rasterize_mono(pitch, ft_glyph, data),
            mode => bail!("unhandled pixel mode: {:?}", mode),
        };

        Ok(glyph)
    }
}

impl FreeTypeRasterizer {
    fn rasterize_mono(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch;
            let dest_offset = y * width * 4;
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
                        for j in 0..4 {
                            rgba[dest_offset + (x * 4) + j] = 0xff;
                        }
                    }
                    b <<= 1;
                    x += 1;
                }
            }
        }
        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: false,
        }
    }

    fn rasterize_gray(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch;
            let dest_offset = y * width * 4;
            for x in 0..width {
                let linear_gray = data[src_offset + x];
                let gray = linear_u8_to_srgb8(linear_gray);

                // Texture is SRGBA, which in OpenGL means
                // that the RGB values are gamma adjusted
                // non-linear values, but the A value is
                // linear!

                rgba[dest_offset + (x * 4)] = gray;
                rgba[dest_offset + (x * 4) + 1] = gray;
                rgba[dest_offset + (x * 4) + 2] = gray;
                rgba[dest_offset + (x * 4) + 3] = linear_gray;
            }
        }
        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: false,
        }
    }

    fn rasterize_lcd(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize / 3;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch as usize;
            let dest_offset = y * width * 4;
            for x in 0..width {
                let red = data[src_offset + (x * 3)];
                let green = data[src_offset + (x * 3) + 1];
                let blue = data[src_offset + (x * 3) + 2];

                let linear_alpha = red.max(green).max(blue);

                // Texture is SRGBA, which in OpenGL means
                // that the RGB values are gamma adjusted
                // non-linear values, but the A value is
                // linear!

                let red = linear_u8_to_srgb8(red);
                let green = linear_u8_to_srgb8(green);
                let blue = linear_u8_to_srgb8(blue);

                let (red, blue) = match self.display_pixel_geometry {
                    DisplayPixelGeometry::RGB => (red, blue),
                    DisplayPixelGeometry::BGR => (blue, red),
                };

                rgba[dest_offset + (x * 4)] = red;
                rgba[dest_offset + (x * 4) + 1] = green;
                rgba[dest_offset + (x * 4) + 2] = blue;
                rgba[dest_offset + (x * 4) + 3] = linear_alpha;
            }
        }

        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: self.has_color,
        }
    }

    fn rasterize_bgra(
        &self,
        _pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &'static [u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;

        let mut source_image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
            width as u32,
            height as u32,
            data,
        )
        .expect("image data to be valid");

        // emoji glyphs don't always fill the bitmap size, so we compute
        // the non-transparent bounds

        let mut cropped = crate::rasterizer::crop_to_non_transparent(&mut source_image).to_image();
        crate::rasterizer::swap_red_and_blue(&mut cropped);

        let dest_width = cropped.width() as usize;
        let dest_height = cropped.height() as usize;

        RasterizedGlyph {
            data: cropped.into_vec(),
            height: dest_height,
            width: dest_width,
            bearing_x: PixelLength::new(
                f64::from(ft_glyph.bitmap_left) * (dest_width as f64 / width as f64),
            ),
            bearing_y: PixelLength::new(
                f64::from(ft_glyph.bitmap_top) * (dest_height as f64 / height as f64),
            ),
            has_color: self.has_color,
        }
    }

    pub fn from_locator(
        parsed: &ParsedFont,
        display_pixel_geometry: DisplayPixelGeometry,
    ) -> anyhow::Result<Self> {
        log::trace!("Rasterizier wants {:?}", parsed);
        let lib = ftwrap::Library::new()?;
        let mut face = lib.face_from_locator(&parsed.handle)?;
        let has_color = unsafe {
            (((*face.face).face_flags as u32) & (ftwrap::FT_FACE_FLAG_COLOR as u32)) != 0
        };

        if parsed.synthesize_italic {
            face.set_transform(Some(FT_Matrix {
                xx: 1 * 65536,                         // scale x
                yy: 1 * 65536,                         // scale y
                xy: (FAKE_ITALIC_SKEW * 65536.0) as _, // skew x
                yx: 0 * 65536,                         // skew y
            }));
        }

        Ok(Self {
            _lib: lib,
            face: RefCell::new(face),
            has_color,
            synthesize_bold: parsed.synthesize_bold,
            freetype_load_flags: parsed.freetype_load_flags,
            freetype_load_target: parsed.freetype_load_target,
            freetype_render_target: parsed.freetype_render_target,
            display_pixel_geometry,
            scale: parsed.scale.unwrap_or(1.),
        })
    }

    fn rasterize_outlines(
        &self,
        glyph_pos: u32,
        load_flags: FT_Int32,
    ) -> anyhow::Result<RasterizedGlyph> {
        let mut face = self.face.borrow_mut();
        let paint = face.get_color_glyph_paint(
            glyph_pos,
            FT_Color_Root_Transform::FT_COLOR_INCLUDE_ROOT_TRANSFORM,
        )?;
        let clip_box = face.get_color_glyph_clip_box(glyph_pos)?;

        let palette = face.get_palette_data()?;
        log::info!("Palette: {palette:#?}");
        face.select_palette(0)?;

        log::info!("got clip_box: {clip_box:?}");
        let mut walker = Walker {
            load_flags,
            face: &mut face,
            ops: vec![],
        };
        walker.walk_paint(paint, 0)?;

        log::trace!("ops: {:#?}", walker.ops);

        rasterize_from_ops(walker.ops, 1., -1.)
    }
}

fn rasterize_from_ops(
    ops: Vec<PaintOp>,
    scale_x: f64,
    scale_y: f64,
) -> anyhow::Result<RasterizedGlyph> {
    let (surface, has_color) = record_to_cairo_surface(ops, scale_x, scale_y)?;
    let (left, top, width, height) = surface.ink_extents();
    log::info!("extents: left={left} top={top} width={width} height={height}");

    if width as usize == 0 || height as usize == 0 {
        return Ok(RasterizedGlyph {
            data: vec![],
            height: 0,
            width: 0,
            bearing_x: PixelLength::new(0.),
            bearing_y: PixelLength::new(0.),
            has_color: false,
        });
    }

    let mut bounds_adjust = Matrix::identity();
    bounds_adjust.translate(left * -1., top * -1.);
    log::info!("dims: {width}x{height} {bounds_adjust:?}");

    let target = ImageSurface::create(Format::ARgb32, width as i32, height as i32)?;
    {
        let context = Context::new(&target)?;
        context.transform(bounds_adjust);
        context.set_antialias(cairo::Antialias::Best);
        context.set_source_surface(surface, 0., 0.)?;
        context.paint()?;
    }

    let mut data = target.take_data()?.to_vec();
    argb_to_rgba(&mut data);

    Ok(RasterizedGlyph {
        data,
        height: height as usize,
        width: width as usize,
        bearing_x: PixelLength::new(left.min(0.)),
        bearing_y: PixelLength::new(top * -1.),
        has_color,
    })
}

struct Walker<'a> {
    load_flags: FT_Int32,
    face: &'a mut crate::ftwrap::Face,
    ops: Vec<PaintOp>,
}

impl<'a> Walker<'a> {
    fn walk_paint(&mut self, paint: FT_Opaque_Paint_, level: usize) -> anyhow::Result<()> {
        use FT_PaintFormat_::*;

        let paint = self.face.get_paint(paint)?;

        unsafe {
            match paint.format {
                FT_COLR_PAINTFORMAT_COLR_LAYERS => {
                    log::info!("{level:>3} {:?}", paint.format);
                    let mut iter = paint.u.colr_layers.as_ref().layer_iterator;
                    while let Ok(inner_paint) = self.face.get_paint_layers(&mut iter) {
                        self.walk_paint(inner_paint, level + 1)?;
                    }
                }
                FT_COLR_PAINTFORMAT_SOLID => {
                    let op = PaintOp::PaintSolid(
                        self.decode_color_index(&paint.u.solid.as_ref().color)?,
                    );
                    log::info!("{level:>3} {:?} {op:x?}", paint.format);
                    self.ops.push(op);
                }
                FT_COLR_PAINTFORMAT_LINEAR_GRADIENT => {
                    let grad = paint.u.linear_gradient.as_ref();
                    log::info!("{level:>3} {grad:?}");
                    let (x0, y0) = vector_x_y(&grad.p0);
                    let (x1, y1) = vector_x_y(&grad.p1);
                    let (x2, y2) = vector_x_y(&grad.p2);
                    // FIXME: gradient vectors are expressed as font units,
                    // do we need to adjust them here?
                    let paint = PaintOp::PaintLinearGradient {
                        x0,
                        y0,
                        x1,
                        y1,
                        x2,
                        y2,
                        color_line: self.decode_color_line(&grad.colorline)?,
                    };
                    self.ops.push(paint);
                }
                FT_COLR_PAINTFORMAT_RADIAL_GRADIENT => {
                    let grad = paint.u.radial_gradient.as_ref();
                    log::info!("{level:>3} {grad:?}");
                    let (x0, y0) = vector_x_y(&grad.c0);
                    let (x1, y1) = vector_x_y(&grad.c1);

                    let paint = PaintOp::PaintRadialGradient {
                        x0,
                        y0,
                        x1,
                        y1,
                        r0: grad.r0 as f32,
                        r1: grad.r1 as f32,
                        color_line: self.decode_color_line(&grad.colorline)?,
                    };
                    self.ops.push(paint);
                }
                FT_COLR_PAINTFORMAT_SWEEP_GRADIENT => {
                    let grad = paint.u.sweep_gradient.as_ref();
                    log::info!("{level:>3} {grad:?}");
                    let (x0, y0) = vector_x_y(&grad.center);
                    let start_angle = fixed_to_f32(grad.start_angle);
                    let end_angle = fixed_to_f32(grad.end_angle);

                    let paint = PaintOp::PaintSweepGradient {
                        x0,
                        y0,
                        start_angle,
                        end_angle,
                        color_line: self.decode_color_line(&grad.colorline)?,
                    };
                    self.ops.push(paint);
                }
                FT_COLR_PAINTFORMAT_GLYPH => {
                    // FIXME: harfbuzz, in COLR.hh, pushes the inverse of
                    // the root transform before emitting the glyph
                    // DrawOps, then pops it prior to recursing into
                    // the child paint
                    log::info!("{level:>3} {:?}", paint.u.glyph.as_ref());

                    let glyph_index = paint.u.glyph.as_ref().glyphID;

                    let ops = self
                        .face
                        .load_glyph_outlines(glyph_index, self.load_flags)?;
                    log::trace!("{level:>3} -> {ops:?}");
                    self.ops.push(PaintOp::PushClip(ops));

                    self.walk_paint(paint.u.glyph.as_ref().paint, level + 1)?;

                    self.ops.push(PaintOp::PopClip);
                }
                FT_COLR_PAINTFORMAT_COLR_GLYPH => {
                    let g = paint.u.colr_glyph.as_ref();
                    log::info!("{level:>3} {g:?}");
                    self.ops.push(PaintOp::PushGroup);
                    let paint = self.face.get_color_glyph_paint(
                        g.glyphID,
                        FT_Color_Root_Transform::FT_COLOR_NO_ROOT_TRANSFORM,
                    )?;

                    self.walk_paint(paint, level + 1)?;
                    self.ops.push(PaintOp::PopGroup(Operator::Over));
                }
                FT_COLR_PAINTFORMAT_TRANSFORM => {
                    let t = paint.u.transform.as_ref();
                    let matrix = affine2x3_to_matrix(t.affine);
                    log::info!("{level:>3} {t:?} -> {matrix:?}");
                    self.ops.push(PaintOp::PushTransform(matrix));
                    self.walk_paint(t.paint, level + 1)?;
                    self.ops.push(PaintOp::PopTransform);
                }
                FT_COLR_PAINTFORMAT_TRANSLATE => {
                    let t = paint.u.translate.as_ref();
                    log::info!("{level:>3} {t:?}");

                    let mut matrix = Matrix::identity();
                    matrix.translate(fixed_to_f64(t.dx), fixed_to_f64(t.dy));
                    self.ops.push(PaintOp::PushTransform(matrix));
                    self.walk_paint(t.paint, level + 1)?;
                    self.ops.push(PaintOp::PopTransform);
                }
                FT_COLR_PAINTFORMAT_SCALE => {
                    let scale = paint.u.scale.as_ref();
                    log::info!("{level:>3} {scale:?}");

                    // Scaling around a center coordinate
                    let center_x = fixed_to_f64(scale.center_x);
                    let center_y = fixed_to_f64(scale.center_x);

                    let mut p1 = Matrix::identity();
                    p1.translate(center_x, center_y);

                    let mut p2 = Matrix::identity();
                    p2.scale(fixed_to_f64(scale.scale_x), fixed_to_f64(scale.scale_y));

                    let mut p3 = Matrix::identity();
                    p3.translate(-center_x, -center_y);

                    self.ops.push(PaintOp::PushTransform(p1));
                    self.ops.push(PaintOp::PushTransform(p2));
                    self.ops.push(PaintOp::PushTransform(p3));
                    self.walk_paint(scale.paint, level + 1)?;
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                }
                FT_COLR_PAINTFORMAT_ROTATE => {
                    let rot = paint.u.rotate.as_ref();
                    log::info!("{level:>3} {rot:?}");

                    // Rotating around a center coordinate
                    let center_x = fixed_to_f64(rot.center_x);
                    let center_y = fixed_to_f64(rot.center_x);

                    let mut p1 = Matrix::identity();
                    p1.translate(center_x, center_y);

                    let mut p2 = Matrix::identity();
                    p2.rotate(PI * fixed_to_f64(rot.angle));

                    let mut p3 = Matrix::identity();
                    p3.translate(-center_x, -center_y);

                    self.ops.push(PaintOp::PushTransform(p1));
                    self.ops.push(PaintOp::PushTransform(p2));
                    self.ops.push(PaintOp::PushTransform(p3));
                    self.walk_paint(rot.paint, level + 1)?;
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                }
                FT_COLR_PAINTFORMAT_SKEW => {
                    let skew = paint.u.skew.as_ref();
                    log::info!("{level:>3} {skew:?}");

                    // Skewing around a center coordinate
                    let center_x = fixed_to_f64(skew.center_x);
                    let center_y = fixed_to_f64(skew.center_x);

                    let mut p1 = Matrix::identity();
                    p1.translate(center_x, center_y);

                    let x_skew_angle = fixed_to_f64(skew.x_skew_angle);
                    let y_skew_angle = fixed_to_f64(skew.y_skew_angle);
                    let x = (PI * -x_skew_angle).tan();
                    let y = (PI * y_skew_angle).tan();

                    let p2 = Matrix::new(1., y, x, 1., 0., 0.);

                    let mut p3 = Matrix::identity();
                    p3.translate(-center_x, -center_y);

                    self.ops.push(PaintOp::PushTransform(p1));
                    self.ops.push(PaintOp::PushTransform(p2));
                    self.ops.push(PaintOp::PushTransform(p3));
                    self.walk_paint(skew.paint, level + 1)?;
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                    self.ops.push(PaintOp::PopTransform);
                }
                FT_COLR_PAINTFORMAT_COMPOSITE => {
                    let comp = paint.u.composite.as_ref();
                    log::info!("{level:>3} {comp:?}");

                    self.walk_paint(comp.backdrop_paint, level + 1)?;
                    self.ops.push(PaintOp::PushGroup);
                    self.walk_paint(comp.source_paint, level + 1)?;
                    self.ops.push(PaintOp::PopGroup(composite_mode_to_operator(
                        comp.composite_mode,
                    )));
                }
                wat => {
                    anyhow::bail!("unknown/unhandled FT_PaintFormat_ value {wat:?}");
                }
            }
        }

        Ok(())
    }

    fn decode_color_index(&mut self, c: &FT_ColorIndex) -> anyhow::Result<SrgbaPixel> {
        let alpha = two_dot_14_to_f64(c.alpha);
        let (r, g, b, a) = if c.palette_index == 0xffff {
            // Foreground color.
            // We use white here because the rendering stage will
            // tint this with the actual color in the correct context
            (0xff, 0xff, 0xff, 1.0)
        } else {
            let color = self.face.get_palette_entry(c.palette_index as _)?;
            (color.red, color.green, color.blue, c.alpha as f64 / 255.)
        };

        let alpha = (a * alpha * 255.) as u8;
        Ok(SrgbaPixel::rgba(r, g, b, alpha))
    }

    fn decode_color_line(&mut self, line: &FT_ColorLine) -> anyhow::Result<ColorLine> {
        let mut iter = line.color_stop_iterator;
        let mut color_stops = vec![];
        loop {
            let mut stop = MaybeUninit::<FT_ColorStop>::zeroed();

            if unsafe { FT_Get_Colorline_Stops(self.face.face, stop.as_mut_ptr(), &mut iter) } == 0
            {
                break;
            }

            let stop = unsafe { stop.assume_init() };

            color_stops.push(ColorStop {
                offset: stop.stop_offset as f32 / 65535.,
                color: self.decode_color_index(&stop.color)?,
            });
        }

        Ok(ColorLine {
            extend: match line.extend {
                FT_PaintExtend::FT_COLR_PAINT_EXTEND_PAD => Extend::Pad,
                FT_PaintExtend::FT_COLR_PAINT_EXTEND_REPEAT => Extend::Repeat,
                FT_PaintExtend::FT_COLR_PAINT_EXTEND_REFLECT => Extend::Reflect,
            },
            color_stops,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ColorStop {
    pub offset: f32,
    pub color: SrgbaPixel,
}

#[derive(Clone, Debug)]
pub struct ColorLine {
    pub color_stops: Vec<ColorStop>,
    pub extend: Extend,
}

#[derive(Debug, Clone)]
pub enum PaintOp {
    PushTransform(Matrix),
    PopTransform,
    PushClip(Vec<DrawOp>),
    PopClip,
    PaintSolid(SrgbaPixel),
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
    PopGroup(Operator),
}

fn affine2x3_to_matrix(t: FT_Affine23) -> Matrix {
    Matrix::new(
        fixed_to_f64(t.xx),
        fixed_to_f64(t.yx),
        fixed_to_f64(t.xy),
        fixed_to_f64(t.yy),
        fixed_to_f64(t.dy),
        fixed_to_f64(t.dx),
    )
}

fn record_to_cairo_surface(
    paint_ops: Vec<PaintOp>,
    scale_x: f64,
    scale_y: f64,
) -> anyhow::Result<(RecordingSurface, bool)> {
    let mut has_color = false;
    let surface = RecordingSurface::create(Content::ColorAlpha, None)?;
    let context = Context::new(&surface)?;
    context.scale(scale_x, scale_y);
    context.set_antialias(cairo::Antialias::Best);

    for pop in paint_ops {
        match pop {
            PaintOp::PushTransform(matrix) => {
                context.save()?;
                context.transform(matrix);
            }
            PaintOp::PopTransform => {
                context.restore()?;
            }
            PaintOp::PushClip(draw) => {
                context.save()?;
                apply_draw_ops_to_context(&draw, &context)?;
                context.clip();
            }
            PaintOp::PopClip => {
                context.restore()?;
            }
            PaintOp::PushGroup => {
                context.save()?;
                context.push_group();
            }
            PaintOp::PopGroup(operator) => {
                context.pop_group_to_source()?;
                context.set_operator(operator);
                context.paint()?;
                context.restore()?;
            }
            PaintOp::PaintSolid(color) => {
                if color.as_srgba32() != 0xffffffff {
                    has_color = true;
                }
                let (r, g, b, a) = color.as_srgba_tuple();
                context.set_source_rgba(r.into(), g.into(), b.into(), a.into());
                context.paint()?;
            }
            PaintOp::PaintLinearGradient {
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                color_line,
            } => {
                has_color = true;
                paint_linear_gradient(
                    &context,
                    x0.into(),
                    y0.into(),
                    x1.into(),
                    y1.into(),
                    x2.into(),
                    y2.into(),
                    color_line,
                )?;
            }
            PaintOp::PaintRadialGradient {
                x0,
                y0,
                r0,
                x1,
                y1,
                r1,
                color_line,
            } => {
                has_color = true;
                paint_radial_gradient(
                    &context,
                    x0.into(),
                    y0.into(),
                    r0.into(),
                    x1.into(),
                    y1.into(),
                    r1.into(),
                    color_line,
                )?;
            }
            PaintOp::PaintSweepGradient {
                x0,
                y0,
                start_angle,
                end_angle,
                color_line,
            } => {
                has_color = true;
                anyhow::bail!("NOT IMPL: PaintSweepGradient");
            }
        }
    }

    Ok((surface, has_color))
}

fn apply_draw_ops_to_context(ops: &[DrawOp], context: &Context) -> anyhow::Result<()> {
    let mut current = None;
    context.new_path();
    for op in ops {
        match op {
            DrawOp::MoveTo { to_x, to_y } => {
                context.move_to((*to_x).into(), (*to_y).into());
                current.replace((to_x, to_y));
            }
            DrawOp::LineTo { to_x, to_y } => {
                context.line_to((*to_x).into(), (*to_y).into());
                current.replace((to_x, to_y));
            }
            DrawOp::QuadTo {
                control_x,
                control_y,
                to_x,
                to_y,
            } => {
                let (x, y) =
                    current.ok_or_else(|| anyhow::anyhow!("QuadTo has no current position"))?;
                // Express quadratic as a cubic
                // <https://stackoverflow.com/a/55034115/149111>

                context.curve_to(
                    (x + (2. / 3.) * (control_x - x)).into(),
                    (y + (2. / 3.) * (control_y - y)).into(),
                    (to_x + (2. / 3.) * (control_x - to_x)).into(),
                    (to_y + (2. / 3.) * (control_y - to_y)).into(),
                    (*to_x).into(),
                    (*to_y).into(),
                );
                current.replace((to_x, to_y));
            }
            DrawOp::CubicTo {
                control1_x,
                control1_y,
                control2_x,
                control2_y,
                to_x,
                to_y,
            } => {
                context.curve_to(
                    (*control1_x).into(),
                    (*control1_y).into(),
                    (*control2_x).into(),
                    (*control2_y).into(),
                    (*to_x).into(),
                    (*to_y).into(),
                );
                current.replace((to_x, to_y));
            }
            DrawOp::ClosePath => {
                context.close_path();
            }
        }
    }
    Ok(())
}

fn normalize_color_line(color_line: &mut ColorLine) -> (f64, f64) {
    let mut smallest = color_line.color_stops[0].offset;
    let mut largest = smallest;

    for stop in &color_line.color_stops[1..] {
        smallest = smallest.min(stop.offset);
        largest = largest.max(stop.offset);
    }

    if smallest != largest {
        for stop in &mut color_line.color_stops {
            stop.offset = (stop.offset - smallest) / (largest - smallest);
        }
    }

    // NOTE: hb-cairo-utils will call back out to some other state
    // to fill in the color when is_foreground is true, defaulting
    // to black with alpha varying by the alpha channel of the
    // color value in the stop. Do we need to do something like
    // that here?

    (smallest as f64, largest as f64)
}

struct ReduceAnchorsIn {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

struct ReduceAnchorsOut {
    xx0: f64,
    yy0: f64,
    xx1: f64,
    yy1: f64,
}

fn reduce_anchors(
    ReduceAnchorsIn {
        x0,
        y0,
        x1,
        y1,
        x2,
        y2,
    }: ReduceAnchorsIn,
) -> ReduceAnchorsOut {
    let q2x = x2 - x0;
    let q2y = y2 - y0;
    let q1x = x1 - x0;
    let q1y = y1 - y0;

    let s = q2x * q2x + q2y * q2y;
    if s < 0.000001 {
        return ReduceAnchorsOut {
            xx0: x0,
            yy0: y0,
            xx1: x1,
            yy1: y1,
        };
    }

    let k = (q2x * q1x + q2y * q1y) / s;
    ReduceAnchorsOut {
        xx0: x0,
        yy0: y0,
        xx1: x1 - k * q2x,
        yy1: y1 - k * q2y,
    }
}

fn paint_linear_gradient(
    context: &Context,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mut color_line: ColorLine,
) -> anyhow::Result<()> {
    let (min_stop, max_stop) = normalize_color_line(&mut color_line);
    let anchors = reduce_anchors(ReduceAnchorsIn {
        x0,
        y0,
        x1,
        y1,
        x2,
        y2,
    });

    let xxx0 = anchors.xx0 + min_stop * (anchors.xx1 - anchors.xx0);
    let yyy0 = anchors.yy0 + min_stop * (anchors.yy1 - anchors.yy0);
    let xxx1 = anchors.xx0 + max_stop * (anchors.xx1 - anchors.xx0);
    let yyy1 = anchors.yy0 + max_stop * (anchors.yy1 - anchors.yy0);

    let pattern = LinearGradient::new(xxx0, yyy0, xxx1, yyy1);
    pattern.set_extend(color_line.extend);

    for stop in &color_line.color_stops {
        let (r, g, b, a) = stop.color.as_srgba_tuple();
        pattern.add_color_stop_rgba(stop.offset.into(), r.into(), g.into(), b.into(), a.into());
    }

    context.set_source(pattern)?;
    context.paint()?;

    Ok(())
}

fn paint_radial_gradient(
    context: &Context,
    x0: f64,
    y0: f64,
    r0: f64,
    x1: f64,
    y1: f64,
    r1: f64,
    mut color_line: ColorLine,
) -> anyhow::Result<()> {
    let (min_stop, max_stop) = normalize_color_line(&mut color_line);

    let xx0 = x0 + min_stop * (x1 - x0);
    let yy0 = y0 + min_stop * (y1 - y0);
    let xx1 = x0 + max_stop * (x1 - x0);
    let yy1 = y0 + max_stop * (y1 - y0);
    let rr0 = r0 + min_stop * (r1 - r0);
    let rr1 = r0 + max_stop * (r1 - r0);

    let pattern = RadialGradient::new(xx0, yy0, rr0, xx1, yy1, rr1);
    pattern.set_extend(color_line.extend);

    for stop in &color_line.color_stops {
        let (r, g, b, a) = stop.color.as_srgba_tuple();
        pattern.add_color_stop_rgba(stop.offset.into(), r.into(), g.into(), b.into(), a.into());
    }

    context.set_source(pattern)?;
    context.paint()?;

    Ok(())
}
