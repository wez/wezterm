//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
use crate::font::loader::FontDataHandle;
use allsorts::fontfile::FontFile;
use allsorts::tables::{
    FontTableProvider, HeadTable, MaxpTable, NameTable, OffsetTable, OpenTypeFont, TTCHeader,
};
use failure::{bail, format_err, Fallible};

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

impl ParsedFont {
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
            Ok(Names {
                full_name: get_name(name_table, 4)?,
                unique: get_name(name_table, 3).ok(),
                family: get_name(name_table, 1).ok(),
                sub_family: get_name(name_table, 2).ok(),
                postscript_name: get_name(name_table, 6).ok(),
            })
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
