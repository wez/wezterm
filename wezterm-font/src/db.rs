//! A font-database to keep track of fonts that we've located

use crate::locator::FontDataSource;
use crate::parser::{load_built_in_fonts, parse_and_collect_font_info, ParsedFont};
use anyhow::Context;
use config::{Config, FontAttributes};
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub struct FontDatabase {
    by_full_name: HashMap<String, ParsedFont>,
}

impl FontDatabase {
    pub fn new() -> Self {
        Self {
            by_full_name: HashMap::new(),
        }
    }

    fn load_font_info(&mut self, font_info: Vec<ParsedFont>) {
        for parsed in font_info {
            self.by_full_name
                .entry(parsed.names().full_name.clone())
                .or_insert(parsed);
        }
    }

    /// Build up the database from the fonts found in the configured font dirs
    /// and from the built-in selection of fonts
    pub fn with_font_dirs(config: &Config) -> anyhow::Result<Self> {
        let mut font_info = vec![];
        for path in &config.font_dirs {
            for entry in walkdir::WalkDir::new(path).into_iter() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let source = FontDataSource::OnDisk(entry.path().to_path_buf());
                parse_and_collect_font_info(&source, &mut font_info)
                    .map_err(|err| {
                        log::trace!("failed to read {:?}: {:#}", source, err);
                        err
                    })
                    .ok();
            }
        }

        let mut db = Self::new();
        db.load_font_info(font_info);
        db.print_available();
        Ok(db)
    }

    pub fn print_available(&self) {
        let mut names = self.by_full_name.keys().collect::<Vec<_>>();
        names.sort();
        for name in names {
            log::debug!("available font: wezterm.font(\"{}\") ", name);
        }
    }

    pub fn with_built_in() -> anyhow::Result<Self> {
        let mut font_info = vec![];
        load_built_in_fonts(&mut font_info)?;
        let mut db = Self::new();
        db.load_font_info(font_info);
        db.print_available();
        Ok(db)
    }

    pub fn resolve_multiple(
        &self,
        fonts: &[FontAttributes],
        handles: &mut Vec<ParsedFont>,
        loaded: &mut HashSet<FontAttributes>,
    ) {
        for attr in fonts {
            if loaded.contains(attr) {
                continue;
            }
            if let Some(handle) = self.resolve(attr) {
                handles.push(handle.clone());
                loaded.insert(attr.clone());
            }
        }
    }

    /// Equivalent to FontLocator::locate_fallback_for_codepoints
    pub fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>> {
        let mut wanted_range = RangeSet::new();
        for &c in codepoints {
            wanted_range.add(c as u32);
        }

        let mut matches = vec![];

        for parsed in self.by_full_name.values() {
            if parsed.names().family.as_ref().map(|s| s.as_str())
                == Some("Last Resort High-Efficiency")
            {
                continue;
            }
            let covered = parsed
                .coverage_intersection(&wanted_range)
                .with_context(|| format!("coverage_interaction for {:?}", parsed))?;
            let len = covered.len();
            if len > 0 {
                matches.push((len, parsed.clone()));
            }
        }

        // Add the handles in order of descending coverage; the idea being
        // that if a font has a large coverage then it is probably a better
        // candidate and more likely to result in other glyphs matching
        // in future shaping calls.
        matches.sort_by(|(a_len, a), (b_len, b)| {
            let primary = a_len.cmp(&b_len).reverse();
            if primary == Ordering::Equal {
                a.cmp(b)
            } else {
                primary
            }
        });

        Ok(matches.into_iter().map(|(_len, handle)| handle).collect())
    }

    pub fn resolve(&self, font_attr: &FontAttributes) -> Option<&ParsedFont> {
        let candidates: Vec<&ParsedFont> = self
            .by_full_name
            .values()
            .filter_map(|parsed| {
                if parsed.matches_name(font_attr) {
                    Some(parsed)
                } else {
                    None
                }
            })
            .collect();

        if let Some(idx) = ParsedFont::best_matching_index(font_attr, &candidates) {
            return candidates.get(idx).map(|&p| p);
        }

        None
    }
}
