//! A font-database to keep track of fonts that we've located

use crate::parser::{font_info_matches, load_built_in_fonts, parse_and_collect_font_info, Names};
use crate::FontDataHandle;
use config::{Config, FontAttributes};
use rangeset::RangeSet;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

struct Entry {
    names: Names,
    handle: FontDataHandle,
    _coverage: RefCell<Option<RangeSet<u32>>>,
}

pub struct FontDatabase {
    by_family: HashMap<String, Vec<Rc<Entry>>>,
    by_full_name: HashMap<String, Rc<Entry>>,
}

impl FontDatabase {
    pub fn new() -> Self {
        Self {
            by_family: HashMap::new(),
            by_full_name: HashMap::new(),
        }
    }

    fn load_font_info(&mut self, font_info: Vec<(Names, PathBuf, FontDataHandle)>) {
        for (names, _path, handle) in font_info {
            let entry = Rc::new(Entry {
                names,
                handle,
                _coverage: RefCell::new(None),
            });

            if let Some(family) = entry.names.family.as_ref() {
                self.by_family
                    .entry(family.to_string())
                    .or_insert_with(Vec::new)
                    .push(Rc::clone(&entry));
            }

            self.by_full_name
                .entry(entry.names.full_name.clone())
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

                let path = entry.path();
                parse_and_collect_font_info(path, &mut font_info)
                    .map_err(|err| {
                        log::trace!("failed to read {}: {}", path.display(), err);
                        err
                    })
                    .ok();
            }
        }

        let mut db = Self::new();
        db.load_font_info(font_info);
        Ok(db)
    }

    pub fn print_available(&self) {
        let mut names = self.by_full_name.keys().collect::<Vec<_>>();
        names.sort();
        for name in names {
            log::warn!("available font: {}", name);
        }
    }

    pub fn with_built_in() -> anyhow::Result<Self> {
        let mut font_info = vec![];
        load_built_in_fonts(&mut font_info)?;
        let mut db = Self::new();
        db.load_font_info(font_info);
        Ok(db)
    }

    pub fn resolve_multiple(
        &self,
        fonts: &[FontAttributes],
        handles: &mut Vec<FontDataHandle>,
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

    pub fn resolve(&self, font_attr: &FontAttributes) -> Option<&FontDataHandle> {
        if let Some(entry) = self.by_full_name.get(&font_attr.family) {
            if font_info_matches(font_attr, &entry.names) {
                return Some(&entry.handle);
            }
        }

        if let Some(family) = self.by_family.get(&font_attr.family) {
            for entry in family {
                if font_info_matches(font_attr, &entry.names) {
                    return Some(&entry.handle);
                }
            }
        }

        None
    }
}
