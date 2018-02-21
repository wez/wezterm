//! Abstracts over the font selection system for the system

use super::super::config::{Config, TextStyle};
use failure::Error;
use harfbuzz;
use unicode_width::UnicodeWidthStr;

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
    pub fn new(
        text: &str,
        font_idx: usize,
        info: &harfbuzz::hb_glyph_info_t,
        pos: &harfbuzz::hb_glyph_position_t,
    ) -> GlyphInfo {
        let num_cells = UnicodeWidthStr::width(text) as u8;
        GlyphInfo {
            #[cfg(debug_assertions)]
            text: text.into(),
            num_cells,
            font_idx,
            glyph_pos: info.codepoint,
            cluster: info.cluster,
            x_advance: pos.x_advance as f64 / 64.0,
            y_advance: pos.y_advance as f64 / 64.0,
            x_offset: pos.x_offset as f64 / 64.0,
            y_offset: pos.y_offset as f64 / 64.0,
        }
    }
}

/// Represents a numbered index in the fallback sequence for a NamedFont.
/// 0 is the first, best match.  If a glyph isn't present then we will
/// want to search for a fallback in later indices.
pub type FallbackIdx = usize;

/// Represents a named, user-selected font.
/// This is really a set of fallback fonts indexed by FallbackIdx with
/// zero as the best/most preferred font.
pub trait NamedFont {
    /// Get a reference to a numbered fallback Font instance
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&Font, Error>;

    /// Shape text and return a vector of GlyphInfo
    fn shape(&mut self, text: &str) -> Result<Vec<GlyphInfo>, Error>;
}

/// FontSystem is a handle to the system font selection system
pub trait FontSystem {
    /// Given a text style, load (without caching) the font that
    /// best matches according to the fontconfig pattern.
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error>;
}

/// Describes the key font metrics that we use in rendering
pub struct FontMetrics {
    /// Width of a character cell in pixels
    pub cell_width: f64,
    /// Height of a character cell in pixels
    pub cell_height: f64,
    /// Added to the bottom y coord to find the baseline.
    /// descender is typically negative.
    pub descender: i16,
}

use super::ftwrap;

/// Represents a concrete instance of a font.
pub trait Font {
    /// Returns true if the font rasterizes with true color glyphs,
    /// or false if it produces gray scale glyphs that need to be
    /// colorized.
    fn has_color(&self) -> bool;

    /// Returns the font metrics
    fn metrics(&self) -> FontMetrics;

    /// FIXME: This is a temporary hack and will be replaced
    /// with a rasterize method.
    fn load_glyph(&self, glyph_pos: u32) -> Result<ftwrap::FT_GlyphSlotRec_, Error>;

    /// Perform shaping on the supplied harfbuzz buffer.
    /// This is really just a proxy for calling the harfbuzz::Font::shape()
    /// method on the contained harfbuzz font instance.
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    );
}
