use crate::fcwrap;
use crate::locator::{FontDataHandle, FontLocator};
use anyhow::Context;
use config::FontAttributes;
use fcwrap::{CharSet, Pattern as FontPattern};
use std::collections::HashSet;
use std::convert::TryInto;

/// A FontLocator implemented using the system font loading
/// functions provided by font-config
pub struct FontConfigFontLocator {}

impl FontLocator for FontConfigFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = vec![];

        for attr in fonts_selection {
            let mut pattern = FontPattern::new()?;
            let start = std::time::Instant::now();
            pattern.family(&attr.family)?;
            pattern.add_integer("weight", if attr.bold { 200 } else { 80 })?;
            pattern.add_integer("slant", if attr.italic { 100 } else { 0 })?;
            pattern.monospace()?;
            log::trace!("fc pattern before config subst: {:?}", pattern);
            pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
            pattern.default_substitute();

            let best = pattern.get_best_match()?;
            log::trace!(
                "best match took {:?} to compute and is {:?}",
                start.elapsed(),
                best
            );

            let file = best.get_file()?;
            let handle = FontDataHandle::OnDisk {
                path: file.into(),
                index: best.get_integer("index")?.try_into()?,
            };

            // fontconfig will give us a boatload of random fallbacks.
            // so we need to parse the returned font
            // here to see if we got what we asked for.
            if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                if crate::parser::font_info_matches(attr, parsed.names()) {
                    fonts.push(handle);
                    loaded.insert(attr.clone());
                    log::trace!("found font-config match for {:?}", parsed.names());
                }
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut charset = CharSet::new()?;
        for &c in codepoints {
            charset.add(c)?;
        }

        let mut pattern = FontPattern::new()?;
        pattern.add_charset(&charset)?;
        pattern.add_integer("weight", 80)?;
        pattern.add_integer("slant", 0)?;

        let any_spacing = pattern
            .list()
            .context("pattern.list with no spacing constraint")?;
        pattern.monospace()?;
        let mono_spacing = pattern
            .list()
            .context("pattern.list with monospace constraint")?;

        let mut fonts = vec![];

        for list in &[mono_spacing, any_spacing] {
            for pat in list.iter() {
                let num = pat.charset_intersect_count(&charset)?;
                if num == 0 {
                    log::error!(
                        "Skipping bogus font-config result {:?} because it doesn't overlap",
                        pat
                    );
                    continue;
                }

                let file = pat.get_file().context("pat.get_file")?;

                let handle = FontDataHandle::OnDisk {
                    path: file.into(),
                    index: pat.get_integer("index")?.try_into()?,
                };

                fonts.push(handle);
            }
        }

        Ok(fonts)
    }
}
