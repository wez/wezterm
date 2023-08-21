use cairo::{Context, Extend, LinearGradient, Matrix, Operator, RadialGradient};
use wezterm_color_types::SrgbaPixel;

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

#[derive(Debug, Clone)]
pub enum DrawOp {
    MoveTo {
        to_x: f32,
        to_y: f32,
    },
    LineTo {
        to_x: f32,
        to_y: f32,
    },
    QuadTo {
        control_x: f32,
        control_y: f32,
        to_x: f32,
        to_y: f32,
    },
    CubicTo {
        control1_x: f32,
        control1_y: f32,
        control2_x: f32,
        control2_y: f32,
        to_x: f32,
        to_y: f32,
    },
    ClosePath,
}

pub fn paint_linear_gradient(
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

pub fn paint_radial_gradient(
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

pub fn paint_sweep_gradient(
    context: &Context,
    x0: f64,
    y0: f64,
    start_angle: f64,
    end_angle: f64,
    mut color_line: ColorLine,
) -> anyhow::Result<()> {
    let (min_stop, max_stop) = normalize_color_line(&mut color_line);
    anyhow::bail!("NOT IMPL: SweepGradient");
}

fn normalize_color_line(color_line: &mut ColorLine) -> (f64, f64) {
    let mut smallest = color_line.color_stops[0].offset;
    let mut largest = smallest;

    color_line
        .color_stops
        .sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());

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

pub fn apply_draw_ops_to_context(ops: &[DrawOp], context: &Context) -> anyhow::Result<()> {
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
