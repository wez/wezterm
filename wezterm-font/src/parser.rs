//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
#![allow(dead_code)]
use crate::locator::FontDataHandle;
use crate::shaper::GlyphInfo;
use allsorts::binary::read::{ReadScope, ReadScopeOwned};
use allsorts::font_data_impl::read_cmap_subtable;
use allsorts::layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB};
use allsorts::post::PostTable;
use allsorts::tables::cmap::{Cmap, CmapSubtable};
use allsorts::tables::{
    HeadTable, HheaTable, HmtxTable, MaxpTable, OffsetTable, OpenTypeFile, OpenTypeFont,
};
use allsorts::tag;
use anyhow::{anyhow, Context};
use config::FontAttributes;
use std::collections::HashSet;
use std::convert::TryInto;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

/// Represents a parsed font
pub struct ParsedFont {
    names: Names,

    cmap_subtable: CmapSubtable<'static>,
    gpos_cache: Option<LayoutCache<GPOS>>,
    gsub_cache: Option<LayoutCache<GSUB>>,
    gdef_table: Option<GDEFTable>,
    hmtx: HmtxTable<'static>,
    post: PostTable<'static>,
    hhea: HheaTable,
    num_glyphs: u16,
    units_per_em: u16,

    // Must be last: this keeps the 'static items alive
    _scope: ReadScopeOwned,
}

#[derive(Debug)]
pub struct Names {
    pub full_name: String,
    pub unique: Option<String>,
    pub family: Option<String>,
    pub sub_family: Option<String>,
    pub postscript_name: Option<String>,
}

impl Names {
    fn from_name_table_data(name_table: &[u8]) -> anyhow::Result<Names> {
        Ok(Names {
            full_name: get_name(name_table, 4).context("full_name")?,
            unique: get_name(name_table, 3).ok(),
            family: get_name(name_table, 1).ok(),
            sub_family: get_name(name_table, 2).ok(),
            postscript_name: get_name(name_table, 6).ok(),
        })
    }
}

impl ParsedFont {
    fn match_font_info(
        fonts_selection: &[FontAttributes],
        mut font_info: Vec<(Names, std::path::PathBuf, FontDataHandle)>,
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        font_info.sort_by_key(|(names, _, _)| names.full_name.clone());
        for (names, _, _) in &font_info {
            log::warn!("available font: {}", names.full_name);
        }

        // Second, apply matching rules in order. We can't match
        // against the font files as we discover them because the
        // filesystem iteration order is arbitrary whereas our
        // fonts_selection is strictly ordered
        let mut handles = vec![];
        for attr in fonts_selection {
            for (names, path, handle) in &font_info {
                if font_info_matches(attr, &names) {
                    log::warn!(
                        "Using {} from {} for {:?}",
                        names.full_name,
                        path.display(),
                        attr
                    );
                    handles.push(handle.clone());
                    loaded.insert(attr.clone());
                    break;
                }
            }
        }
        Ok(handles)
    }

    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let (data, index) = match handle {
            FontDataHandle::Memory { data, index, .. } => (data.to_vec(), *index),
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
        let file: OpenTypeFile<'static> = unsafe {
            std::mem::transmute(
                owned_scope
                    .scope()
                    .read::<OpenTypeFile>()
                    .context("read OpenTypeFile")?,
            )
        };

        let otf = locate_offset_table(&file, index).context("locate_offset_table")?;
        let name_table = name_table_data(&otf, &file.scope).context("name_table_data")?;
        let names =
            Names::from_name_table_data(name_table).context("Names::from_name_table_data")?;

        let head = otf
            .read_table(&file.scope, tag::HEAD)?
            .ok_or_else(|| anyhow!("HEAD table missing or broken"))?
            .read::<HeadTable>()
            .context("read HeadTable")?;
        let cmap = otf
            .read_table(&file.scope, tag::CMAP)?
            .ok_or_else(|| anyhow!("CMAP table missing or broken"))?
            .read::<Cmap>()
            .context("read Cmap")?;
        let cmap_subtable: CmapSubtable<'static> = read_cmap_subtable(&cmap)?
            .ok_or_else(|| anyhow!("CMAP subtable not found"))?
            .1;

        let maxp = otf
            .read_table(&file.scope, tag::MAXP)?
            .ok_or_else(|| anyhow!("MAXP table not found"))?
            .read::<MaxpTable>()
            .context("read MaxpTable")?;
        let num_glyphs = maxp.num_glyphs;

        let post = otf
            .read_table(&file.scope, tag::POST)?
            .ok_or_else(|| anyhow!("POST table not found"))?
            .read::<PostTable>()
            .context("read PostTable")?;

        let hhea = otf
            .read_table(&file.scope, tag::HHEA)?
            .ok_or_else(|| anyhow!("HHEA table not found"))?
            .read::<HheaTable>()
            .context("read HheaTable")?;
        let hmtx = otf
            .read_table(&file.scope, tag::HMTX)?
            .ok_or_else(|| anyhow!("HMTX table not found"))?
            .read_dep::<HmtxTable>((
                usize::from(maxp.num_glyphs),
                usize::from(hhea.num_h_metrics),
            ))
            .context("read_dep HmtxTable")?;

        let gdef_table: Option<GDEFTable> = otf
            .find_table_record(tag::GDEF)
            .map(|gdef_record| -> anyhow::Result<GDEFTable> {
                Ok(gdef_record
                    .read_table(&file.scope)?
                    .read::<GDEFTable>()
                    .context("read GDEFTable")?)
            })
            .transpose()?;
        let opt_gpos_table = otf
            .find_table_record(tag::GPOS)
            .map(|gpos_record| -> anyhow::Result<LayoutTable<GPOS>> {
                Ok(gpos_record
                    .read_table(&file.scope)?
                    .read::<LayoutTable<GPOS>>()
                    .context("read LayoutTable<GPOS>")?)
            })
            .transpose()?;
        let gpos_cache = opt_gpos_table.map(new_layout_cache);

        let gsub_cache = otf
            .find_table_record(tag::GSUB)
            .map(|gsub| -> anyhow::Result<LayoutTable<GSUB>> {
                Ok(gsub
                    .read_table(&file.scope)?
                    .read::<LayoutTable<GSUB>>()
                    .context("read LayoutTable<GSUB>")?)
            })
            .transpose()?
            .map(new_layout_cache);

        Ok(Self {
            names,
            cmap_subtable,
            post,
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
}

fn collect_font_info(
    name_table_data: &[u8],
    path: &Path,
    index: usize,
    infos: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    let names = Names::from_name_table_data(name_table_data)?;
    infos.push((
        names,
        path.to_path_buf(),
        FontDataHandle::OnDisk {
            path: path.to_path_buf(),
            index: index.try_into()?,
        },
    ));
    Ok(())
}

pub fn font_info_matches(attr: &FontAttributes, names: &Names) -> bool {
    if let Some(fam) = names.family.as_ref() {
        // TODO: correctly match using family and sub-family;
        // this is a pretty rough approximation
        if attr.family == *fam {
            match names.sub_family.as_ref().map(String::as_str) {
                Some("Italic") if attr.italic && !attr.bold => return true,
                Some("Bold") if attr.bold && !attr.italic => return true,
                Some("Bold Italic") if attr.bold && attr.italic => return true,
                Some("Medium") | Some("Regular") | None if !attr.italic && !attr.bold => {
                    return true
                }
                _ => {}
            }
        }
    }
    if attr.family == names.full_name && !attr.bold && !attr.italic {
        true
    } else {
        false
    }
}

/// Given a blob representing a True Type Collection (.ttc) file,
/// and a desired font, enumerate the collection to resolve the index of
/// the font inside that collection that matches it.
/// Even though this is intended to work with a TTC, this also returns
/// the index of a singular TTF file, if it matches.
pub fn resolve_font_from_ttc_data(
    attr: &FontAttributes,
    data: &[u8],
) -> anyhow::Result<Option<usize>> {
    let scope = allsorts::binary::read::ReadScope::new(&data);
    let file = scope.read::<OpenTypeFile>()?;

    match &file.font {
        OpenTypeFont::Single(ttf) => {
            let name_table_data = ttf
                .read_table(&file.scope, allsorts::tag::NAME)?
                .ok_or_else(|| anyhow!("name table is not present"))?;

            let names = Names::from_name_table_data(name_table_data.data())?;
            if font_info_matches(attr, &names) {
                Ok(Some(0))
            } else {
                Ok(None)
            }
        }
        OpenTypeFont::Collection(ttc) => {
            for (index, offset_table_offset) in ttc.offset_tables.iter().enumerate() {
                let ttf = file
                    .scope
                    .offset(offset_table_offset as usize)
                    .read::<OffsetTable>()?;
                let name_table_data = ttf
                    .read_table(&file.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| anyhow!("name table is not present"))?;
                let names = Names::from_name_table_data(name_table_data.data())?;
                if font_info_matches(attr, &names) {
                    return Ok(Some(index));
                }
            }
            Ok(None)
        }
    }
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    macro_rules! font {
        ($font:literal) => {
            (include_bytes!($font) as &'static [u8], $font)
        };
    }
    for (data, name) in &[
        font!("../../assets/fonts/JetBrainsMono-Bold-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBold-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLight-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLight.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Light-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Light.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Medium-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
        font!("../../assets/fonts/JetBrainsMono-SemiLight-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-SemiLight.ttf"),
        font!("../../assets/fonts/NotoColorEmoji.ttf"),
        font!("../../assets/fonts/LastResortHE-Regular.ttf"),
    ] {
        let scope = allsorts::binary::read::ReadScope::new(&data);
        let file = scope.read::<OpenTypeFile>()?;
        let path = Path::new("memory");

        match &file.font {
            OpenTypeFont::Single(ttf) => {
                let name_table_data = ttf
                    .read_table(&file.scope, allsorts::tag::NAME)?
                    .ok_or_else(|| anyhow!("name table is not present"))?;

                let names = Names::from_name_table_data(name_table_data.data())?;
                font_info.push((
                    names,
                    path.to_path_buf(),
                    FontDataHandle::Memory {
                        data: data.to_vec(),
                        index: 0,
                        name: name.to_string(),
                    },
                ));
            }
            OpenTypeFont::Collection(ttc) => {
                for (index, offset_table_offset) in ttc.offset_tables.iter().enumerate() {
                    let ttf = file
                        .scope
                        .offset(offset_table_offset as usize)
                        .read::<OffsetTable>()?;
                    let name_table_data = ttf
                        .read_table(&file.scope, allsorts::tag::NAME)?
                        .ok_or_else(|| anyhow!("name table is not present"))?;
                    let names = Names::from_name_table_data(name_table_data.data())?;
                    font_info.push((
                        names,
                        path.to_path_buf(),
                        FontDataHandle::Memory {
                            data: data.to_vec(),
                            index: index.try_into()?,
                            name: name.to_string(),
                        },
                    ));
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn parse_and_collect_font_info(
    path: &Path,
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    let data = std::fs::read(path)?;
    let scope = allsorts::binary::read::ReadScope::new(&data);
    let file = scope.read::<OpenTypeFile>()?;

    match &file.font {
        OpenTypeFont::Single(ttf) => {
            let data = ttf
                .read_table(&file.scope, allsorts::tag::NAME)?
                .ok_or_else(|| anyhow!("name table is not present"))?;
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
                    .ok_or_else(|| anyhow!("name table is not present"))?;
                collect_font_info(data.data(), path, index, font_info).ok();
            }
        }
    }

    Ok(())
}

fn locate_offset_table<'a>(f: &OpenTypeFile<'a>, idx: usize) -> anyhow::Result<OffsetTable<'a>> {
    match &f.font {
        OpenTypeFont::Single(ttf) if idx == 0 => Ok(ttf.clone()),
        OpenTypeFont::Single(_) => Err(anyhow!("requested idx {} not present in single ttf", idx)),
        OpenTypeFont::Collection(ttc) => {
            // Ideally `read_item` would simply error when idx is out of range,
            // but it generates a panic, so we need to check for ourselves.
            if idx >= ttc.offset_tables.len() {
                anyhow::bail!("requested idx {} out of range for ttc", idx);
            }
            let offset_table_offset = ttc
                .offset_tables
                .read_item(idx)
                .map_err(|e| anyhow!("font idx={} is not present in ttc file: {}", idx, e))?;
            let ttf = f
                .scope
                .offset(offset_table_offset as usize)
                .read::<OffsetTable>()?;
            Ok(ttf.clone())
        }
    }
}

/// Extract the name table data from a font
fn name_table_data<'a>(otf: &OffsetTable<'a>, scope: &ReadScope<'a>) -> anyhow::Result<&'a [u8]> {
    let data = otf
        .read_table(scope, allsorts::tag::NAME)?
        .ok_or_else(|| anyhow!("name table is not present"))?;
    Ok(data.data())
}

/// Extract a name from the name table
fn get_name(name_table_data: &[u8], name_id: u16) -> anyhow::Result<String> {
    let cstr = allsorts::get_name::fontcode_get_name(name_table_data, name_id)
        .with_context(|| anyhow!("fontcode_get_name name_id:{}", name_id))?
        .ok_or_else(|| anyhow!("name_id {} not found", name_id))?;
    cstr.into_string()
        .map_err(|e| anyhow!("name_id {} is not representable as String: {}", name_id, e))
}
