use crate::hbwrap::{
    hb_color, hb_color_get_alpha, hb_color_get_blue, hb_color_get_green, hb_color_get_red,
    hb_color_t, hb_glyph_extents_t, hb_paint_composite_mode_t, hb_paint_extend_t, hb_tag_t,
    hb_tag_to_string, ColorLine, DrawOp, Face, Font, PaintOp, IS_PNG,
};
use crate::rasterizer::FAKE_ITALIC_SKEW;
use crate::units::PixelLength;
use crate::{FontRasterizer, ParsedFont, RasterizedGlyph};
use anyhow::Context;
use image::DynamicImage::{ImageLuma8, ImageLumaA8};
use once_cell::sync::Lazy;
use resvg::tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, LinearGradient, Paint, Path,
    PathBuilder, Pixmap, PixmapPaint, PixmapRef, Point, RadialGradient, Rect, Shader, SpreadMode,
    Transform,
};

static ONE_64TH: Lazy<Transform> = Lazy::new(|| Transform::from_scale(1. / 64., -1. / 64.));

pub struct HarfbuzzRasterizer {
    face: Face,
    font: Font,
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

        Ok(Self { face, font })
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

        self.font.set_ppem(ppem, ppem);
        self.font.set_ptem(size as f32);
        self.font.set_font_scale(scale, scale);

        let white = hb_color(0xff, 0xff, 0xff, 0xff);

        let palette_index = 0;
        let ops = self
            .font
            .get_paint_ops_for_glyph(glyph_pos, palette_index, white)?;

        log::trace!("ops: {ops:#?}");

        let ops = GlyphOp::new(ops)?;

        let bounds = GlyphOp::compute_bounds(&ops)?;
        let width = (bounds.right() + (bounds.left().min(0.) * -1.0)).ceil();
        let height = (bounds.bottom() + (bounds.top().min(0.) * -1.0)).ceil();

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

        let bounds_adjust =
            Transform::from_translate(bounds.left().min(0.) * -1., bounds.top().min(0.) * -1.);

        log::info!("Overall bounds: {bounds:?} -> {width}x{height}");
        let pixmap = Pixmap::new(width as u32, height as u32)
            .ok_or_else(|| anyhow::anyhow!("invalid pixmap dimensions"))?;

        let mut pixmap_stack = vec![pixmap];
        let mut transforms = TransformStack::new();
        let mut clip_stack = vec![];
        let mut has_color = false;

        for op in ops {
            log::info!(
                "pixmap_stack.len={} clip_stack.len={}",
                pixmap_stack.len(),
                clip_stack.len()
            );
            match dbg!(op) {
                GlyphOp::PushTransform(t) => {
                    transforms.push(t);
                }
                GlyphOp::PopTransform => {
                    transforms.pop();
                }
                GlyphOp::PushClip(path) => {
                    let path = path
                        .clone()
                        .transform(transforms.current())
                        .ok_or_else(|| {
                            anyhow::anyhow!("PushClip: transform produced invalid path")
                        })?;
                    clip_stack.push(path);
                }
                GlyphOp::PopClip => {
                    clip_stack.pop();
                }
                GlyphOp::Paint(shader) => {
                    let path = clip_stack
                        .last()
                        .ok_or_else(|| anyhow::anyhow!("Paint: no current clip"))?;
                    let pixmap = pixmap_stack
                        .last_mut()
                        .ok_or_else(|| anyhow::anyhow!("Paint: no current pixmap"))?;

                    if !has_color {
                        has_color = shader.has_color();
                    }

                    pixmap.fill_path(
                        path,
                        &Paint {
                            shader: shader.to_tiny_skia(transforms.current())?,
                            blend_mode: BlendMode::SourceOver,
                            anti_alias: true,
                            force_hq_pipeline: true,
                        },
                        FillRule::default(),
                        bounds_adjust,
                        None,
                    );
                }
                GlyphOp::PaintImage {
                    image,
                    width: _,
                    height: _,
                    format,
                    slant: _, // FIXME: rotate/skew image?
                    extents: _,
                } => {
                    if format == IS_PNG {
                        let decoded =
                            image::io::Reader::new(std::io::Cursor::new(image.as_slice()))
                                .with_guessed_format()?
                                .decode()?;

                        match &decoded {
                            ImageLuma8(_) | ImageLumaA8(_) => {
                                // Not color, but don't replace a potential
                                // existing has_color=true
                            }
                            _ => {
                                has_color = true;
                            }
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
                        let cropped =
                            crate::rasterizer::crop_to_non_transparent(&mut decoded).to_image();

                        log::info!("cropped to {:?}", cropped.dimensions());

                        let pixmap = PixmapRef::from_bytes(
                            cropped.as_raw(),
                            cropped.width(),
                            cropped.height(),
                        )
                        .ok_or_else(|| anyhow::anyhow!("making PixmapRef from cropped failed"))?;

                        let target = pixmap_stack
                            .last_mut()
                            .ok_or_else(|| anyhow::anyhow!("Paint: no current pixmap"))?;

                        let scale = Transform::from_scale(
                            width as f32 / cropped.width() as f32,
                            height as f32 / cropped.height() as f32,
                        );

                        target.draw_pixmap(
                            0,
                            0,
                            pixmap,
                            &PixmapPaint {
                                opacity: 1.,
                                blend_mode: BlendMode::SourceOver,
                                quality: FilterQuality::Nearest,
                            },
                            scale,
                            None,
                        );
                    } else {
                        anyhow::bail!(
                            "PaintImage format {format:?} {} not implemented",
                            hb_tag_to_string(format)
                        );
                    }
                }
                GlyphOp::PushGroup => {
                    let pixmap = Pixmap::new(width as u32, height as u32)
                        .ok_or_else(|| anyhow::anyhow!("invalid pixmap dimensions"))?;
                    pixmap_stack.push(pixmap);
                }
                GlyphOp::PopGroup(blend_mode) => {
                    let pixmap = pixmap_stack
                        .pop()
                        .ok_or_else(|| anyhow::anyhow!("no more groups to pop"))?;
                    let target = pixmap_stack
                        .last_mut()
                        .ok_or_else(|| anyhow::anyhow!("Paint: no current pixmap"))?;

                    target.draw_pixmap(
                        0,
                        0,
                        pixmap.as_ref(),
                        &PixmapPaint {
                            opacity: 1.,
                            blend_mode,
                            quality: FilterQuality::Nearest,
                        },
                        bounds_adjust,
                        None,
                    );
                }
            }
        }

        anyhow::ensure!(
            pixmap_stack.len() == 1,
            "expected 1 pixmap to remain in stack, but had {}",
            pixmap_stack.len()
        );

        Ok(RasterizedGlyph {
            data: pixmap_stack.pop().expect("bounds check above").take(),
            height: height as usize,
            width: width as usize,
            bearing_x: PixelLength::new(bounds.left().min(0.) as f64),
            bearing_y: PixelLength::new(bounds.top() as f64 * -1.),
            has_color,
        })
    }
}

struct TransformStack {
    stack: Vec<Transform>,
    current: Transform,
}

impl TransformStack {
    fn new() -> Self {
        Self {
            stack: vec![],
            current: ONE_64TH.clone(),
        }
    }

    pub fn push(&mut self, t: Transform) {
        self.stack.push(t);
        self.current = self.current.post_concat(t);
    }

    pub fn pop(&mut self) {
        if self.stack.pop().is_some() {
            self.current = ONE_64TH.clone();
            for &t in &self.stack {
                self.current = self.current.post_concat(t);
            }
        }
    }

    pub fn current(&self) -> Transform {
        self.current
    }
}

#[derive(Debug)]
enum GlyphOp {
    PushTransform(Transform),
    PopTransform,
    PushClip(Path),
    PopClip,
    Paint(SimpleShader),
    PaintImage {
        image: crate::hbwrap::Blob,
        width: u32,
        height: u32,
        format: hb_tag_t,
        slant: f32,
        extents: Option<hb_glyph_extents_t>,
    },
    PushGroup,
    PopGroup(BlendMode),
}

impl GlyphOp {
    pub fn new(ops: Vec<PaintOp>) -> anyhow::Result<Vec<Self>> {
        let mut result = vec![];
        for op in ops {
            result.push(match op {
                PaintOp::PushTransform {
                    xx,
                    yx,
                    xy,
                    yy,
                    dx,
                    dy,
                } => Self::PushTransform(Transform {
                    sx: xx,
                    sy: xy,
                    kx: yx,
                    ky: yy,
                    tx: dx,
                    ty: dy,
                }),
                PaintOp::PopTransform => Self::PopTransform,
                PaintOp::PushGlyphClip { glyph: _, draw } => {
                    let path = build_path(draw).context("PushGlyphClip: build_path")?;
                    Self::PushClip(path)
                }
                PaintOp::PushRectClip {
                    xmin,
                    ymin,
                    ymax,
                    xmax,
                } => {
                    let rect = Rect::from_ltrb(xmin, ymin, xmax, ymax).ok_or_else(|| {
                        anyhow::anyhow!(
                            "PushRectClip: Rect::from_ltrb failed for {xmin},{ymin},{xmax},{ymax}"
                        )
                    })?;
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    let path = pb.finish().ok_or_else(|| {
                        anyhow::anyhow!("PushRectClip: pathbuilder finish failed")
                    })?;
                    Self::PushClip(path)
                }
                PaintOp::PopClip => Self::PopClip,
                PaintOp::PaintSolid {
                    is_foreground: _,
                    color,
                } => {
                    let color = hb_color_to_color(color);
                    Self::Paint(SimpleShader::Color(color))
                }
                PaintOp::PaintImage {
                    image,
                    width,
                    height,
                    format,
                    slant,
                    extents,
                } => Self::PaintImage {
                    image,
                    width,
                    height,
                    format,
                    slant,
                    extents,
                },
                PaintOp::PaintLinearGradient {
                    x0,
                    y0,
                    x1,
                    y1,
                    x2,
                    y2,
                    color_line,
                } => {
                    let fallback_color = hb_color_to_color(color_line.color_stops[0].color);
                    Self::Paint(SimpleShader::LinearGradient {
                        p0: Point::from_xy(x0, y0),
                        p1: Point::from_xy(x1, y1),
                        p2: Point::from_xy(x2, y2),
                        stops: color_line_to_stops(&color_line),
                        spread: hb_extend_to_spread_mode(color_line.extend),
                        fallback_color,
                    })
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
                    let fallback_color = hb_color_to_color(color_line.color_stops[0].color);
                    Self::Paint(SimpleShader::RadialGradient {
                        p0: Point::from_xy(x0, y0),
                        p1: Point::from_xy(x1, y1),
                        r0,
                        r1,
                        stops: color_line_to_stops(&color_line),
                        spread: hb_extend_to_spread_mode(color_line.extend),
                        fallback_color,
                    })
                }
                PaintOp::PaintSweepGradient {
                    x0,
                    y0,
                    start_angle,
                    end_angle,
                    color_line,
                } => {
                    let fallback_color = hb_color_to_color(color_line.color_stops[0].color);
                    Self::Paint(SimpleShader::SweepGradient {
                        p: Point::from_xy(x0, y0),
                        start_angle,
                        end_angle,
                        stops: color_line_to_stops(&color_line),
                        spread: hb_extend_to_spread_mode(color_line.extend),
                        fallback_color,
                    })
                }
                PaintOp::PushGroup => Self::PushGroup,
                PaintOp::PopGroup { mode } => {
                    use hb_paint_composite_mode_t::*;
                    let mode = match mode {
                        HB_PAINT_COMPOSITE_MODE_CLEAR => BlendMode::Clear,
                        HB_PAINT_COMPOSITE_MODE_SRC => BlendMode::Source,
                        HB_PAINT_COMPOSITE_MODE_DEST => BlendMode::Destination,
                        HB_PAINT_COMPOSITE_MODE_SRC_OVER => BlendMode::SourceOver,
                        HB_PAINT_COMPOSITE_MODE_DEST_OVER => BlendMode::DestinationOver,
                        HB_PAINT_COMPOSITE_MODE_SRC_IN => BlendMode::SourceIn,
                        HB_PAINT_COMPOSITE_MODE_DEST_IN => BlendMode::DestinationIn,
                        HB_PAINT_COMPOSITE_MODE_SRC_OUT => BlendMode::SourceOut,
                        HB_PAINT_COMPOSITE_MODE_DEST_OUT => BlendMode::DestinationOut,
                        HB_PAINT_COMPOSITE_MODE_SRC_ATOP => BlendMode::SourceAtop,
                        HB_PAINT_COMPOSITE_MODE_DEST_ATOP => BlendMode::DestinationAtop,
                        HB_PAINT_COMPOSITE_MODE_XOR => BlendMode::Xor,
                        HB_PAINT_COMPOSITE_MODE_PLUS => BlendMode::Plus,
                        HB_PAINT_COMPOSITE_MODE_SCREEN => BlendMode::Screen,
                        HB_PAINT_COMPOSITE_MODE_OVERLAY => BlendMode::Overlay,
                        HB_PAINT_COMPOSITE_MODE_DARKEN => BlendMode::Darken,
                        HB_PAINT_COMPOSITE_MODE_LIGHTEN => BlendMode::Lighten,
                        HB_PAINT_COMPOSITE_MODE_COLOR_DODGE => BlendMode::ColorDodge,
                        HB_PAINT_COMPOSITE_MODE_COLOR_BURN => BlendMode::ColorBurn,
                        HB_PAINT_COMPOSITE_MODE_HARD_LIGHT => BlendMode::HardLight,
                        HB_PAINT_COMPOSITE_MODE_SOFT_LIGHT => BlendMode::SoftLight,
                        HB_PAINT_COMPOSITE_MODE_DIFFERENCE => BlendMode::Difference,
                        HB_PAINT_COMPOSITE_MODE_EXCLUSION => BlendMode::Exclusion,
                        HB_PAINT_COMPOSITE_MODE_MULTIPLY => BlendMode::Multiply,
                        HB_PAINT_COMPOSITE_MODE_HSL_HUE => BlendMode::Hue,
                        HB_PAINT_COMPOSITE_MODE_HSL_SATURATION => BlendMode::Saturation,
                        HB_PAINT_COMPOSITE_MODE_HSL_COLOR => BlendMode::Color,
                        HB_PAINT_COMPOSITE_MODE_HSL_LUMINOSITY => BlendMode::Luminosity,
                    };
                    Self::PopGroup(mode)
                }
            });
        }

        Ok(result)
    }

    pub fn compute_bounds(ops: &[Self]) -> anyhow::Result<Rect> {
        let mut transforms = TransformStack::new();
        let mut pb = PathBuilder::new();

        for op in ops {
            match op {
                Self::PushTransform(t) => {
                    transforms.push(*t);
                }
                Self::PopTransform => {
                    transforms.pop();
                }
                Self::PushClip(path) => {
                    let path = path
                        .clone()
                        .transform(transforms.current())
                        .ok_or_else(|| {
                            anyhow::anyhow!("PushClip: transform produced invalid path")
                        })?;
                    pb.push_path(&path);
                }
                Self::PopClip => {}
                Self::Paint(_) => {}
                Self::PaintImage {
                    width,
                    height,
                    slant,
                    extents,
                    ..
                } => {
                    let rect = match extents {
                        Some(e) => {
                            let left = e.x_bearing.min(e.width) as f32;
                            let top = e.y_bearing.min(e.height) as f32;
                            let right = e.x_bearing.max(e.width) as f32;
                            let bottom = e.y_bearing.max(e.height) as f32;

                            Rect::from_ltrb(left, top, right, bottom)
                                .ok_or_else(|| anyhow::anyhow!("Rect::from_ltrb {left},{top},{right},{bottom} from {e:?} failed"))?
                        }
                        None => Rect::from_ltrb(0., 0., *width as f32 * 64., *height as f32 * 64.)
                            .ok_or_else(|| {
                                anyhow::anyhow!("Rect::from_ltrb 0,0,{width},{height} failed")
                            })?,
                    };

                    let rect = PathBuilder::from_rect(rect);

                    let path = rect
                        .transform(Transform::from_rotate(*slant).post_concat(transforms.current()))
                        .ok_or_else(|| anyhow::anyhow!("path transform failed"))?;

                    pb.push_path(&path);
                }
                Self::PushGroup => {}
                Self::PopGroup(_) => {}
            }
        }

        let path = pb
            .finish()
            .ok_or_else(|| anyhow::anyhow!("path builder failed"))?;
        Ok(path.bounds())
    }
}

fn build_path(ops: Vec<DrawOp>) -> anyhow::Result<Path> {
    if ops.is_empty() {
        return Ok(PathBuilder::from_rect(
            Rect::from_ltrb(0., 0., 0., 0.).expect("zero size rect to be ok"),
        ));
    }

    let mut pb = PathBuilder::new();
    let mut current = None;

    for op in ops {
        match op {
            DrawOp::MoveTo { to_x, to_y } => {
                pb.move_to(to_x, to_y);
                current.replace((to_x, to_y));
            }
            DrawOp::LineTo { to_x, to_y } => {
                pb.line_to(to_x, to_y);
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

                pb.cubic_to(
                    x + (2. / 3.) * (control_x - x),
                    y + (2. / 3.) * (control_y - y),
                    to_x + (2. / 3.) * (control_x - to_x),
                    to_y + (2. / 3.) * (control_y - to_y),
                    to_x,
                    to_y,
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
                pb.cubic_to(control1_x, control1_y, control2_x, control2_y, to_x, to_y);
                current.replace((to_x, to_y));
            }
            DrawOp::ClosePath => {
                pb.close();
            }
        }
    }

    let path = pb
        .finish()
        .ok_or_else(|| anyhow::anyhow!("pathbuilder finish failed"))?;

    Ok(path)
}

#[derive(Clone, Debug)]
enum SimpleShader {
    Color(Color),
    LinearGradient {
        p0: Point,
        #[allow(dead_code)]
        p1: Point,
        p2: Point,
        stops: Vec<GradientStop>,
        spread: SpreadMode,
        fallback_color: Color,
    },
    RadialGradient {
        p0: Point,
        p1: Point,
        #[allow(dead_code)]
        r0: f32,
        r1: f32,
        stops: Vec<GradientStop>,
        spread: SpreadMode,
        fallback_color: Color,
    },
    SweepGradient {
        #[allow(dead_code)]
        p: Point,
        #[allow(dead_code)]
        start_angle: f32,
        #[allow(dead_code)]
        end_angle: f32,
        #[allow(dead_code)]
        stops: Vec<GradientStop>,
        #[allow(dead_code)]
        spread: SpreadMode,
        // Remove me once/if real sweep gradient support
        // is added to tiny_skia
        fallback_color: Color,
    },
}

impl SimpleShader {
    fn has_color(&self) -> bool {
        match self {
            Self::Color(c) => *c != Color::WHITE,
            _ => true,
        }
    }

    fn to_tiny_skia(self, transform: Transform) -> anyhow::Result<Shader<'static>> {
        log::warn!(
            "build shader from {self:?} and {transform:?}. is invertible? {}",
            transform.invert().is_some()
        );
        match self {
            Self::Color(c) => Ok(Shader::SolidColor(c)),
            Self::LinearGradient {
                p0,
                p1: _,
                p2,
                stops,
                spread,
                fallback_color,
            } => {
                if transform.invert().is_none() {
                    log::warn!("cannot build LinearGradient with non-invertible transform, using solid color");
                    return Ok(Shader::SolidColor(fallback_color));
                }

                // Note: tiny_skia doesn't have a way for us to set
                // the midpoint in the gradient, so we're ignoring
                // that coordinate pair
                LinearGradient::new(p0, p2, stops, spread, transform)
                    .ok_or_else(|| anyhow::anyhow!("failed to build LinearGradient"))
            }
            Self::RadialGradient {
                p0,
                p1,
                r0: _,
                r1,
                stops,
                spread,
                fallback_color,
            } => {
                if transform.invert().is_none() {
                    log::warn!("cannot build RadialGradient with non-invertible transform, using solid color");
                    return Ok(Shader::SolidColor(fallback_color));
                }
                // Note: tiny_skia doesn't have a way for us to set
                // the inner radius, so we ignore it
                RadialGradient::new(p0, p1, r1, stops, spread, transform)
                    .ok_or_else(|| anyhow::anyhow!("failed to build RadialGradient"))
            }
            Self::SweepGradient {
                p: _,
                start_angle: _,
                end_angle: _,
                stops: _,
                spread: _,
                fallback_color,
            } => {
                // Note: tiny_skia doesn't support sweep gradients, so we
                // just use a solid color
                Ok(Shader::SolidColor(fallback_color))
            }
        }
    }
}

fn color_line_to_stops(line: &ColorLine) -> Vec<GradientStop> {
    line.color_stops
        .iter()
        .map(|stop| GradientStop::new(stop.offset, hb_color_to_color(stop.color)))
        .collect()
}

fn hb_color_to_color(color: hb_color_t) -> Color {
    let red = unsafe { hb_color_get_red(color) };
    let green = unsafe { hb_color_get_green(color) };
    let blue = unsafe { hb_color_get_blue(color) };
    let alpha = unsafe { hb_color_get_alpha(color) };

    Color::from_rgba8(red, green, blue, alpha)
}

fn hb_extend_to_spread_mode(extend: hb_paint_extend_t) -> SpreadMode {
    match extend {
        hb_paint_extend_t::HB_PAINT_EXTEND_PAD => SpreadMode::Pad,
        hb_paint_extend_t::HB_PAINT_EXTEND_REPEAT => SpreadMode::Repeat,
        hb_paint_extend_t::HB_PAINT_EXTEND_REFLECT => SpreadMode::Reflect,
    }
}
