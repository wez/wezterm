use crate::locator::FontDataHandle;
use crate::units::PixelLength;

pub mod allsorts;
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
#[derive(Copy, Clone, Debug, PartialEq)]
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
    fn shape(
        &self,
        text: &str,
        size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
    ) -> anyhow::Result<Vec<GlyphInfo>>;

    /// Compute the font metrics for the preferred font
    /// at the specified size.
    fn metrics(&self, size: f64, dpi: u32) -> anyhow::Result<FontMetrics>;

    /// Compute the metrics for a given fallback font at the specified size
    fn metrics_for_idx(&self, font_idx: usize, size: f64, dpi: u32) -> anyhow::Result<FontMetrics>;
}

pub use config::FontShaperSelection;

pub fn new_shaper(
    shaper: FontShaperSelection,
    handles: &[FontDataHandle],
) -> anyhow::Result<Box<dyn FontShaper>> {
    match shaper {
        FontShaperSelection::Harfbuzz => Ok(Box::new(harfbuzz::HarfbuzzShaper::new(handles)?)),
        FontShaperSelection::Allsorts => Ok(Box::new(allsorts::AllsortsShaper::new(handles)?)),
    }
}
