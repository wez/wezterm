use crate::hbwrap::{
    hb_color, hb_color_get_alpha, hb_color_get_blue, hb_color_get_green, hb_color_get_red,
    hb_color_t, hb_paint_composite_mode_t, hb_tag_to_string, Font, PaintOp, IS_PNG,
};
use crate::rasterizer::colr::{
    apply_draw_ops_to_context, paint_linear_gradient, paint_radial_gradient, paint_sweep_gradient,
};
use crate::rasterizer::FAKE_ITALIC_SKEW;
use crate::units::PixelLength;
use crate::{FontRasterizer, ParsedFont, RasterizedGlyph};
use cairo::{Content, Context, Format, ImageSurface, Matrix, Operator, RecordingSurface};
use image::DynamicImage::{ImageLuma8, ImageLumaA8};
use image::GenericImageView;

pub struct HarfbuzzRasterizer {
    font: Font,
}

impl HarfbuzzRasterizer {
    pub fn from_locator(parsed: &ParsedFont) -> anyhow::Result<Self> {
        let mut font = Font::from_locator(&parsed.handle)?;
        font.set_ot_funcs();

        if parsed.synthesize_italic {
            font.set_synthetic_slant(FAKE_ITALIC_SKEW as f32);
        }
        if parsed.synthesize_bold {
            font.set_synthetic_bold(0.02, 0.02, false);
        }

        Ok(Self { font })
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

        let scale = pixel_size as i32 * 64;
        let ppem = pixel_size;

        self.font.set_ppem(ppem, ppem);
        self.font.set_ptem(size as f32);
        self.font.set_font_scale(scale, scale);

        let white = hb_color(0xff, 0xff, 0xff, 0xff);

        let palette_index = 0;
        let ops = self
            .font
            .get_paint_ops_for_glyph(glyph_pos, palette_index, white)?;

        log::trace!("ops: {ops:#?}");

        let (surface, has_color) = record_to_cairo_surface(ops)?;
        let (left, top, width, height) = surface.ink_extents();
        log::trace!("extents: left={left} top={top} width={width} height={height}");

        if width as usize == 0 || height as usize == 0 {
            return Ok(RasterizedGlyph {
                data: vec![],
                height: 0,
                width: 0,
                bearing_x: PixelLength::new(0.),
                bearing_y: PixelLength::new(0.),
                has_color: false,
                is_scaled: true,
            });
        }

        let mut bounds_adjust = Matrix::identity();
        bounds_adjust.translate(left * -1., top * -1.);
        log::trace!("dims: {width}x{height} {bounds_adjust:?}");

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
            is_scaled: true,
        })
    }
}

fn record_to_cairo_surface(paint_ops: Vec<PaintOp>) -> anyhow::Result<(RecordingSurface, bool)> {
    let mut has_color = false;
    let surface = RecordingSurface::create(Content::ColorAlpha, None)?;
    let context = Context::new(&surface)?;
    context.scale(1. / 64., -1. / 64.);
    context.set_antialias(cairo::Antialias::Best);

    for pop in paint_ops {
        match pop {
            PaintOp::PushTransform {
                xx,
                yx,
                xy,
                yy,
                dx,
                dy,
            } => {
                context.save()?;
                context.transform(Matrix::new(
                    xx.into(),
                    yx.into(),
                    xy.into(),
                    yy.into(),
                    dx.into(),
                    dy.into(),
                ));
            }
            PaintOp::PopTransform => {
                context.restore()?;
            }
            PaintOp::PushGlyphClip { glyph: _, draw } => {
                context.save()?;
                apply_draw_ops_to_context(&draw, &context)?;
                context.clip();
            }
            PaintOp::PushRectClip {
                xmin,
                ymin,
                ymax,
                xmax,
            } => {
                context.save()?;
                context.rectangle(
                    xmin.into(),
                    ymin.into(),
                    (xmax - xmin).into(),
                    (ymax - ymin).into(),
                );
                context.clip();
            }
            PaintOp::PopClip => {
                context.restore()?;
            }
            PaintOp::PushGroup => {
                context.save()?;
                context.push_group();
            }
            PaintOp::PopGroup { mode } => {
                context.pop_group_to_source()?;
                context.set_operator(hb_paint_mode_to_operator(mode));
                context.paint()?;
                context.restore()?;
            }
            PaintOp::PaintSolid {
                is_foreground: _,
                color,
            } => {
                if color != 0xffffffff {
                    has_color = true;
                }
                let (r, g, b, a) = hb_color_to_rgba(color);
                context.set_source_rgba(r, g, b, a);
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
                paint_sweep_gradient(
                    &context,
                    x0.into(),
                    y0.into(),
                    start_angle.into(),
                    end_angle.into(),
                    color_line,
                )?;
            }
            PaintOp::PaintImage {
                image,
                width: _,
                height: _,
                format,
                slant,
                extents,
            } => {
                let image_surface = if format == IS_PNG {
                    let decoded = image::ImageReader::new(std::io::Cursor::new(image.as_slice()))
                        .with_guessed_format()?
                        .decode()?;

                    if !matches!(&decoded, ImageLuma8(_) | ImageLumaA8(_)) {
                        // Not a monochrome image
                        has_color = true;
                    }

                    let (width, height) = decoded.dimensions();
                    let mut data = decoded.into_rgba8().into_vec();

                    // Cairo wants ARGB. Walk through the pixels and
                    // premultiply and get into that form
                    rgba_to_argb_and_multiply(&mut data);
                    // premultiply(&mut data);

                    let width = width as i32;
                    let height = height as i32;
                    ImageSurface::create_for_data(data, Format::ARgb32, width, height, width * 4)?
                } else {
                    anyhow::bail!("NOT IMPL: PaintImage {}", hb_tag_to_string(format));
                };

                // Use the decoded dimensions; not all fonts encode
                // the dimensions correctly in the font data
                let width = image_surface.width();
                let height = image_surface.height();

                let extents = extents.ok_or_else(|| {
                    anyhow::anyhow!("expected to have extents for non-svg image data")
                })?;

                context.save()?;
                // Ensure that we clip to the image rectangle
                context.rectangle(
                    extents.x_bearing.into(),
                    extents.y_bearing.into(),
                    extents.width.into(),
                    extents.height.into(),
                );
                context.clip();

                let pattern = cairo::SurfacePattern::create(image_surface);
                pattern.set_extend(cairo::Extend::Pad);
                pattern.set_matrix(Matrix::new(width.into(), 0., 0., height.into(), 0., 0.));

                let slanted_width = extents.width as f64 - extents.height as f64 * slant as f64;
                let slanted_x_bearing =
                    extents.x_bearing as f64 - extents.y_bearing as f64 * slant as f64;
                context.transform(Matrix::new(1., 0., slant.into(), 1., 0., 0.));
                context.translate(slanted_x_bearing.into(), extents.y_bearing.into());
                context.scale(slanted_width.into(), extents.height.into());
                context.set_source(pattern)?;
                context.paint()?;
                context.restore()?;
            }
        }
    }

    Ok((surface, has_color))
}

fn multiply_alpha(alpha: u8, color: u8) -> u8 {
    let temp: u32 = alpha as u32 * (color as u32 + 0x80);

    ((temp + (temp >> 8)) >> 8) as u8
}

#[allow(dead_code)]
fn demultiply_alpha(alpha: u8, color: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }
    let v = ((color as u32) * 255) / alpha as u32;
    if v > 255 {
        255
    } else {
        v as u8
    }
}

#[allow(dead_code)]
fn premultiply(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        pixel[0] = multiply_alpha(a, r);
        pixel[1] = multiply_alpha(a, g);
        pixel[2] = multiply_alpha(a, b);
        pixel[3] = a;
    }
}

fn rgba_to_argb_and_multiply(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let [mut r, mut g, mut b, a] = *pixel else {
            unreachable!()
        };

        if a != 0xff {
            r = multiply_alpha(a, r);
            g = multiply_alpha(a, g);
            b = multiply_alpha(a, b);
        }

        #[cfg(target_endian = "big")]
        let result = [a, r, g, b];
        #[cfg(target_endian = "little")]
        let result = [b, g, r, a];

        pixel.copy_from_slice(&result);
    }
}

pub fn argb_to_rgba(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        #[cfg(target_endian = "little")]
        let [b, g, r, a] = *pixel
        else {
            unreachable!()
        };
        #[cfg(target_endian = "big")]
        let [a, r, g, b] = *pixel
        else {
            unreachable!()
        };
        pixel.copy_from_slice(&[r, g, b, a]);
    }
}

fn hb_paint_mode_to_operator(mode: hb_paint_composite_mode_t) -> Operator {
    use hb_paint_composite_mode_t::*;
    match mode {
        HB_PAINT_COMPOSITE_MODE_CLEAR => Operator::Clear,
        HB_PAINT_COMPOSITE_MODE_SRC => Operator::Source,
        HB_PAINT_COMPOSITE_MODE_DEST => Operator::Dest,
        HB_PAINT_COMPOSITE_MODE_SRC_OVER => Operator::Over,
        HB_PAINT_COMPOSITE_MODE_DEST_OVER => Operator::DestOver,
        HB_PAINT_COMPOSITE_MODE_SRC_IN => Operator::In,
        HB_PAINT_COMPOSITE_MODE_DEST_IN => Operator::DestIn,
        HB_PAINT_COMPOSITE_MODE_SRC_OUT => Operator::Out,
        HB_PAINT_COMPOSITE_MODE_DEST_OUT => Operator::DestOut,
        HB_PAINT_COMPOSITE_MODE_SRC_ATOP => Operator::Atop,
        HB_PAINT_COMPOSITE_MODE_DEST_ATOP => Operator::DestAtop,
        HB_PAINT_COMPOSITE_MODE_XOR => Operator::Xor,
        HB_PAINT_COMPOSITE_MODE_PLUS => Operator::Add,
        HB_PAINT_COMPOSITE_MODE_SCREEN => Operator::Screen,
        HB_PAINT_COMPOSITE_MODE_OVERLAY => Operator::Overlay,
        HB_PAINT_COMPOSITE_MODE_DARKEN => Operator::Darken,
        HB_PAINT_COMPOSITE_MODE_LIGHTEN => Operator::Lighten,
        HB_PAINT_COMPOSITE_MODE_COLOR_DODGE => Operator::ColorDodge,
        HB_PAINT_COMPOSITE_MODE_COLOR_BURN => Operator::ColorBurn,
        HB_PAINT_COMPOSITE_MODE_HARD_LIGHT => Operator::HardLight,
        HB_PAINT_COMPOSITE_MODE_SOFT_LIGHT => Operator::SoftLight,
        HB_PAINT_COMPOSITE_MODE_DIFFERENCE => Operator::Difference,
        HB_PAINT_COMPOSITE_MODE_EXCLUSION => Operator::Exclusion,
        HB_PAINT_COMPOSITE_MODE_MULTIPLY => Operator::Multiply,
        HB_PAINT_COMPOSITE_MODE_HSL_HUE => Operator::HslHue,
        HB_PAINT_COMPOSITE_MODE_HSL_SATURATION => Operator::HslSaturation,
        HB_PAINT_COMPOSITE_MODE_HSL_COLOR => Operator::HslColor,
        HB_PAINT_COMPOSITE_MODE_HSL_LUMINOSITY => Operator::HslLuminosity,
    }
}

fn hb_color_to_rgba(color: hb_color_t) -> (f64, f64, f64, f64) {
    let red = unsafe { hb_color_get_red(color) } as f64;
    let green = unsafe { hb_color_get_green(color) } as f64;
    let blue = unsafe { hb_color_get_blue(color) } as f64;
    let alpha = unsafe { hb_color_get_alpha(color) } as f64;
    (red / 255., green / 255., blue / 255., alpha / 255.)
}
