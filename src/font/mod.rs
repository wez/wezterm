use failure::{format_err, Error, Fallible};
mod hbwrap;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub mod ftwrap;
pub mod loader;
pub mod rasterizer;
pub mod shaper;

#[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
pub mod fcwrap;

use crate::font::loader::{FontLocator, FontLocatorSelection};
pub use crate::font::rasterizer::RasterizedGlyph;
use crate::font::rasterizer::{FontRasterizer, FontRasterizerSelection};
pub use crate::font::shaper::{FallbackIdx, FontMetrics, GlyphInfo};
use crate::font::shaper::{FontShaper, FontShaperSelection};

use super::config::{configuration, ConfigHandle, TextStyle};
use term::CellAttributes;

pub struct LoadedFont {
    rasterizers: Vec<Box<dyn FontRasterizer>>,
    shaper: Box<dyn FontShaper>,
    metrics: FontMetrics,
    font_size: f64,
    dpi: u32,
}

impl LoadedFont {
    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    pub fn shape(&self, text: &str) -> Fallible<Vec<GlyphInfo>> {
        self.shaper.shape(text, self.font_size, self.dpi)
    }

    pub fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        fallback: FallbackIdx,
    ) -> Fallible<RasterizedGlyph> {
        let rasterizer = self
            .rasterizers
            .get(fallback)
            .ok_or_else(|| format_err!("no such fallback index: {}", fallback))?;
        rasterizer.rasterize_glyph(glyph_pos, self.font_size, self.dpi)
    }
}

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    fonts: RefCell<HashMap<TextStyle, Rc<LoadedFont>>>,
    metrics: RefCell<Option<FontMetrics>>,
    dpi_scale: RefCell<f64>,
    font_scale: RefCell<f64>,
    config_generation: RefCell<usize>,
    loader: Box<dyn FontLocator>,
}

impl FontConfiguration {
    /// Create a new empty configuration
    pub fn new() -> Self {
        let loader = FontLocatorSelection::get_default().new_locator();
        Self {
            fonts: RefCell::new(HashMap::new()),
            loader,
            metrics: RefCell::new(None),
            font_scale: RefCell::new(1.0),
            dpi_scale: RefCell::new(1.0),
            config_generation: RefCell::new(configuration().generation()),
        }
    }

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    pub fn resolve_font(&self, style: &TextStyle) -> Fallible<Rc<LoadedFont>> {
        let mut fonts = self.fonts.borrow_mut();

        let config = configuration();
        let current_generation = config.generation();
        if current_generation != *self.config_generation.borrow() {
            // Config was reloaded, invalidate our caches
            fonts.clear();
            self.metrics.borrow_mut().take();
            *self.config_generation.borrow_mut() = current_generation;
        }

        if let Some(entry) = fonts.get(style) {
            return Ok(Rc::clone(entry));
        }

        let attributes = style.font_with_fallback();
        let handles = self.loader.load_fonts(&attributes)?;
        let mut rasterizers = vec![];
        for handle in &handles {
            rasterizers.push(FontRasterizerSelection::get_default().new_rasterizer(&handle)?);
        }
        let shaper = FontShaperSelection::get_default().new_shaper(&handles)?;

        let config = configuration();
        let font_size = config.font_size * *self.font_scale.borrow();
        let dpi = config.dpi as u32;
        let metrics = shaper.metrics(font_size, dpi)?;

        let loaded = Rc::new(LoadedFont {
            rasterizers,
            shaper,
            metrics,
            font_size,
            dpi,
        });

        fonts.insert(style.clone(), Rc::clone(&loaded));

        Ok(loaded)
    }

    pub fn change_scaling(&self, font_scale: f64, dpi_scale: f64) {
        *self.dpi_scale.borrow_mut() = dpi_scale;
        *self.font_scale.borrow_mut() = font_scale;
        self.fonts.borrow_mut().clear();
        self.metrics.borrow_mut().take();
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self) -> Fallible<Rc<LoadedFont>> {
        self.resolve_font(&configuration().font)
    }

    pub fn get_font_scale(&self) -> f64 {
        *self.font_scale.borrow()
    }

    pub fn default_font_metrics(&self) -> Result<FontMetrics, Error> {
        {
            let metrics = self.metrics.borrow();
            if let Some(metrics) = metrics.as_ref() {
                return Ok(*metrics);
            }
        }

        let font = self.default_font()?;
        let metrics = font.metrics();

        *self.metrics.borrow_mut() = Some(metrics);

        Ok(metrics)
    }

    /// Apply the defined font_rules from the user configuration to
    /// produce the text style that best matches the supplied input
    /// cell attributes.
    pub fn match_style<'a>(
        &self,
        config: &'a ConfigHandle,
        attrs: &CellAttributes,
    ) -> &'a TextStyle {
        // a little macro to avoid boilerplate for matching the rules.
        // If the rule doesn't specify a value for an attribute then
        // it will implicitly match.  If it specifies an attribute
        // then it has to have the same value as that in the input attrs.
        macro_rules! attr_match {
            ($ident:ident, $rule:expr) => {
                if let Some($ident) = $rule.$ident {
                    if $ident != attrs.$ident() {
                        // Does not match
                        continue;
                    }
                }
                // matches so far...
            };
        };

        for rule in &config.font_rules {
            attr_match!(intensity, &rule);
            attr_match!(underline, &rule);
            attr_match!(italic, &rule);
            attr_match!(blink, &rule);
            attr_match!(reverse, &rule);
            attr_match!(strikethrough, &rule);
            attr_match!(invisible, &rule);

            // If we get here, then none of the rules didn't match,
            // so we therefore assume that it did match overall.
            return &rule.font;
        }
        &config.font
    }
}
