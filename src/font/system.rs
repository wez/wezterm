//! Abstracts over the font selection system for the system

use super::super::config::{Config, TextStyle};
use super::hbwrap as harfbuzz;
use failure::Error;

/// A bitmap representation of a glyph.
/// The data is stored as pre-multiplied RGBA 32bpp.
pub struct RasterizedGlyph {
    pub data: Vec<u8>,
    pub height: usize,
    pub width: usize,
    pub bearing_x: f64,
    pub bearing_y: f64,
}

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
    pub font_idx: usize,
    /// Which freetype glyph to load
    pub glyph_pos: u32,
    /// How far to advance the render cursor after drawing this glyph
    pub x_advance: f64,
    /// How far to advance the render cursor after drawing this glyph
    pub y_advance: f64,
    /// Destination render offset
    pub x_offset: f64,
    /// Destination render offset
    pub y_offset: f64,
}

impl GlyphInfo {
    #[allow(dead_code)]
    pub fn new(
        text: &str,
        font_idx: usize,
        info: &harfbuzz::hb_glyph_info_t,
        pos: &harfbuzz::hb_glyph_position_t,
    ) -> GlyphInfo {
        use unicode_width::UnicodeWidthStr;
        let num_cells = UnicodeWidthStr::width(text) as u8;
        GlyphInfo {
            #[cfg(debug_assertions)]
            text: text.into(),
            num_cells,
            font_idx,
            glyph_pos: info.codepoint,
            cluster: info.cluster,
            x_advance: f64::from(pos.x_advance) / 64.0,
            y_advance: f64::from(pos.y_advance) / 64.0,
            x_offset: f64::from(pos.x_offset) / 64.0,
            y_offset: f64::from(pos.y_offset) / 64.0,
        }
    }
}

/// Represents a numbered index in the fallback sequence for a `NamedFont`.
/// 0 is the first, best match.  If a glyph isn't present then we will
/// want to search for a fallback in later indices.
pub type FallbackIdx = usize;

/// Represents a named, user-selected font.
/// This is really a set of fallback fonts indexed by `FallbackIdx` with
/// zero as the best/most preferred font.
pub trait NamedFont {
    /// Get a reference to a numbered fallback Font instance
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&Font, Error>;

    /// Shape text and return a vector of GlyphInfo
    fn shape(&mut self, text: &str) -> Result<Vec<GlyphInfo>, Error>;
}

/// `FontSystem` is a handle to the system font selection system
pub trait FontSystem {
    /// Given a text style, load (without caching) the font that
    /// best matches according to the fontconfig pattern.
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<NamedFont>, Error>;
}

/// Describes the key font metrics that we use in rendering
#[derive(Copy, Clone, Debug, Default)]
pub struct FontMetrics {
    /// Width of a character cell in pixels
    pub cell_width: f64,
    /// Height of a character cell in pixels
    pub cell_height: f64,
    /// Added to the bottom y coord to find the baseline.
    /// descender is typically negative.
    pub descender: f64,
}

/// Represents a concrete instance of a font.
pub trait Font {
    /// Returns true if the font rasterizes with true color glyphs,
    /// or false if it produces gray scale glyphs that need to be
    /// colorized.
    fn has_color(&self) -> bool;

    /// Returns the font metrics
    fn metrics(&self) -> FontMetrics;

    /// Rasterize the glyph
    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error>;

    /// Perform shaping on the supplied harfbuzz buffer.
    /// This is really just a proxy for calling the harfbuzz::Font::shape()
    /// method on the contained harfbuzz font instance.
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    );
}
