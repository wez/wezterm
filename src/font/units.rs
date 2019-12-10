use euclid::{Length, Scale};

pub type PixelUnit = window::PixelUnit;
pub struct FontUnit;
pub struct PointUnit;
pub struct EMUnit;

/// The length of a side of the imaginary EM square
/// which sets the resolution of a glyph.
/// eg: an EMSize of 2048 means that there are 2048
/// FontUnit's per em square.
/// 1em corresponds to the selected point size of a font.
pub type EMSize = Length<usize, EMUnit>;

/// A dimension expressed in font coordinates.
/// For example, if the EMSize is 2000 and we have a FontLength
/// of 500, it represents 500/2000 or 1/4 of the nominal size
/// of a glyph.
pub type FontLength = Length<isize, FontUnit>;

/// Describes a distance measured in points.
pub type PointLength = Length<f64, PointUnit>;

/// Returns the scaling factor required to convert from
/// points to pixels at a given dpi; multiply a `PointLength` by
/// this to produce a `PixelLength`
pub fn pixels_per_point(dpi: u32) -> Scale<f64, PointUnit, PixelUnit> {
    Scale::new(dpi as f64 / 72.)
}

/// Returns the scaling factor required to convert from
/// font units in a particular font to points.
pub fn units_per_em(units_per_em: EMSize) -> Scale<f64, FontUnit, PointUnit> {
    Scale::new(1.0 / units_per_em.get() as f64)
}

pub type PixelLength = euclid::Length<f64, PixelUnit>;
pub type IntPixelLength = isize; // euclid::Length<isize, PixelUnit>;
