use crate::db::FontDatabase;
use crate::locator::{new_locator, FontLocator};
use crate::parser::ParsedFont;
use crate::rasterizer::{new_rasterizer, FontRasterizer};
use crate::shaper::{new_shaper, FontShaper, PresentationWidth};
use anyhow::{Context, Error};
use config::{
    configuration, BoldBrightening, ConfigHandle, DisplayPixelGeometry, FontAttributes,
    FontRasterizerSelection, FontStretch, FontStyle, FontWeight, TextStyle,
};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use termwiz::cell::Presentation;
use thiserror::Error;
use wezterm_bidi::Direction;
use wezterm_term::{CellAttributes, Intensity};
use wezterm_toast_notification::ToastNotification;

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

static FONT_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type LoadedFontId = usize;
pub fn alloc_font_id() -> LoadedFontId {
    FONT_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

lazy_static::lazy_static! {
    static ref LAST_WARNING: Mutex<Option<(Instant, usize)>> = Mutex::new(None);
}

pub struct LoadedFont {
    rasterizers: RefCell<HashMap<FallbackIdx, Box<dyn FontRasterizer>>>,
    handles: RefCell<Vec<ParsedFont>>,
    shaper: RefCell<Box<dyn FontShaper>>,
    metrics: FontMetrics,
    pixel_geometry: DisplayPixelGeometry,
    font_size: f64,
    dpi: u32,
    font_config: Weak<FontConfigInner>,
    pending_fallback: Arc<Mutex<Vec<ParsedFont>>>,
    text_style: TextStyle,
    id: LoadedFontId,
    /// Glyphs for which no font was found and for which we should
    /// stop searching
    tried_glyphs: RefCell<HashSet<char>>,
}

impl std::fmt::Debug for LoadedFont {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("LoadedFont")
            .field("handles", &self.handles)
            .field("metrics", &self.metrics)
            .field("font_size", &self.font_size)
            .field("dpi", &self.dpi)
            .field("pending_fallback", &self.pending_fallback)
            .field("text_style", &self.text_style)
            .finish()
    }
}

impl LoadedFont {
    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    pub fn style(&self) -> &TextStyle {
        &self.text_style
    }

    pub fn id(&self) -> LoadedFontId {
        self.id
    }

    fn insert_fallback_handles(&self, extra_handles: Vec<ParsedFont>) -> anyhow::Result<bool> {
        let mut loaded = false;
        {
            let mut handles = self.handles.borrow_mut();
            for h in extra_handles {
                if !handles.iter().any(|existing| *existing == h) {
                    handles.push(h);
                    loaded = true;
                }
            }
            if loaded {
                log::trace!("revised fallback: {:#?}", handles);
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

    pub fn blocking_shape(
        &self,
        text: &str,
        presentation: Option<Presentation>,
        direction: Direction,
        range: Option<Range<usize>>,
        presentation_width: Option<&PresentationWidth>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        loop {
            let (tx, rx) = channel();

            let (async_resolve, res) = match self.shape_impl(
                text,
                move || {
                    let _ = tx.send(());
                },
                |_| {},
                presentation,
                direction,
                range.clone(),
                presentation_width,
            ) {
                Ok(tuple) => tuple,
                Err(err) if err.downcast_ref::<ClearShapeCache>().is_some() => {
                    continue;
                }
                Err(err) => return Err(err),
            };

            if !async_resolve {
                return Ok(res);
            }
            if rx.recv().is_err() {
                return Ok(res);
            }
        }
    }

    pub fn shape<F: FnOnce() + Send + 'static, FS: FnOnce(&mut Vec<char>)>(
        &self,
        text: &str,
        completion: F,
        filter_out_synthetic: FS,
        presentation: Option<Presentation>,
        direction: Direction,
        range: Option<Range<usize>>,
        presentation_width: Option<&PresentationWidth>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let (_async_resolve, res) = self.shape_impl(
            text,
            completion,
            filter_out_synthetic,
            presentation,
            direction,
            range,
            presentation_width,
        )?;
        Ok(res)
    }

    fn shape_impl<F: FnOnce() + Send + 'static, FS: FnOnce(&mut Vec<char>)>(
        &self,
        text: &str,
        completion: F,
        filter_out_synthetic: FS,
        presentation: Option<Presentation>,
        direction: Direction,
        range: Option<Range<usize>>,
        presentation_width: Option<&PresentationWidth>,
    ) -> anyhow::Result<(bool, Vec<GlyphInfo>)> {
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

        let result = self.shaper.borrow().shape(
            text,
            self.font_size,
            self.dpi,
            &mut no_glyphs,
            presentation,
            direction,
            range,
            presentation_width,
        );

        no_glyphs.retain(|&c| c != '\u{FE0F}' && c != '\u{FE0E}');
        filter_out_synthetic(&mut no_glyphs);

        let mut tried_glyphs = self.tried_glyphs.borrow_mut();
        no_glyphs.retain(|c| !tried_glyphs.contains(c));
        for c in &no_glyphs {
            tried_glyphs.insert(*c);
        }

        no_glyphs.sort();
        no_glyphs.dedup();

        let mut async_resolve = false;

        if !no_glyphs.is_empty() {
            if let Some(font_config) = self.font_config.upgrade() {
                font_config.schedule_fallback_resolve(
                    no_glyphs,
                    &self.pending_fallback,
                    completion,
                );
                async_resolve = true;
            }
        }

        result.map(|r| (async_resolve, r))
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
            let raster = new_rasterizer(
                raster_selection,
                &(self.handles.borrow())[fallback],
                self.pixel_geometry,
            )?;
            let result = raster.rasterize_glyph(glyph_pos, self.font_size, self.dpi);
            rasterizers.insert(fallback, raster);
            result
        }
    }

    pub fn clone_handles(&self) -> Vec<ParsedFont> {
        self.handles.borrow().clone()
    }
}

struct FallbackResolveInfo {
    no_glyphs: Vec<char>,
    pending: Arc<Mutex<Vec<ParsedFont>>>,
    completion: Box<dyn FnOnce() + Send>,
    font_dirs: Arc<FontDatabase>,
    built_in: Arc<FontDatabase>,
    locator: Arc<dyn FontLocator + Send + Sync>,
    config: ConfigHandle,
}

impl FallbackResolveInfo {
    fn process(self) {
        let fallback_str = self.no_glyphs.iter().collect::<String>();
        let mut extra_handles = vec![];

        log::trace!(
            "Looking for {} in fallback fonts",
            fallback_str.escape_unicode()
        );

        match self.locator.locate_fallback_for_codepoints(&self.no_glyphs) {
            Ok(ref mut handles) => extra_handles.append(handles),
            Err(err) => log::error!(
                "Error: {:#} while resolving fallback for {} from font-locator",
                err,
                fallback_str.escape_unicode()
            ),
        }

        if self.config.search_font_dirs_for_fallback {
            match self
                .font_dirs
                .locate_fallback_for_codepoints(&self.no_glyphs)
            {
                Ok(ref mut handles) => extra_handles.append(handles),
                Err(err) => log::error!(
                    "Error: {:#} while resolving fallback for {} from font_dirs",
                    err,
                    fallback_str.escape_unicode()
                ),
            }
        }

        match self
            .built_in
            .locate_fallback_for_codepoints(&self.no_glyphs)
        {
            Ok(ref mut handles) => extra_handles.append(handles),
            Err(err) => log::error!(
                "Error: {:#} while resolving fallback for {} for built-in fonts",
                err,
                fallback_str.escape_unicode()
            ),
        }

        let mut wanted = RangeSet::new();
        for c in self.no_glyphs {
            wanted.add(c as u32);
        }
        log::trace!(
            "Fallback fonts that match {} before sorting are: {:#?}",
            fallback_str.escape_unicode(),
            extra_handles
        );

        if wanted.len() > 1 && self.config.sort_fallback_fonts_by_coverage {
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
            let mut pending = self.pending.lock().unwrap();
            pending.append(&mut extra_handles);
            (self.completion)();
        }

        if !wanted.is_empty() {
            // There were some glyphs we couldn't resolve!
            let fallback_str = wanted
                .iter_values()
                .map(|c| std::char::from_u32(c).unwrap_or(' '))
                .collect::<String>();

            let current_gen = self.config.generation();
            let show_warning = self.config.warn_about_missing_glyphs
                && LAST_WARNING
                    .lock()
                    .unwrap()
                    .map(|(instant, generation)| {
                        generation != current_gen
                            || instant.elapsed() > Duration::from_secs(60 * 60)
                    })
                    .unwrap_or(true);

            if show_warning {
                LAST_WARNING
                    .lock()
                    .unwrap()
                    .replace((Instant::now(), self.config.generation()));
                let url = "https://wezterm.org/config/fonts.html";
                log::warn!(
                    "No fonts contain glyphs for these codepoints: {}.\n\
                     Placeholder glyphs are being displayed instead.\n\
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
                log::debug!(
                    "No fonts contain glyphs for these codepoints: {}",
                    fallback_str.escape_unicode()
                );
            }
        }
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
    title_font: RefCell<Option<Rc<LoadedFont>>>,
    pane_select_font: RefCell<Option<Rc<LoadedFont>>>,
    char_select_font: RefCell<Option<Rc<LoadedFont>>>,
    command_palette_font: RefCell<Option<Rc<LoadedFont>>>,
    fallback_channel: RefCell<Option<Sender<FallbackResolveInfo>>>,
}

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    inner: Rc<FontConfigInner>,
}

impl FontConfigInner {
    /// Create a new empty configuration
    pub fn new(config: Option<ConfigHandle>, dpi: usize) -> anyhow::Result<Self> {
        let config = config.unwrap_or_else(configuration);
        let locator = new_locator(config.font_locator);
        Ok(Self {
            fonts: RefCell::new(HashMap::new()),
            locator,
            metrics: RefCell::new(None),
            title_font: RefCell::new(None),
            pane_select_font: RefCell::new(None),
            char_select_font: RefCell::new(None),
            command_palette_font: RefCell::new(None),
            font_scale: RefCell::new(1.0),
            dpi: RefCell::new(dpi),
            config: RefCell::new(config.clone()),
            font_dirs: RefCell::new(Arc::new(FontDatabase::with_font_dirs(&config)?)),
            built_in: RefCell::new(Arc::new(FontDatabase::with_built_in()?)),
            fallback_channel: RefCell::new(None),
        })
    }

    fn config_changed(&self, config: &ConfigHandle) -> anyhow::Result<()> {
        let mut fonts = self.fonts.borrow_mut();
        *self.config.borrow_mut() = config.clone();
        // Config was reloaded, invalidate our caches
        fonts.clear();
        self.title_font.borrow_mut().take();
        self.pane_select_font.borrow_mut().take();
        self.char_select_font.borrow_mut().take();
        self.command_palette_font.borrow_mut().take();
        self.metrics.borrow_mut().take();
        *self.font_dirs.borrow_mut() = Arc::new(FontDatabase::with_font_dirs(config)?);
        Ok(())
    }

    fn schedule_fallback_resolve<F: FnOnce() + Send + 'static>(
        &self,
        no_glyphs: Vec<char>,
        pending: &Arc<Mutex<Vec<ParsedFont>>>,
        completion: F,
    ) {
        if no_glyphs.is_empty() {
            return;
        }

        let info = FallbackResolveInfo {
            completion: Box::new(completion),
            no_glyphs,
            pending: Arc::clone(pending),
            font_dirs: Arc::clone(&*self.font_dirs.borrow()),
            built_in: Arc::clone(&*self.built_in.borrow()),
            locator: Arc::clone(&self.locator),
            config: self.config.borrow().clone(),
        };

        let mut fallback = self.fallback_channel.borrow_mut();

        if fallback.is_none() {
            let (tx, rx) = channel::<FallbackResolveInfo>();

            std::thread::spawn(move || {
                for info in rx {
                    info.process();
                }
            });

            fallback.replace(tx);
        }

        if let Err(err) = fallback.as_mut().expect("channel to exist").send(info) {
            log::error!("Failed to schedule font fallback resolve: {:#}", err);
        }
    }

    fn compute_title_font(&self, config: &ConfigHandle, make_bold: bool) -> (TextStyle, f64) {
        fn bold(family: &str) -> FontAttributes {
            FontAttributes {
                family: family.to_string(),
                weight: FontWeight::BOLD,
                ..Default::default()
            }
        }

        let mut fonts = vec![if make_bold {
            bold("Roboto")
        } else {
            FontAttributes::new("Roboto")
        }];

        // Fallback to their main font selection, so that we can pick up
        // any fallback fonts they might have configured in the main
        // config and so that they don't have to replicate that list for
        // the title font.
        for font in &config.font.font {
            let mut font = font.clone();
            font.is_fallback = true;
            fonts.push(font);
        }

        let font_size = if cfg!(windows) { 10. } else { 12. };

        (
            TextStyle {
                foreground: None,
                font: fonts,
            },
            font_size,
        )
    }

    fn make_title_font_impl(
        &self,
        myself: &Rc<Self>,
        pref_size: Option<f64>,
        make_bold: bool,
    ) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();
        let (sys_font, sys_size) = self.compute_title_font(&config, make_bold);

        let font_size = pref_size.unwrap_or(sys_size);

        let text_style = config.window_frame.font.as_ref().unwrap_or(&sys_font);

        let dpi = *self.dpi.borrow() as u32;
        let pixel_size = (font_size * dpi as f64 / 72.0) as u16;

        let attributes = text_style.font_with_fallback();
        let (handles, _loaded) = self.resolve_font_helper_impl(&attributes, pixel_size)?;

        let shaper = new_shaper(&*config, &handles)?;

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
            text_style: text_style.clone(),
            id: alloc_font_id(),
            tried_glyphs: RefCell::new(HashSet::new()),
            pixel_geometry: config.display_pixel_geometry,
        });

        Ok(loaded)
    }

    fn title_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();

        let mut title_font = self.title_font.borrow_mut();

        if let Some(entry) = title_font.as_ref() {
            return Ok(Rc::clone(entry));
        }

        let loaded = self.make_title_font_impl(myself, config.window_frame.font_size, true)?;

        title_font.replace(Rc::clone(&loaded));

        Ok(loaded)
    }

    fn command_palette_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();

        let mut command_palette_font = self.command_palette_font.borrow_mut();

        if let Some(entry) = command_palette_font.as_ref() {
            return Ok(Rc::clone(entry));
        }

        let loaded =
            self.make_title_font_impl(myself, Some(config.command_palette_font_size), false)?;

        command_palette_font.replace(Rc::clone(&loaded));

        Ok(loaded)
    }

    fn char_select_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();

        let mut char_select_font = self.char_select_font.borrow_mut();

        if let Some(entry) = char_select_font.as_ref() {
            return Ok(Rc::clone(entry));
        }

        let loaded = self.make_title_font_impl(myself, Some(config.char_select_font_size), true)?;

        char_select_font.replace(Rc::clone(&loaded));

        Ok(loaded)
    }

    fn pane_select_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();

        let mut pane_select_font = self.pane_select_font.borrow_mut();

        if let Some(entry) = pane_select_font.as_ref() {
            return Ok(Rc::clone(entry));
        }

        let loaded = self.make_title_font_impl(myself, Some(config.pane_select_font_size), true)?;

        pane_select_font.replace(Rc::clone(&loaded));

        Ok(loaded)
    }

    fn resolve_font_helper_impl(
        &self,
        attributes: &[FontAttributes],
        pixel_size: u16,
    ) -> anyhow::Result<(Vec<ParsedFont>, HashSet<FontAttributes>)> {
        let preferred_attributes = attributes
            .iter()
            .filter(|a| !a.is_fallback)
            .cloned()
            .collect::<Vec<_>>();
        let fallback_attributes = attributes
            .iter()
            .filter(|a| a.is_fallback)
            .cloned()
            .collect::<Vec<_>>();
        let mut loaded = HashSet::new();
        let mut handles = vec![];

        for &attrs in &[&preferred_attributes, &fallback_attributes] {
            let mut candidates = vec![];

            let font_dirs = self.font_dirs.borrow();
            for attr in attrs {
                candidates.append(&mut font_dirs.candidates(attr));
            }

            let mut loaded_ignored = HashSet::new();
            let located = self
                .locator
                .load_fonts(attrs, &mut loaded_ignored, pixel_size)?;
            for font in &located {
                candidates.push(font);
            }

            let built_in = self.built_in.borrow();
            for attr in attrs {
                candidates.append(&mut built_in.candidates(attr));
            }

            let mut is_fallback = false;

            for attr in attrs {
                if attr.is_fallback {
                    is_fallback = true;
                }

                if loaded.contains(attr) {
                    continue;
                }
                let named_candidates: Vec<&ParsedFont> = candidates
                    .iter()
                    .filter_map(|&p| if p.matches_name(attr) { Some(p) } else { None })
                    .collect();
                if let Some(idx) =
                    ParsedFont::best_matching_index(attr, &named_candidates, pixel_size)
                {
                    named_candidates.get(idx).map(|&p| {
                        loaded.insert(attr.clone());
                        handles.push(p.clone().synthesize(attr))
                    });
                }
            }

            if !is_fallback && loaded.is_empty() {
                // We didn't explicitly match any names.
                // When using fontconfig, the system may have expanded a family name
                // like "monospace" into the real font, in which case we wouldn't have
                // found a match in the `named_candidates` vec above, because of the
                // name mismatch.
                // So what we do now is make a second pass over all the located candidates,
                // ignoring their names, and just match based on font attributes.
                let located_candidates: Vec<_> = located.iter().collect();
                for attr in attrs {
                    if let Some(idx) =
                        ParsedFont::best_matching_index(attr, &located_candidates, pixel_size)
                    {
                        located_candidates.get(idx).map(|&p| {
                            loaded.insert(attr.clone());
                            handles.push(p.clone().synthesize(attr))
                        });
                    }
                }
            }
        }

        Ok((handles, loaded))
    }

    fn resolve_font_helper(
        &self,
        style: &TextStyle,
        config: &ConfigHandle,
        pixel_size: u16,
    ) -> anyhow::Result<(Box<dyn FontShaper>, Vec<ParsedFont>)> {
        let attributes = style.font_with_fallback();

        let (handles, loaded) = self.resolve_font_helper_impl(&attributes, pixel_size)?;

        for attr in &attributes {
            if !attr.is_synthetic && !attr.is_fallback && !loaded.contains(attr) {
                let styled_extra = if attr.weight != FontWeight::default()
                    || attr.style != FontStyle::default()
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
                    https://wezterm.org/config/fonts.html for more information",
                    explanation, styled_extra
                ));
            }
        }

        Ok((new_shaper(&*config, &handles)?, handles))
    }

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    fn resolve_font(&self, myself: &Rc<Self>, style: &TextStyle) -> anyhow::Result<Rc<LoadedFont>> {
        let config = self.config.borrow();
        let is_default = *style == config.font;
        let def_font = if !is_default && config.use_cap_height_to_scale_fallback_fonts {
            Some(self.default_font(myself)?)
        } else {
            None
        };

        let mut fonts = self.fonts.borrow_mut();

        if let Some(entry) = fonts.get(style) {
            return Ok(Rc::clone(entry));
        }

        let mut font_size = config.font_size * *self.font_scale.borrow();
        let dpi = *self.dpi.borrow() as u32;
        let pixel_size = (font_size * dpi as f64 / 72.0) as u16;

        let (mut shaper, mut handles) = self.resolve_font_helper(style, &config, pixel_size)?;

        let mut metrics = shaper.metrics(font_size, dpi).with_context(|| {
            format!(
                "obtaining metrics for font_size={} @ dpi {}",
                font_size, dpi
            )
        })?;

        if let Some(def_font) = def_font {
            let def_metrics = def_font.metrics();
            match (def_metrics.cap_height, metrics.cap_height) {
                (Some(d), Some(m)) => {
                    // Scale by the ratio of the pixel heights of the default
                    // and this font; this causes the `I` glyphs to appear to
                    // have the same height.
                    let scale = d.get() / m.get();
                    if scale != 1.0 {
                        let scaled_pixel_size = (pixel_size as f64 * scale) as u16;
                        let scaled_font_size = font_size * scale;
                        log::trace!(
                            "using cap height adjusted: pixel_size {} -> {}, font_size {} -> {}, {:?}",
                            pixel_size,
                            scaled_pixel_size,
                            font_size,
                            scaled_font_size,
                            metrics,
                        );
                        let (alt_shaper, alt_handles) =
                            self.resolve_font_helper(style, &config, scaled_pixel_size)?;
                        shaper = alt_shaper;
                        handles = alt_handles;

                        metrics = shaper.metrics(scaled_font_size, dpi).with_context(|| {
                            format!(
                                "obtaining cap-height adjusted metrics for font_size={} @ dpi {}",
                                scaled_font_size, dpi
                            )
                        })?;

                        font_size = scaled_font_size;
                    }
                }
                _ => {}
            }
        }

        let loaded = Rc::new(LoadedFont {
            rasterizers: RefCell::new(HashMap::new()),
            handles: RefCell::new(handles),
            shaper: RefCell::new(shaper),
            metrics,
            font_size,
            dpi,
            font_config: Rc::downgrade(myself),
            pending_fallback: Arc::new(Mutex::new(vec![])),
            text_style: style.clone(),
            id: alloc_font_id(),
            tried_glyphs: RefCell::new(HashSet::new()),
            pixel_geometry: config.display_pixel_geometry,
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
        self.title_font.borrow_mut().take();

        (prior_font, prior_dpi)
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self, myself: &Rc<Self>) -> anyhow::Result<Rc<LoadedFont>> {
        self.resolve_font(myself, &self.config.borrow().font)
    }

    pub fn get_font_scale(&self) -> f64 {
        *self.font_scale.borrow()
    }

    pub fn get_dpi(&self) -> usize {
        *self.dpi.borrow()
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

        let would_bright = match attrs.foreground() {
            wezterm_term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                attrs.intensity() == Intensity::Bold
            }
            _ => false,
        };

        for rule in &config.font_rules {
            if let Some(intensity) = rule.intensity {
                let effective_intensity = match config.bold_brightens_ansi_colors {
                    BoldBrightening::BrightOnly if would_bright => Intensity::Normal,
                    BoldBrightening::No
                    | BoldBrightening::BrightAndBold
                    | BoldBrightening::BrightOnly => attrs.intensity(),
                };
                if intensity != effective_intensity {
                    // Rule does not match
                    continue;
                }
                // matches so far
            }
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
    pub fn new(config: Option<ConfigHandle>, dpi: usize) -> anyhow::Result<Self> {
        let inner = Rc::new(FontConfigInner::new(config, dpi)?);
        Ok(Self { inner })
    }

    pub fn config_changed(&self, config: &ConfigHandle) -> anyhow::Result<()> {
        self.inner.config_changed(config)
    }

    pub fn config(&self) -> ConfigHandle {
        self.inner.config.borrow().clone()
    }

    pub fn title_font(&self) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.title_font(&self.inner)
    }

    pub fn command_palette_font(&self) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.command_palette_font(&self.inner)
    }

    pub fn pane_select_font(&self) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.pane_select_font(&self.inner)
    }

    pub fn char_select_font(&self) -> anyhow::Result<Rc<LoadedFont>> {
        self.inner.char_select_font(&self.inner)
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

    pub fn get_dpi(&self) -> usize {
        self.inner.get_dpi()
    }

    pub fn default_font_metrics(&self) -> Result<FontMetrics, Error> {
        self.inner.default_font_metrics(&self.inner)
    }

    pub fn list_fonts_in_font_dirs(&self) -> Vec<ParsedFont> {
        let mut font_dirs = self.inner.font_dirs.borrow().list_available();
        let mut built_in = self.inner.built_in.borrow().list_available();

        font_dirs.append(&mut built_in);
        font_dirs.sort();
        font_dirs
    }

    pub fn list_system_fonts(&self) -> anyhow::Result<Vec<ParsedFont>> {
        self.inner.locator.enumerate_all_fonts()
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
