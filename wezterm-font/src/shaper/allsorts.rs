use crate::locator::FontDataHandle;
use crate::parser::*;
use crate::shaper::{FallbackIdx, FontMetrics, FontShaper, GlyphInfo};
use crate::units::*;
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
use anyhow::{anyhow, bail, Context};
use termwiz::cell::unicode_column_width;
use tinyvec::*;
use unicode_general_category::{get_general_category, GeneralCategory};

/// Represents a parsed font
struct ParsedFont {
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

impl ParsedFont {
    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let data = handle.source.load_data()?;
        let index = handle.index as usize;

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

    /// Resolve a char to the corresponding glyph in the font
    pub fn glyph_index_for_char(&self, c: char) -> anyhow::Result<u16> {
        let glyph = self
            .cmap_subtable
            .map_glyph(c as u32)
            .map_err(|e| anyhow!("Error while looking up glyph {}: {}", c, e))?;

        if c == '\u{200C}' && glyph.is_none() {
            // If ZWNJ is missing, substitute a space
            self.glyph_index_for_char(' ')
        } else {
            glyph.ok_or_else(|| anyhow!("Font doesn't contain glyph for char {:?}", c))
        }
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
            if let Ok(glyph_index) = self.glyph_index_for_char(i as char) {
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
        #[derive(Debug)]
        enum Run {
            Unresolved(String),
            Glyphs(Vec<RawGlyph<()>>),
        }

        let mut runs = vec![];
        use allsorts::unicode::VariationSelector;
        use std::convert::TryFrom;

        let mut chars_iter = text.as_ref().chars().peekable();
        while let Some(c) = chars_iter.next() {
            match VariationSelector::try_from(c) {
                Ok(_) => {
                    // Ignore variation selector; we already accounted for it
                    // in the lookahead in the case below
                }
                Err(_) => {
                    // Lookahead for a variation selector
                    let variation = chars_iter
                        .peek()
                        .and_then(|&next| VariationSelector::try_from(next).ok());

                    match self.glyph_index_for_char(c) {
                        Ok(glyph_index) => {
                            let glyph = RawGlyph {
                                unicodes: tiny_vec!([char; 1] => c),
                                glyph_index,
                                liga_component_pos: 0,
                                glyph_origin: GlyphOrigin::Char(c),
                                small_caps: false,
                                multi_subst_dup: false,
                                is_vert_alt: false,
                                fake_bold: false,
                                fake_italic: false,
                                variation,
                                extra_data: (),
                            };
                            if let Some(Run::Glyphs(ref mut glyphs)) = runs.last_mut() {
                                glyphs.push(glyph);
                            } else {
                                runs.push(Run::Glyphs(vec![glyph]));
                            }
                        }
                        Err(_) => {
                            // There wasn't a match for this character.
                            // We may need to do a bit of fiddly unwinding here.
                            // If the character we tried to resolve modifies the preceding
                            // character then we need to sweep that into an Unresolved block
                            // along with the current character; if we don't do that then
                            // eg: U+0030 U+FE0F U+20E3 (keycap 0; 0 in VS16 with combining
                            // enclosing keycap) can match a plain `0` numeral and leave
                            // the combining mark as a leftover.
                            match get_general_category(c) {
                                GeneralCategory::EnclosingMark => {
                                    // We modify the prior character, so pop it off and
                                    // bundle it together for the fallback
                                    let glyph = match runs.last_mut() {
                                        Some(Run::Glyphs(ref mut glyphs)) => glyphs.pop(),
                                        _ => None,
                                    };
                                    if let Some(glyph) = glyph {
                                        // Synthesize the prior glyph into a string
                                        let mut s = glyph.unicodes[0].to_string();
                                        // and reverse the variation selector
                                        match glyph.variation {
                                            None => {}
                                            Some(VariationSelector::VS01) => s.push('\u{FE00}'),
                                            Some(VariationSelector::VS02) => s.push('\u{FE01}'),
                                            Some(VariationSelector::VS03) => s.push('\u{FE02}'),
                                            Some(VariationSelector::VS15) => s.push('\u{FE0E}'),
                                            Some(VariationSelector::VS16) => s.push('\u{FE0F}'),
                                        }
                                        runs.push(Run::Unresolved(s));
                                    }
                                }
                                _ => {}
                            }

                            if let Some(Run::Unresolved(ref mut s)) = runs.last_mut() {
                                s.push(c);
                            } else {
                                runs.push(Run::Unresolved(c.to_string()));
                            }

                            // And finally, if the next character is a variation selector,
                            // that belongs with this sequence as well.
                            if variation.is_some() {
                                if let Some(Run::Unresolved(ref mut s)) = runs.last_mut() {
                                    s.push(*chars_iter.peek().unwrap());
                                }
                            }
                        }
                    }
                }
            }
        }

        // TODO: construct from configuation
        let feature_mask = GsubFeatureMask::default();
        let mut pos = Vec::new();
        let mut cluster = slice_index;

        for run in runs {
            match run {
                Run::Unresolved(raw) => {
                    let len = raw.len();
                    pos.push(MaybeShaped::Unresolved {
                        raw,
                        slice_start: cluster,
                    });
                    cluster += len;
                }
                Run::Glyphs(mut glyphs) => {
                    if let Some(gsub_cache) = self.gsub_cache.as_ref() {
                        gsub_apply_default(
                            &|| vec![], //map_char('\u{25cc}')],
                            gsub_cache,
                            self.gdef_table.as_ref(),
                            script,
                            Some(lang),
                            feature_mask,
                            self.num_glyphs,
                            &mut glyphs,
                        )?;
                    }

                    let mut infos = Info::init_from_glyphs(self.gdef_table.as_ref(), glyphs)?;
                    if let Some(gpos_cache) = self.gpos_cache.as_ref() {
                        let kerning = true;

                        gpos_apply(
                            gpos_cache,
                            self.gdef_table.as_ref(),
                            kerning,
                            script,
                            Some(lang),
                            &mut infos,
                        )?;
                    }

                    fn reverse_engineer_glyph_text(glyph: &RawGlyph<()>) -> String {
                        glyph.unicodes.iter().collect()
                    }

                    for glyph_info in infos.into_iter() {
                        let glyph_index = glyph_info.glyph.glyph_index;

                        let horizontal_advance = i32::from(
                            self.hmtx
                                .horizontal_advance(glyph_index, self.hhea.num_h_metrics)?,
                        );

                        // Adjust for distance placement
                        let (x_advance, y_advance) = match glyph_info.placement {
                            Placement::Distance(dx, dy) => (horizontal_advance + dx, dy),
                            Placement::Anchor(_, _) | Placement::None => (horizontal_advance, 0),
                        };

                        let text = reverse_engineer_glyph_text(&glyph_info.glyph);
                        let text_len = text.len();
                        let num_cells = unicode_column_width(&text);

                        let pixel_scale =
                            (dpi as f64 / 72.) * point_size / self.units_per_em as f64;
                        let x_advance = PixelLength::new(x_advance as f64 * pixel_scale);
                        let y_advance = PixelLength::new(y_advance as f64 * pixel_scale);

                        let is_space = text == " ";

                        let info = GlyphInfo {
                            #[cfg(debug_assertions)]
                            text,
                            is_space,
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
                }
            }
        }

        Ok(pos)
    }
}

pub struct AllsortsShaper {
    fonts: Vec<Option<ParsedFont>>,
}

impl AllsortsShaper {
    pub fn new(_: &config::ConfigHandle, handles: &[FontDataHandle]) -> anyhow::Result<Self> {
        let mut fonts = vec![];
        let mut success = false;
        for handle in handles {
            match ParsedFont::from_locator(handle) {
                Ok(font) => {
                    fonts.push(Some(font));
                    success = true;
                }
                Err(err) => {
                    log::warn!("Failed to parse {:?}: {}", handle, err);
                    fonts.push(None);
                }
            }
        }
        if !success {
            bail!("failed to load any fonts in this fallback set!?");
        }
        Ok(Self { fonts })
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_into(
        &self,
        font_index: FallbackIdx,
        s: &str,
        slice_index: usize,
        script: u32,
        lang: u32,
        font_size: f64,
        dpi: u32,
        results: &mut Vec<GlyphInfo>,
        no_glyphs: &mut Vec<char>,
    ) -> anyhow::Result<()> {
        let font = match self.fonts.get(font_index) {
            Some(Some(font)) => font,
            Some(None) => {
                return self.shape_into(
                    font_index + 1,
                    s,
                    slice_index,
                    script,
                    lang,
                    font_size,
                    dpi,
                    results,
                    no_glyphs,
                );
            }
            None => {
                // Note: since we added a last resort font, this case shouldn't
                // ever get hit in practice.
                // We ran out of fallback fonts, so use a replacement
                // character that is likely to be in one of those fonts.
                let mut alt_text = String::new();
                for c in s.chars() {
                    no_glyphs.push(c);
                    alt_text.push('?');
                }
                if alt_text == s {
                    // We already tried to fallback to this and failed
                    return Err(anyhow!("could not fallback to ? character"));
                }
                return self.shape_into(
                    0,
                    &alt_text,
                    slice_index,
                    script,
                    lang,
                    font_size,
                    dpi,
                    results,
                    no_glyphs,
                );
            }
        };

        if font_index + 1 == self.fonts.len() {
            // We are the last resort font, so each codepoint is considered
            // to be worthy of a fallback lookup
            for c in s.chars() {
                no_glyphs.push(c);
            }
        }

        let first_pass =
            font.shape_text(s, slice_index, font_index, script, lang, font_size, dpi)?;

        let mut item_iter = first_pass.into_iter();
        while let Some(item) = item_iter.next() {
            match item {
                MaybeShaped::Resolved(info) => {
                    results.push(info);
                }
                MaybeShaped::Unresolved { raw, slice_start } => {
                    // There was no glyph in that font, so we'll need to shape
                    // using a fallback.  Let's collect together any potential
                    // run of unresolved entries first
                    self.shape_into(
                        font_index + 1,
                        &raw,
                        slice_start,
                        script,
                        lang,
                        font_size,
                        dpi,
                        results,
                        no_glyphs,
                    )?;
                }
            }
        }

        Ok(())
    }
}

impl FontShaper for AllsortsShaper {
    fn shape(
        &self,
        text: &str,
        size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let mut results = vec![];
        let script = allsorts::tag::LATN;
        let lang = allsorts::tag::DFLT;
        self.shape_into(0, text, 0, script, lang, size, dpi, &mut results, no_glyphs)?;
        // log::error!("shape {} into {:?}", text, results);
        Ok(results)
    }

    fn metrics_for_idx(&self, font_idx: usize, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        let font = self
            .fonts
            .get(font_idx)
            .ok_or_else(|| anyhow::anyhow!("invalid font_idx {}", font_idx))?;
        let font = font
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("failed to load font_idx {}", font_idx))?;
        Ok(font.get_metrics(size, dpi))
    }

    fn metrics(&self, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        for font in &self.fonts {
            if let Some(font) = font {
                return Ok(font.get_metrics(size, dpi));
            }
        }
        bail!("no fonts available for collecting metrics!?");
    }
}
