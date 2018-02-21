use failure::{self, Error};
use harfbuzz;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::slice;

pub mod ftwrap;
pub mod fcwrap;
pub mod system;
use self::system::{FontSystem, NamedFont};

pub use self::fcwrap::Pattern as FontPattern;

pub use self::system::GlyphInfo;
use super::config::{Config, TextStyle};
use term::CellAttributes;

struct FontConfigAndFreeType {}

impl system::FontSystem for FontConfigAndFreeType {
    fn load_font(&self, config: &Config, style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        let mut pattern = FontPattern::parse(&style.fontconfig_pattern)?;
        pattern.add_double("size", config.font_size)?;
        pattern.add_double("dpi", config.dpi)?;

        Ok(Box::new(Font::new(pattern)?))
    }
}

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    config: Config,
    fonts: RefCell<HashMap<TextStyle, Rc<RefCell<Box<NamedFont>>>>>,
    system: FontConfigAndFreeType,
}

impl FontConfiguration {
    /// Create a new empty configuration
    pub fn new(config: Config) -> Self {
        Self {
            config,
            fonts: RefCell::new(HashMap::new()),
            system: FontConfigAndFreeType {},
        }
    }

    /// Given a text style, load (with caching) the font that best
    /// matches according to the fontconfig pattern.
    pub fn cached_font(&self, style: &TextStyle) -> Result<Rc<RefCell<Box<NamedFont>>>, Error> {
        let mut fonts = self.fonts.borrow_mut();

        if let Some(entry) = fonts.get(style) {
            return Ok(Rc::clone(entry));
        }

        let font = Rc::new(RefCell::new(self.system.load_font(&self.config, style)?));
        fonts.insert(style.clone(), Rc::clone(&font));
        Ok(font)
    }

    /// Returns the baseline font specified in the configuration
    pub fn default_font(&self) -> Result<Rc<RefCell<Box<NamedFont>>>, Error> {
        self.cached_font(&self.config.font)
    }

    /// Apply the defined font_rules from the user configuration to
    /// produce the text style that best matches the supplied input
    /// cell attributes.
    pub fn match_style(&self, attrs: &CellAttributes) -> &TextStyle {
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
            }
        };

        for rule in self.config.font_rules.iter() {
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
        &self.config.font
    }
}

/// Holds a loaded font alternative
struct FontInfo {
    face: RefCell<ftwrap::Face>,
    font: RefCell<harfbuzz::Font>,
    /// nominal monospace cell height
    cell_height: f64,
    /// nominal monospace cell width
    cell_width: f64,
}


impl system::Font for FontInfo {
    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        self.font.borrow_mut().shape(buf, features)
    }
    fn has_color(&self) -> bool {
        let face = self.face.borrow();
        unsafe { ((*face.face).face_flags & ftwrap::FT_FACE_FLAG_COLOR as i64) != 0 }
    }

    fn metrics(&self) -> system::FontMetrics {
        let face = self.face.borrow();
        system::FontMetrics {
            cell_height: self.cell_height,
            cell_width: self.cell_width,
            descender: unsafe { (*face.face).descender },
        }
    }

    fn load_glyph(&self, glyph_pos: u32) -> Result<ftwrap::FT_GlyphSlotRec_, Error> {
        let render_mode = //ftwrap::FT_Render_Mode::FT_RENDER_MODE_NORMAL;
 //       ftwrap::FT_Render_Mode::FT_RENDER_MODE_LCD;
        ftwrap::FT_Render_Mode::FT_RENDER_MODE_LIGHT;

        // when changing the load flags, we also need
        // to change them for harfbuzz otherwise it won't
        // hint correctly
        let load_flags = (ftwrap::FT_LOAD_COLOR) as i32 |
            // enable FT_LOAD_TARGET bits.  There are no flags defined
            // for these in the bindings so we do some bit magic for
            // ourselves.  This is how the FT_LOAD_TARGET_() macro
            // assembles these bits.
            (render_mode as i32) << 16;

        self.font.borrow_mut().set_load_flags(load_flags);
        // This clone is conceptually unsafe, but ok in practice as we are
        // single threaded and don't load any other glyphs in the body of
        // this load_glyph() function.
        self.face
            .borrow_mut()
            .load_and_render_glyph(glyph_pos, load_flags, render_mode)
            .map(|g| g.clone())
    }
}

/// Holds "the" font selected by the user.  In actuality, it
/// holds the set of fallback fonts that match their criteria
pub struct Font {
    lib: ftwrap::Library,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FontInfo>,
}

impl Drop for Font {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

impl NamedFont for Font {
    fn get_fallback(&mut self, idx: system::FallbackIdx) -> Result<&system::Font, Error> {
        Ok(self.get_font(idx)?)
    }
    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        shape_with_harfbuzz(self, 0, s)
    }
}

impl Font {
    /// Construct a new Font from the user supplied pattern
    pub fn new(mut pattern: FontPattern) -> Result<Font, Error> {
        let mut lib = ftwrap::Library::new()?;
        lib.set_lcd_filter(
            ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT,
        )?;

        // Enable some filtering options and pull in the standard
        // fallback font selection from the user configuration
        pattern.monospace()?;
        pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
        pattern.default_substitute();

        // and obtain the selection with the best preference
        // at index 0.
        let font_list = pattern.sort(true)?;

        Ok(Font {
            lib,
            font_list,
            pattern,
            fonts: Vec::new(),
        })
    }

    fn load_next_fallback(&mut self) -> Result<(), Error> {
        let idx = self.fonts.len();
        let pat = self.font_list.iter().nth(idx).ok_or(failure::err_msg(
            "no more fallbacks",
        ))?;
        let pat = self.pattern.render_prepare(&pat)?;
        let file = pat.get_file()?;

        debug!("load_next_fallback: file={}", file);
        debug!("{}", pat.format("%{=unparse}")?);

        let size = pat.get_double("size")?;
        let dpi = pat.get_double("dpi")? as u32;
        debug!("set_char_size {} dpi={}", size, dpi);
        // Scaling before truncating to integer minimizes the chances of hitting
        // the fallback code for set_pixel_sizes below.
        let size = (size * 64.0) as i64;

        let mut face = self.lib.new_face(file, 0)?;

        match face.set_char_size(size, size, dpi, dpi) {
            Err(err) => {
                let sizes = unsafe {
                    let rec = &(*face.face);
                    slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.len() == 0 {
                    return Err(err);
                } else {
                    // Find the best matching size.
                    // We just take the biggest.
                    let mut size = 0i16;
                    for info in sizes.iter() {
                        size = size.max(info.height);
                    }
                    face.set_pixel_sizes(size as u32, size as u32)?;
                    debug!("fall back to set_pixel_sizes {}", size);
                }
            }
            Ok(_) => {}
        }
        let font = harfbuzz::Font::new(face.face);

        // Compute metrics for the nominal monospace cell
        let (cell_width, cell_height) = face.cell_metrics();
        debug!("metrics: width={} height={}", cell_width, cell_height);

        self.fonts.push(FontInfo {
            face: RefCell::new(face),
            font: RefCell::new(font),
            cell_height,
            cell_width,
        });
        Ok(())
    }

    fn get_font(&mut self, idx: usize) -> Result<&mut FontInfo, Error> {
        if idx >= self.fonts.len() {
            self.load_next_fallback()?;
            ensure!(
                idx < self.fonts.len(),
                "should not ask for a font later than the next prepared font"
            );
        }

        Ok(&mut self.fonts[idx])
    }
}

pub fn shape_with_harfbuzz(
    font: &mut NamedFont,
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
    buf.set_script(harfbuzz::HB_SCRIPT_LATIN);
    buf.set_direction(harfbuzz::HB_DIRECTION_LTR);
    buf.set_language(harfbuzz::language_from_string("en")?);
    buf.add_str(s);

    {
        let fallback = font.get_fallback(font_idx)?;
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
        } else if let Some(start) = first_fallback_pos {
            // End of a fallback run
            //debug!("range: {:?}-{:?} needs fallback", start, pos);

            let substr = &s[start..pos];
            let mut shape = shape_with_harfbuzz(font, font_idx + 1, substr)?;

            // Fixup the cluster member to match our current offset
            for info in shape.iter_mut() {
                info.cluster += start as u32;
            }
            cluster.append(&mut shape);

            first_fallback_pos = None;
        }
        if info.codepoint != 0 {
            let text = &s[pos..pos + sizes[i]];
            //debug!("glyph from `{}`", text);
            cluster.push(GlyphInfo::new(text, font_idx, info, &positions[i]));
        }
    }

    // Check to see if we started and didn't finish a
    // fallback run.
    if let Some(start) = first_fallback_pos {
        let substr = &s[start..];
        if false {
            debug!(
                "at end {:?}-{:?} needs fallback {}",
                start,
                s.len() - 1,
                substr,
            );
        }
        let mut shape = shape_with_harfbuzz(font, font_idx + 1, substr)?;
        // Fixup the cluster member to match our current offset
        for info in shape.iter_mut() {
            info.cluster += start as u32;
        }
        cluster.append(&mut shape);
    }

    //debug!("shaped: {:#?}", cluster);

    Ok(cluster)

}
