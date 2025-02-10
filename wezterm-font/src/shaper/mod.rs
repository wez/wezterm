use crate::parser::ParsedFont;
use crate::units::PixelLength;
use std::ops::Range;
use termwiz::cell::Presentation;
use termwiz::cellcluster::CellCluster;

pub mod harfbuzz;
pub use wezterm_bidi::Direction;

/// Holds information about a shaped glyph
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphInfo {
    /// We only retain text in debug mode for diagnostic purposes
    #[cfg(any(debug_assertions, test))]
    pub text: String,
    /// If text is comprised of a single char, this is it
    pub only_char: Option<char>,
    pub is_space: bool,
    /// Number of cells occupied by this single glyph.
    /// This accounts for eg: the shaper combining adjacent graphemes
    /// into a single glyph, such as in `!=` and other ligatures.
    /// Without tracking this version of the width, we may not detect
    /// the combined case as the corresponding cluster index is simply
    /// omitted from the shaped result.
    /// <https://github.com/wezterm/wezterm/issues/1563>
    pub num_cells: u8,
    /// Offset within text
    pub cluster: u32,
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

    /// Fraction of the EM square occupied by the cap height
    pub cap_height_ratio: Option<f64>,
    pub cap_height: Option<PixelLength>,

    /// True if the font is scalable and this is a scaled metric.
    /// False if the font only has bitmap strikes and what we
    /// have here is a best approximation.
    pub is_scaled: bool,

    pub presentation: Presentation,

    /// When the user has configured a fallback-specific override,
    /// this field contains the difference in the descender heights
    /// between the scaled and unscaled versions of the descender.
    /// This represents a y-adjustment that should be applied to
    /// the glyph to make it appear to line up better.
    /// <https://github.com/wezterm/wezterm/issues/1803>
    pub force_y_adjust: PixelLength,
}

#[derive(Debug)]
pub struct PresentationWidth<'a> {
    cluster: &'a CellCluster,
}

impl<'a> PresentationWidth<'a> {
    pub fn with_cluster(cluster: &'a CellCluster) -> Self {
        Self { cluster }
    }

    pub fn num_cells(&self, cluster_range: Range<usize>) -> u8 {
        let mut width = 0;
        let mut done_cells = vec![];

        for byte_idx in cluster_range {
            let cell_idx = self.cluster.byte_to_cell_idx(byte_idx);
            if done_cells.contains(&cell_idx) {
                continue;
            }
            done_cells.push(cell_idx);
            width += self.cluster.byte_to_cell_width(byte_idx);
        }
        width
    }

    pub fn byte_to_cell_idx(&self, start_byte: usize) -> usize {
        self.cluster.byte_to_cell_idx(start_byte)
    }
}

pub trait FontShaper {
    /// Shape text and return a vector of GlyphInfo
    fn shape(
        &self,
        text: &str,
        size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
        presentation: Option<termwiz::cell::Presentation>,
        direction: Direction,
        range: Option<Range<usize>>,
        presentation_width: Option<&PresentationWidth>,
    ) -> anyhow::Result<Vec<GlyphInfo>>;

    /// Compute the font metrics for the preferred font
    /// at the specified size.
    fn metrics(&self, size: f64, dpi: u32) -> anyhow::Result<FontMetrics>;

    /// Compute the metrics for a given fallback font at the specified size
    fn metrics_for_idx(&self, font_idx: usize, size: f64, dpi: u32) -> anyhow::Result<FontMetrics>;
}

pub use config::FontShaperSelection;

pub fn new_shaper(
    config: &config::ConfigHandle,
    handles: &[ParsedFont],
) -> anyhow::Result<Box<dyn FontShaper>> {
    match config.font_shaper {
        FontShaperSelection::Harfbuzz => {
            Ok(Box::new(harfbuzz::HarfbuzzShaper::new(config, handles)?))
        }
        FontShaperSelection::Allsorts => {
            anyhow::bail!("The incomplete Allsorts shaper has been removed");
        }
    }
}
