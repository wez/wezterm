//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
use crate::config::Config;
use crate::config::FontAttributes;
use crate::font::locator::FontDataHandle;
use allsorts::fontfile::FontFile;
use allsorts::tables::{
    FontTableProvider, HeadTable, MaxpTable, NameTable, OffsetTable, OpenTypeFont, TTCHeader,
};
use failure::{bail, format_err, Fallible};
use std::convert::TryInto;
use std::path::{Path, PathBuf};

/// Represents a parsed font
pub struct ParsedFont {
    index: usize,
    file: gah::OwnedFontFile,
    names: Names,
}

#[derive(Debug)]
pub struct Names {
    full_name: String,
    unique: Option<String>,
    family: Option<String>,
    sub_family: Option<String>,
    postscript_name: Option<String>,
}

impl Names {
    fn from_name_table_data(name_table: &[u8]) -> Fallible<Names> {
        Ok(Names {
            full_name: get_name(name_table, 4)?,
            unique: get_name(name_table, 3).ok(),
            family: get_name(name_table, 1).ok(),
            sub_family: get_name(name_table, 2).ok(),
            postscript_name: get_name(name_table, 6).ok(),
        })
    }
}

impl ParsedFont {
    /// Load FontDataHandle's for fonts that match the configuration
    /// and that are found in the config font_dirs list.
    pub fn load_fonts(
        config: &Config,
        fonts_selection: &[FontAttributes],
    ) -> Fallible<Vec<FontDataHandle>> {
        // First discover the available fonts
        let mut font_info = vec![];
        for path in &config.font_dirs {
            for entry in walkdir::WalkDir::new(path).into_iter() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let path = entry.path();
                parse_and_collect_font_info(path, &mut font_info).ok();
            }
        }

        // Second, apply matching rules in order. We can't match
        // against the font files as we discover them because the
        // filesystem iteration order is arbitrary whereas our
        // fonts_selection is strictly ordered
        let mut handles = vec![];
        for attr in fonts_selection {
            for (names, path, index) in &font_info {
                if font_info_matches(attr, names) {
                    log::warn!(
                        "Using {} from {} index {}",
                        names.full_name,
                        path.display(),
                        index
                    );
                    handles.push(FontDataHandle::OnDisk {
                        path: path.clone(),
                        index: (*index).try_into()?,
                    });
                }
            }
        }
        Ok(handles)
    }

    pub fn from_locator(handle: &FontDataHandle) -> Fallible<Self> {
        let (data, index) = match handle {
            FontDataHandle::Memory { data, index } => (data.to_vec(), *index),
            FontDataHandle::OnDisk { path, index } => {
                let data = std::fs::read(path)?;
                (data, *index)
            }
        };

        let index = index as usize;

        let file = parse(data)?;

        let names = file.rent(|file| -> Fallible<Names> {
            let name_table = name_table_data(file, index)?;
            let names = Names::from_name_table_data(name_table)?;
            Ok(names)
        })?;

        Ok(Self { index, file, names })
    }

    pub fn names(&self) -> &Names {
        &self.names
    }
}

rental! {
    /// This is horrible, but I don't see an obvious way to
    /// end up with an owned FontFile instance from the standard
    /// set of functions exported by allsorts
    mod gah {
        use super::*;
        #[rental]
        pub struct OwnedFontFile {
            head: Vec<u8>,
            file: FontFile<'head>,
        }
    }
}

/// Parse bytes into an OwnedFontFile
fn parse(data: Vec<u8>) -> Fallible<gah::OwnedFontFile> {
    gah::OwnedFontFile::try_new(data, |data| {
        let scope = allsorts::binary::read::ReadScope::new(&data);
        let file = scope.read::<FontFile>()?;
        Ok(file)
    })
    .map_err(|e| e.0)
}

fn collect_font_info(
    name_table_data: &[u8],
    path: &Path,
    index: usize,
    infos: &mut Vec<(Names, PathBuf, usize)>,
) -> Fallible<()> {
    let names = Names::from_name_table_data(name_table_data)?;
    infos.push((names, path.to_path_buf(), index));
    Ok(())
}

fn font_info_matches(attr: &FontAttributes, names: &Names) -> bool {
    if attr.family == names.full_name {
        true
    } else if let Some(fam) = names.family.as_ref() {
        // TODO: correctly match using family and sub-family;
        // this is a pretty rough approximation
        if attr.family == *fam {
            match names.sub_family.as_ref().map(String::as_str) {
                Some("Italic") if attr.italic => true,
                Some("Bold") if attr.bold => true,
                None => true,
                _ => false,
            }
        } else {
            false
        }
    } else {
        false
    }
}

fn parse_and_collect_font_info(
    path: &Path,
    font_info: &mut Vec<(Names, PathBuf, usize)>,
) -> Fallible<()> {
    let data = std::fs::read(path)?;
    let scope = allsorts::binary::read::ReadScope::new(&data);
    let file = scope.read::<FontFile>()?;

    match file {
        FontFile::OpenType(f) => match &f.font {
            OpenTypeFont::Single(ttf) => {
                let data = ttf
                    .read_table(&f.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| format_err!("name table is not present"))?;
                collect_font_info(data.data(), path, 0, font_info)?;
            }
            OpenTypeFont::Collection(ttc) => {
                for (index, offset_table_offset) in ttc.offset_tables.iter().enumerate() {
                    let ttf = f
                        .scope
                        .offset(offset_table_offset as usize)
                        .read::<OffsetTable>()?;
                    let data = ttf
                        .read_table(&f.scope, allsorts::tag::NAME)?
                        .ok_or_else(|| format_err!("name table is not present"))?;
                    collect_font_info(data.data(), path, index, font_info).ok();
                }
            }
        },
        _ => bail!("WOFFs not supported"),
    }

    Ok(())
}

/// Extract the name table data from a font
fn name_table_data<'a>(font_file: &FontFile<'a>, idx: usize) -> Fallible<&'a [u8]> {
    match font_file {
        FontFile::OpenType(f) => match &f.font {
            OpenTypeFont::Single(ttf) => {
                let data = ttf
                    .read_table(&f.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| format_err!("name table is not present"))?;
                Ok(data.data())
            }
            OpenTypeFont::Collection(ttc) => {
                let offset_table_offset = ttc.offset_tables.read_item(idx).map_err(|e| {
                    format_err!("font idx={} is not present in ttc file: {}", idx, e)
                })?;
                let ttf = f
                    .scope
                    .offset(offset_table_offset as usize)
                    .read::<OffsetTable>()?;
                let data = ttf
                    .read_table(&f.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| format_err!("name table is not present"))?;
                Ok(data.data())
            }
        },
        _ => bail!("WOFFs not supported"),
    }
}

/// Extract a name from the name table
fn get_name(name_table_data: &[u8], name_id: u16) -> Fallible<String> {
    let cstr = allsorts::get_name::fontcode_get_name(name_table_data, name_id)?
        .ok_or_else(|| format_err!("name_id {} not found", name_id))?;
    cstr.into_string()
        .map_err(|e| format_err!("name_id {} is not representable as String: {}", name_id, e))
}
