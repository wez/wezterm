use failure::Error;
mod ftfont;
#[cfg(unix)]
mod hbwrap;
#[cfg(unix)]
use self::hbwrap as harfbuzz;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub mod system;
pub use self::system::*;

pub mod ftwrap;

#[cfg(all(unix, not(target_os = "macos"), not(feature = "force-rusttype")))]
pub mod fcwrap;
#[cfg(all(unix, not(target_os = "macos"), not(feature = "force-rusttype")))]
pub mod fontconfigandfreetype;
#[cfg(all(unix, not(target_os = "macos"), not(feature = "force-rusttype")))]
use self::fontconfigandfreetype::FontSystemImpl;

#[cfg(all(target_os = "macos", not(feature = "force-rusttype")))]
pub mod coretext;
#[cfg(all(target_os = "macos", not(feature = "force-rusttype")))]
use self::coretext::FontSystemImpl;

pub mod fontloader;
#[cfg(any(windows, feature = "force-rusttype"))]
pub mod fontloader_and_rusttype;
pub mod rtype;
#[cfg(any(windows, feature = "force-rusttype"))]
use self::fontloader_and_rusttype::FontSystemImpl;

use super::config::{Config, TextStyle};
use term::CellAttributes;

type FontPtr = Rc<RefCell<Box<NamedFont>>>;

/// Matches and loads fonts for a given input style
pub struct FontConfiguration {
    config: Rc<Config>,
    fonts: RefCell<HashMap<TextStyle, FontPtr>>,
    system: FontSystemImpl,
}

impl FontConfiguration {
    /// Create a new empty configuration
    pub fn new(config: Rc<Config>) -> Self {
        Self {
            config,
            fonts: RefCell::new(HashMap::new()),
            system: FontSystemImpl::new(),
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
            };
        };

        for rule in &self.config.font_rules {
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

#[allow(dead_code)]
#[cfg(unix)]
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
        } else if let Some(start_pos) = first_fallback_pos {
            // End of a fallback run
            //debug!("range: {:?}-{:?} needs fallback", start, pos);

            let substr = &s[start_pos..pos];
            let mut shape = match shape_with_harfbuzz(font, font_idx + 1, substr) {
                Ok(shape) => Ok(shape),
                Err(e) => {
                    eprintln!("{:?} for {:?}", e, substr);
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
                eprintln!("{:?} for {:?}", e, substr);
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
