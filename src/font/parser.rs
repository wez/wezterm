//! This module uses the allsorts crate to parse font data.
//! At this time it is used only to extract name information,
//! but in the future I'd like to use its shaping functionality
#![allow(dead_code)]
use crate::config::Config;
use crate::config::FontAttributes;
use crate::font::locator::FontDataHandle;
use crate::font::shaper::{FallbackIdx, FontMetrics, GlyphInfo};
use crate::font::units::*;
use allsorts::binary::read::{ReadScope, ReadScopeOwned};
use allsorts::font_data_impl::read_cmap_subtable;
use allsorts::fontfile::FontFile;
use allsorts::gpos::{gpos_apply, Info, Placement};
use allsorts::gsub::{gsub_apply_default, GlyphOrigin, RawGlyph};
use allsorts::layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB};
use allsorts::post::PostTable;
use allsorts::tables::cmap::{Cmap, CmapSubtable};
use allsorts::tables::{
    FontTableProvider, HeadTable, HheaTable, HmtxTable, MaxpTable, NameTable, OffsetTable,
    OpenTypeFile, OpenTypeFont, TTCHeader,
};
use allsorts::tag;
use failure::{bail, format_err, Fallible};
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use termwiz::cell::unicode_column_width;

#[derive(Debug)]
enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved(RawGlyph<()>),
}

/// Represents a parsed font
pub struct ParsedFont {
    otf: OffsetTable<'static>,
    names: Names,

    cmap_subtable: CmapSubtable<'static>,
    gpos_cache: Option<LayoutCache<GPOS>>,
    gsub_cache: LayoutCache<GSUB>,
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

        let post = otf
            .read_table(&file.scope, tag::POST)?
            .ok_or_else(|| format_err!("POST table not found"))?
            .read::<PostTable>()?;

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
    pub fn glyph_index_for_char(&self, c: char) -> Fallible<Option<u16>> {
        self.cmap_subtable
            .map_glyph(c as u32)
            .map_err(|e| format_err!("Error while looking up glyph {}: {}", c, e))
    }

    pub fn get_metrics(&self, point_size: f64, dpi: u32) -> FontMetrics {
        let pixel_scale = (dpi as f64 / 72.) * point_size / self.units_per_em as f64;
        let underline_thickness =
            PixelLength::new(self.post.header.underline_thickness as f64 * pixel_scale);
        let underline_position =
            PixelLength::new(self.post.header.underline_position as f64 * pixel_scale);
        let descender = PixelLength::new(self.hhea.descender as f64 * pixel_scale);
        let cell_height = PixelLength::new(self.hhea.line_gap as f64 * pixel_scale);

        // FIXME: for freetype/harfbuzz, we look at a number of glyphs and compute this for
        // ourselves
        let cell_width = PixelLength::new(self.hhea.advance_width_max as f64 * pixel_scale);

        FontMetrics {
            cell_width,
            cell_height,
            descender,
            underline_thickness,
            underline_position,
        }
    }

    pub fn shape_text<T: AsRef<str>>(
        &self,
        text: T,
        font_index: usize,
        script: u32,
        lang: u32,
        point_size: f64,
        dpi: u32,
    ) -> Fallible<Vec<MaybeShaped>> {
        let mut glyphs = vec![];
        for c in text.as_ref().chars() {
            glyphs.push(RawGlyph {
                unicodes: vec![c],
                glyph_index: self.glyph_index_for_char(c)?,
                liga_component_pos: 0,
                glyph_origin: GlyphOrigin::Char(c),
                small_caps: false,
                multi_subst_dup: false,
                is_vert_alt: false,
                fake_bold: false,
                fake_italic: false,
                extra_data: (),
            });
        }

        let vertical = false;

        gsub_apply_default(
            &|| vec![], //map_char('\u{25cc}')],
            &self.gsub_cache,
            self.gdef_table.as_ref(),
            script,
            lang,
            vertical,
            self.num_glyphs,
            &mut glyphs,
        )?;

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

        for (cluster, glyph_info) in infos.into_iter().enumerate() {
            let mut input_glyph = glyph_iter
                .next()
                .ok_or_else(|| format_err!("more output infos than input glyphs!"))?;

            while input_glyph.unicodes != glyph_info.glyph.unicodes {
                // Info::init_from_glyphs skipped the input glyph, so let's be
                // sure to emit a placeholder for it
                pos.push(MaybeShaped::Unresolved(input_glyph));

                input_glyph = glyph_iter.next().ok_or_else(|| {
                    format_err!("more output infos than input glyphs! (loop bottom)")
                })?;
            }

            let glyph_index = glyph_info
                .glyph
                .glyph_index
                .ok_or_else(|| format_err!("no mapped glyph_index for {:?}", glyph_info))?;

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

            let text: String = glyph_info.glyph.unicodes.iter().collect();
            let num_cells = unicode_column_width(&text);

            pos.push(MaybeShaped::Resolved(GlyphInfo {
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
            }));
        }

        Ok(pos)
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
