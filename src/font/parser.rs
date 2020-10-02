//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
#![allow(dead_code)]
use crate::font::locator::{FontDataHandle, FontLocatorSelection};
use crate::font::shaper::{FallbackIdx, FontMetrics, GlyphInfo};
use crate::font::units::*;
use allsorts::binary::read::{ReadScope, ReadScopeOwned};
use allsorts::font_data_impl::read_cmap_subtable;
use allsorts::gpos::{gpos_apply, Info, Placement};
use allsorts::gsub::{gsub_apply_default, GlyphOrigin, GsubFeatureMask, RawGlyph};
use allsorts::layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB};
use allsorts::post::PostTable;
use allsorts::tables::cmap::{Cmap, CmapSubtable};
use allsorts::tables::{
    HeadTable, HheaTable, HmtxTable, MaxpTable, OffsetTable, OpenTypeFile, OpenTypeFont,
};
use allsorts::tag;
use anyhow::anyhow;
use config::{Config, FontAttributes};
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use termwiz::cell::unicode_column_width;
use tinyvec::*;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved {
        raw: RawGlyph<()>,
        slice_start: usize,
    },
}

/// Represents a parsed font
pub struct ParsedFont {
    otf: OffsetTable<'static>,
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
    full_name: String,
    unique: Option<String>,
    family: Option<String>,
    sub_family: Option<String>,
    postscript_name: Option<String>,
}

impl Names {
    fn from_name_table_data(name_table: &[u8]) -> anyhow::Result<Names> {
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
    ) -> anyhow::Result<Vec<FontDataHandle>> {
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

        Self::match_font_info(fonts_selection, font_info)
    }

    pub fn load_built_in_fonts(
        fonts_selection: &[FontAttributes],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut font_info = vec![];
        load_built_in_fonts(&mut font_info).ok();
        Self::match_font_info(fonts_selection, font_info)
    }

    fn match_font_info(
        fonts_selection: &[FontAttributes],
        mut font_info: Vec<(Names, std::path::PathBuf, FontDataHandle)>,
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
            let mut found = false;
            for (names, path, handle) in &font_info {
                if font_info_matches(attr, &names) {
                    log::warn!("Using {} from {}", names.full_name, path.display(),);
                    handles.push(handle.clone());
                    found = true;
                    break;
                }
            }
            if !found && FontLocatorSelection::get_default() == FontLocatorSelection::ConfigDirsOnly
            {
                log::error!("Did not locate a font match for {:?}", attr);
            }
        }
        Ok(handles)
    }

    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
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
            .ok_or_else(|| anyhow!("HEAD table missing or broken"))?
            .read::<HeadTable>()?;
        let cmap = otf
            .read_table(&file.scope, tag::CMAP)?
            .ok_or_else(|| anyhow!("CMAP table missing or broken"))?
            .read::<Cmap>()?;
        let cmap_subtable: CmapSubtable<'static> = read_cmap_subtable(&cmap)?
            .ok_or_else(|| anyhow!("CMAP subtable not found"))?
            .1;

        let maxp = otf
            .read_table(&file.scope, tag::MAXP)?
            .ok_or_else(|| anyhow!("MAXP table not found"))?
            .read::<MaxpTable>()?;
        let num_glyphs = maxp.num_glyphs;

        let post = otf
            .read_table(&file.scope, tag::POST)?
            .ok_or_else(|| anyhow!("POST table not found"))?
            .read::<PostTable>()?;

        let hhea = otf
            .read_table(&file.scope, tag::HHEA)?
            .ok_or_else(|| anyhow!("HHEA table not found"))?
            .read::<HheaTable>()?;
        let hmtx = otf
            .read_table(&file.scope, tag::HMTX)?
            .ok_or_else(|| anyhow!("HMTX table not found"))?
            .read_dep::<HmtxTable>((
                usize::from(maxp.num_glyphs),
                usize::from(hhea.num_h_metrics),
            ))?;

        let gdef_table: Option<GDEFTable> = otf
            .find_table_record(tag::GDEF)
            .map(|gdef_record| -> anyhow::Result<GDEFTable> {
                Ok(gdef_record.read_table(&file.scope)?.read::<GDEFTable>()?)
            })
            .transpose()?;
        let opt_gpos_table = otf
            .find_table_record(tag::GPOS)
            .map(|gpos_record| -> anyhow::Result<LayoutTable<GPOS>> {
                Ok(gpos_record
                    .read_table(&file.scope)?
                    .read::<LayoutTable<GPOS>>()?)
            })
            .transpose()?;
        let gpos_cache = opt_gpos_table.map(new_layout_cache);

        let gsub_cache = otf
            .find_table_record(tag::GSUB)
            .map(|gsub| -> anyhow::Result<LayoutTable<GSUB>> {
                Ok(gsub.read_table(&file.scope)?.read::<LayoutTable<GSUB>>()?)
            })
            .transpose()?
            .map(new_layout_cache);

        Ok(Self {
            otf,
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

    /// Resolve a char to the corresponding glyph in the font
    pub fn glyph_index_for_char(&self, c: char) -> anyhow::Result<Option<u16>> {
        self.cmap_subtable
            .map_glyph(c as u32)
            .map_err(|e| anyhow!("Error while looking up glyph {}: {}", c, e))
    }

    pub fn get_metrics(&self, point_size: f64, dpi: u32) -> FontMetrics {
        let pixel_scale = (dpi as f64 / 72.) * point_size / self.units_per_em as f64;
        let underline_thickness =
            PixelLength::new(self.post.header.underline_thickness as f64 * pixel_scale);
        let underline_position =
            PixelLength::new(self.post.header.underline_position as f64 * pixel_scale);
        let descender = PixelLength::new(self.hhea.descender as f64 * pixel_scale);
        let cell_height = PixelLength::new(
            (self.hhea.ascender - self.hhea.descender + self.hhea.line_gap) as f64 * pixel_scale,
        );
        log::trace!(
            "hhea: ascender={} descender={} line_gap={} \
             advance_width_max={} min_lsb={} min_rsb={} \
             x_max_extent={}",
            self.hhea.ascender,
            self.hhea.descender,
            self.hhea.line_gap,
            self.hhea.advance_width_max,
            self.hhea.min_left_side_bearing,
            self.hhea.min_right_side_bearing,
            self.hhea.x_max_extent
        );

        let mut cell_width = 0;
        // Compute the max width based on ascii chars
        for i in 0x20..0x7fu8 {
            if let Ok(Some(glyph_index)) = self.glyph_index_for_char(i as char) {
                if let Ok(h) = self
                    .hmtx
                    .horizontal_advance(glyph_index, self.hhea.num_h_metrics)
                {
                    cell_width = cell_width.max(h);
                }
            }
        }
        let cell_width = PixelLength::new(
            (PixelLength::new(cell_width as f64) * pixel_scale)
                .get()
                .floor(),
        );

        let metrics = FontMetrics {
            cell_width,
            cell_height,
            descender,
            underline_thickness,
            underline_position,
        };

        log::trace!("metrics: {:?}", metrics);

        metrics
    }

    #[allow(clippy::too_many_arguments)]
    pub fn shape_text<T: AsRef<str>>(
        &self,
        text: T,
        slice_index: usize,
        font_index: FallbackIdx,
        script: u32,
        lang: u32,
        point_size: f64,
        dpi: u32,
    ) -> anyhow::Result<Vec<MaybeShaped>> {
        let mut glyphs = vec![];
        for c in text.as_ref().chars() {
            glyphs.push(RawGlyph {
                unicodes: tiny_vec!([char; 1], c),
                glyph_index: self.glyph_index_for_char(c)?,
                liga_component_pos: 0,
                glyph_origin: GlyphOrigin::Char(c),
                small_caps: false,
                multi_subst_dup: false,
                is_vert_alt: false,
                fake_bold: false,
                fake_italic: false,
                variation: None,
                extra_data: (),
            });
        }

        // TODO: construct from configuation
        let feature_mask = GsubFeatureMask::CLIG | GsubFeatureMask::LIGA | GsubFeatureMask::CALT;

        if let Some(gsub_cache) = self.gsub_cache.as_ref() {
            gsub_apply_default(
                &|| vec![], //map_char('\u{25cc}')],
                gsub_cache,
                self.gdef_table.as_ref(),
                script,
                lang,
                feature_mask,
                self.num_glyphs,
                &mut glyphs,
            )?;
        }

        // Note: init_from_glyphs silently elides entries that
        // have no glyph in the current font!  we need to deal
        // with this so that we can perform font fallback, so
        // we pass a copy of the glyphs here and detect this
        // during below.
        let mut infos = Info::init_from_glyphs(self.gdef_table.as_ref(), glyphs.clone())?;
        if let Some(gpos_cache) = self.gpos_cache.as_ref() {
            let kerning = true;

            gpos_apply(
                gpos_cache,
                self.gdef_table.as_ref(),
                kerning,
                script,
                lang,
                &mut infos,
            )?;
        }

        let mut pos = Vec::new();
        let mut glyph_iter = glyphs.into_iter();
        let mut cluster = slice_index;

        fn reverse_engineer_glyph_text(glyph: &RawGlyph<()>) -> String {
            glyph.unicodes.iter().collect()
        }

        for glyph_info in infos.into_iter() {
            let mut input_glyph = glyph_iter
                .next()
                .ok_or_else(|| anyhow!("more output infos than input glyphs!"))?;

            while input_glyph.unicodes != glyph_info.glyph.unicodes {
                // Info::init_from_glyphs skipped the input glyph, so let's be
                // sure to emit a placeholder for it
                let text = reverse_engineer_glyph_text(&input_glyph);
                pos.push(MaybeShaped::Unresolved {
                    raw: input_glyph,
                    slice_start: cluster,
                });

                cluster += text.len();

                input_glyph = glyph_iter
                    .next()
                    .ok_or_else(|| anyhow!("more output infos than input glyphs! (loop bottom)"))?;
            }

            let glyph_index = glyph_info
                .glyph
                .glyph_index
                .ok_or_else(|| anyhow!("no mapped glyph_index for {:?}", glyph_info))?;

            let horizontal_advance = i32::from(
                self.hmtx
                    .horizontal_advance(glyph_index, self.hhea.num_h_metrics)?,
            );

            /*
            let width = if glyph_info.kerning != 0 {
                horizontal_advance + i32::from(glyph_info.kerning)
            } else {
                horizontal_advance
            };
            */

            // Adjust for distance placement
            let (x_advance, y_advance) = match glyph_info.placement {
                Placement::Distance(dx, dy) => (horizontal_advance + dx, dy),
                Placement::Anchor(_, _) | Placement::None => (horizontal_advance, 0),
            };

            let pixel_scale = (dpi as f64 / 72.) * point_size / self.units_per_em as f64;
            let x_advance = PixelLength::new(x_advance as f64 * pixel_scale);
            let y_advance = PixelLength::new(y_advance as f64 * pixel_scale);

            let text = reverse_engineer_glyph_text(&glyph_info.glyph);
            let text_len = text.len();

            // let num_cells = glyph_info.glyph.unicodes.len();
            let num_cells = unicode_column_width(&text);

            let info = GlyphInfo {
                #[cfg(debug_assertions)]
                text,
                cluster: cluster as u32,
                num_cells: num_cells as u8,
                font_idx: font_index,
                glyph_pos: glyph_index as u32,
                x_advance,
                y_advance,
                x_offset: PixelLength::new(0.),
                y_offset: PixelLength::new(0.),
            };

            cluster += text_len;

            pos.push(MaybeShaped::Resolved(info));
        }

        Ok(pos)
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
                Some("Regular") | None => true,
                _ => false,
            }
        } else {
            false
        }
    } else {
        false
    }
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
fn load_built_in_fonts(
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    for data in &[
        include_bytes!("../../assets/fonts/JetBrainsMono-Bold-Italic.ttf") as &'static [u8],
        include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-ExtraBold-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-ExtraBold.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-ExtraLight-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-ExtraLight.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Light-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Light.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Medium-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-SemiLight-Italic.ttf"),
        include_bytes!("../../assets/fonts/JetBrainsMono-SemiLight.ttf"),
        include_bytes!("../../assets/fonts/NotoColorEmoji.ttf"),
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
                        },
                    ));
                }
            }
        }
    }

    Ok(())
}

fn parse_and_collect_font_info(
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
        OpenTypeFont::Single(ttf) => Ok(ttf.clone()),
        OpenTypeFont::Collection(ttc) => {
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
    let cstr = allsorts::get_name::fontcode_get_name(name_table_data, name_id)?
        .ok_or_else(|| anyhow!("name_id {} not found", name_id))?;
    cstr.into_string()
        .map_err(|e| anyhow!("name_id {} is not representable as String: {}", name_id, e))
}
