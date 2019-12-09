use failure::{bail, format_err, Error, Fallible};
use log::{debug, error};
use serde_derive::*;
mod ftfont;
mod hbwrap;
use self::hbwrap as harfbuzz;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub mod loader;
pub mod rasterizer;
pub mod shaper;

pub mod system;
pub use self::system::*;

pub mod ftwrap;

#[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
pub mod fcwrap;
#[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
pub mod fontconfigandfreetype;

#[cfg(target_os = "macos")]
pub mod coretext;

#[cfg(any(target_os = "macos", windows))]
pub mod fontloader;
#[cfg(any(target_os = "macos", windows))]
pub mod fontloader_and_freetype;

#[cfg(any(target_os = "macos", windows))]
pub mod fontkit;

use crate::font::loader::{FontLocator, FontLocatorSelection};
use crate::font::rasterizer::{FontRasterizer, FontRasterizerSelection};
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

type FontPtr = Rc<RefCell<Box<dyn NamedFont>>>;

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    fonts: RefCell<HashMap<TextStyle, FontPtr>>,
    new_fonts: RefCell<HashMap<TextStyle, Rc<LoadedFont>>>,
    system: Rc<dyn FontSystem>,
    metrics: RefCell<Option<FontMetrics>>,
    dpi_scale: RefCell<f64>,
    font_scale: RefCell<f64>,
    config_generation: RefCell<usize>,
    loader: Box<dyn FontLocator>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontSystemSelection {
    FontConfigAndFreeType,
    FontLoaderAndFreeType,
    FontKitAndFreeType,
    FontKit,
    CoreText,
}

impl Default for FontSystemSelection {
    fn default() -> Self {
        if cfg!(all(unix, not(target_os = "macos"),)) {
            FontSystemSelection::FontConfigAndFreeType
        /*
        } else if cfg!(target_os = "macos") {
            FontSystemSelection::CoreText
        */
        } else {
            FontSystemSelection::FontLoaderAndFreeType
        }
    }
}

thread_local! {
    static DEFAULT_FONT_SYSTEM: RefCell<FontSystemSelection> = RefCell::new(Default::default());
}

impl FontSystemSelection {
    fn new_font_system(self) -> Rc<dyn FontSystem> {
        match self {
            FontSystemSelection::FontConfigAndFreeType => {
                #[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
                return Rc::new(fontconfigandfreetype::FontSystemImpl::new());
                #[cfg(not(all(unix, any(feature = "fontconfig", not(target_os = "macos")))))]
                panic!("fontconfig not compiled in");
            }
            FontSystemSelection::FontLoaderAndFreeType => {
                #[cfg(any(target_os = "macos", windows))]
                return Rc::new(fontloader_and_freetype::FontSystemImpl::new());
                #[cfg(not(any(target_os = "macos", windows)))]
                panic!("font-loader not compiled in");
            }
            FontSystemSelection::CoreText => {
                #[cfg(target_os = "macos")]
                return Rc::new(coretext::FontSystemImpl::new());
                #[cfg(not(target_os = "macos"))]
                panic!("coretext not compiled in");
            }
            FontSystemSelection::FontKit => {
                #[cfg(any(target_os = "macos", windows))]
                return Rc::new(crate::font::fontkit::FontSystemImpl::new(true));
                #[cfg(not(any(target_os = "macos", windows)))]
                panic!("fontkit not compiled in");
            }
            FontSystemSelection::FontKitAndFreeType => {
                #[cfg(any(target_os = "macos", windows))]
                return Rc::new(crate::font::fontkit::FontSystemImpl::new(false));
                #[cfg(not(any(target_os = "macos", windows)))]
                panic!("fontkit not compiled in");
            }
        }
    }

    pub fn variants() -> Vec<&'static str> {
        vec![
            "FontConfigAndFreeType",
            "FontLoaderAndFreeType",
            "FontKitAndFreeType",
            "FontKit",
            "CoreText",
        ]
    }

    pub fn set_default(self) {
        DEFAULT_FONT_SYSTEM.with(|def| {
            *def.borrow_mut() = self;
        });
    }

    pub fn get_default() -> Self {
        DEFAULT_FONT_SYSTEM.with(|def| *def.borrow())
    }
}

impl std::str::FromStr for FontSystemSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "fontconfigandfreetype" => Ok(FontSystemSelection::FontConfigAndFreeType),
            "fontloaderandfreetype" => Ok(FontSystemSelection::FontLoaderAndFreeType),
            "coretext" => Ok(FontSystemSelection::CoreText),
            "fontkit" => Ok(FontSystemSelection::FontKit),
            "fontkitandfreetype" => Ok(FontSystemSelection::FontKitAndFreeType),
            _ => Err(format_err!(
                "{} is not a valid FontSystemSelection variant, possible values are {:?}",
                s,
                FontSystemSelection::variants()
            )),
        }
    }
}

impl FontConfiguration {
    /// Create a new empty configuration
    pub fn new(system: FontSystemSelection) -> Self {
        let loader = FontLocatorSelection::get_default().new_locator();
        Self {
            fonts: RefCell::new(HashMap::new()),
            loader,
            new_fonts: RefCell::new(HashMap::new()),
            system: system.new_font_system(),
            metrics: RefCell::new(None),
            font_scale: RefCell::new(1.0),
            dpi_scale: RefCell::new(1.0),
            config_generation: RefCell::new(configuration().generation()),
        }
    }

    pub fn resolve_font(&self, style: &TextStyle) -> Fallible<Rc<LoadedFont>> {
        let mut fonts = self.new_fonts.borrow_mut();
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

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    pub fn cached_font(&self, style: &TextStyle) -> Result<Rc<RefCell<Box<dyn NamedFont>>>, Error> {
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

        let scale = *self.dpi_scale.borrow() * *self.font_scale.borrow();
        let font = Rc::new(RefCell::new(self.system.load_font(&config, style, scale)?));
        fonts.insert(style.clone(), Rc::clone(&font));
        Ok(font)
    }

    pub fn change_scaling(&self, font_scale: f64, dpi_scale: f64) {
        *self.dpi_scale.borrow_mut() = dpi_scale;
        *self.font_scale.borrow_mut() = font_scale;
        self.fonts.borrow_mut().clear();
        self.metrics.borrow_mut().take();
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self) -> Result<Rc<RefCell<Box<dyn NamedFont>>>, Error> {
        self.cached_font(&configuration().font)
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
        let metrics = font.borrow_mut().get_fallback(0)?.metrics();

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

#[allow(dead_code)]
pub fn shape_with_harfbuzz(
    font: &mut dyn NamedFont,
    font_idx: system::FallbackIdx,
    s: &str,
) -> Result<Vec<GlyphInfo>, Error> {
    let features = vec![
        // kerning
        harfbuzz::feature_from_string("kern")?,
        // ligatures
        harfbuzz::feature_from_string("liga")?,
        // contextual ligatures
        harfbuzz::feature_from_string("clig")?,
    ];

    let mut buf = harfbuzz::Buffer::new()?;
    buf.set_script(harfbuzz::hb_script_t::HB_SCRIPT_LATIN);
    buf.set_direction(harfbuzz::hb_direction_t::HB_DIRECTION_LTR);
    buf.set_language(harfbuzz::language_from_string("en")?);
    buf.add_str(s);

    {
        let fallback = font.get_fallback(font_idx).map_err(|e| {
            let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
            e.context(format!("while shaping {:x?}", chars))
        })?;
        fallback.harfbuzz_shape(&mut buf, Some(features.as_slice()));
    }

    let infos = buf.glyph_infos();
    let positions = buf.glyph_positions();

    let mut cluster = Vec::new();

    let mut last_text_pos = None;
    let mut first_fallback_pos = None;

    // Compute the lengths of the text clusters.
    // Ligatures and combining characters mean
    // that a single glyph can take the place of
    // multiple characters.  The 'cluster' member
    // of the glyph info is set to the position
    // in the input utf8 text, so we make a pass
    // over the set of clusters to look for differences
    // greater than 1 and backfill the length of
    // the corresponding text fragment.  We need
    // the fragments to properly handle fallback,
    // and they're handy to have for debugging
    // purposes too.
    let mut sizes = Vec::with_capacity(s.len());
    for (i, info) in infos.iter().enumerate() {
        let pos = info.cluster as usize;
        let mut size = 1;
        if let Some(last_pos) = last_text_pos {
            let diff = pos - last_pos;
            if diff > 1 {
                sizes[i - 1] = diff;
            }
        } else if pos != 0 {
            size = pos;
        }
        last_text_pos = Some(pos);
        sizes.push(size);
    }
    if let Some(last_pos) = last_text_pos {
        let diff = s.len() - last_pos;
        if diff > 1 {
            let last = sizes.len() - 1;
            sizes[last] = diff;
        }
    }
    //debug!("sizes: {:?}", sizes);

    // Now make a second pass to determine if we need
    // to perform fallback to a later font.
    // We can determine this by looking at the codepoint.
    for (i, info) in infos.iter().enumerate() {
        let pos = info.cluster as usize;
        if info.codepoint == 0 {
            if first_fallback_pos.is_none() {
                // Start of a run that needs fallback
                first_fallback_pos = Some(pos);
            }
        } else if let Some(start_pos) = first_fallback_pos {
            // End of a fallback run
            //debug!("range: {:?}-{:?} needs fallback", start, pos);

            let substr = &s[start_pos..pos];
            let mut shape = match shape_with_harfbuzz(font, font_idx + 1, substr) {
                Ok(shape) => Ok(shape),
                Err(e) => {
                    error!("{:?} for {:?}", e, substr);
                    if font_idx == 0 && s == "?" {
                        bail!("unable to find any usable glyphs for `?` in font_idx 0");
                    }
                    shape_with_harfbuzz(font, 0, "?")
                }
            }?;

            // Fixup the cluster member to match our current offset
            for mut info in &mut shape {
                info.cluster += start_pos as u32;
            }
            cluster.append(&mut shape);

            first_fallback_pos = None;
        }
        if info.codepoint != 0 {
            if s.is_char_boundary(pos) && s.is_char_boundary(pos + sizes[i]) {
                let text = &s[pos..pos + sizes[i]];
                //debug!("glyph from `{}`", text);
                cluster.push(GlyphInfo::new(text, font_idx, info, &positions[i]));
            } else {
                cluster.append(&mut shape_with_harfbuzz(font, 0, "?")?);
            }
        }
    }

    // Check to see if we started and didn't finish a
    // fallback run.
    if let Some(start_pos) = first_fallback_pos {
        let substr = &s[start_pos..];
        if false {
            debug!(
                "at end {:?}-{:?} needs fallback {}",
                start_pos,
                s.len() - 1,
                substr,
            );
        }
        let mut shape = match shape_with_harfbuzz(font, font_idx + 1, substr) {
            Ok(shape) => Ok(shape),
            Err(e) => {
                error!("{:?} for {:?}", e, substr);
                if font_idx == 0 && s == "?" {
                    bail!("unable to find any usable glyphs for `?` in font_idx 0");
                }
                shape_with_harfbuzz(font, 0, "?")
            }
        }?;
        // Fixup the cluster member to match our current offset
        for mut info in &mut shape {
            info.cluster += start_pos as u32;
        }
        cluster.append(&mut shape);
    }

    //debug!("shaped: {:#?}", cluster);

    Ok(cluster)
}
