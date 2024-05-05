use crate::glyphcache::{GlyphCache, SizedBlockKey};
use crate::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::Sprite;
use ::window::color::SrgbaPixel;
use config::DimensionContext;
use std::ops::Range;
use termwiz::surface::CursorShape;
use tiny_skia::{FillRule, Paint, Path, PathBuilder, PixmapMut, Stroke, Transform};
use wezterm_font::units::{IntPixelLength, PixelLength};
use window::{BitmapImage, Image, Point, Rect, Size};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum PolyAA {
    AntiAlias,
    MoarPixels,
}

bitflags::bitflags! {
    //  ╭──────╮
    //  │UL╱╲UR│
    //  │ ╱  ╲ │
    //  │╱    ╲│
    //  │╲    ╱│
    //  │ ╲  ╱ │
    //  │LL╲╱LR│
    //  ╰──────╯
    pub struct CellDiagonal: u8{
        const UPPER_LEFT = 1<<1;
        const UPPER_RIGHT = 1<<2;
        const LOWER_LEFT = 1<<3;
        const LOWER_RIGHT = 1<<4;
    }
}

bitflags::bitflags! {
    // ╭────╮
    // │╲U ╱│
    // │ ╲╱R│
    // │L╱╲ │
    // │╱ D╲│
    // ╰────╯
    pub struct Triangle: u8{
        const UPPER = 1<<1;
        const RIGHT = 1<<2;
        const LOWER = 1<<3;
        const LEFT = 1<<4;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockAlpha {
    /// 100%
    Full,
    /// 75%
    Dark,
    /// 50%
    Medium,
    /// 25%
    Light,
}

impl BlockAlpha {
    pub fn to_scale(self) -> f32 {
        match self {
            BlockAlpha::Full => 1.0,
            BlockAlpha::Dark => 0.75,
            BlockAlpha::Medium => 0.5,
            BlockAlpha::Light => 0.25,
        }
    }
}

/// Represents a scaled width of the underline thickness.
/// Can either multiple or divide by the specified amount
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum LineScale {
    Mul(i8),
    Div(i8),
}

impl LineScale {
    fn to_scale(self) -> f32 {
        match self {
            Self::Mul(n) => n as f32,
            Self::Div(n) => 1. / n as f32,
        }
    }
}

/// Represents a coordinate in a glyph expressed in relation
/// to the dimension of the glyph.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockCoord {
    /// 0 pixels in; either the leftmost or topmost pixel position
    Zero,
    /// 100% of the dimension; either the rightmost or bottom pixel
    /// position
    One,
    /// A fraction of the width/height.  The first value is the
    /// numerator, the second is the denominator.
    Frac(i8, i8),

    /// Like Frac() above, but also specifies a scale to use
    /// together with the underline height to adjust the position.
    /// This is helpful because the line drawing routines stroke
    /// along the center of the line in the direction of the line,
    /// but don't pad the end of the line out by the width automatically.
    /// zeno has Cap::Square to specify that, but we can't use it
    /// directly and it isn't necessarily the adjustment that we want.
    /// This is most useful when joining lines that have different
    /// stroke widths; if the widths were all the same then you'd
    /// just specify the points in the path and not worry about it.
    FracWithOffset(i8, i8, LineScale),
}

impl BlockCoord {
    /// Compute the actual pixel value given the max dimension.
    pub fn to_pixel(self, max: usize, underline_height: f32) -> f32 {
        /// For interior points, adjust so that we get the middle of the row;
        /// in AA modes with 1px wide strokes this gives better results.
        fn hint(v: f32) -> f32 {
            if v.fract() == 0. {
                v - 0.5
            } else {
                v
            }
        }
        match self {
            Self::Zero => 0.,
            Self::One => max as f32,
            Self::Frac(num, den) => hint(max as f32 * num as f32 / den as f32),
            Self::FracWithOffset(num, den, under) => {
                hint((max as f32 * num as f32 / den as f32) + (underline_height * under.to_scale()))
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Block {
    /// Number of 1/8ths: x0, x1, y0, y1 with custom alpha
    Custom(u8, u8, u8, u8, BlockAlpha),
    /// Number of 1/8ths in the upper half
    UpperBlock(u8),
    /// Number of 1/8ths in the lower half
    LowerBlock(u8),
    /// Number of 1/8ths in the left half
    LeftBlock(u8),
    /// Number of 1/8ths in the right half
    RightBlock(u8),
    /// Number of 1/8ths: x0, x1
    VerticalBlock(u8, u8),
    /// Number of 1/8ths: y0, y1
    HorizontalBlock(u8, u8),
    /// Quadrants
    // ╭──┬──╮
    // │UL│UR│
    // ├──┼──┤
    // │LL│LR│
    // ╰──┴──╯
    QuadrantUL,
    QuadrantUR,
    QuadrantLL,
    QuadrantLR,
    /// Sextants by enum combination
    // ╭───┬───╮
    // │ 1 │ 2 │
    // ├───┼───┤
    // │ 3 │ 4 │
    // ├───┼───┤
    // │ 5 │ 6 │
    // ╰───┴───╯
    Sextant1,
    Sextant2,
    Sextant3,
    Sextant4,
    Sextant5,
    Sextant6,
}

/// Represents a Block Element glyph, decoded from
/// <https://en.wikipedia.org/wiki/Block_Elements>
/// <https://www.unicode.org/charts/PDF/U2580.pdf>
/// <https://unicode.org/charts/PDF/U1FB00.pdf>
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockKey {
    /// List of block rectangles
    Blocks(&'static [Block]),
    /// List of triangles
    Triangles(Triangle, BlockAlpha),
    /// A combination of small diagonal lines
    CellDiagonals(CellDiagonal),
    /// A braille dot pattern
    Braille(u8),

    Poly(&'static [Poly]),

    PolyWithCustomMetrics {
        polys: &'static [Poly],
        underline_height: IntPixelLength,
        cell_size: Size,
    },
}

/// Filled polygon used to describe the more complex shapes in
/// <https://unicode.org/charts/PDF/U1FB00.pdf>
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Poly {
    pub path: &'static [PolyCommand],
    pub intensity: BlockAlpha,
    pub style: PolyStyle,
}

pub type BlockPoint = (BlockCoord, BlockCoord);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PolyCommand {
    MoveTo(BlockCoord, BlockCoord),
    LineTo(BlockCoord, BlockCoord),
    QuadTo {
        control: BlockPoint,
        to: BlockPoint,
    },
    Oval {
        center: BlockPoint,
        radiuses: BlockPoint,
    },
    Close,
}

impl PolyCommand {
    fn to_skia(&self, width: usize, height: usize, underline_height: f32, pb: &mut PathBuilder) {
        match self {
            Self::MoveTo(x, y) => pb.move_to(
                x.to_pixel(width, underline_height),
                y.to_pixel(height, underline_height),
            ),
            Self::LineTo(x, y) => pb.line_to(
                x.to_pixel(width, underline_height),
                y.to_pixel(height, underline_height),
            ),
            Self::QuadTo {
                control: (x1, y1),
                to: (x, y),
            } => pb.quad_to(
                x1.to_pixel(width, underline_height),
                y1.to_pixel(height, underline_height),
                x.to_pixel(width, underline_height),
                y.to_pixel(height, underline_height),
            ),
            Self::Oval {
                center: (x, y),
                radiuses: (w, h),
            } => {
                let x = x.to_pixel(width, underline_height) - width as f32;
                let y = y.to_pixel(height, underline_height) - height as f32;
                let w = w.to_pixel(width, underline_height) * 2.0;
                let h = h.to_pixel(height, underline_height) * 2.0;

                if let Some(oval) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                    pb.push_oval(oval);
                } else {
                    log::error!("Can't push oval, values: {:?}", self);
                }
            }
            Self::Close => pb.close(),
        };
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PolyStyle {
    Fill,
    OutlineAlpha,
    OutlineThin,
    // A line with the thickness as underlines
    Outline,
    // A line with twice the thickness of underlines
    OutlineHeavy,
}

impl PolyStyle {
    fn apply(self, width: f32, paint: &Paint, path: &Path, pixmap: &mut PixmapMut) {
        match self {
            PolyStyle::Fill => {
                pixmap.fill_path(path, paint, FillRule::Winding, Transform::identity(), None);
            }

            PolyStyle::OutlineThin
            | PolyStyle::Outline
            | PolyStyle::OutlineHeavy
            | PolyStyle::OutlineAlpha => {
                let mut stroke = Stroke::default();
                stroke.width = width;
                if self == PolyStyle::OutlineHeavy {
                    stroke.width *= 3.01; // NOTE: Changing this makes block cursor disproportionate at different font sizes and resolutions
                } else if self == PolyStyle::OutlineThin {
                    stroke.width = 1.2;
                } else if self == PolyStyle::OutlineAlpha {
                    stroke.width = 0.25; // NOTE: This is for filling antialiased border between triangles when using the alpha style
                }

                pixmap.stroke_path(path, paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

impl BlockKey {
    pub fn filter_out_synthetic(glyphs: &mut Vec<char>) {
        let config = config::configuration();
        if config.custom_block_glyphs {
            glyphs.retain(|&c| Self::from_char(c).is_none());
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let mut chars = s.chars();
        let first_char = chars.next()?;
        if chars.next().is_some() {
            None
        } else {
            Self::from_char(first_char)
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        let c = c as u32;
        Some(match c {
            // [─] BOX DRAWINGS LIGHT HORIZONTAL
            0x2500 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [━] BOX DRAWINGS HEAVY HORIZONTAL
            0x2501 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [│] BOX DRAWINGS LIGHT VERTICAL
            0x2502 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┃] BOX DRAWINGS HEAVY VERTICAL
            0x2503 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [┄] BOX DRAWINGS LIGHT TRIPLE DASH HORIZONTAL
            // A dash segment is wider than the gap segment.
            // We use a 2:1 ratio, which gives 9 total segments
            // with a pattern of `-- -- -- `
            0x2504 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 9), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(6, 9), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(8, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┅] BOX DRAWINGS HEAVY TRIPLE DASH HORIZONTAL
            0x2505 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 9), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(6, 9), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(8, 9), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┆] BOX DRAWINGS LIGHT TRIPLE DASH VERTICAL
            0x2506 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 9)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(6, 9)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(8, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┇] BOX DRAWINGS HEAVY TRIPLE DASH VERTICAL
            0x2507 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 9)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(6, 9)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(8, 9)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┈] BOX DRAWINGS LIGHT QUADRUPLE DASH HORIZONTAL
            // A dash segment is wider than the gap segment.
            // We use a 2:1 ratio, which gives 12 total segments
            // with a pattern of `-- -- -- -- `
            0x2508 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(6, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(8, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(9, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(11, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┉] BOX DRAWINGS HEAVY QUADRUPLE DASH HORIZONTAL
            0x2509 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(6, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(8, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(9, 12), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(11, 12), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┊] BOX DRAWINGS LIGHT QUADRUPLE DASH VERTICAL
            0x250a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(6, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(8, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(9, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(11, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┋] BOX DRAWINGS HEAVY QUADRUPLE DASH VERTICAL
            0x250b => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(6, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(8, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(9, 12)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(11, 12)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┌] BOX DRAWINGS LIGHT DOWN AND RIGHT
            0x250c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┍] BOX DRAWINGS DOWN LIGHT AND RIGHT HEAVY
            0x250d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-2)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┎] BOX DRAWINGS DOWN HEAVY AND RIGHT LIGHT
            0x250e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┏] BOX DRAWINGS HEAVY DOWN AND RIGHT
            0x250f => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),

            // [┐] BOX DRAWINGS LIGHT DOWN AND LEFT
            0x2510 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┑] BOX DRAWINGS DOWN LIGHT AND LEFT HEAVY
            0x2511 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(2)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┒] BOX DRAWINGS DOWN HEAVY AND LEFT LIGHT
            0x2512 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┓] BOX DRAWINGS HEAVY DOWN AND LEFT
            0x2513 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),

            // [└] BOX DRAWINGS LIGHT UP AND RIGHT
            0x2514 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┕] BOX DRAWINGS UP LIGHT AND RIGHT HEAVY
            0x2515 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-2)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┖] BOX DRAWINGS UP HEAVY AND RIGHT LIGHT
            0x2516 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┗] BOX DRAWINGS HEAVY UP AND RIGHT
            0x2517 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),

            // [┘] BOX DRAWINGS LIGHT UP AND LEFT
            0x2518 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┙] BOX DRAWINGS UP LIGHT AND LEFT HEAVY
            0x2519 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(2)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┚] BOX DRAWINGS UP HEAVY AND LEFT LIGHT
            0x251a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┛] BOX DRAWINGS HEAVY UP AND LEFT
            0x251b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),

            // [├] BOX DRAWINGS LIGHT VERTICAL AND RIGHT
            0x251c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┝] BOX DRAWINGS LIGHT VERTICAL LIGHT AND RIGHT HEAVY
            0x251d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┞] BOX DRAWINGS UP HEAVY and RIGHT DOWN LIGHT
            0x251e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┟] BOX DRAWINGS DOWN HEAVY and RIGHT UP LIGHT
            0x251f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [┠] BOX DRAWINGS HEAVY VERTICAL and RIGHT LIGHT
            0x2520 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┡] BOX DRAWINGS DOWN LIGHT AND RIGHT UP HEAVY
            0x2521 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┢] BOX DRAWINGS UP LIGHT AND RIGHT DOWN HEAVY
            0x2522 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┣] BOX DRAWINGS HEAVY VERTICAL and RIGHT
            0x2523 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [┤] BOX DRAWINGS LIGHT VERTICAL and LEFT
            0x2524 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┥] BOX DRAWINGS VERTICAL LIGHT and LEFT HEAVY
            0x2525 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┦] BOX DRAWINGS UP HEAVY and LEFT DOWN LIGHT
            0x2526 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┧] BOX DRAWINGS DOWN HEAVY and LEFT UP LIGHT
            0x2527 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┨] BOX DRAWINGS VERTICAL HEAVY and LEFT LIGHT
            0x2528 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┩] BOX DRAWINGS DOWN LIGHT and LEFT UP HEAVY
            0x2529 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┪] BOX DRAWINGS UP LIGHT and LEFT DOWN HEAVY
            0x252a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┫] BOX DRAWINGS HEAVY VERTICAL and LEFT
            0x252b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [┬] BOX DRAWINGS LIGHT DOWN AND HORIZONTAL
            0x252c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┭] BOX DRAWINGS LEFT HEAVY AND RIGHT DOWN LIGHT
            0x252d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┮] BOX DRAWINGS RIGHT HEAVY AND LEFT DOWN LIGHT
            0x252e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┯] BOX DRAWINGS DOWN LIGHT AND HORIZONTAL HEAVY
            0x252f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),

            // [┰] BOX DRAWINGS DOWN HEAVY AND HORIZONTAL LIGHT
            0x2530 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [┱] BOX DRAWINGS RIGHT LIGHT AND LEFT DOWN HEAVY
            0x2531 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┲] BOX DRAWINGS LEFT LIGHT AND RIGHT DOWN HEAVY
            0x2532 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┳] BOX DRAWINGS HEAVY DOWN AND HORIZONTAL
            0x2533 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [┴] BOX DRAWINGS LIGHT UP AND HORIZONTAL
            0x2534 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┵] BOX DRAWINGS LEFT HEAVY AND RIGHT UP LIGHT
            0x2535 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┶] BOX DRAWINGS RIGHT HEAVY AND LEFT UP LIGHT
            0x2536 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┷] BOX DRAWINGS UP LIGHT AND HORIZONTAL HEAVY
            0x2537 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),

            // [┸] BOX DRAWINGS UP HEAVY AND HORIZONTAL LIGHT
            0x2538 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [┹] BOX DRAWINGS RIGHT LIGHT AND LEFT UP HEAVY
            0x2539 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┺] BOX DRAWINGS LEFT LIGHT AND RIGHT UP HEAVY
            0x253a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [┻] BOX DRAWINGS HEAVY UP AND HORIZONTAL
            0x253b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [┼] BOX DRAWINGS LIGHT VERTICAL AND HORIZONTAL
            0x253c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [┽] BOX DRAWINGS LEFT HEAVY AND RIGHT VERTICAL LIGHT
            0x253d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┾] BOX DRAWINGS RIGHT HEAVY AND LEFT VERTICAL LIGHT
            0x253e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [┿] BOX DRAWINGS VERTICAL LIGHT AND HORIZONTAL HEAVY
            0x253f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╀] BOX DRAWINGS UP HEAVY AND DOWN HORIZONTAL LIGHT
            0x2540 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╁] BOX DRAWINGS DOWN HEAVY AND UP HORIZONTAL LIGHT
            0x2541 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╂] BOX DRAWINGS VERTICAL HEAVY AND HORIZONTAL LIGHT
            0x2542 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╃] BOX DRAWINGS LEFT UP HEAVY and RIGHT DOWN LIGHT
            0x2543 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╄] BOX DRAWINGS RIGHT UP HEAVY and LEFT DOWN LIGHT
            0x2544 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╅] BOX DRAWINGS LEFT DOWN HEAVY and RIGHT UP LIGHT
            0x2545 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╆] BOX DRAWINGS RIGHT DOWN HEAVY and LEFT UP LIGHT
            0x2546 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╇] BOX DRAWINGS DOWN LIGHT AND UP HORIZONTAL HEAVY
            0x2547 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╈] BOX DRAWINGS UP LIGHT AND DOWN HORIZONTAL HEAVY
            0x2548 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╉] BOX DRAWINGS RIGHT LIGHT AND LEFT VERTICAL HEAVY
            0x2549 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╊] BOX DRAWINGS LEFT LIGHT AND RIGHT VERTICAL HEAVY
            0x254a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╋] BOX DRAWINGS HEAVY VERTICAL AND HORIZONTAL
            0x254b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),

            // [╌] BOX DRAWINGS LIGHT DOUBLE DASH HORIZONTAL
            // A dash segment is wider than the gap segment.
            // We use a 2:1 ratio, which gives 6 total segments
            // with a pattern of `-- -- `
            0x254c => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 6), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 6), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 6), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╍] BOX DRAWINGS HEAVY DOUBLE DASH HORIZONTAL
            0x254d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(2, 6), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 6), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 6), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╎] BOX DRAWINGS LIGHT DOUBLE DASH VERTICAL
            0x254e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 6)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 6)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 6)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╏] BOX DRAWINGS HEAVY DOUBLE DASH VERTICAL
            0x254f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(2, 6)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 6)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(5, 6)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),

            // [═] BOX DRAWINGS DOUBLE HORIZONTAL
            0x2550 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [║] BOX DRAWINGS DOUBLE VERTICAL
            0x2551 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╒] BOX DRAWINGS DOWN SINGLE AND RIGHT DOUBLE
            0x2552 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╓] BOX DRAWINGS DOWN DOUBLE AND RIGHT SINGLE
            0x2553 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [╔] BOX DRAWINGS DOUBLE DOWN AND RIGHT
            0x2554 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╕] BOX DRAWINGS DOWN SINGLE AND LEFT DOUBLE
            0x2555 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╖] BOX DRAWINGS DOWN DOUBLE AND LEFT SINGLE
            0x2556 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╗] BOX DRAWINGS DOUBLE DOWN AND LEFT
            0x2557 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╘] BOX DRAWINGS UP SINGLE AND RIGHT DOUBLE
            0x2558 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╙] BOX DRAWINGS UP DOUBLE AND RIGHT SINGLE
            0x2559 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╚] BOX DRAWINGS DOUBLE UP AND RIGHT
            0x255a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╛] BOX DRAWINGS UP SINGLE AND LEFT DOUBLE
            0x255b => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╜] BOX DRAWINGS UP DOUBLE AND LEFT SINGLE
            0x255c => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╝] BOX DRAWINGS DOUBLE UP AND LEFT
            0x255d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [╞] BOX DRAWINGS VERTICAL SINGLE AND RIGHT DOUBLE
            0x255e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╟] BOX DRAWINGS VERTICAL DOUBLE AND RIGHT SINGLE
            0x255f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [╠] BOX DRAWINGS DOUBLE VERTICAL AND RIGHT
            0x2560 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╡] BOX DRAWINGS VERTICAL SINGLE AND LEFT DOUBLE
            0x2561 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╢] BOX DRAWINGS VERTICAL DOUBLE AND LEFT SINGLE
            0x2562 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╣] BOX DRAWINGS DOUBLE VERTICAL AND LEFT
            0x2563 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╤] BOX DRAWINGS DOWN SINGLE AND HORIZONTAL DOUBLE
            0x2564 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╥] BOX DRAWINGS DOWN DOUBLE AND HORIZONTAL SINGLE
            0x2565 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╦] BOX DRAWINGS DOUBLE DOWN AND HORIZONTAL
            0x2566 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╧] BOX DRAWINGS UP SINGLE AND HORIZONTAL DOUBLE
            0x2567 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╨] BOX DRAWINGS UP DOUBLE AND HORIZONTAL SINGLE
            0x2568 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Frac(1, 2),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╩] BOX DRAWINGS DOUBLE UP AND HORIZONTAL
            0x2569 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╪] BOX DRAWINGS VERTICAL SINGLE AND HORIZONTAL DOUBLE
            0x256a => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╫] BOX DRAWINGS VERTICAL DOUBLE AND HORIZONTAL SINGLE
            0x256b => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [╬] BOX DRAWINGS DOUBLE VERTICAL AND HORIZONTAL
            0x256c => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::Zero,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Zero,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(-1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::One,
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                        ),
                        PolyCommand::LineTo(
                            BlockCoord::FracWithOffset(1, 2, LineScale::Mul(1)),
                            BlockCoord::One,
                        ),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [╭] BOX DRAWINGS LIGHT ARC DOWN AND RIGHT
            0x256d => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 4)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(3, 4), BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╮] BOX DRAWINGS LIGHT ARC DOWN AND LEFT
            0x256e => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(3, 4)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(1, 4), BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╯] BOX DRAWINGS LIGHT ARC UP AND LEFT
            0x256f => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 4)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(1, 4), BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╰] BOX DRAWINGS LIGHT ARC UP AND RIGHT
            0x2570 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 4)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(3, 4), BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),

            // [╱] BOX DRAWINGS LIGHT DIAGONAL UPPER RIGHT TO LOWER LEFT
            0x2571 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╲] BOX DRAWINGS LIGHT DIAGONAL UPPER LEFT TO LOWER RIGHT
            0x2572 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╳] BOX DRAWINGS LIGHT DIAGONAL CROSS
            0x2573 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╴] BOX DRAWINGS LIGHT LEFT
            0x2574 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╵] BOX DRAWINGS LIGHT UP
            0x2575 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╶] BOX DRAWINGS LIGHT RIGHT
            0x2576 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╷] BOX DRAWINGS LIGHT DOWN
            0x2577 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [╸] BOX DRAWINGS HEAVY LEFT
            0x2578 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [╹] BOX DRAWINGS HEAVY UP
            0x2579 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [╺] BOX DRAWINGS HEAVY RIGHT
            0x257a => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [╻] BOX DRAWINGS HEAVY DOWN
            0x257b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::OutlineHeavy,
            }]),
            // [╼] BOX DRAWINGS LIGHT LEFT AND HEAVY RIGHT
            0x257c => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╽] BOX DRAWINGS LIGHT UP AND HEAVY DOWN
            0x257d => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
            ]),
            // [╾] BOX DRAWINGS HEAVY LEFT AND LIGHT RIGHT
            0x257e => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),
            // [╿] BOX DRAWINGS HEAVY UP AND LIGHT DOWN
            0x257f => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2)),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineHeavy,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(
                            BlockCoord::Frac(1, 2),
                            BlockCoord::FracWithOffset(1, 2, LineScale::Div(-1)),
                        ),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // [▀] UPPER HALF BLOCK
            0x2580 => Self::Blocks(&[Block::UpperBlock(4)]),
            // [▁] LOWER 1 EIGHTH BLOCK
            0x2581 => Self::Blocks(&[Block::LowerBlock(1)]),
            // [▂] LOWER 2 EIGHTHS BLOCK
            0x2582 => Self::Blocks(&[Block::LowerBlock(2)]),
            // [▃] LOWER 3 EIGHTHS BLOCK
            0x2583 => Self::Blocks(&[Block::LowerBlock(3)]),
            // [▄] LOWER 4 EIGHTHS BLOCK
            0x2584 => Self::Blocks(&[Block::LowerBlock(4)]),
            // [▅] LOWER 5 EIGHTHS BLOCK
            0x2585 => Self::Blocks(&[Block::LowerBlock(5)]),
            // [▆] LOWER 6 EIGHTHS BLOCK
            0x2586 => Self::Blocks(&[Block::LowerBlock(6)]),
            // [▇] LOWER 7 EIGHTHS BLOCK
            0x2587 => Self::Blocks(&[Block::LowerBlock(7)]),
            // [█] FULL BLOCK
            0x2588 => Self::Blocks(&[Block::Custom(0, 8, 0, 8, BlockAlpha::Full)]),
            // [▉] LEFT 7 EIGHTHS BLOCK
            0x2589 => Self::Blocks(&[Block::LeftBlock(7)]),
            // [▊] LEFT 6 EIGHTHS BLOCK
            0x258a => Self::Blocks(&[Block::LeftBlock(6)]),
            // [▋] LEFT 5 EIGHTHS BLOCK
            0x258b => Self::Blocks(&[Block::LeftBlock(5)]),
            // [▌] LEFT 4 EIGHTHS BLOCK
            0x258c => Self::Blocks(&[Block::LeftBlock(4)]),
            // [▍] LEFT 3 EIGHTHS BLOCK
            0x258d => Self::Blocks(&[Block::LeftBlock(3)]),
            // [▎] LEFT 2 EIGHTHS BLOCK
            0x258e => Self::Blocks(&[Block::LeftBlock(2)]),
            // [▏] LEFT 1 EIGHTHS BLOCK
            0x258f => Self::Blocks(&[Block::LeftBlock(1)]),
            // [▐] RIGHT HALF BLOCK
            0x2590 => Self::Blocks(&[Block::RightBlock(4)]),
            // [░] LIGHT SHADE
            0x2591 => Self::Blocks(&[Block::Custom(0, 8, 0, 8, BlockAlpha::Light)]),
            // [▒] MEDIUM SHADE
            0x2592 => Self::Blocks(&[Block::Custom(0, 8, 0, 8, BlockAlpha::Medium)]),
            // [▓] DARK SHADE
            0x2593 => Self::Blocks(&[Block::Custom(0, 8, 0, 8, BlockAlpha::Dark)]),
            // [▔] UPPER ONE EIGHTH BLOCK
            0x2594 => Self::Blocks(&[Block::UpperBlock(1)]),
            // [▕] RIGHT ONE EIGHTH BLOCK
            0x2595 => Self::Blocks(&[Block::RightBlock(1)]),
            // [▖] QUADRANT LOWER LEFT
            0x2596 => Self::Blocks(&[Block::QuadrantLL]),
            // [▗] QUADRANT LOWER RIGHT
            0x2597 => Self::Blocks(&[Block::QuadrantLR]),
            // [▘] QUADRANT UPPER LEFT
            0x2598 => Self::Blocks(&[Block::QuadrantUL]),
            // [▙] QUADRANT UPPER LEFT AND LOWER LEFT AND LOWER RIGHT
            0x2599 => Self::Blocks(&[Block::QuadrantUL, Block::QuadrantLL, Block::QuadrantLR]),
            // [▚] QUADRANT UPPER LEFT AND LOWER RIGHT
            0x259a => Self::Blocks(&[Block::QuadrantUL, Block::QuadrantLR]),
            // [▛] QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT
            0x259b => Self::Blocks(&[Block::QuadrantUL, Block::QuadrantUR, Block::QuadrantLL]),
            // [▜] QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER RIGHT
            0x259c => Self::Blocks(&[Block::QuadrantUL, Block::QuadrantUR, Block::QuadrantLR]),
            // [▝] QUADRANT UPPER RIGHT
            0x259d => Self::Blocks(&[Block::QuadrantUR]),
            // [▞] QUADRANT UPPER RIGHT AND LOWER LEFT
            0x259e => Self::Blocks(&[Block::QuadrantUR, Block::QuadrantLL]),
            // [▟] QUADRANT UPPER RIGHT AND LOWER LEFT AND LOWER RIGHT
            0x259f => Self::Blocks(&[Block::QuadrantUR, Block::QuadrantLL, Block::QuadrantLR]),
            // [🬀] BLOCK SEXTANT-1
            0x1fb00 => Self::Blocks(&[Block::Sextant1]),
            // [🬁] BLOCK SEXTANT-2
            0x1fb01 => Self::Blocks(&[Block::Sextant2]),
            // [🬂] BLOCK SEXTANT-12
            0x1fb02 => Self::Blocks(&[Block::Sextant1, Block::Sextant2]),
            // [🬃] BLOCK SEXTANT-3
            0x1fb03 => Self::Blocks(&[Block::Sextant3]),
            // [🬄] BLOCK SEXTANT-13
            0x1fb04 => Self::Blocks(&[Block::Sextant1, Block::Sextant3]),
            // [🬅] BLOCK SEXTANT-23
            0x1fb05 => Self::Blocks(&[Block::Sextant2, Block::Sextant3]),
            // [🬆] BLOCK SEXTANT-123
            0x1fb06 => Self::Blocks(&[Block::Sextant1, Block::Sextant2, Block::Sextant3]),
            // [🬇] BLOCK SEXTANT-4
            0x1fb07 => Self::Blocks(&[Block::Sextant4]),
            // [🬈] BLOCK SEXTANT-14
            0x1fb08 => Self::Blocks(&[Block::Sextant1, Block::Sextant4]),
            // [🬉] BLOCK SEXTANT-24
            0x1fb09 => Self::Blocks(&[Block::Sextant2, Block::Sextant4]),
            // [🬊] BLOCK SEXTANT-124
            0x1fb0a => Self::Blocks(&[Block::Sextant1, Block::Sextant2, Block::Sextant4]),
            // [🬋] BLOCK SEXTANT-34
            0x1fb0b => Self::Blocks(&[Block::Sextant3, Block::Sextant4]),
            // [🬌] BLOCK SEXTANT-134
            0x1fb0c => Self::Blocks(&[Block::Sextant1, Block::Sextant3, Block::Sextant4]),
            // [🬍] BLOCK SEXTANT-234
            0x1fb0d => Self::Blocks(&[Block::Sextant2, Block::Sextant3, Block::Sextant4]),
            // [🬎] BLOCK SEXTANT-1234
            0x1fb0e => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
            ]),
            // [🬏] BLOCK SEXTANT-5
            0x1fb0f => Self::Blocks(&[Block::Sextant5]),
            // [🬐] BLOCK SEXTANT-15
            0x1fb10 => Self::Blocks(&[Block::Sextant1, Block::Sextant5]),
            // [🬑] BLOCK SEXTANT-25
            0x1fb11 => Self::Blocks(&[Block::Sextant2, Block::Sextant5]),
            // [🬒] BLOCK SEXTANT-125
            0x1fb12 => Self::Blocks(&[Block::Sextant1, Block::Sextant2, Block::Sextant5]),
            // [🬓] BLOCK SEXTANT-35
            0x1fb13 => Self::Blocks(&[Block::Sextant3, Block::Sextant5]),
            // [🬔] BLOCK SEXTANT-235
            0x1fb14 => Self::Blocks(&[Block::Sextant2, Block::Sextant3, Block::Sextant5]),
            // [🬕] BLOCK SEXTANT-1235
            0x1fb15 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant5,
            ]),
            // [🬖] BLOCK SEXTANT-45
            0x1fb16 => Self::Blocks(&[Block::Sextant4, Block::Sextant5]),
            // [🬗] BLOCK SEXTANT-145
            0x1fb17 => Self::Blocks(&[Block::Sextant1, Block::Sextant4, Block::Sextant5]),
            // [🬘] BLOCK SEXTANT-245
            0x1fb18 => Self::Blocks(&[Block::Sextant2, Block::Sextant4, Block::Sextant5]),
            // [🬙] BLOCK SEXTANT-1245
            0x1fb19 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant4,
                Block::Sextant5,
            ]),
            // [🬚] BLOCK SEXTANT-345
            0x1fb1a => Self::Blocks(&[Block::Sextant3, Block::Sextant4, Block::Sextant5]),
            // [🬛] BLOCK SEXTANT-1345
            0x1fb1b => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
            ]),
            // [🬜] BLOCK SEXTANT-2345
            0x1fb1c => Self::Blocks(&[
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
            ]),
            // [🬝] BLOCK SEXTANT-12345
            0x1fb1d => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
            ]),
            // [🬞] BLOCK SEXTANT-6
            0x1fb1e => Self::Blocks(&[Block::Sextant6]),
            // [🬟] BLOCK SEXTANT-16
            0x1fb1f => Self::Blocks(&[Block::Sextant1, Block::Sextant6]),
            // [🬠] BLOCK SEXTANT-26
            0x1fb20 => Self::Blocks(&[Block::Sextant2, Block::Sextant6]),
            // [🬡] BLOCK SEXTANT-126
            0x1fb21 => Self::Blocks(&[Block::Sextant1, Block::Sextant2, Block::Sextant6]),
            // [🬢] BLOCK SEXTANT-36
            0x1fb22 => Self::Blocks(&[Block::Sextant3, Block::Sextant6]),
            // [🬣] BLOCK SEXTANT-136
            0x1fb23 => Self::Blocks(&[Block::Sextant1, Block::Sextant3, Block::Sextant6]),
            // [🬤] BLOCK SEXTANT-236
            0x1fb24 => Self::Blocks(&[Block::Sextant2, Block::Sextant3, Block::Sextant6]),
            // [🬥] BLOCK SEXTANT-1236
            0x1fb25 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant6,
            ]),
            // [🬦] BLOCK SEXTANT-46
            0x1fb26 => Self::Blocks(&[Block::Sextant4, Block::Sextant6]),
            // [🬧] BLOCK SEXTANT-146
            0x1fb27 => Self::Blocks(&[Block::Sextant1, Block::Sextant4, Block::Sextant6]),
            // [🬨] BLOCK SEXTANT-1246
            0x1fb28 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant4,
                Block::Sextant6,
            ]),
            // [🬩] BLOCK SEXTANT-346
            0x1fb29 => Self::Blocks(&[Block::Sextant3, Block::Sextant4, Block::Sextant6]),
            // [🬪] BLOCK SEXTANT-1346
            0x1fb2a => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant6,
            ]),
            // [🬫] BLOCK SEXTANT-2346
            0x1fb2b => Self::Blocks(&[
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant6,
            ]),
            // [🬬] BLOCK SEXTANT-12346
            0x1fb2c => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant6,
            ]),
            // [🬭] BLOCK SEXTANT-56
            0x1fb2d => Self::Blocks(&[Block::Sextant5, Block::Sextant6]),
            // [🬮] BLOCK SEXTANT-156
            0x1fb2e => Self::Blocks(&[Block::Sextant1, Block::Sextant5, Block::Sextant6]),
            // [🬯] BLOCK SEXTANT-256
            0x1fb2f => Self::Blocks(&[Block::Sextant2, Block::Sextant5, Block::Sextant6]),
            // [🬰] BLOCK SEXTANT-1256
            0x1fb30 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬱] BLOCK SEXTANT-356
            0x1fb31 => Self::Blocks(&[Block::Sextant3, Block::Sextant5, Block::Sextant6]),
            // [🬲] BLOCK SEXTANT-1356
            0x1fb32 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant3,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬳] BLOCK SEXTANT-2356
            0x1fb33 => Self::Blocks(&[
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬴] BLOCK SEXTANT-12356
            0x1fb34 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬵] BLOCK SEXTANT-456
            0x1fb35 => Self::Blocks(&[Block::Sextant4, Block::Sextant5, Block::Sextant6]),
            // [🬶] BLOCK SEXTANT-1456
            0x1fb36 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬷] BLOCK SEXTANT-2456
            0x1fb37 => Self::Blocks(&[
                Block::Sextant2,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬸] BLOCK SEXTANT-12456
            0x1fb38 => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant2,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬹] BLOCK SEXTANT-3456
            0x1fb39 => Self::Blocks(&[
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬺] BLOCK SEXTANT-13456
            0x1fb3a => Self::Blocks(&[
                Block::Sextant1,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬻] BLOCK SEXTANT-23456
            0x1fb3b => Self::Blocks(&[
                Block::Sextant2,
                Block::Sextant3,
                Block::Sextant4,
                Block::Sextant5,
                Block::Sextant6,
            ]),
            // [🬼] LOWER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER CENTRE
            0x1fb3c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🬽] LOWER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER RIGHT
            0x1fb3d => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🬾] LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER CENTRE
            0x1fb3e => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🬿] LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER RIGHT
            0x1fb3f => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭀] LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO LOWER CENTRE
            0x1fb40 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭁] LOWER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER CENTRE
            0x1fb41 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭂] LOWER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER RIGHT
            0x1fb42 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭃] LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER CENTRE
            0x1fb43 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭄] LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER RIGHT
            0x1fb44 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭅] LOWER RIGHT BLOCK DIAGONAL UPPER LEFT TO UPPER CENTRE
            0x1fb45 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭆] LOWER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER MIDDLE RIGHT
            0x1fb46 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭇] LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO LOWER MIDDLE RIGHT
            0x1fb47 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭈] LOWER RIGHT BLOCK DIAGONAL LOWER LEFT TO LOWER MIDDLE RIGHT
            0x1fb48 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭉] LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO UPPER MIDDLE RIGHT
            0x1fb49 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭊] LOWER RIGHT BLOCK DIAGONAL LOWER LEFT TO UPPER MIDDLE RIGHT
            0x1fb4a => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭋] LOWER RIGHT BLOCK DIAGONAL LOWER CENTRE TO UPPER RIGHT
            0x1fb4b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭌] LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO UPPER MIDDLE RIGHT
            0x1fb4c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭍] LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO UPPER MIDDLE RIGHT
            0x1fb4d => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭎] LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO LOWER MIDDLE RIGHT
            0x1fb4e => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭏] LOWER LEFT BLOCK DIAGONAL UPPER LEFT TO LOWER MIDDLE RIGHT
            0x1fb4f => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭐] LOWER LEFT BLOCK DIAGONAL UPPER CENTRE TO LOWER RIGHT
            0x1fb50 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭑] LOWER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER MIDDLE RIGHT
            0x1fb51 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭒] UPPER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER CENTRE
            0x1fb52 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭓] UPPER RIGHT BLOCK DIAGONAL LOWER MIDDLE LEFT TO LOWER RIGHT
            0x1fb53 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭔] UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER CENTRE
            0x1fb54 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭕] UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER RIGHT
            0x1fb55 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭖] UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO LOWER CENTRE
            0x1fb56 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭗] UPPER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER CENTRE
            0x1fb57 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭘] UPPER LEFT BLOCK DIAGONAL UPPER MIDDLE LEFT TO UPPER RIGHT
            0x1fb58 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭙] UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER CENTRE
            0x1fb59 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭚] UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER RIGHT
            0x1fb5a => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭛] UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO UPPER CENTRE
            0x1fb5b => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭜] UPPER LEFT BLOCK DIAGONAL LOWER MIDDLE LEFT TO UPPER MIDDLE RIGHT
            0x1fb5c => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭝] UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO LOWER MIDDLE RIGHT
            0x1fb5d => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭞] UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO LOWER MIDDLE RIGHT
            0x1fb5e => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭟] UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO UPPER MIDDLE RIGHT
            0x1fb5f => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭠] UPPER LEFT BLOCK DIAGONAL LOWER LEFT TO UPPER MIDDLE RIGHT
            0x1fb60 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭡] UPPER LEFT BLOCK DIAGONAL LOWER CENTRE TO UPPER RIGHT
            0x1fb61 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭢] UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO UPPER MIDDLE RIGHT
            0x1fb62 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭣] UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO UPPER MIDDLE RIGHT
            0x1fb63 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭤] UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO LOWER MIDDLE RIGHT
            0x1fb64 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭥] UPPER RIGHT BLOCK DIAGONAL UPPER LEFT TO LOWER MIDDLE RIGHT
            0x1fb65 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭦] UPPER RIGHT BLOCK DIAGONAL UPPER CENTRE TO LOWER RIGHT
            0x1fb66 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭧] UPPER RIGHT BLOCK DIAGONAL UPPER MIDDLE LEFT TO LOWER MIDDLE RIGHT
            0x1fb67 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(2, 3)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 3)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [🭨] UPPER AND RIGHT AND LOWER TRIANGULAR THREE QUARTERS BLOCK
            0x1fb68 => Self::Triangles(
                Triangle::UPPER | Triangle::RIGHT | Triangle::LOWER,
                BlockAlpha::Full,
            ),
            // [🭩] LEFT AND LOWER AND RIGHT TRIANGULAR THREE QUARTERS BLOCK
            0x1fb69 => Self::Triangles(
                Triangle::LEFT | Triangle::LOWER | Triangle::RIGHT,
                BlockAlpha::Full,
            ),
            // [🭪] UPPER AND LEFT AND LOWER TRIANGULAR THREE QUARTERS BLOCK
            0x1fb6a => Self::Triangles(
                Triangle::UPPER | Triangle::LEFT | Triangle::LOWER,
                BlockAlpha::Full,
            ),
            // [🭫] LEFT AND UPPER AND RIGHT TRIANGULAR THREE QUARTERS BLOCK
            0x1fb6b => Self::Triangles(
                Triangle::LEFT | Triangle::UPPER | Triangle::RIGHT,
                BlockAlpha::Full,
            ),
            // [🭬] LEFT TRIANGULAR ONE QUARTER BLOCK
            0x1fb6c => Self::Triangles(Triangle::LEFT, BlockAlpha::Full),
            // [🭭] UPPER TRIANGULAR ONE QUARTER BLOCK
            0x1fb6d => Self::Triangles(Triangle::UPPER, BlockAlpha::Full),
            // [🭮] RIGHT TRIANGULAR ONE QUARTER BLOCK
            0x1fb6e => Self::Triangles(Triangle::RIGHT, BlockAlpha::Full),
            // [🭯] LOWER TRIANGULAR ONE QUARTER BLOCK
            0x1fb6f => Self::Triangles(Triangle::LOWER, BlockAlpha::Full),
            // [🭰] VERTICAL ONE EIGHTH BLOCK-2
            0x1fb70 => Self::Blocks(&[Block::VerticalBlock(1, 2)]),
            // [🭱] VERTICAL ONE EIGHTH BLOCK-3
            0x1fb71 => Self::Blocks(&[Block::VerticalBlock(2, 3)]),
            // [🭲] VERTICAL ONE EIGHTH BLOCK-4
            0x1fb72 => Self::Blocks(&[Block::VerticalBlock(3, 4)]),
            // [🭳] VERTICAL ONE EIGHTH BLOCK-5
            0x1fb73 => Self::Blocks(&[Block::VerticalBlock(4, 5)]),
            // [🭴] VERTICAL ONE EIGHTH BLOCK-6
            0x1fb74 => Self::Blocks(&[Block::VerticalBlock(5, 6)]),
            // [🭵] VERTICAL ONE EIGHTH BLOCK-7
            0x1fb75 => Self::Blocks(&[Block::VerticalBlock(6, 7)]),
            // [🭶] HORIZONTAL ONE EIGHTH BLOCK-2
            0x1fb76 => Self::Blocks(&[Block::HorizontalBlock(1, 2)]),
            // [🭷] HORIZONTAL ONE EIGHTH BLOCK-3
            0x1fb77 => Self::Blocks(&[Block::HorizontalBlock(2, 3)]),
            // [🭸] HORIZONTAL ONE EIGHTH BLOCK-4
            0x1fb78 => Self::Blocks(&[Block::HorizontalBlock(3, 4)]),
            // [🭹] HORIZONTAL ONE EIGHTH BLOCK-5
            0x1fb79 => Self::Blocks(&[Block::HorizontalBlock(4, 5)]),
            // [🭺] HORIZONTAL ONE EIGHTH BLOCK-6
            0x1fb7a => Self::Blocks(&[Block::HorizontalBlock(5, 6)]),
            // [🭻] HORIZONTAL ONE EIGHTH BLOCK-7
            0x1fb7b => Self::Blocks(&[Block::HorizontalBlock(6, 7)]),
            // [🭼] Left and lower one eighth block
            0x1fb7c => Self::Blocks(&[Block::LeftBlock(1), Block::LowerBlock(1)]),
            // [🭽] Left and upper one eighth block
            0x1fb7d => Self::Blocks(&[Block::LeftBlock(1), Block::UpperBlock(1)]),
            // [🭾] Right and upper one eighth block
            0x1fb7e => Self::Blocks(&[Block::RightBlock(1), Block::UpperBlock(1)]),
            // [🭿] Right and lower one eighth block
            0x1fb7f => Self::Blocks(&[Block::RightBlock(1), Block::LowerBlock(1)]),
            // [🮀] UPPER AND LOWER ONE EIGHTH BLOCK
            0x1fb80 => Self::Blocks(&[Block::UpperBlock(1), Block::LowerBlock(1)]),
            // [🮁] HORIZONTAL ONE EIGHTH BLOCK-1358
            0x1fb81 => Self::Blocks(&[
                Block::UpperBlock(1),
                Block::HorizontalBlock(2, 3),
                Block::HorizontalBlock(4, 5),
                Block::LowerBlock(1),
            ]),
            // [🮂] Upper One Quarter Block
            0x1fb82 => Self::Blocks(&[Block::UpperBlock(2)]),
            // [🮃] Upper three eighths block
            0x1fb83 => Self::Blocks(&[Block::UpperBlock(3)]),
            // [🮄] Upper five eighths block
            0x1fb84 => Self::Blocks(&[Block::UpperBlock(5)]),
            // [🮅] Upper three quarters block
            0x1fb85 => Self::Blocks(&[Block::UpperBlock(6)]),
            // [🮆] Upper seven eighths block
            0x1fb86 => Self::Blocks(&[Block::UpperBlock(7)]),
            // [🮇] Right One Quarter Block
            0x1fb87 => Self::Blocks(&[Block::RightBlock(2)]),
            // [🮈] Right three eighths block
            0x1fb88 => Self::Blocks(&[Block::RightBlock(3)]),
            // [🮉] Right five eighths block
            0x1fb89 => Self::Blocks(&[Block::RightBlock(5)]),
            // [🮊] Right three quarters block
            0x1fb8a => Self::Blocks(&[Block::RightBlock(6)]),
            // [🮋] Right seven eighths block
            0x1fb8b => Self::Blocks(&[Block::RightBlock(7)]),
            // [🮌] LEFT HALF MEDIUM SHADE
            0x1fb8c => Self::Blocks(&[Block::Custom(0, 4, 0, 8, BlockAlpha::Medium)]),
            // [🮍] RIGHT HALF MEDIUM SHADE
            0x1fb8d => Self::Blocks(&[Block::Custom(4, 8, 0, 8, BlockAlpha::Medium)]),
            // [🮎] UPPER HALF MEDIUM SHADE
            0x1fb8e => Self::Blocks(&[Block::Custom(0, 8, 0, 4, BlockAlpha::Medium)]),
            // [🮏] LOWER HALF MEDIUM SHADE
            0x1fb8f => Self::Blocks(&[Block::Custom(0, 8, 4, 8, BlockAlpha::Medium)]),
            // [🮐] INVERSE MEDIUM SHADE
            0x1fb90 => Self::Blocks(&[Block::Custom(0, 8, 0, 8, BlockAlpha::Medium)]),
            // [🮑] UPPER HALF BLOCK AND LOWER HALF INVERSE MEDIUM SHADE
            0x1fb91 => Self::Blocks(&[
                Block::UpperBlock(4),
                Block::Custom(0, 8, 4, 8, BlockAlpha::Medium),
            ]),
            // [🮒] UPPER HALF INVERSE MEDIUM SHADE AND LOWER HALF BLOCK
            0x1fb92 => Self::Blocks(&[
                Block::Custom(0, 8, 0, 4, BlockAlpha::Medium),
                Block::LowerBlock(4),
            ]),
            // [🮓] LEFT HALF BLOCK AND RIGHT HALF INVERSE MEDIUM SHADE
            // NOTE: not official!
            0x1fb93 => Self::Blocks(&[
                Block::LeftBlock(4),
                Block::Custom(4, 8, 0, 8, BlockAlpha::Medium),
            ]),
            // [🮔] LEFT HALF INVERSE MEDIUM SHADE AND RIGHT HALF BLOCK
            0x1fb94 => Self::Blocks(&[
                Block::Custom(0, 4, 0, 8, BlockAlpha::Medium),
                Block::RightBlock(4),
            ]),
            // [🮕] CHECKER BOARD FILL
            0x1fb95 => Self::Blocks(&[
                Block::Custom(0, 2, 0, 2, BlockAlpha::Full),
                Block::Custom(0, 2, 4, 6, BlockAlpha::Full),
                Block::Custom(2, 4, 2, 4, BlockAlpha::Full),
                Block::Custom(2, 4, 6, 8, BlockAlpha::Full),
                Block::Custom(4, 6, 0, 2, BlockAlpha::Full),
                Block::Custom(4, 6, 4, 6, BlockAlpha::Full),
                Block::Custom(6, 8, 2, 4, BlockAlpha::Full),
                Block::Custom(6, 8, 6, 8, BlockAlpha::Full),
            ]),
            // [🮖] INVERSE CHECKER BOARD FILL
            0x1fb96 => Self::Blocks(&[
                Block::Custom(0, 2, 2, 4, BlockAlpha::Full),
                Block::Custom(0, 2, 6, 8, BlockAlpha::Full),
                Block::Custom(2, 4, 0, 2, BlockAlpha::Full),
                Block::Custom(2, 4, 4, 6, BlockAlpha::Full),
                Block::Custom(4, 6, 2, 4, BlockAlpha::Full),
                Block::Custom(4, 6, 6, 8, BlockAlpha::Full),
                Block::Custom(6, 8, 0, 2, BlockAlpha::Full),
                Block::Custom(6, 8, 4, 6, BlockAlpha::Full),
            ]),
            // [🮗] HEAVY HORIZONTAL FILL
            0x1fb97 => Self::Blocks(&[Block::HorizontalBlock(2, 4), Block::HorizontalBlock(6, 8)]),
            // [🮘] UPPER LEFT TO LOWER RIGHT FILL
            // NOTE: This is a quick placeholder which doesn't scale correctly
            0x1fb98 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(3, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(3, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(5, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(7, 10)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(9, 10)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(3, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(5, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(7, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(5, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(9, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
            ]),
            // [🮙] UPPER RIGHT TO LOWER LEFT FILL
            // NOTE: This is a quick placeholder which doesn't scale correctly
            0x1fb99 => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(1, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(5, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(3, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(3, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(5, 10)),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 6), BlockCoord::Zero),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(7, 10)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Frac(9, 10)),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(3, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(5, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(5, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(3, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(7, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 6), BlockCoord::One),
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(9, 10)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::OutlineThin,
                },
            ]),
            // [🮚] UPPER AND LOWER TRIANGULAR HALF BLOCK
            0x1fb9a => Self::Triangles(Triangle::UPPER | Triangle::LOWER, BlockAlpha::Full),
            // [🮛] LEFT AND RIGHT TRIANGULAR HALF BLOCK
            0x1fb9b => Self::Triangles(Triangle::LEFT | Triangle::RIGHT, BlockAlpha::Full),
            // [🮜] UPPER UPPER LEFT TRIANGULAR MEDIUM SHADE
            0x1fb9c => Self::Triangles(Triangle::LEFT | Triangle::UPPER, BlockAlpha::Medium),
            // [🮝] UPPER RIGHT TRIANGULAR MEDIUM SHADE
            0x1fb9d => Self::Triangles(Triangle::RIGHT | Triangle::UPPER, BlockAlpha::Medium),
            // [🮞] LOWER RIGHT TRIANGULAR MEDIUM SHADE
            0x1fb9e => Self::Triangles(Triangle::RIGHT | Triangle::LOWER, BlockAlpha::Medium),
            // [🮟] LOWER LEFT TRIANGULAR MEDIUM SHADE
            0x1fb9f => Self::Triangles(Triangle::LEFT | Triangle::LOWER, BlockAlpha::Medium),
            // [🮠] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE LEFT
            0x1fba0 => Self::CellDiagonals(CellDiagonal::UPPER_LEFT),
            // [🮡] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE RIGHT
            0x1fba1 => Self::CellDiagonals(CellDiagonal::UPPER_RIGHT),
            // [🮢] BOX DRAWINGS LIGHT DIAGONAL MIDDLE LEFT TO LOWER CENTRE
            0x1fba2 => Self::CellDiagonals(CellDiagonal::LOWER_LEFT),
            // [🮣] BOX DRAWINGS LIGHT DIAGONAL MIDDLE RIGHT TO LOWER CENTRE
            0x1fba3 => Self::CellDiagonals(CellDiagonal::LOWER_RIGHT),
            // [🮤] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE LEFT TO LOWER CENTRE
            0x1fba4 => Self::CellDiagonals(CellDiagonal::UPPER_LEFT | CellDiagonal::LOWER_LEFT),
            // [🮥] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE RIGHT TO LOWER CENTRE
            0x1fba5 => Self::CellDiagonals(CellDiagonal::UPPER_RIGHT | CellDiagonal::LOWER_RIGHT),
            // [🮦] BOX DRAWINGS LIGHT DIAGONAL MIDDLE LEFT TO LOWER CENTRE TO MIDDLE RIGHT
            0x1fba6 => Self::CellDiagonals(CellDiagonal::LOWER_LEFT | CellDiagonal::LOWER_RIGHT),
            // [🮧] BOX DRAWINGS LIGHT DIAGONAL MIDDLE LEFT TO UPPER CENTRE TO MIDDLE RIGHT
            0x1fba7 => Self::CellDiagonals(CellDiagonal::UPPER_LEFT | CellDiagonal::UPPER_RIGHT),
            // [🮨] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE LEFT AND MIDDLE RIGHT TO LOWER CENTRE
            0x1fba8 => Self::CellDiagonals(CellDiagonal::UPPER_LEFT | CellDiagonal::LOWER_RIGHT),
            // [🮩] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE RIGHT AND MIDDLE LEFT TO LOWER CENTRE
            0x1fba9 => Self::CellDiagonals(CellDiagonal::UPPER_RIGHT | CellDiagonal::LOWER_LEFT),
            // [🮪] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE RIGHT TO LOWER CENTRE TO MIDDLE LEFT
            0x1fbaa => Self::CellDiagonals(
                CellDiagonal::UPPER_RIGHT | CellDiagonal::LOWER_LEFT | CellDiagonal::LOWER_RIGHT,
            ),
            // [🮫] BOX DRAWINGS LIGHT DIAGONAL UPPER CENTRE TO MIDDLE LEFT TO LOWER CENTRE TO MIDDLE RIGHT
            0x1fbab => Self::CellDiagonals(
                CellDiagonal::UPPER_LEFT | CellDiagonal::LOWER_LEFT | CellDiagonal::LOWER_RIGHT,
            ),
            // [🮬] BOX DRAWINGS LIGHT DIAGONAL MIDDLE LEFT TO UPPER CENTRE TO MIDDLE RIGHT TO LOWER CENTRE
            0x1fbac => Self::CellDiagonals(
                CellDiagonal::UPPER_LEFT | CellDiagonal::UPPER_RIGHT | CellDiagonal::LOWER_RIGHT,
            ),
            // [🮭] BOX DRAWINGS LIGHT DIAGONAL MIDDLE RIGHT TO UPPER CENTRE TO MIDDLE LEFT TO LOWER CENTRE
            0x1fbad => Self::CellDiagonals(
                CellDiagonal::UPPER_LEFT | CellDiagonal::UPPER_RIGHT | CellDiagonal::LOWER_LEFT,
            ),
            // [🮮] BOX DRAWINGS LIGHT DIAGONAL DIAMOND
            0x1fbae => Self::CellDiagonals(
                CellDiagonal::UPPER_LEFT
                    | CellDiagonal::UPPER_RIGHT
                    | CellDiagonal::LOWER_LEFT
                    | CellDiagonal::LOWER_RIGHT,
            ),
            // [🮯] BOX DRAWINGS LIGHT HORIZONTAL WITH VERTICAL STROKE
            0x1fbaf => Self::Poly(&[
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
                        PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
                Poly {
                    path: &[
                        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                        PolyCommand::Close,
                    ],
                    intensity: BlockAlpha::Full,
                    style: PolyStyle::Outline,
                },
            ]),

            // Braille dot patterns
            // ⠀ ⠁ ⠂ ⠃ ⠄ ⠅ ⠆ ⠇ ⠈ ⠉ ⠊ ⠋ ⠌ ⠍ ⠎ ⠏
            // ⠐ ⠑ ⠒ ⠓ ⠔ ⠕ ⠖ ⠗ ⠘ ⠙ ⠚ ⠛ ⠜ ⠝ ⠞ ⠟
            // ⠠ ⠡ ⠢ ⠣ ⠤ ⠥ ⠦ ⠧ ⠨ ⠩ ⠪ ⠫ ⠬ ⠭ ⠮ ⠯
            // ⠰ ⠱ ⠲ ⠳ ⠴ ⠵ ⠶ ⠷ ⠸ ⠹ ⠺ ⠻ ⠼ ⠽ ⠾ ⠿
            // ⡀ ⡁ ⡂ ⡃ ⡄ ⡅ ⡆ ⡇ ⡈ ⡉ ⡊ ⡋ ⡌ ⡍ ⡎ ⡏
            // ⡐ ⡑ ⡒ ⡓ ⡔ ⡕ ⡖ ⡗ ⡘ ⡙ ⡚ ⡛ ⡜ ⡝ ⡞ ⡟
            // ⡠ ⡡ ⡢ ⡣ ⡤ ⡥ ⡦ ⡧ ⡨ ⡩ ⡪ ⡫ ⡬ ⡭ ⡮ ⡯
            // ⡰ ⡱ ⡲ ⡳ ⡴ ⡵ ⡶ ⡷ ⡸ ⡹ ⡺ ⡻ ⡼ ⡽ ⡾ ⡿
            // ⢀ ⢁ ⢂ ⢃ ⢄ ⢅ ⢆ ⢇ ⢈ ⢉ ⢊ ⢋ ⢌ ⢍ ⢎ ⢏
            // ⢐ ⢑ ⢒ ⢓ ⢔ ⢕ ⢖ ⢗ ⢘ ⢙ ⢚ ⢛ ⢜ ⢝ ⢞ ⢟
            // ⢠ ⢡ ⢢ ⢣ ⢤ ⢥ ⢦ ⢧ ⢨ ⢩ ⢪ ⢫ ⢬ ⢭ ⢮ ⢯
            // ⢰ ⢱ ⢲ ⢳ ⢴ ⢵ ⢶ ⢷ ⢸ ⢹ ⢺ ⢻ ⢼ ⢽ ⢾ ⢿
            // ⣀ ⣁ ⣂ ⣃ ⣄ ⣅ ⣆ ⣇ ⣈ ⣉ ⣊ ⣋ ⣌ ⣍ ⣎ ⣏
            // ⣐ ⣑ ⣒ ⣓ ⣔ ⣕ ⣖ ⣗ ⣘ ⣙ ⣚ ⣛ ⣜ ⣝ ⣞ ⣟
            // ⣠ ⣡ ⣢ ⣣ ⣤ ⣥ ⣦ ⣧ ⣨ ⣩ ⣪ ⣫ ⣬ ⣭ ⣮ ⣯
            // ⣰ ⣱ ⣲ ⣳ ⣴ ⣵ ⣶ ⣷ ⣸ ⣹ ⣺ ⣻ ⣼ ⣽ ⣾ ⣿
            n @ 0x2800..=0x28ff => Self::Braille((n & 0xff) as u8),
            // [] Powerline filled right arrow
            0xe0b0 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [] Powerline outline right arrow
            0xe0b1 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [] Powerline filled left arrow
            0xe0b2 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [] Powerline outline left arrow
            0xe0b3 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),

            // [] Powerline filled left semicircle
            0xe0b4 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::One, BlockCoord::Zero),
                        to: (BlockCoord::One, BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::QuadTo {
                        control: (BlockCoord::One, BlockCoord::One),
                        to: (BlockCoord::Zero, BlockCoord::One),
                    },
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [] Powerline outline left semicircle
            0xe0b5 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(-1, 4), BlockCoord::Frac(-1, 3)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(7, 4), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(-1, 4), BlockCoord::Frac(4, 3)),
                    },
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [] Powerline filled right semicircle
            0xe0b6 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Zero, BlockCoord::Zero),
                        to: (BlockCoord::Zero, BlockCoord::Frac(1, 2)),
                    },
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Zero, BlockCoord::One),
                        to: (BlockCoord::One, BlockCoord::One),
                    },
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [] Powerline outline right semicircle
            0xe0b7 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Frac(5, 4), BlockCoord::Frac(-1, 3)),
                    PolyCommand::QuadTo {
                        control: (BlockCoord::Frac(-3, 4), BlockCoord::Frac(1, 2)),
                        to: (BlockCoord::Frac(5, 4), BlockCoord::Frac(4, 3)),
                    },
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),

            // [] Powerline filled bottom left half triangle
            0xe0b8 => Self::Triangles(Triangle::LEFT | Triangle::LOWER, BlockAlpha::Full),
            // [] Powerline outline bottom left half triangle
            0xe0b9 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [] Powerline filled bottom right half triangle
            0xe0ba => Self::Triangles(Triangle::RIGHT | Triangle::LOWER, BlockAlpha::Full),
            // [] Powerline outline bottom right half triangle
            0xe0bb => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [] Powerline filled top left half triangle
            0xe0bc => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
            // [] Powerline outline top left half triangle
            0xe0bd => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            // [] Powerline filled top right half triangle
            0xe0be => Self::Triangles(Triangle::RIGHT | Triangle::UPPER, BlockAlpha::Full),
            // [] Powerline outline top right half triangle
            0xe0bf => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Outline,
            }]),
            _ => return None,
        })
    }

    pub fn from_cell_iter(cell: termwiz::surface::line::CellRef) -> Option<Self> {
        let mut chars = cell.str().chars();
        let first_char = chars.next()?;
        if chars.next().is_some() {
            None
        } else {
            Self::from_char(first_char)
        }
    }

    pub fn from_cell(cell: &termwiz::cell::Cell) -> Option<Self> {
        let mut chars = cell.str().chars();
        let first_char = chars.next()?;
        if chars.next().is_some() {
            None
        } else {
            Self::from_char(first_char)
        }
    }
}

impl GlyphCache {
    fn draw_polys(
        &mut self,
        metrics: &RenderMetrics,
        polys: &[Poly],
        buffer: &mut Image,
        aa: PolyAA,
    ) {
        let (width, height) = buffer.image_dimensions();
        let mut pixmap =
            PixmapMut::from_bytes(buffer.pixel_data_slice_mut(), width as u32, height as u32)
                .expect("make pixmap from existing bitmap");

        for Poly {
            path,
            intensity,
            style,
        } in polys
        {
            let mut paint = Paint::default();
            let intensity = intensity.to_scale();
            paint.set_color(
                tiny_skia::Color::from_rgba(intensity, intensity, intensity, intensity).unwrap(),
            );
            paint.anti_alias = match aa {
                PolyAA::AntiAlias => true,
                PolyAA::MoarPixels => false,
            };
            paint.force_hq_pipeline = true;
            let mut pb = PathBuilder::new();
            for item in path.iter() {
                item.to_skia(width, height, metrics.underline_height as f32, &mut pb);
            }
            let path = pb.finish().expect("poly path to be valid");
            style.apply(metrics.underline_height as f32, &paint, &path, &mut pixmap);
        }
    }

    pub fn cursor_sprite(
        &mut self,
        shape: Option<CursorShape>,
        metrics: &RenderMetrics,
        width: u8,
    ) -> anyhow::Result<Sprite> {
        if let Some(sprite) = self.cursor_glyphs.get(&(shape, width)) {
            return Ok(sprite.clone());
        }

        let mut metrics = metrics.scale_cell_width(width as f64);
        if let Some(d) = &self.fonts.config().cursor_thickness {
            metrics.underline_height = d.evaluate_as_pixels(DimensionContext {
                dpi: self.fonts.get_dpi() as f32,
                pixel_max: metrics.underline_height as f32,
                pixel_cell: metrics.cell_size.height as f32,
            }) as isize;
        }

        let mut buffer = Image::new(
            metrics.cell_size.width as usize,
            metrics.cell_size.height as usize,
        );
        let black = SrgbaPixel::rgba(0, 0, 0, 0);
        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);
        buffer.clear_rect(cell_rect, black);

        match shape {
            None => {}
            Some(CursorShape::Default) => {
                buffer.clear_rect(cell_rect, SrgbaPixel::rgba(0xff, 0xff, 0xff, 0xff));
            }
            Some(CursorShape::BlinkingBlock | CursorShape::SteadyBlock) => {
                self.draw_polys(
                    &metrics,
                    &[Poly {
                        path: &[
                            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Zero),
                        ],
                        intensity: BlockAlpha::Full,
                        style: PolyStyle::OutlineHeavy,
                    }],
                    &mut buffer,
                    PolyAA::AntiAlias,
                );
            }
            Some(CursorShape::BlinkingBar | CursorShape::SteadyBar) => {
                self.draw_polys(
                    &metrics,
                    &[Poly {
                        path: &[
                            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                        ],
                        intensity: BlockAlpha::Full,
                        style: PolyStyle::OutlineHeavy,
                    }],
                    &mut buffer,
                    PolyAA::AntiAlias,
                );
            }
            Some(CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline) => {
                self.draw_polys(
                    &metrics,
                    &[Poly {
                        path: &[
                            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                        ],
                        intensity: BlockAlpha::Full,
                        style: PolyStyle::OutlineHeavy,
                    }],
                    &mut buffer,
                    PolyAA::AntiAlias,
                );
            }
        }

        let sprite = self.atlas.allocate(&buffer)?;
        self.cursor_glyphs.insert((shape, width), sprite.clone());
        Ok(sprite)
    }

    pub fn block_sprite(
        &mut self,
        render_metrics: &RenderMetrics,
        key: SizedBlockKey,
    ) -> anyhow::Result<Sprite> {
        let metrics = match &key.block {
            BlockKey::PolyWithCustomMetrics {
                underline_height,
                cell_size,
                ..
            } => RenderMetrics {
                descender: PixelLength::new(0.),
                descender_row: 0,
                descender_plus_two: 0,
                underline_height: *underline_height,
                strike_row: 0,
                cell_size: cell_size.clone(),
            },
            _ => render_metrics.clone(),
        };

        let mut buffer = Image::new(
            metrics.cell_size.width as usize,
            metrics.cell_size.height as usize,
        );
        let black = SrgbaPixel::rgba(0, 0, 0, 0);

        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);

        buffer.clear_rect(cell_rect, black);

        match key.block {
            BlockKey::Blocks(blocks) => {
                let width = metrics.cell_size.width as f32;
                let height = metrics.cell_size.height as f32;
                let (x_half, y_half, y_third) = (width / 2., height / 2., height / 3.);
                let (x_eighth, y_eighth) = (width / 8., height / 8.);

                for block in blocks.iter() {
                    match block {
                        Block::Custom(x0, x1, y0, y1, alpha) => {
                            let left = (*x0 as f32) * x_eighth;
                            let right = (*x1 as f32) * x_eighth;
                            let top = (*y0 as f32) * y_eighth;
                            let bottom = (*y1 as f32) * y_eighth;
                            fill_rect(&mut buffer, left..right, top..bottom, *alpha);
                        }
                        Block::UpperBlock(num) => {
                            let lower = (*num as f32) * y_eighth;
                            fill_rect(&mut buffer, 0.0..width, 0.0..lower, BlockAlpha::Full);
                        }
                        Block::LowerBlock(num) => {
                            let upper = ((8 - num) as f32) * y_eighth;
                            fill_rect(&mut buffer, 0.0..width, upper..height, BlockAlpha::Full);
                        }
                        Block::LeftBlock(num) => {
                            let right = (*num as f32) * x_eighth;
                            fill_rect(&mut buffer, 0.0..right, 0.0..height, BlockAlpha::Full);
                        }
                        Block::RightBlock(num) => {
                            let left = ((8 - num) as f32) * x_eighth;
                            fill_rect(&mut buffer, left..width, 0.0..height, BlockAlpha::Full);
                        }
                        Block::VerticalBlock(x0, x1) => {
                            let left = (*x0 as f32) * x_eighth;
                            let right = (*x1 as f32) * x_eighth;
                            fill_rect(&mut buffer, left..right, 0.0..height, BlockAlpha::Full);
                        }
                        Block::HorizontalBlock(y0, y1) => {
                            let top = (*y0 as f32) * y_eighth;
                            let bottom = (*y1 as f32) * y_eighth;
                            fill_rect(&mut buffer, 0.0..width, top..bottom, BlockAlpha::Full);
                        }
                        Block::QuadrantUL => {
                            fill_rect(&mut buffer, 0.0..x_half, 0.0..y_half, BlockAlpha::Full)
                        }
                        Block::QuadrantUR => {
                            fill_rect(&mut buffer, x_half..width, 0.0..y_half, BlockAlpha::Full)
                        }
                        Block::QuadrantLL => {
                            fill_rect(&mut buffer, 0.0..x_half, y_half..height, BlockAlpha::Full)
                        }
                        Block::QuadrantLR => {
                            fill_rect(&mut buffer, x_half..width, y_half..height, BlockAlpha::Full)
                        }
                        Block::Sextant1 => {
                            fill_rect(&mut buffer, 0.0..x_half, 0.0..y_third, BlockAlpha::Full)
                        }
                        Block::Sextant2 => {
                            fill_rect(&mut buffer, x_half..width, 0.0..y_third, BlockAlpha::Full)
                        }
                        Block::Sextant3 => fill_rect(
                            &mut buffer,
                            0.0..x_half,
                            y_third..(y_third * 2.),
                            BlockAlpha::Full,
                        ),
                        Block::Sextant4 => fill_rect(
                            &mut buffer,
                            x_half..width,
                            y_third..(y_third * 2.),
                            BlockAlpha::Full,
                        ),
                        Block::Sextant5 => fill_rect(
                            &mut buffer,
                            0.0..x_half,
                            (y_third * 2.)..height,
                            BlockAlpha::Full,
                        ),
                        Block::Sextant6 => fill_rect(
                            &mut buffer,
                            x_half..width,
                            (y_third * 2.)..height,
                            BlockAlpha::Full,
                        ),
                    }
                }
            }
            BlockKey::Triangles(triangles, alpha) => {
                let mut draw = |cmd: &'static [PolyCommand], style: PolyStyle| {
                    self.draw_polys(
                        &metrics,
                        &[Poly {
                            path: cmd,
                            intensity: alpha,
                            style: style,
                        }],
                        &mut buffer,
                        if config::configuration().anti_alias_custom_block_glyphs {
                            PolyAA::AntiAlias
                        } else {
                            PolyAA::MoarPixels
                        },
                    );
                };

                macro_rules! start {
                    () => {
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Frac(1, 2))
                    };
                }
                macro_rules! close {
                    () => {
                        PolyCommand::Close
                    };
                }
                macro_rules! p0 {
                    () => {
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Zero)
                    };
                }
                macro_rules! p1 {
                    () => {
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero)
                    };
                }
                macro_rules! p2 {
                    () => {
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One)
                    };
                }
                macro_rules! p3 {
                    () => {
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One)
                    };
                }

                // Draw triangles
                if triangles.contains(Triangle::UPPER) {
                    draw(&[start!(), p0!(), p1!(), close!()], PolyStyle::Fill);
                }
                if triangles.contains(Triangle::LOWER) {
                    draw(&[start!(), p2!(), p3!(), close!()], PolyStyle::Fill);
                }
                if triangles.contains(Triangle::LEFT) {
                    draw(&[start!(), p0!(), p2!(), close!()], PolyStyle::Fill);
                }
                if triangles.contains(Triangle::RIGHT) {
                    draw(&[start!(), p1!(), p3!(), close!()], PolyStyle::Fill);
                }

                // Fill antialiased lines between triangles
                let style = if alpha == BlockAlpha::Full {
                    PolyStyle::Outline
                } else {
                    PolyStyle::OutlineAlpha
                };
                if triangles.contains(Triangle::UPPER | Triangle::LEFT) {
                    draw(&[start!(), p0!()], style);
                }
                if triangles.contains(Triangle::UPPER | Triangle::RIGHT) {
                    draw(&[start!(), p1!()], style);
                }
                if triangles.contains(Triangle::LOWER | Triangle::LEFT) {
                    draw(&[start!(), p2!()], style);
                }
                if triangles.contains(Triangle::LOWER | Triangle::RIGHT) {
                    draw(&[start!(), p3!()], style);
                }
            }
            BlockKey::CellDiagonals(diagonals) => {
                let mut draw = |cmd: &'static [PolyCommand]| {
                    self.draw_polys(
                        &metrics,
                        &[Poly {
                            path: cmd,
                            intensity: BlockAlpha::Full,
                            style: PolyStyle::Outline,
                        }],
                        &mut buffer,
                        if config::configuration().anti_alias_custom_block_glyphs {
                            PolyAA::AntiAlias
                        } else {
                            PolyAA::MoarPixels
                        },
                    );
                };

                macro_rules! U {
                    () => {
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero)
                    };
                }
                macro_rules! D {
                    () => {
                        PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::One)
                    };
                }
                macro_rules! L {
                    () => {
                        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Frac(1, 2))
                    };
                }
                macro_rules! R {
                    () => {
                        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2))
                    };
                }

                if diagonals.contains(CellDiagonal::UPPER_LEFT) {
                    draw(&[U!(), L!()]);
                }
                if diagonals.contains(CellDiagonal::UPPER_RIGHT) {
                    draw(&[U!(), R!()]);
                }
                if diagonals.contains(CellDiagonal::LOWER_LEFT) {
                    draw(&[D!(), L!()]);
                }
                if diagonals.contains(CellDiagonal::LOWER_RIGHT) {
                    draw(&[D!(), R!()]);
                }
            }
            BlockKey::Braille(dots_pattern) => {
                // `dots_pattern` is a byte whose bits corresponds to dots
                // on a 2 by 4 dots-grid.
                // The position of a dot for a bit position (1-indexed) is as follow:
                // 1 4  |
                // 2 5  |<- These 3 lines are filled first (for the first 64 symbols)
                // 3 6  |
                // 7 8  <- This last line is filled last (for the remaining 192 symbols)
                //
                // NOTE: for simplicity & performance reasons, a dot is a square not a circle.

                let dot_area_width = metrics.cell_size.width as f32 / 2.;
                let dot_area_height = metrics.cell_size.height as f32 / 4.;
                let square_length = dot_area_width / 2.;
                let topleft_offset_x = dot_area_width / 2. - square_length / 2.;
                let topleft_offset_y = dot_area_height / 2. - square_length / 2.;

                let (width, height) = buffer.image_dimensions();
                let mut pixmap = PixmapMut::from_bytes(
                    buffer.pixel_data_slice_mut(),
                    width as u32,
                    height as u32,
                )
                .expect("make pixmap from existing bitmap");
                let mut paint = Paint::default();
                paint.set_color(tiny_skia::Color::WHITE);
                paint.force_hq_pipeline = true;
                paint.anti_alias = true;
                let identity = Transform::identity();

                const BIT_MASK_AND_DOT_POSITION: [(u8, f32, f32); 8] = [
                    (1 << 0, 0., 0.),
                    (1 << 1, 0., 1.),
                    (1 << 2, 0., 2.),
                    (1 << 3, 1., 0.),
                    (1 << 4, 1., 1.),
                    (1 << 5, 1., 2.),
                    (1 << 6, 0., 3.),
                    (1 << 7, 1., 3.),
                ];
                for (bit_mask, dot_pos_x, dot_pos_y) in &BIT_MASK_AND_DOT_POSITION {
                    if dots_pattern & bit_mask == 0 {
                        // Bit for this dot position is not set
                        continue;
                    }
                    let topleft_x = (*dot_pos_x) * dot_area_width + topleft_offset_x;
                    let topleft_y = (*dot_pos_y) * dot_area_height + topleft_offset_y;

                    let path = PathBuilder::from_rect(
                        tiny_skia::Rect::from_xywh(
                            topleft_x,
                            topleft_y,
                            square_length,
                            square_length,
                        )
                        .expect("valid rect"),
                    );
                    pixmap.fill_path(&path, &paint, FillRule::Winding, identity, None);
                }
            }
            BlockKey::Poly(polys) | BlockKey::PolyWithCustomMetrics { polys, .. } => {
                self.draw_polys(
                    &metrics,
                    polys,
                    &mut buffer,
                    if config::configuration().anti_alias_custom_block_glyphs {
                        PolyAA::AntiAlias
                    } else {
                        PolyAA::MoarPixels
                    },
                );
            }
        }

        /*
        log::info!("{:?}", block);
        buffer.log_bits();
        */

        let sprite = self.atlas.allocate(&buffer)?;
        self.block_glyphs.insert(key, sprite.clone());
        Ok(sprite)
    }
}

// Fill a rectangular region described by the x and y ranges
fn fill_rect(buffer: &mut Image, x: Range<f32>, y: Range<f32>, intensity: BlockAlpha) {
    let (width, height) = buffer.image_dimensions();
    let mut pixmap =
        PixmapMut::from_bytes(buffer.pixel_data_slice_mut(), width as u32, height as u32)
            .expect("make pixmap from existing bitmap");

    let path = PathBuilder::from_rect(
        tiny_skia::Rect::from_xywh(x.start, y.start, x.end - x.start, y.end - y.start)
            .expect("valid rect"),
    );

    let mut paint = Paint::default();
    let intensity = intensity.to_scale();
    paint.set_color(
        tiny_skia::Color::from_rgba(intensity, intensity, intensity, intensity).unwrap(),
    );
    paint.anti_alias = false;
    paint.force_hq_pipeline = true;

    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}
