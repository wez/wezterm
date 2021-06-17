use crate::fcwrap;
use crate::locator::{FontDataHandle, FontDataSource, FontLocator, FontOrigin};
use crate::parser::ParsedFont;
use anyhow::Context;
use config::FontAttributes;
use fcwrap::{CharSet, FontSet, Pattern as FontPattern, FC_DUAL, FC_MONO};
use std::collections::HashSet;
use std::convert::TryInto;

/// Allow for both monospace and dual spacing; both of these are
/// fixed width styles so are desirable for a terminal use case.
const SPACING: [i32; 2] = [FC_MONO, FC_DUAL];

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

        /// Returns a FontSet list filtered to only mono/dual spaced fonts
        fn monospaced(matches: FontSet) -> Vec<FontPattern> {
            matches
                .iter()
                .filter_map(|p| match p.get_integer("spacing") {
                    Ok(n) if n == FC_MONO || n == FC_DUAL => Some(p),
                    // (probably!) no spacing defined. Assume monospace.
                    Err(_) => Some(p),
                    _ => None,
                })
                .collect()
        }

        /// Search fontconfig using only the family name
        fn by_family(attr: &FontAttributes) -> anyhow::Result<Vec<FontPattern>> {
            let mut pattern = FontPattern::new()?;
            let start = std::time::Instant::now();
            pattern.family(&attr.family)?;
            let matches = monospaced(pattern.list()?);
            log::trace!(
                "listing by family took {:?} to compute and is {:?}",
                start.elapsed(),
                matches
            );
            Ok(matches)
        }

        /// Search fontconfig using on the postscript name
        fn by_postscript(attr: &FontAttributes) -> anyhow::Result<Vec<FontPattern>> {
            let mut pattern = FontPattern::new()?;
            let start = std::time::Instant::now();
            pattern.add_string("postscriptname", &attr.family)?;
            let matches = monospaced(pattern.list()?);
            log::trace!(
                "listing by postscriptname took {:?} to compute and is {:?}",
                start.elapsed(),
                matches
            );
            Ok(matches)
        }

        for attr in fonts_selection {
            let mut candidates = vec![];

            // Aggregate results of both family and postscript name lookups
            for resolver in &[by_family, by_postscript] {
                match resolver(attr) {
                    Ok(matches) => {
                        for pat in matches {
                            let file = pat.get_file()?;
                            let index = pat.get_integer("index")? as u32;
                            let variation = index >> 16;
                            let index = index & 0xffff;
                            let handle = FontDataHandle {
                                source: FontDataSource::OnDisk(file.into()),
                                index,
                                variation,
                                origin: FontOrigin::FontConfig,
                            };

                            // fontconfig will give us a boatload of random fallbacks.
                            // so we need to parse the returned font
                            // here to see if we got what we asked for.
                            if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                                if parsed.matches_name(attr) {
                                    log::trace!("found font-config match for {:?}", parsed.names());
                                    candidates.push(parsed);
                                }
                            }
                        }
                    }
                    Err(err) => log::trace!("while searching for {:?}: {:#}", attr, err),
                }
            }

            // and apply our CSS-style font matching criteria
            if let Some(parsed) = ParsedFont::best_match(attr, candidates) {
                log::trace!("selected best font-config match {:?}", parsed.names());
                fonts.push(parsed);
                loaded.insert(attr.clone());
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
                    origin: FontOrigin::FontConfig,
                };
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    fonts.push(parsed);
                }
            }
        }

        Ok(fonts)
    }

    fn enumerate_all_fonts(&self) -> anyhow::Result<Vec<ParsedFont>> {
        let mut fonts = vec![];
        let pattern = FontPattern::new()?;
        let mut files = HashSet::new();
        for pat in pattern
            .list()
            .context("listing fonts from font-config")?
            .iter()
        {
            let file = pat.get_file().context("pat.get_file")?;
            if files.contains(&file) {
                continue;
            }
            files.insert(file.clone());

            let source = FontDataSource::OnDisk(file.into());
            if let Err(err) = crate::parser::parse_and_collect_font_info(
                &source,
                &mut fonts,
                FontOrigin::FontConfig,
            ) {
                log::warn!("While parsing: {:?}: {:#}", source, err);
            }
        }
        fonts.sort();

        Ok(fonts)
    }
}
