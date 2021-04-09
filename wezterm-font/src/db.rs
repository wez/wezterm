//! A font-database to keep track of fonts that we've located

use crate::ftwrap::Library;
use crate::locator::FontDataSource;
use crate::parser::{load_built_in_fonts, parse_and_collect_font_info, FontMatch, ParsedFont};
use anyhow::Context;
use config::{Config, FontAttributes};
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

struct Entry {
    parsed: ParsedFont,
    coverage: Mutex<Option<RangeSet<u32>>>,
}

impl Entry {
    /// Parses out the underlying TTF data and produces a RangeSet holding
    /// the set of codepoints for which the font has coverage.
    fn compute_coverage(&self) -> anyhow::Result<RangeSet<u32>> {
        let lib = Library::new()?;
        let face = lib
            .face_from_locator(&self.parsed.handle)
            .with_context(|| format!("freetype parsing {:?}", self.parsed))?;

        Ok(face.compute_coverage())
    }

    /// Computes the intersection of the wanted set of codepoints with
    /// the set of codepoints covered by this font entry.
    /// Computes the codepoint coverage for this font entry if we haven't
    /// already done so.
    fn coverage_intersection(&self, wanted: &RangeSet<u32>) -> anyhow::Result<RangeSet<u32>> {
        let mut coverage = self.coverage.lock().unwrap();
        if coverage.is_none() {
            let t = std::time::Instant::now();
            coverage.replace(self.compute_coverage().context("compute_coverage")?);
            let elapsed = t.elapsed();
            metrics::histogram!("font.compute.codepoint.coverage", elapsed);
            log::debug!(
                "{} codepoint coverage computed in {:?}",
                self.parsed.names().full_name,
                elapsed
            );
        }

        Ok(wanted.intersection(coverage.as_ref().unwrap()))
    }
}

pub struct FontDatabase {
    by_family: HashMap<String, Vec<Arc<Entry>>>,
    by_full_name: HashMap<String, Arc<Entry>>,
}

impl FontDatabase {
    pub fn new() -> Self {
        Self {
            by_family: HashMap::new(),
            by_full_name: HashMap::new(),
        }
    }

    fn load_font_info(&mut self, font_info: Vec<ParsedFont>) {
        for parsed in font_info {
            let entry = Arc::new(Entry {
                parsed,
                coverage: Mutex::new(None),
            });

            if let Some(family) = entry.parsed.names().family.as_ref() {
                self.by_family
                    .entry(family.to_string())
                    .or_insert_with(Vec::new)
                    .push(Arc::clone(&entry));
            }

            self.by_full_name
                .entry(entry.parsed.names().full_name.clone())
                .or_insert(entry);
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

        for entry in self.by_full_name.values() {
            let covered = entry
                .coverage_intersection(&wanted_range)
                .with_context(|| format!("coverage_interaction for {:?}", entry.parsed))?;
            let len = covered.len();
            if len > 0 {
                matches.push((len, entry.parsed.clone()));
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
        if let Some(entry) = self.by_full_name.get(&font_attr.family) {
            if entry.parsed.matches_attributes(font_attr) == FontMatch::FullName {
                return Some(&entry.parsed);
            }
        }

        if let Some(family) = self.by_family.get(&font_attr.family) {
            let mut candidates = vec![];
            for entry in family {
                let res = entry.parsed.matches_attributes(font_attr);
                if res != FontMatch::NoMatch {
                    candidates.push((res, entry));
                }
            }
            candidates.sort_by(|a, b| a.0.cmp(&b.0));
            let best = candidates.first()?;
            return Some(&best.1.parsed);
        }

        None
    }
}
