use crate::fcwrap;
use crate::locator::{FontDataHandle, FontDataSource, FontLocator};
use crate::parser::{FontMatch, ParsedFont};
use anyhow::Context;
use config::FontAttributes;
use fcwrap::{to_fc_weight, to_fc_width, CharSet, Pattern as FontPattern};
use std::collections::HashSet;
use std::convert::TryInto;

/// Allow for both monospace and dual spacing; both of these are
/// fixed width styles so are desirable for a terminal use case.
const SPACING: [i32; 2] = [fcwrap::FC_MONO, fcwrap::FC_DUAL];

/// A FontLocator implemented using the system font loading
/// functions provided by font-config
pub struct FontConfigFontLocator {}

impl FontLocator for FontConfigFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut fonts = vec![];

        fn by_family(attr: &FontAttributes, spacing: i32) -> anyhow::Result<FontPattern> {
            let mut pattern = FontPattern::new()?;
            let start = std::time::Instant::now();
            pattern.family(&attr.family)?;
            pattern.add_integer("weight", to_fc_weight(attr.weight))?;
            pattern.add_integer("width", to_fc_width(attr.width))?;
            pattern.add_integer(
                "slant",
                if attr.italic {
                    fcwrap::FC_SLANT_ITALIC
                } else {
                    fcwrap::FC_SLANT_ROMAN
                },
            )?;
            pattern.add_integer("spacing", spacing)?;

            log::trace!("fc pattern before config subst: {:?}", pattern);
            pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
            pattern.default_substitute();

            let best = pattern.get_best_match()?;
            log::trace!(
                "best match took {:?} to compute and is {:?}",
                start.elapsed(),
                best
            );
            Ok(best)
        }

        fn by_postscript(attr: &FontAttributes, _spacing: i32) -> anyhow::Result<FontPattern> {
            let mut pattern = FontPattern::new()?;
            let start = std::time::Instant::now();
            pattern.add_string("postscriptname", &attr.family)?;
            let matches = pattern.list()?;
            for best in matches.iter() {
                log::trace!(
                    "listing by postscriptname took {:?} to compute and is {:?}",
                    start.elapsed(),
                    best
                );
                return Ok(best);
            }

            log::trace!(
                "listing by postscriptname took {:?} to compute and produced no results",
                start.elapsed(),
            );
            anyhow::bail!("no match for postscript name");
        }

        for attr in fonts_selection {
            for &spacing in &SPACING {
                if loaded.contains(&attr) {
                    continue;
                }

                // First, we assume that attr.family is the family name.
                // If that doesn't work, we try by postscript name.
                for resolver in &[by_family, by_postscript] {
                    match resolver(attr, spacing) {
                        Ok(best) => {
                            let file = best.get_file()?;
                            let index = best.get_integer("index")? as u32;
                            let variation = index >> 16;
                            let index = index & 0xffff;
                            let handle = FontDataHandle {
                                source: FontDataSource::OnDisk(file.into()),
                                index,
                                variation,
                            };

                            // fontconfig will give us a boatload of random fallbacks.
                            // so we need to parse the returned font
                            // here to see if we got what we asked for.
                            if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                                if parsed.matches_attributes(attr) != FontMatch::NoMatch {
                                    log::trace!("found font-config match for {:?}", parsed.names());
                                    fonts.push(parsed);
                                    loaded.insert(attr.clone());
                                }
                            }
                        }
                        Err(err) => log::trace!("while searching for {:?}: {:#}", attr, err),
                    }
                }
            }
        }

        Ok(fonts)
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut charset = CharSet::new()?;
        for &c in codepoints {
            charset.add(c)?;
        }

        let mut pattern = FontPattern::new()?;
        pattern.add_charset(&charset)?;
        pattern.add_integer("weight", 80)?;
        pattern.add_integer("slant", 0)?;

        let mut lists = vec![pattern
            .list()
            .context("pattern.list with no spacing constraint")?];

        for &spacing in &SPACING {
            pattern.delete_property("spacing")?;
            pattern.add_integer("spacing", spacing)?;
            lists.push(
                pattern
                    .list()
                    .with_context(|| format!("pattern.list with spacing={}", spacing))?,
            );
        }

        let mut fonts = vec![];

        for list in lists {
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

                let handle = FontDataHandle {
                    source: FontDataSource::OnDisk(file.into()),
                    index: pat.get_integer("index")?.try_into()?,
                    variation: 0,
                };
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    fonts.push(parsed);
                }
            }
        }

        Ok(fonts)
    }
}
