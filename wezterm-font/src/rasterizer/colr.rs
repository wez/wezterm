use cairo::{Context, Extend, LinearGradient, Matrix, Mesh, MeshCorner, Operator, RadialGradient};
use wezterm_color_types::{SrgbaPixel, SrgbaTuple};

/* The gradient related routines in this file were ported from HarfBuzz, which
 * were in turn ported from BlackRenderer by Black Foundry.
 * Used by permission to relicense to HarfBuzz license,
 * which is in turn compatible with wezterm's license.
 *
 * https://github.com/BlackFoundryCom/black-renderer
 */

#[derive(Clone, Debug)]
pub struct ColorStop {
    pub offset: f64,
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

#[derive(Copy, Clone, Debug)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn dot(&self, other: Self) -> f64 {
        (self.x * other.x) + (self.y * other.y)
    }

    fn normalize(self) -> Self {
        let len = self.dot(self).sqrt();
        Self {
            x: self.x / len,
            y: self.y / len,
        }
    }

    pub fn sum(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    pub fn difference(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    pub fn scale(self, factor: f64) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    /// Compute a vector from the supplied angle
    pub fn from_angle(angle: f64) -> Self {
        let (y, x) = angle.sin_cos();
        Self { x, y }
    }
}

fn interpolate(f0: f64, f1: f64, f: f64) -> f64 {
    f0 + f * (f1 - f0)
}

#[derive(Debug)]
struct Patch {
    p0: Point,
    c0: Point,
    c1: Point,
    p1: Point,
    color0: SrgbaTuple,
    color1: SrgbaTuple,
}

impl Patch {
    fn add_to_mesh(&self, center: Point, mesh: &Mesh) {
        mesh.begin_patch();
        mesh.move_to(center.x, center.y);
        mesh.line_to(self.p0.x, self.p0.y);
        mesh.curve_to(
            self.c0.x, self.c0.y, self.c1.x, self.c1.y, self.p1.x, self.p1.y,
        );
        mesh.line_to(center.x, center.y);

        fn set_corner_color(mesh: &Mesh, corner: MeshCorner, color: SrgbaTuple) {
            let SrgbaTuple(r, g, b, a) = color;

            mesh.set_corner_color_rgba(corner, r.into(), g.into(), b.into(), a.into());
        }

        set_corner_color(mesh, MeshCorner::MeshCorner0, self.color0);
        set_corner_color(mesh, MeshCorner::MeshCorner1, self.color0);
        set_corner_color(mesh, MeshCorner::MeshCorner2, self.color1);
        set_corner_color(mesh, MeshCorner::MeshCorner3, self.color1);

        mesh.end_patch();
    }
}

fn add_sweep_gradient_patches(
    mesh: &Mesh,
    center: Point,
    radius: f64,
    a0: f64,
    c0: SrgbaTuple,
    a1: f64,
    c1: SrgbaTuple,
) {
    const MAX_ANGLE: f64 = std::f64::consts::PI / 8.;
    let num_splits = ((a1 - a0).abs() / MAX_ANGLE).ceil() as usize;

    let mut p0 = Point::from_angle(a0);
    let mut color0 = c0;

    for idx in 0..num_splits {
        let k = (idx as f64 + 1.) / num_splits as f64;

        let angle1 = interpolate(a0, a1, k);
        let color1 = c0.interpolate(c1, k);

        let p1 = Point::from_angle(angle1);

        let a = p0.sum(p1).normalize();
        let u = Point { x: -a.y, y: a.x };

        fn compute_control(a: Point, u: Point, p: Point, center: Point, radius: f64) -> Point {
            let c = a.sum(u.scale(p.difference(a).dot(p) / u.dot(p)));
            c.difference(p)
                .scale(0.33333)
                .sum(c)
                .scale(radius)
                .sum(center)
        }

        let patch = Patch {
            color0,
            color1,
            p0: center.sum(p0.scale(radius)),
            p1: center.sum(p1.scale(radius)),
            c0: compute_control(a, u, p0, center, radius),
            c1: compute_control(a, u, p1, center, radius),
        };

        patch.add_to_mesh(center, mesh);

        p0 = p1;
        color0 = color1;
    }
}

const PI_TIMES_2: f64 = std::f64::consts::PI * 2.;

fn apply_sweep_gradient_patches(
    mesh: &Mesh,
    mut color_line: ColorLine,
    center: Point,
    radius: f64,
    mut start_angle: f64,
    mut end_angle: f64,
) {
    if start_angle == end_angle {
        if color_line.extend == Extend::Pad {
            if start_angle > 0. {
                let c = color_line.color_stops[0].color.into();
                add_sweep_gradient_patches(mesh, center, radius, 0., c, start_angle, c);
            }
            if end_angle < PI_TIMES_2 {
                let c = color_line.color_stops.last().unwrap().color.into();
                add_sweep_gradient_patches(mesh, center, radius, end_angle, c, PI_TIMES_2, c);
            }
        }
        return;
    }

    if end_angle < start_angle {
        std::mem::swap(&mut start_angle, &mut end_angle);
        color_line.color_stops.reverse();
        for stop in &mut color_line.color_stops {
            stop.offset = 1.0 - stop.offset;
        }
    }

    let angles: Vec<f64> = color_line
        .color_stops
        .iter()
        .map(|stop| start_angle + stop.offset * (end_angle - start_angle))
        .collect();
    let colors: Vec<SrgbaTuple> = color_line
        .color_stops
        .iter()
        .map(|stop| stop.color.into())
        .collect();

    let n_stops = angles.len();

    if color_line.extend == Extend::Pad {
        let mut color0 = colors[0];
        let mut pos = 0;
        while pos < n_stops {
            if angles[pos] >= 0. {
                if pos > 0 {
                    let k = (0. - angles[pos - 1]) / (angles[pos] - angles[pos - 1]);

                    color0 = colors[pos - 1].interpolate(colors[pos], k);
                }
                break;
            }
            pos += 1;
        }
        if pos == n_stops {
            /* everything is below 0 */
            color0 = colors[n_stops - 1];
            add_sweep_gradient_patches(mesh, center, radius, 0., color0, PI_TIMES_2, color0);
            return;
        }

        add_sweep_gradient_patches(mesh, center, radius, 0., color0, angles[pos], colors[pos]);

        pos += 1;
        while pos < n_stops {
            if angles[pos] <= PI_TIMES_2 {
                add_sweep_gradient_patches(
                    mesh,
                    center,
                    radius,
                    angles[pos - 1],
                    colors[pos - 1],
                    angles[pos],
                    colors[pos],
                );
            } else {
                let k = (PI_TIMES_2 - angles[pos - 1]) / (angles[pos] - angles[pos - 1]);
                let color1 = colors[pos - 1].interpolate(colors[pos], k);
                add_sweep_gradient_patches(
                    mesh,
                    center,
                    radius,
                    angles[pos - 1],
                    colors[pos - 1],
                    PI_TIMES_2,
                    color1,
                );
                break;
            }
            pos += 1;
        }

        if pos == n_stops {
            /* everything is below 2*M_PI */
            color0 = colors[n_stops - 1];
            add_sweep_gradient_patches(
                mesh,
                center,
                radius,
                angles[n_stops - 1],
                color0,
                PI_TIMES_2,
                color0,
            );
            return;
        }
    } else {
        let span = angles[n_stops - 1] - angles[0];
        let mut k = 0isize;
        if angles[0] >= 0. {
            let mut ss = angles[0];
            while ss > 0. {
                if span > 0. {
                    ss -= span;
                    k -= 1;
                } else {
                    ss += span;
                    k += 1;
                }
            }
        } else if angles[0] < 0. {
            let mut ee = angles[n_stops - 1];
            while ee < 0. {
                if span > 0. {
                    ee += span;
                    k += 1;
                } else {
                    ee -= span;
                    k -= 1;
                }
            }
        }

        debug_assert!(
            angles[0] + (k as f64) * span <= 0. && 0. < angles[n_stops - 1] + (k as f64) * span
        );
        let span = span.abs();

        for l in k..k.min(1000) {
            for i in 1..n_stops {
                let (a0, a1, c0, c1);

                if l % 2 != 0 && color_line.extend == Extend::Reflect {
                    a0 = angles[0] + angles[n_stops - 1] - angles[n_stops - 1 - (i - 1)]
                        + (l as f64) * span;
                    a1 = angles[0] + angles[n_stops - 1] - angles[n_stops - 1 - i]
                        + (l as f64) * span;
                    c0 = colors[n_stops - 1 - (i - 1)];
                    c1 = colors[n_stops - 1 - i];
                } else {
                    a0 = angles[i - 1] + (l as f64) * span;
                    a1 = angles[i] + (l as f64) * span;
                    c0 = colors[i - 1];
                    c1 = colors[i];
                }

                if a1 < 0. {
                    continue;
                }

                if a0 < 0. {
                    let f = (0. - a0) / (a1 - a0);
                    let color = c0.interpolate(c1, f);
                    add_sweep_gradient_patches(mesh, center, radius, 0., color, a1, c1);
                } else if a1 >= PI_TIMES_2 {
                    let f = (PI_TIMES_2 - a0) / (a1 - a0);
                    let color = c0.interpolate(c1, f);
                    add_sweep_gradient_patches(mesh, center, radius, a0, c0, PI_TIMES_2, color);
                    return;
                } else {
                    add_sweep_gradient_patches(mesh, center, radius, a0, c0, a1, c1);
                }
            }
        }
    }
}

pub fn paint_sweep_gradient(
    context: &Context,
    x0: f64,
    y0: f64,
    start_angle: f64,
    end_angle: f64,
    color_line: ColorLine,
) -> anyhow::Result<()> {
    let (x1, y1, x2, y2) = context.clip_extents()?;

    let max_x = ((x1 - x0) * (x1 - x0)).max((x2 - x0) * (x2 - x0));
    let max_y = ((y1 - y0) * (y1 - y0)).max((y2 - y0) * (y2 - y0));
    let radius = (max_x + max_y).sqrt();

    let mesh = Mesh::new();
    let center = Point { x: x0, y: y0 };
    apply_sweep_gradient_patches(&mesh, color_line, center, radius, start_angle, end_angle);
    context.set_source(mesh)?;
    context.paint()?;

    Ok(())
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
