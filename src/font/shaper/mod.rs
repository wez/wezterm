use crate::font::locator::FontDataHandle;
use crate::font::units::PixelLength;
use failure::{format_err, Error, Fallible};
use serde_derive::*;
use std::sync::Mutex;

pub mod harfbuzz;

/// Holds information about a shaped glyph
#[derive(Clone, Debug)]
pub struct GlyphInfo {
    /// We only retain text in debug mode for diagnostic purposes
    #[cfg(debug_assertions)]
    pub text: String,
    /// Offset within text
    pub cluster: u32,
    /// How many cells/columns this glyph occupies horizontally
    pub num_cells: u8,
    /// Which font alternative to use; index into Font.fonts
    pub font_idx: FallbackIdx,
    /// Which freetype glyph to load
    pub glyph_pos: u32,
    /// How far to advance the render cursor after drawing this glyph
    pub x_advance: PixelLength,
    /// How far to advance the render cursor after drawing this glyph
    pub y_advance: PixelLength,
    /// Destination render offset
    pub x_offset: PixelLength,
    /// Destination render offset
    pub y_offset: PixelLength,
}

/// Represents a numbered index in the fallback sequence for a `NamedFont`.
/// 0 is the first, best match.  If a glyph isn't present then we will
/// want to search for a fallback in later indices.
pub type FallbackIdx = usize;

/// Describes the key font metrics that we use in rendering
#[derive(Copy, Clone, Debug)]
pub struct FontMetrics {
    /// Width of a character cell in pixels
    pub cell_width: PixelLength,
    /// Height of a character cell in pixels
    pub cell_height: PixelLength,
    /// Added to the bottom y coord to find the baseline.
    /// descender is typically negative.
    pub descender: PixelLength,

    /// Vertical size of underline/strikethrough in pixels
    pub underline_thickness: PixelLength,

    /// Position of underline relative to descender. Negative
    /// values are below the descender.
    pub underline_position: PixelLength,
}

pub trait FontShaper {
    /// Shape text and return a vector of GlyphInfo
    fn shape(&self, text: &str, size: f64, dpi: u32) -> Fallible<Vec<GlyphInfo>>;

    /// Compute the font metrics for the preferred font
    /// at the specified size.
    fn metrics(&self, size: f64, dpi: u32) -> Fallible<FontMetrics>;
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontShaperSelection {
    Harfbuzz,
}

lazy_static::lazy_static! {
    static ref DEFAULT_SHAPER: Mutex<FontShaperSelection> = Mutex::new(Default::default());
}

impl Default for FontShaperSelection {
    fn default() -> Self {
        FontShaperSelection::Harfbuzz
    }
}

impl FontShaperSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_SHAPER.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_SHAPER.lock().unwrap();
        *def
    }

    pub fn variants() -> Vec<&'static str> {
        vec!["Harfbuzz"]
    }

    pub fn new_shaper(self, handles: &[FontDataHandle]) -> Fallible<Box<dyn FontShaper>> {
        match self {
            Self::Harfbuzz => Ok(Box::new(harfbuzz::HarfbuzzShaper::new(handles)?)),
        }
    }
}

impl std::str::FromStr for FontShaperSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "harfbuzz" => Ok(Self::Harfbuzz),
            _ => Err(format_err!(
                "{} is not a valid FontShaperSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}
