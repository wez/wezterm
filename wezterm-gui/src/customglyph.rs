use crate::glyphcache::{GlyphCache, SizedBlockKey};
use crate::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::Sprite;
use ::window::color::{LinearRgba, SrgbaPixel};
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
    pub struct Quadrant: u8{
        const UPPER_LEFT = 1<<1;
        const UPPER_RIGHT = 1<<2;
        const LOWER_LEFT = 1<<3;
        const LOWER_RIGHT = 1<<4;
    }
}

bitflags::bitflags! {
    pub struct Sextant: u8{
        /// Upper-left
        const ONE = 1<<1;
        /// Upper-right
        const TWO = 1<<2;
        /// Middle left
        const THREE = 1<<3;
        /// Middle Right
        const FOUR = 1<<4;
        /// Lower left
        const FIVE = 1<<5;
        /// Lower right
        const SIX = 1<<6;
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

/// Represents a Block Element glyph, decoded from
/// <https://en.wikipedia.org/wiki/Block_Elements>
/// <https://www.unicode.org/charts/PDF/U2580.pdf>
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BlockKey {
    /// Number of 1/8ths in the upper half
    Upper(u8),
    /// Number of 1/8ths in the lower half
    Lower(u8),
    /// Number of 1/8ths in the left half
    Left(u8),
    /// Number of 1/8ths in the right half
    Right(u8),
    /// Full block with alpha level
    Full(BlockAlpha),
    /// A combination of quadrants
    Quadrants(Quadrant),
    /// A combination of sextants <https://unicode.org/charts/PDF/U1FB00.pdf>
    Sextants(Sextant),
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
    QuadTo { control: BlockPoint, to: BlockPoint },
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
            Self::Close => pb.close(),
        };
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PolyStyle {
    Fill,
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

            PolyStyle::Outline | PolyStyle::OutlineHeavy => {
                let mut stroke = Stroke::default();
                stroke.width = width;
                if self == PolyStyle::OutlineHeavy {
                    stroke.width *= 3.0; // NOTE: Using 2.0, the difference is almost invisible
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
            0x2580 => Self::Upper(4),
            // LOWER 1..7 EIGHTH BLOCK
            // [▁] [▂] [▃] [▄] [▅] [▆] [▇]
            0x2581..=0x2587 => Self::Lower((c - 0x2580) as u8),
            // [█] FULL BLOCK
            0x2588 => Self::Full(BlockAlpha::Full),
            // LEFT 7..1 EIGHTHS BLOCK
            // [▉] [▊] [▋] [▌] [▍] [▎] [▏]
            0x2589..=0x258f => Self::Left((0x2590 - c) as u8),
            // [▐] RIGHT HALF BLOCK
            0x2590 => Self::Right(4),
            // [░] LIGHT SHADE
            0x2591 => Self::Full(BlockAlpha::Light),
            // [▒] MEDIUM SHADE
            0x2592 => Self::Full(BlockAlpha::Medium),
            // [▓] DARK SHADE
            0x2593 => Self::Full(BlockAlpha::Dark),
            // [▔] UPPER ONE EIGHTH BLOCK
            0x2594 => Self::Upper(1),
            // [▕] RIGHT ONE EIGHTH BLOCK
            0x2595 => Self::Right(1),
            // [▖] QUADRANT LOWER LEFT
            0x2596 => Self::Quadrants(Quadrant::LOWER_LEFT),
            // [▗] QUADRANT LOWER RIGHT
            0x2597 => Self::Quadrants(Quadrant::LOWER_RIGHT),
            // [▘] QUADRANT UPPER LEFT
            0x2598 => Self::Quadrants(Quadrant::UPPER_LEFT),
            // [▙] QUADRANT UPPER LEFT AND LOWER LEFT AND LOWER RIGHT
            0x2599 => {
                Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::LOWER_LEFT | Quadrant::LOWER_RIGHT)
            }
            // [▚] QUADRANT UPPER LEFT AND LOWER RIGHT
            0x259a => Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::LOWER_RIGHT),
            // [▛] QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT
            0x259b => {
                Self::Quadrants(Quadrant::UPPER_LEFT | Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT)
            }
            // [▜] QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER RIGHT
            0x259c => Self::Quadrants(
                Quadrant::UPPER_LEFT | Quadrant::UPPER_RIGHT | Quadrant::LOWER_RIGHT,
            ),
            // [▝] QUADRANT UPPER RIGHT
            0x259d => Self::Quadrants(Quadrant::UPPER_RIGHT),
            // [▞] QUADRANT UPPER RIGHT AND LOWER LEFT
            0x259e => Self::Quadrants(Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT),
            // [▟] QUADRANT UPPER RIGHT AND LOWER LEFT AND LOWER RIGHT
            0x259f => Self::Quadrants(
                Quadrant::UPPER_RIGHT | Quadrant::LOWER_LEFT | Quadrant::LOWER_RIGHT,
            ),
            // [🬀] BLOCK SEXTANT-1
            0x1fb00 => Self::Sextants(Sextant::ONE),
            // [🬁] BLOCK SEXTANT-2
            0x1fb01 => Self::Sextants(Sextant::TWO),
            // [🬂] BLOCK SEXTANT-12
            0x1fb02 => Self::Sextants(Sextant::ONE | Sextant::TWO),
            // [🬃] BLOCK SEXTANT-3
            0x1fb03 => Self::Sextants(Sextant::THREE),
            // [🬄] BLOCK SEXTANT-13
            0x1fb04 => Self::Sextants(Sextant::ONE | Sextant::THREE),
            // [🬅] BLOCK SEXTANT-23
            0x1fb05 => Self::Sextants(Sextant::TWO | Sextant::THREE),
            // [🬆] BLOCK SEXTANT-123
            0x1fb06 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE),
            // [🬇] BLOCK SEXTANT-4
            0x1fb07 => Self::Sextants(Sextant::FOUR),
            // [🬈] BLOCK SEXTANT-14
            0x1fb08 => Self::Sextants(Sextant::ONE | Sextant::FOUR),
            // [🬉] BLOCK SEXTANT-24
            0x1fb09 => Self::Sextants(Sextant::TWO | Sextant::FOUR),
            // [🬊] BLOCK SEXTANT-124
            0x1fb0a => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR),
            // [🬋] BLOCK SEXTANT-34
            0x1fb0b => Self::Sextants(Sextant::THREE | Sextant::FOUR),
            // [🬌] BLOCK SEXTANT-134
            0x1fb0c => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR),
            // [🬍] BLOCK SEXTANT-234
            0x1fb0d => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR),
            // [🬎] BLOCK SEXTANT-1234
            0x1fb0e => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR),
            // [🬏] BLOCK SEXTANT-5
            0x1fb0f => Self::Sextants(Sextant::FIVE),
            // [🬐] BLOCK SEXTANT-15
            0x1fb10 => Self::Sextants(Sextant::ONE | Sextant::FIVE),
            // [🬑] BLOCK SEXTANT-25
            0x1fb11 => Self::Sextants(Sextant::TWO | Sextant::FIVE),
            // [🬒] BLOCK SEXTANT-125
            0x1fb12 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FIVE),
            // [🬓] BLOCK SEXTANT-35
            0x1fb13 => Self::Sextants(Sextant::THREE | Sextant::FIVE),
            // [🬔] BLOCK SEXTANT-235
            0x1fb14 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FIVE),
            // [🬕] BLOCK SEXTANT-1235
            0x1fb15 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FIVE),
            // [🬖] BLOCK SEXTANT-45
            0x1fb16 => Self::Sextants(Sextant::FOUR | Sextant::FIVE),
            // [🬗] BLOCK SEXTANT-145
            0x1fb17 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::FIVE),
            // [🬘] BLOCK SEXTANT-245
            0x1fb18 => Self::Sextants(Sextant::TWO | Sextant::FOUR | Sextant::FIVE),
            // [🬙] BLOCK SEXTANT-1245
            0x1fb19 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::FIVE),
            // [🬚] BLOCK SEXTANT-345
            0x1fb1a => Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::FIVE),
            // [🬛] BLOCK SEXTANT-1345
            0x1fb1b => {
                Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::FIVE)
            }
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
            // [🬜] BLOCK SEXTANT-2345
            0x1fb1c => {
                Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE)
            }
            // [🬝] BLOCK SEXTANT-12345
            0x1fb1d => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE,
            ),
            // [🬞] BLOCK SEXTANT-6
            0x1fb1e => Self::Sextants(Sextant::SIX),
            // [🬟] BLOCK SEXTANT-16
            0x1fb1f => Self::Sextants(Sextant::ONE | Sextant::SIX),
            // [🬠] BLOCK SEXTANT-26
            0x1fb20 => Self::Sextants(Sextant::TWO | Sextant::SIX),
            // [🬡] BLOCK SEXTANT-126
            0x1fb21 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::SIX),
            // [🬢] BLOCK SEXTANT-36
            0x1fb22 => Self::Sextants(Sextant::THREE | Sextant::SIX),
            // [🬣] BLOCK SEXTANT-136
            0x1fb23 => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::SIX),
            // [🬤] BLOCK SEXTANT-236
            0x1fb24 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::SIX),
            // [🬥] BLOCK SEXTANT-1236
            0x1fb25 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::SIX),
            // [🬦] BLOCK SEXTANT-46
            0x1fb26 => Self::Sextants(Sextant::FOUR | Sextant::SIX),
            // [🬧] BLOCK SEXTANT-146
            0x1fb27 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::SIX),
            // [🬨] BLOCK SEXTANT-1246
            0x1fb28 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::SIX),
            // [🬩] BLOCK SEXTANT-346
            0x1fb29 => Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            // [🬪] BLOCK SEXTANT-1346
            0x1fb2a => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            // [🬫] BLOCK SEXTANT-2346
            0x1fb2b => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::SIX),
            // [🬬] BLOCK SEXTANT-12346
            0x1fb2c => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::SIX,
            ),
            // [🬭] BLOCK SEXTANT-56
            0x1fb2d => Self::Sextants(Sextant::FIVE | Sextant::SIX),
            // [🬮] BLOCK SEXTANT-156
            0x1fb2e => Self::Sextants(Sextant::ONE | Sextant::FIVE | Sextant::SIX),
            // [🬯] BLOCK SEXTANT-256
            0x1fb2f => Self::Sextants(Sextant::TWO | Sextant::FIVE | Sextant::SIX),
            // [🬰] BLOCK SEXTANT-1256
            0x1fb30 => Self::Sextants(Sextant::ONE | Sextant::TWO | Sextant::FIVE | Sextant::SIX),
            // [🬱] BLOCK SEXTANT-356
            0x1fb31 => Self::Sextants(Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            // [🬲] BLOCK SEXTANT-1356
            0x1fb32 => Self::Sextants(Sextant::ONE | Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            // [🬳] BLOCK SEXTANT-2356
            0x1fb33 => Self::Sextants(Sextant::TWO | Sextant::THREE | Sextant::FIVE | Sextant::SIX),
            // [🬴] BLOCK SEXTANT-12356
            0x1fb34 => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::THREE | Sextant::FIVE | Sextant::SIX,
            ),
            // [🬵] BLOCK SEXTANT-456
            0x1fb35 => Self::Sextants(Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            // [🬶] BLOCK SEXTANT-1456
            0x1fb36 => Self::Sextants(Sextant::ONE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            // [🬷] BLOCK SEXTANT-2456
            0x1fb37 => Self::Sextants(Sextant::TWO | Sextant::FOUR | Sextant::FIVE | Sextant::SIX),
            // [🬸] BLOCK SEXTANT-12456
            0x1fb38 => Self::Sextants(
                Sextant::ONE | Sextant::TWO | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
            // [🬹] BLOCK SEXTANT-3456
            0x1fb39 => {
                Self::Sextants(Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX)
            }
            // [🬺] BLOCK SEXTANT-13456
            0x1fb3a => Self::Sextants(
                Sextant::ONE | Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
            // [🬻] BLOCK SEXTANT-23456
            0x1fb3b => Self::Sextants(
                Sextant::TWO | Sextant::THREE | Sextant::FOUR | Sextant::FIVE | Sextant::SIX,
            ),
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
            // [🮂] Upper One Quarter Block
            0x1fb82 => Self::Upper(2),
            // [🮃] Upper three eighths block
            0x1fb83 => Self::Upper(3),
            // [🮄] Upper five eighths block
            0x1fb84 => Self::Upper(5),
            // [🮅] Upper three quarters block
            0x1fb85 => Self::Upper(6),
            // [🮆] Upper seven eighths block
            0x1fb86 => Self::Upper(7),
            // [🮇] Right One Quarter Block
            0x1fb87 => Self::Right(2),
            // [🮈] Right three eighths block
            0x1fb88 => Self::Right(3),
            // [🮉] Right five eighths block
            0x1fb89 => Self::Right(5),
            // [🮊] Right three quarters block
            0x1fb8a => Self::Right(6),
            // [🮋] Right seven eighths block
            0x1fb8b => Self::Right(7),

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
            0xe0b8 => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
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
            0xe0ba => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
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
            0xe0be => Self::Poly(&[Poly {
                path: &[
                    PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
                    PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
                    PolyCommand::Close,
                ],
                intensity: BlockAlpha::Full,
                style: PolyStyle::Fill,
            }]),
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
            BlockKey::Upper(num) => {
                let lower = metrics.cell_size.height as f32 * (num as f32) / 8.;
                let width = metrics.cell_size.width as usize;
                fill_rect(&mut buffer, 0..width, 0..scale(lower));
            }
            BlockKey::Lower(num) => {
                let upper = metrics.cell_size.height as f32 * ((8 - num) as f32) / 8.;
                let width = metrics.cell_size.width as usize;
                let height = metrics.cell_size.height as usize;
                fill_rect(&mut buffer, 0..width, scale(upper)..height);
            }
            BlockKey::Left(num) => {
                let width = metrics.cell_size.width as f32 * (num as f32) / 8.;
                let height = metrics.cell_size.height as usize;
                fill_rect(&mut buffer, 0..scale(width), 0..height);
            }
            BlockKey::Right(num) => {
                let left = metrics.cell_size.width as f32 * ((8 - num) as f32) / 8.;
                let width = metrics.cell_size.width as usize;
                let height = metrics.cell_size.height as usize;
                fill_rect(&mut buffer, scale(left)..width, 0..height);
            }
            BlockKey::Full(alpha) => {
                let alpha = alpha.to_scale();
                let fill = LinearRgba::with_components(alpha, alpha, alpha, alpha);

                buffer.clear_rect(cell_rect, fill.srgba_pixel());
            }
            BlockKey::Quadrants(quads) => {
                let y_half = metrics.cell_size.height as f32 / 2.;
                let x_half = metrics.cell_size.width as f32 / 2.;
                let width = metrics.cell_size.width as usize;
                let height = metrics.cell_size.height as usize;
                if quads.contains(Quadrant::UPPER_LEFT) {
                    fill_rect(&mut buffer, 0..scale(x_half), 0..scale(y_half));
                }
                if quads.contains(Quadrant::UPPER_RIGHT) {
                    fill_rect(&mut buffer, scale(x_half)..width, 0..scale(y_half));
                }
                if quads.contains(Quadrant::LOWER_LEFT) {
                    fill_rect(&mut buffer, 0..scale(x_half), scale(y_half)..height);
                }
                if quads.contains(Quadrant::LOWER_RIGHT) {
                    fill_rect(&mut buffer, scale(x_half)..width, scale(y_half)..height);
                }
            }
            BlockKey::Sextants(s) => {
                let y_third = metrics.cell_size.height as f32 / 3.;
                let x_half = metrics.cell_size.width as f32 / 2.;
                let width = metrics.cell_size.width as usize;
                let height = metrics.cell_size.height as usize;

                if s.contains(Sextant::ONE) {
                    fill_rect(&mut buffer, 0..scale(x_half), 0..scale(y_third));
                }
                if s.contains(Sextant::TWO) {
                    fill_rect(&mut buffer, scale(x_half)..width, 0..scale(y_third));
                }
                if s.contains(Sextant::THREE) {
                    fill_rect(
                        &mut buffer,
                        0..scale(x_half),
                        scale(y_third)..scale(y_third * 2.),
                    );
                }
                if s.contains(Sextant::FOUR) {
                    fill_rect(
                        &mut buffer,
                        scale(x_half)..width,
                        scale(y_third)..scale(y_third * 2.),
                    );
                }
                if s.contains(Sextant::FIVE) {
                    fill_rect(&mut buffer, 0..scale(x_half), scale(y_third * 2.)..height);
                }
                if s.contains(Sextant::SIX) {
                    fill_rect(
                        &mut buffer,
                        scale(x_half)..width,
                        scale(y_third * 2.)..height,
                    );
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
fn fill_rect(buffer: &mut Image, x: Range<usize>, y: Range<usize>) {
    let (width, height) = buffer.image_dimensions();
    let mut pixmap =
        PixmapMut::from_bytes(buffer.pixel_data_slice_mut(), width as u32, height as u32)
            .expect("make pixmap from existing bitmap");

    let x = x.start as f32..x.end as f32;
    let y = y.start as f32..y.end as f32;

    let path = PathBuilder::from_rect(
        tiny_skia::Rect::from_xywh(x.start, y.start, x.end - x.start, y.end - y.start)
            .expect("valid rect"),
    );

    let mut paint = Paint::default();
    paint.set_color(tiny_skia::Color::WHITE);
    paint.force_hq_pipeline = true;

    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn scale(f: f32) -> usize {
    f.ceil().max(1.) as usize
}
