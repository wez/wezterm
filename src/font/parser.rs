//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
#![allow(dead_code)]
use crate::config::Config;
use crate::config::FontAttributes;
use crate::font::locator::FontDataHandle;
use crate::font::shaper::{FallbackIdx, FontMetrics, GlyphInfo};
use allsorts::binary::read::{ReadScope, ReadScopeOwned};
use allsorts::font_data_impl::read_cmap_subtable;
use allsorts::fontfile::FontFile;
use allsorts::gpos::{gpos_apply, Info, Placement};
use allsorts::gsub::{gsub_apply_default, GlyphOrigin, RawGlyph};
use allsorts::layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB};
use allsorts::tables::cmap::{Cmap, CmapSubtable};
use allsorts::tables::{
    FontTableProvider, HeadTable, HheaTable, HmtxTable, MaxpTable, NameTable, OffsetTable,
    OpenTypeFile, OpenTypeFont, TTCHeader,
};
use allsorts::tag;
use failure::{bail, format_err, Fallible};
use std::convert::TryInto;
use std::path::{Path, PathBuf};

/// Represents a parsed font
pub struct ParsedFont {
    otf: OffsetTable<'static>,
    names: Names,

    cmap_subtable: CmapSubtable<'static>,
    gpos_cache: Option<LayoutCache<GPOS>>,
    gsub_cache: LayoutCache<GSUB>,
    gdef_table: Option<GDEFTable>,
    hmtx: HmtxTable<'static>,
    hhea: HheaTable,
    num_glyphs: u16,
    units_per_em: u16,

    // Must be last: this keeps the 'static items alive
    _scope: ReadScopeOwned,
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

        let owned_scope = ReadScopeOwned::new(ReadScope::new(&data));

        // This unsafe block and transmute are present so that we can
        // extend the lifetime of the OpenTypeFile that we produce here.
        // That in turn allows us to store all of these derived items
        // into a struct and manage their lifetimes together.
        let file: OpenTypeFile<'static> =
            unsafe { std::mem::transmute(owned_scope.scope().read::<OpenTypeFile>()?) };

        let otf = locate_offset_table(&file, index)?;
        let name_table = name_table_data(&otf, &file.scope)?;
        let names = Names::from_name_table_data(name_table)?;

        let head = otf
            .read_table(&file.scope, tag::HEAD)?
            .ok_or_else(|| format_err!("HEAD table missing or broken"))?
            .read::<HeadTable>()?;
        let cmap = otf
            .read_table(&file.scope, tag::CMAP)?
            .ok_or_else(|| format_err!("CMAP table missing or broken"))?
            .read::<Cmap>()?;
        let cmap_subtable: CmapSubtable<'static> =
            read_cmap_subtable(&cmap)?.ok_or_else(|| format_err!("CMAP subtable not found"))?;

        let maxp = otf
            .read_table(&file.scope, tag::MAXP)?
            .ok_or_else(|| format_err!("MAXP table not found"))?
            .read::<MaxpTable>()?;
        let num_glyphs = maxp.num_glyphs;

        let hhea = otf
            .read_table(&file.scope, tag::HHEA)?
            .ok_or_else(|| format_err!("HHEA table not found"))?
            .read::<HheaTable>()?;
        let hmtx = otf
            .read_table(&file.scope, tag::HMTX)?
            .ok_or_else(|| format_err!("HMTX table not found"))?
            .read_dep::<HmtxTable>((
                usize::from(maxp.num_glyphs),
                usize::from(hhea.num_h_metrics),
            ))?;

        let gsub_table = otf
            .find_table_record(tag::GSUB)
            .ok_or_else(|| format_err!("GSUB table record not found"))?
            .read_table(&file.scope)?
            .read::<LayoutTable<GSUB>>()?;
        let gdef_table: Option<GDEFTable> = otf
            .find_table_record(tag::GDEF)
            .map(|gdef_record| -> Fallible<GDEFTable> {
                Ok(gdef_record.read_table(&file.scope)?.read::<GDEFTable>()?)
            })
            .transpose()?;
        let opt_gpos_table = otf
            .find_table_record(tag::GPOS)
            .map(|gpos_record| -> Fallible<LayoutTable<GPOS>> {
                Ok(gpos_record
                    .read_table(&file.scope)?
                    .read::<LayoutTable<GPOS>>()?)
            })
            .transpose()?;
        let gsub_cache = new_layout_cache(gsub_table);
        let gpos_cache = opt_gpos_table.map(new_layout_cache);

        Ok(Self {
            otf,
            names,
            cmap_subtable,
            hmtx,
            hhea,
            gpos_cache,
            gsub_cache,
            gdef_table,
            num_glyphs,
            units_per_em: head.units_per_em,
            _scope: owned_scope,
        })
    }

    pub fn names(&self) -> &Names {
        &self.names
    }

    /// Resolve a char to the corresponding glyph in the font
    pub fn glyph_index_for_char(&self, c: char) -> Fallible<Option<u16>> {
        self.cmap_subtable
            .map_glyph(c as u32)
            .map_err(|e| format_err!("Error while looking up glyph {}: {}", c, e))
    }
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
    let file = scope.read::<OpenTypeFile>()?;

    match &file.font {
        OpenTypeFont::Single(ttf) => {
            let data = ttf
                .read_table(&file.scope, allsorts::tag::NAME)?
                .ok_or_else(|| format_err!("name table is not present"))?;
            collect_font_info(data.data(), path, 0, font_info)?;
        }
        OpenTypeFont::Collection(ttc) => {
            for (index, offset_table_offset) in ttc.offset_tables.iter().enumerate() {
                let ttf = file
                    .scope
                    .offset(offset_table_offset as usize)
                    .read::<OffsetTable>()?;
                let data = ttf
                    .read_table(&file.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| format_err!("name table is not present"))?;
                collect_font_info(data.data(), path, index, font_info).ok();
            }
        }
    }

    Ok(())
}

fn locate_offset_table<'a>(f: &OpenTypeFile<'a>, idx: usize) -> Fallible<OffsetTable<'a>> {
    match &f.font {
        OpenTypeFont::Single(ttf) => Ok(ttf.clone()),
        OpenTypeFont::Collection(ttc) => {
            let offset_table_offset = ttc
                .offset_tables
                .read_item(idx)
                .map_err(|e| format_err!("font idx={} is not present in ttc file: {}", idx, e))?;
            let ttf = f
                .scope
                .offset(offset_table_offset as usize)
                .read::<OffsetTable>()?;
            Ok(ttf.clone())
        }
    }
}

/// Extract the name table data from a font
fn name_table_data<'a>(otf: &OffsetTable<'a>, scope: &ReadScope<'a>) -> Fallible<&'a [u8]> {
    let data = otf
        .read_table(scope, allsorts::tag::NAME)?
        .ok_or_else(|| format_err!("name table is not present"))?;
    Ok(data.data())
}

/// Extract a name from the name table
fn get_name(name_table_data: &[u8], name_id: u16) -> Fallible<String> {
    let cstr = allsorts::get_name::fontcode_get_name(name_table_data, name_id)?
        .ok_or_else(|| format_err!("name_id {} not found", name_id))?;
    cstr.into_string()
        .map_err(|e| format_err!("name_id {} is not representable as String: {}", name_id, e))
}
