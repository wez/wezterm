use crate::db::FontDatabase;
use crate::locator::{new_locator, FontLocator};
use crate::parser::ParsedFont;
use crate::rasterizer::{new_rasterizer, FontRasterizer};
use crate::shaper::{new_shaper, FontShaper};
use anyhow::{Context, Error};
use config::{
    configuration, ConfigHandle, FontRasterizerSelection, FontStretch, FontWeight, TextStyle,
};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use wezterm_term::CellAttributes;
use wezterm_toast_notification::ToastNotification;
use window::default_dpi;

mod hbwrap;

pub mod db;
pub mod ftwrap;
pub mod locator;
pub mod parser;
pub mod rasterizer;
pub mod shaper;
pub mod units;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod fcwrap;

pub use crate::rasterizer::RasterizedGlyph;
pub use crate::shaper::{FallbackIdx, FontMetrics, GlyphInfo};

#[derive(Debug, Error)]
#[error("Font fallback recalculated")]
pub struct ClearShapeCache {}

pub struct LoadedFont {
    rasterizers: RefCell<HashMap<FallbackIdx, Box<dyn FontRasterizer>>>,
    handles: RefCell<Vec<ParsedFont>>,
    shaper: RefCell<Box<dyn FontShaper>>,
    metrics: FontMetrics,
    font_size: f64,
    dpi: u32,
    font_config: Weak<FontConfigInner>,
    pending_fallback: Arc<Mutex<Vec<ParsedFont>>>,
}

impl LoadedFont {
    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    fn insert_fallback_handles(&self, extra_handles: Vec<ParsedFont>) -> anyhow::Result<bool> {
        let mut loaded = false;
        {
            let mut handles = self.handles.borrow_mut();
            for h in extra_handles {
                if !handles.iter().any(|existing| *existing == h) {
                    let idx = handles.len() - 1;
                    handles.insert(idx, h);
                    self.rasterizers.borrow_mut().remove(&idx);
                    loaded = true;
                }
            }
            if loaded {
                log::trace!("revised fallback: {:?}", handles);
            }
        }
        if loaded {
            if let Some(font_config) = self.font_config.upgrade() {
                *self.shaper.borrow_mut() =
                    new_shaper(&*font_config.config.borrow(), &self.handles.borrow())?;
            }
        }
        Ok(loaded)
    }

    pub fn shape<F: FnOnce() + Send + Sync + 'static>(
        &self,
        text: &str,
        completion: F,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let mut no_glyphs = vec![];

        {
            let mut pending = self.pending_fallback.lock().unwrap();
            if !pending.is_empty() {
                match self.insert_fallback_handles(pending.split_off(0)) {
                    Ok(true) => return Err(ClearShapeCache {})?,
                    Ok(false) => {}
                    Err(err) => {
                        log::error!("Error adding fallback: {:#}", err);
                    }
                }
            }
        }

        let result = self
            .shaper
            .borrow()
            .shape(text, self.font_size, self.dpi, &mut no_glyphs);

        if !no_glyphs.is_empty() {
            if let Some(font_config) = self.font_config.upgrade() {
                font_config.schedule_fallback_resolve(
                    no_glyphs,
                    &self.pending_fallback,
                    completion,
                );
            }
        }

        result
    }

    pub fn metrics_for_idx(&self, font_idx: usize) -> anyhow::Result<FontMetrics> {
        self.shaper
            .borrow()
            .metrics_for_idx(font_idx, self.font_size, self.dpi)
    }

    pub fn brightness_adjust(&self, font_idx: usize) -> f32 {
        let synthesize_dim = self
            .handles
            .borrow()
            .get(font_idx)
            .map(|p| p.synthesize_dim)
            .unwrap_or(false);
        if synthesize_dim {
            0.5
        } else {
            1.0
        }
    }

    pub fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        fallback: FallbackIdx,
    ) -> anyhow::Result<RasterizedGlyph> {
        let mut rasterizers = self.rasterizers.borrow_mut();
        if let Some(raster) = rasterizers.get(&fallback) {
            raster.rasterize_glyph(glyph_pos, self.font_size, self.dpi)
        } else {
            let raster_selection = self
                .font_config
                .upgrade()
                .map_or(FontRasterizerSelection::default(), |c| {
                    c.config.borrow().font_rasterizer
                });
            let raster = new_rasterizer(raster_selection, &(self.handles.borrow())[fallback])?;
            let result = raster.rasterize_glyph(glyph_pos, self.font_size, self.dpi);
            rasterizers.insert(fallback, raster);
            result
        }
    }

    pub fn clone_handles(&self) -> Vec<ParsedFont> {
        self.handles.borrow().clone()
    }
}

struct FontConfigInner {
    fonts: RefCell<HashMap<TextStyle, Rc<LoadedFont>>>,
    metrics: RefCell<Option<FontMetrics>>,
    dpi: RefCell<usize>,
    font_scale: RefCell<f64>,
    config: RefCell<ConfigHandle>,
    locator: Arc<dyn FontLocator + Send + Sync>,
    font_dirs: RefCell<Arc<FontDatabase>>,
    built_in: RefCell<Arc<FontDatabase>>,
    no_glyphs: RefCell<HashSet<char>>,
}

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    inner: Rc<FontConfigInner>,
}

impl FontConfigInner {
    /// Create a new empty configuration
    pub fn new(config: Option<ConfigHandle>) -> anyhow::Result<Self> {
        let config = config.unwrap_or_else(|| configuration());
        let locator = new_locator(config.font_locator);
        let dpi = config.dpi.unwrap_or_else(|| default_dpi()) as usize;
        Ok(Self {
            fonts: RefCell::new(HashMap::new()),
            locator,
            metrics: RefCell::new(None),
            font_scale: RefCell::new(1.0),
            dpi: RefCell::new(dpi),
            config: RefCell::new(config.clone()),
            font_dirs: RefCell::new(Arc::new(FontDatabase::with_font_dirs(&config)?)),
            built_in: RefCell::new(Arc::new(FontDatabase::with_built_in()?)),
            no_glyphs: RefCell::new(HashSet::new()),
        })
    }

    fn config_changed(&self, config: &ConfigHandle) -> anyhow::Result<()> {
        let mut fonts = self.fonts.borrow_mut();
        *self.config.borrow_mut() = config.clone();
        // Config was reloaded, invalidate our caches
        fonts.clear();
        self.metrics.borrow_mut().take();
        self.no_glyphs.borrow_mut().clear();
        *self.font_dirs.borrow_mut() = Arc::new(FontDatabase::with_font_dirs(config)?);
        Ok(())
    }

    fn schedule_fallback_resolve<F: FnOnce() + Send + Sync + 'static>(
        &self,
        mut no_glyphs: Vec<char>,
        pending: &Arc<Mutex<Vec<ParsedFont>>>,
        completion: F,
    ) {
        let mut ng = self.no_glyphs.borrow_mut();
        no_glyphs.retain(|c| !ng.contains(c));
        for c in &no_glyphs {
            ng.insert(*c);
        }
        if no_glyphs.is_empty() {
            return;
        }

        let font_dirs = Arc::clone(&*self.font_dirs.borrow());
        let built_in = Arc::clone(&*self.built_in.borrow());
        let locator = Arc::clone(&self.locator);
        let pending = Arc::clone(pending);
        let config = self.config.borrow().clone();
        std::thread::spawn(move || {
            let fallback_str = no_glyphs.iter().collect::<String>();
            let mut extra_handles = vec![];

            log::trace!(
                "Looking for {} in fallback fonts",
                fallback_str.escape_unicode()
            );

            match locator.locate_fallback_for_codepoints(&no_glyphs) {
                Ok(ref mut handles) => extra_handles.append(handles),
                Err(err) => log::error!(
                    "Error: {:#} while resolving fallback for {} from font-locator",
                    err,
                    fallback_str.escape_unicode()
                ),
            }

            if config.search_font_dirs_for_fallback {
                match font_dirs.locate_fallback_for_codepoints(&no_glyphs) {
                    Ok(ref mut handles) => extra_handles.append(handles),
                    Err(err) => log::error!(
                        "Error: {:#} while resolving fallback for {} from font_dirs",
                        err,
                        fallback_str.escape_unicode()
                    ),
                }
            }

            match built_in.locate_fallback_for_codepoints(&no_glyphs) {
                Ok(ref mut handles) => extra_handles.append(handles),
                Err(err) => log::error!(
                    "Error: {:#} while resolving fallback for {} for built-in fonts",
                    err,
                    fallback_str.escape_unicode()
                ),
            }

            let mut wanted = RangeSet::new();
            for c in no_glyphs {
                wanted.add(c as u32);
            }
            log::trace!(
                "Fallback fonts that match {} before sorting are: {:#?}",
                fallback_str.escape_unicode(),
                extra_handles
            );

            if wanted.len() > 1 && config.sort_fallback_fonts_by_coverage {
                // Sort by ascending coverage
                extra_handles.sort_by_cached_key(|p| {
                    p.coverage_intersection(&wanted)
                        .map(|r| r.len())
                        .unwrap_or(0)
                });
                // Re-arrange to descending coverage
                extra_handles.reverse();
                log::trace!(
                    "Fallback fonts that match {} after sorting are: {:#?}",
                    fallback_str.escape_unicode(),
                    extra_handles
                );
            }

            // iteratively reduce to just the fonts that we need
            extra_handles.retain(|p| match p.coverage_intersection(&wanted) {
                Ok(cov) if cov.is_empty() => false,
                Ok(cov) => {
                    // Remove the matches from the set, so that we avoid
                    // picking up multiple fonts for the same glyphs
                    wanted = wanted.difference(&cov);
                    true
                }
                Err(_) => false,
            });

            if !extra_handles.is_empty() {
                let mut pending = pending.lock().unwrap();
                pending.append(&mut extra_handles);
                completion();
            }

            if !wanted.is_empty() {
                // There were some glyphs we couldn't resolve!
                let fallback_str = wanted
                    .iter_values()
                    .map(|c| std::char::from_u32(c).unwrap_or(' '))
                    .collect::<String>();

                if config.warn_about_missing_glyphs {
                    let url = "https://wezfurlong.org/wezterm/config/fonts.html";
                    log::warn!(
                        "No fonts contain glyphs for these codepoints: {}.\n\
                     Placeholder 'Last Resort' glyphs are being displayed instead.\n\
                     You may wish to install additional fonts, or adjust your\n\
                     configuration so that it can find them.\n\
                     {} has more information about configuring fonts.\n\
                     Set warn_about_missing_glyphs=false to suppress this message.",
                        fallback_str.escape_unicode(),
                        url,
                    );

                    ToastNotification {
                        title: "Font problem".to_string(),
                        message: format!(
                            "No fonts contain glyphs for these codepoints: {}.\n\
                            Placeholder glyphs are being displayed instead.\n\
                            You may wish to install additional fonts, or adjust\n\
                            your configuration so that it can find them.\n\
                            Set warn_about_missing_glyphs=false to suppress this\n\
                            message.",
                            fallback_str.escape_unicode()
                        ),
                        url: Some(url.to_string()),
                        timeout: Some(Duration::from_secs(15)),
                    }
                    .show();
                } else {
                    log::warn!(
                        "No fonts contain glyphs for these codepoints: {}",
                        fallback_str.escape_unicode()
                    );
                }
            }
        });
    }

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    fn resolve_font(&self, myself: &Rc<Self>, style: &TextStyle) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();

        let mut fonts = self.fonts.borrow_mut();

        if let Some(entry) = fonts.get(style) {
            return Ok(Rc::clone(entry));
        }

        let attributes = style.font_with_fallback();
        let preferred_attributes = attributes
            .iter()
            .filter(|a| !a.is_fallback)
            .map(|a| a.clone())
            .collect::<Vec<_>>();
        let fallback_attributes = attributes
            .iter()
            .filter(|a| a.is_fallback)
            .map(|a| a.clone())
            .collect::<Vec<_>>();
        let mut loaded = HashSet::new();

        let mut handles = vec![];
        for attrs in &[&preferred_attributes, &fallback_attributes] {
            self.font_dirs
                .borrow()
                .resolve_multiple(attrs, &mut handles, &mut loaded);
            handles.append(&mut self.locator.load_fonts(attrs, &mut loaded)?);
            self.built_in
                .borrow()
                .resolve_multiple(attrs, &mut handles, &mut loaded);
        }

        for attr in &attributes {
            if !attr.is_synthetic && !attr.is_fallback && !loaded.contains(attr) {
                let styled_extra = if attr.weight != FontWeight::default()
                    || attr.italic
                    || attr.stretch != FontStretch::default()
                {
                    ". An alternative variant of the font was requested; \
                    TrueType and OpenType fonts don't have an automatic way to \
                    produce these font variants, so a separate font file containing \
                    the bold or italic variant must be installed"
                } else {
                    ""
                };

                let is_primary = config.font.font.iter().any(|a| a == attr);
                let derived_from_primary = config.font.font.iter().any(|a| a.family == attr.family);

                let explanation = if is_primary {
                    // This is the primary font selection
                    format!(
                        "Unable to load a font specified by your font={} configuration",
                        attr
                    )
                } else if derived_from_primary {
                    // it came from font_rules and may have been derived from
                    // their primary font (we can't know for sure)
                    format!(
                        "Unable to load a font matching one of your font_rules: {}. \
                        Note that wezterm will synthesize font_rules to select bold \
                        and italic fonts based on your primary font configuration",
                        attr
                    )
                } else {
                    format!(
                        "Unable to load a font matching one of your font_rules: {}",
                        attr
                    )
                };

                config::show_error(&format!(
                    "{}. Fallback(s) are being used instead, and the terminal \
                    may not render as intended{}. See \
                    https://wezfurlong.org/wezterm/config/fonts.html for more information",
                    explanation, styled_extra
                ));
            }
        }

        let shaper = new_shaper(&*config, &handles)?;

        let font_size = config.font_size * *self.font_scale.borrow();
        let dpi = *self.dpi.borrow() as u32;
        let metrics = shaper.metrics(font_size, dpi).with_context(|| {
            format!(
                "obtaining metrics for font_size={} @ dpi {}",
                font_size, dpi
            )
        })?;

        let loaded = Rc::new(LoadedFont {
            rasterizers: RefCell::new(HashMap::new()),
            handles: RefCell::new(handles),
            shaper: RefCell::new(shaper),
            metrics,
            font_size,
            dpi,
            font_config: Rc::downgrade(myself),
            pending_fallback: Arc::new(Mutex::new(vec![])),
        });

        fonts.insert(style.clone(), Rc::clone(&loaded));

        Ok(loaded)
    }

    pub fn change_scaling(&self, font_scale: f64, dpi: usize) -> (f64, usize) {
        let prior_font = *self.font_scale.borrow();
        let prior_dpi = *self.dpi.borrow();

        *self.dpi.borrow_mut() = dpi;
        *self.font_scale.borrow_mut() = font_scale;
        self.fonts.borrow_mut().clear();
        self.metrics.borrow_mut().take();
        self.no_glyphs.borrow_mut().clear();

        (prior_font, prior_dpi)
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        self.resolve_font(myself, &self.config.borrow().font)
    }

    pub fn get_font_scale(&self) -> f64 {
        *self.font_scale.borrow()
    }

    pub fn default_font_metrics(&self, myself: &Rc<Self>) -> Result<FontMetrics, Error> {
        {
            let metrics = self.metrics.borrow();
            if let Some(metrics) = metrics.as_ref() {
                return Ok(*metrics);
            }
        }

        let font = self.default_font(myself)?;
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
        }

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

impl FontConfiguration {
    /// Create a new empty configuration
    pub fn new(config: Option<ConfigHandle>) -> anyhow::Result<Self> {
        let inner = Rc::new(FontConfigInner::new(config)?);
        Ok(Self { inner })
    }

    pub fn config_changed(&self, config: &ConfigHandle) -> anyhow::Result<()> {
        self.inner.config_changed(config)
    }

    pub fn config(&self) -> ConfigHandle {
        self.inner.config.borrow().clone()
    }

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    pub fn resolve_font(&self, style: &TextStyle) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.resolve_font(&self.inner, style)
    }

    pub fn change_scaling(&self, font_scale: f64, dpi: usize) -> (f64, usize) {
        self.inner.change_scaling(font_scale, dpi)
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.default_font(&self.inner)
    }

    pub fn get_font_scale(&self) -> f64 {
        self.inner.get_font_scale()
    }

    pub fn default_font_metrics(&self) -> Result<FontMetrics, Error> {
        self.inner.default_font_metrics(&self.inner)
    }

    /// Apply the defined font_rules from the user configuration to
    /// produce the text style that best matches the supplied input
    /// cell attributes.
    pub fn match_style<'a>(
        &self,
        config: &'a ConfigHandle,
        attrs: &CellAttributes,
    ) -> &'a TextStyle {
        self.inner.match_style(config, attrs)
    }
}
