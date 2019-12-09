use crate::font::ftwrap;
use crate::font::hbwrap as harfbuzz;
use crate::font::shaper::FontShaper;
use crate::font::system::GlyphInfo;
use failure::{bail, Fallible};
use log::{debug, error};
use std::cell::RefCell;

struct FontPair {
    face: ftwrap::Face,
    font: harfbuzz::Font,
}

pub struct HarfbuzzShaper {
    fonts: Vec<RefCell<FontPair>>,
}

impl HarfbuzzShaper {
    fn do_shape(
        &self,
        font_idx: crate::font::system::FallbackIdx,
        s: &str,
    ) -> Fallible<Vec<GlyphInfo>> {
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
            match self.fonts.get(font_idx) {
                Some(pair) => {
                    pair.borrow_mut()
                        .font
                        .shape(&mut buf, Some(features.as_slice()));
                }
                None => {
                    let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
                    bail!("No more fallbacks while shaping {:x?}", chars);
                }
            }
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
                let mut shape = match self.do_shape(font_idx + 1, substr) {
                    Ok(shape) => Ok(shape),
                    Err(e) => {
                        error!("{:?} for {:?}", e, substr);
                        if font_idx == 0 && s == "?" {
                            bail!("unable to find any usable glyphs for `?` in font_idx 0");
                        }
                        self.do_shape(0, "?")
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
                    cluster.append(&mut self.do_shape(0, "?")?);
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
            let mut shape = match self.do_shape(font_idx + 1, substr) {
                Ok(shape) => Ok(shape),
                Err(e) => {
                    error!("{:?} for {:?}", e, substr);
                    if font_idx == 0 && s == "?" {
                        bail!("unable to find any usable glyphs for `?` in font_idx 0");
                    }
                    self.do_shape(0, "?")
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
}

impl FontShaper for HarfbuzzShaper {
    fn shape(&self, text: &str) -> Fallible<Vec<GlyphInfo>> {
        self.do_shape(0, text)
    }
}
