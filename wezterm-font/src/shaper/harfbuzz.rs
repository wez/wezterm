use crate::parser::ParsedFont;
use crate::shaper::{FallbackIdx, FontMetrics, FontShaper, GlyphInfo};
use crate::units::*;
use crate::{ftwrap, hbwrap as harfbuzz};
use anyhow::{anyhow, Context};
use config::ConfigHandle;
use log::error;
use ordered_float::NotNan;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::ops::Range;
use termwiz::cell::{unicode_column_width, Presentation};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use wezterm_bidi::Direction;

#[derive(Clone, Debug)]
struct Info {
    cluster: usize,
    len: usize,
    codepoint: harfbuzz::hb_codepoint_t,
    x_advance: harfbuzz::hb_position_t,
    y_advance: harfbuzz::hb_position_t,
    x_offset: harfbuzz::hb_position_t,
    y_offset: harfbuzz::hb_position_t,
}

fn make_glyphinfo(text: &str, font_idx: usize, info: &Info) -> GlyphInfo {
    let is_space = text == " ";
    // TODO: this is problematic if the actual text in
    // the terminal specified a different unicode version.
    // We need to find a way to plumb that version through shaping
    // so that it can be used here.
    let num_cells = unicode_column_width(text, None) as u8;
    GlyphInfo {
        #[cfg(any(debug_assertions, test))]
        text: text.into(),
        is_space,
        num_cells,
        font_idx,
        glyph_pos: info.codepoint,
        cluster: info.cluster as u32,
        x_advance: PixelLength::new(f64::from(info.x_advance) / 64.0),
        y_advance: PixelLength::new(f64::from(info.y_advance) / 64.0),
        x_offset: PixelLength::new(f64::from(info.x_offset) / 64.0),
        y_offset: PixelLength::new(f64::from(info.y_offset) / 64.0),
    }
}

struct FontPair {
    face: ftwrap::Face,
    font: RefCell<harfbuzz::Font>,
    shaped_any: bool,
    presentation: Presentation,
    features: Vec<harfbuzz::hb_feature_t>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct MetricsKey {
    font_idx: usize,
    size: NotNan<f64>,
    dpi: u32,
}

pub struct HarfbuzzShaper {
    handles: Vec<ParsedFont>,
    fonts: Vec<RefCell<Option<FontPair>>>,
    lib: ftwrap::Library,
    metrics: RefCell<HashMap<MetricsKey, FontMetrics>>,
    features: Vec<harfbuzz::hb_feature_t>,
    lang: harfbuzz::hb_language_t,
}

#[derive(Error, Debug)]
#[error("No more fallbacks while shaping {}", .text.escape_unicode())]
struct NoMoreFallbacksError {
    text: String,
}

/// Make a string holding a set of unicode replacement
/// characters equal to the number of graphemes in the
/// original string.  That isn't perfect, but it should
/// be good enough to indicate that something isn't right.
fn make_question_string(s: &str) -> String {
    let len = s.graphemes(true).count();
    let mut result = String::new();
    let c = if !is_question_string(s) {
        std::char::REPLACEMENT_CHARACTER
    } else {
        '?'
    };
    for _ in 0..len {
        result.push(c);
    }
    result
}

fn is_question_string(s: &str) -> bool {
    for c in s.chars() {
        if c != std::char::REPLACEMENT_CHARACTER {
            return false;
        }
    }
    true
}

impl HarfbuzzShaper {
    pub fn new(config: &ConfigHandle, handles: &[ParsedFont]) -> anyhow::Result<Self> {
        let lib = ftwrap::Library::new()?;
        let handles = handles.to_vec();
        let mut fonts = vec![];
        for _ in 0..handles.len() {
            fonts.push(RefCell::new(None));
        }

        let lang = harfbuzz::language_from_string("en")?;

        let features: Vec<harfbuzz::hb_feature_t> = config
            .harfbuzz_features
            .iter()
            .filter_map(|s| harfbuzz::feature_from_string(s).ok())
            .collect();

        Ok(Self {
            fonts,
            handles,
            lib,
            metrics: RefCell::new(HashMap::new()),
            features,
            lang,
        })
    }

    fn load_fallback(&self, font_idx: FallbackIdx) -> anyhow::Result<Option<RefMut<FontPair>>> {
        if font_idx >= self.handles.len() {
            return Ok(None);
        }
        match self.fonts.get(font_idx) {
            None => Ok(None),
            Some(opt_pair) => {
                let mut opt_pair = opt_pair.borrow_mut();
                if opt_pair.is_none() {
                    let handle = &self.handles[font_idx];
                    log::trace!("shaper wants {} {:?}", font_idx, handle);
                    let face = self.lib.face_from_locator(&handle.handle)?;
                    let mut font = harfbuzz::Font::new(face.face);
                    let (load_flags, _) = ftwrap::compute_load_flags_from_config(
                        handle.freetype_load_flags,
                        handle.freetype_load_target,
                        handle.freetype_render_target,
                    );
                    font.set_load_flags(load_flags);

                    let features = match &handle.harfbuzz_features {
                        Some(features) => features
                            .iter()
                            .filter_map(|s| harfbuzz::feature_from_string(s).ok())
                            .collect(),
                        None => self.features.clone(),
                    };

                    *opt_pair = Some(FontPair {
                        face,
                        font: RefCell::new(font),
                        shaped_any: false,
                        presentation: if handle.assume_emoji_presentation {
                            Presentation::Emoji
                        } else {
                            Presentation::Text
                        },
                        features,
                    });
                }

                Ok(Some(RefMut::map(opt_pair, |opt_pair| {
                    opt_pair.as_mut().unwrap()
                })))
            }
        }
    }

    fn do_shape(
        &self,
        mut font_idx: FallbackIdx,
        s: &str,
        font_size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
        presentation: Option<Presentation>,
        direction: Direction,
        range: Range<usize>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let mut buf = harfbuzz::Buffer::new()?;
        // We deliberately omit setting the script and leave it to harfbuzz
        // to infer from the buffer contents so that it can correctly
        // enable appropriate preprocessing for eg: Hangul.
        // <https://github.com/wez/wezterm/issues/1474> and
        // <https://github.com/wez/wezterm/issues/1573>
        // buf.set_script(harfbuzz::hb_script_t::HB_SCRIPT_LATIN);
        buf.set_direction(match direction {
            Direction::LeftToRight => harfbuzz::hb_direction_t::HB_DIRECTION_LTR,
            Direction::RightToLeft => harfbuzz::hb_direction_t::HB_DIRECTION_RTL,
        });
        buf.set_language(self.lang);

        buf.add_str(s, range.clone());
        let mut cluster_to_len = vec![];
        for c in s.chars() {
            let len = c.len_utf8();
            for _ in 0..len {
                cluster_to_len.push(len as u8);
            }
        }
        buf.guess_segment_properties();
        buf.set_cluster_level(
            harfbuzz::hb_buffer_cluster_level_t::HB_BUFFER_CLUSTER_LEVEL_MONOTONE_GRAPHEMES,
        );

        let cell_width;
        let shaped_any;
        let initial_font_idx = font_idx;

        loop {
            match self.load_fallback(font_idx).context("load_fallback")? {
                Some(mut pair) => {
                    // Ignore presentation if we've reached the last resort font
                    if font_idx + 1 < self.fonts.len() {
                        if let Some(p) = presentation {
                            if pair.presentation != p {
                                font_idx += 1;
                                continue;
                            }
                        }
                    }
                    let size = pair.face.set_font_size(font_size, dpi)?;
                    // Tell harfbuzz to recompute important font metrics!
                    let mut font = pair.font.borrow_mut();
                    font.font_changed();
                    cell_width = size.width;
                    shaped_any = pair.shaped_any;
                    font.shape(&mut buf, pair.features.as_slice());
                    /*
                    log::info!(
                        "shaped font_idx={} as: {}",
                        font_idx,
                        buf.serialize(Some(&pair.font))
                    );
                    */
                    break;
                }
                None => {
                    // Note: since we added a last resort font, this case
                    // shouldn't ever get hit in practice
                    for c in s.chars() {
                        no_glyphs.push(c);
                    }
                    return Err(NoMoreFallbacksError {
                        text: s.to_string(),
                    }
                    .into());
                }
            }
        }

        if font_idx > 0 && font_idx + 1 == self.fonts.len() {
            // We are the last resort font, so each codepoint is considered
            // to be worthy of a fallback lookup
            for c in s.chars() {
                no_glyphs.push(c);
            }

            if presentation.is_some() {
                // We hit the last resort and we have an explicit presentation.
                // This is a little awkward; we want to record the missing
                // glyphs so that we can resolve them async, but we also
                // want to try the current set of fonts without forcing
                // the presentation match as we might find the results
                // that way.
                // Let's restart the shape but pretend that no specific
                // presentation was used.
                // We'll probably match the emoji presentation for something,
                // but might potentially discover the text presentation for
                // that glyph in a fallback font and swap it out a little
                // later after a flash of showing the emoji one.
                return self.do_shape(
                    initial_font_idx,
                    s,
                    font_size,
                    dpi,
                    no_glyphs,
                    None,
                    direction,
                    range,
                );
            }
        }

        let hb_infos = buf.glyph_infos();
        let positions = buf.glyph_positions();

        let mut cluster = Vec::with_capacity(s.len());

        // Compute the lengths of the text clusters.
        // Ligatures and combining characters mean
        // that a single glyph can take the place of
        // multiple characters.  The 'cluster' member
        // of the glyph info is set to the position
        // in the input utf8 text, so we make a pass
        // over the set of clusters to look for differences
        // greater than 1 and backfill the length of
        // the corresponding text fragment.  We need
        // the fragments to properly handle fallback,
        // and they're handy to have for debugging
        // purposes too.
        let mut info_clusters: Vec<Vec<Info>> = Vec::with_capacity(s.len());
        let mut info_iter = hb_infos.iter().zip(positions.iter()).peekable();
        while let Some((info, pos)) = info_iter.next() {
            let len = cluster_to_len[info.cluster as usize] as usize;

            let mut info = Info {
                cluster: info.cluster as usize,
                len,
                codepoint: info.codepoint,
                x_advance: pos.x_advance,
                y_advance: pos.y_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            };

            if let Some(ref mut cluster) = info_clusters.last_mut() {
                if cluster.last().unwrap().cluster == info.cluster {
                    cluster.push(info);
                    continue;
                }
                // Don't fragment runs of unresolved codepoints; they could be a sequence
                // that shapes together in a fallback font.
                if info.codepoint == 0 {
                    let prior = cluster.last_mut().unwrap();
                    // This logic essentially merges `info` into `prior` by
                    // extending the length of prior by `info`.
                    // We can only do that if they are contiguous.
                    // Take care, as the shaper may have re-ordered things!
                    if prior.codepoint == 0 {
                        if prior.cluster + prior.len == info.cluster {
                            // Coalesce with prior
                            prior.len += info.len;
                            continue;
                        } else if info.cluster + info.len == prior.cluster {
                            // We actually precede prior; we must have been
                            // re-ordered by the shaper. Re-arrange and
                            // coalesce
                            std::mem::swap(&mut info, prior);
                            prior.len += info.len;
                            continue;
                        }
                    }
                }
            }
            info_clusters.push(vec![info]);
        }
        /*
        if font_idx > 0 {
            log::error!("do_shape: font_idx={} {:?} {:?}", font_idx, s, info_clusters);
        }
        */

        let mut direct_clusters = 0;

        for infos in &info_clusters {
            let cluster_start = infos.iter().map(|info| info.cluster).min().unwrap_or(0);
            let cluster_end: usize = infos
                .iter()
                .map(|info| info.cluster + info.len)
                .max()
                .unwrap();
            let sub_range = cluster_start..cluster_end;
            let substr = &s[sub_range.clone()];

            let incomplete = infos.iter().find(|info| info.codepoint == 0).is_some();

            if incomplete {
                // One or more entries didn't have a corresponding glyph,
                // so try a fallback

                /*
                if font_idx == 0 {
                    log::error!("incomplete cluster for text={:?} {:?}", s, info_clusters);
                }
                */

                let mut shape = match self.do_shape(
                    font_idx + 1,
                    s,
                    font_size,
                    dpi,
                    no_glyphs,
                    presentation,
                    direction,
                    sub_range.clone(),
                ) {
                    Ok(shape) => Ok(shape),
                    Err(e) => {
                        error!("{:?} for {:?}", e, substr);
                        self.do_shape(
                            0,
                            &make_question_string(substr),
                            font_size,
                            dpi,
                            no_glyphs,
                            presentation,
                            direction,
                            0..substr.len(),
                        )
                    }
                }?;

                cluster.append(&mut shape);
                continue;
            }

            let mut next_idx = 0;
            for info in infos.iter() {
                if info.x_advance == 0 {
                    continue;
                }

                let nom_width = ((f64::from(info.x_advance) / 64.0) / cell_width).ceil() as usize;

                let len;
                if nom_width == 0 || !substr.is_char_boundary(next_idx + nom_width) {
                    let remainder = &substr[next_idx..];
                    if let Some(g) = remainder.graphemes(true).next() {
                        len = g.len();
                    } else {
                        len = remainder.len();
                    }
                } else {
                    len = nom_width;
                }

                let glyph = if len > 0 {
                    let text = &substr[next_idx..next_idx + len];
                    make_glyphinfo(text, font_idx, info)
                } else {
                    make_glyphinfo("__", font_idx, info)
                };

                if glyph.x_advance != PixelLength::new(0.0) {
                    // log::error!("glyph: {:?}, nominal width: {:?}/{:?} = {:?}", glyph, glyph.x_advance, cell_width, nom_width);
                    cluster.push(glyph);
                    direct_clusters += 1;
                }

                next_idx += len;
            }
        }

        if !shaped_any {
            if let Some(opt_pair) = self.fonts.get(font_idx) {
                if direct_clusters == 0 {
                    // If we've never shaped anything from this font, and we didn't
                    // shape it just now, then we're probably a fallback font from
                    // the system and unlikely to be useful to keep around, so we
                    // unload it.
                    log::trace!(
                        "Shaper didn't resolve glyphs from {:?}, so unload it",
                        self.handles[font_idx]
                    );
                    opt_pair.borrow_mut().take();
                } else if let Some(pair) = &mut *opt_pair.borrow_mut() {
                    // We shaped something: mark this pair up so that it sticks around
                    pair.shaped_any = true;
                }
            }
        }

        Ok(cluster)
    }
}

impl FontShaper for HarfbuzzShaper {
    fn shape(
        &self,
        text: &str,
        size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
        presentation: Option<Presentation>,
        direction: Direction,
        range: Option<Range<usize>>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let range = range.unwrap_or_else(|| 0..text.len());

        log::trace!("shape byte_len={} `{}`", text.len(), text.escape_debug());
        let start = std::time::Instant::now();
        let result = self.do_shape(
            0,
            text,
            size,
            dpi,
            no_glyphs,
            presentation,
            direction,
            range,
        );
        metrics::histogram!("shape.harfbuzz", start.elapsed());
        /*
        if let Ok(glyphs) = &result {
            for g in glyphs {
                if g.font_idx > 0 {
                    log::error!("{:#?}", result);
                    break;
                }
            }
        }
        */
        result
    }

    fn metrics_for_idx(&self, font_idx: usize, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        let mut pair = self
            .load_fallback(font_idx)?
            .ok_or_else(|| anyhow!("unable to load font idx {}!?", font_idx))?;

        let key = MetricsKey {
            font_idx,
            size: NotNan::new(size).unwrap(),
            dpi,
        };
        if let Some(metrics) = self.metrics.borrow().get(&key) {
            return Ok(metrics.clone());
        }

        let selected_size = pair.face.set_font_size(size, dpi)?;
        let y_scale = unsafe { (*(*pair.face.face).size).metrics.y_scale as f64 / 65536.0 };
        let metrics = FontMetrics {
            cell_height: PixelLength::new(selected_size.height),
            cell_width: PixelLength::new(selected_size.width),
            // Note: face.face.descender is useless, we have to go through
            // face.face.size.metrics to get to the real descender!
            descender: PixelLength::new(
                unsafe { (*(*pair.face.face).size).metrics.descender as f64 } / 64.0,
            ),
            underline_thickness: PixelLength::new(
                unsafe { (*pair.face.face).underline_thickness as f64 } * y_scale / 64.,
            ),
            underline_position: PixelLength::new(
                unsafe { (*pair.face.face).underline_position as f64 } * y_scale / 64.,
            ),
            cap_height_ratio: selected_size.cap_height_to_height_ratio,
            cap_height: selected_size.cap_height.map(PixelLength::new),
            is_scaled: selected_size.is_scaled,
            presentation: pair.presentation,
        };

        self.metrics.borrow_mut().insert(key, metrics.clone());

        log::trace!(
            "metrics_for_idx={}, size={}, dpi={} -> {:?}",
            font_idx,
            size,
            dpi,
            metrics
        );

        Ok(metrics)
    }

    fn metrics(&self, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        // Returns the metrics for the selected font... but look out
        // for implausible sizes.
        // Ideally we wouldn't need this, but in the event that a user
        // has a wonky configuration we don't want to pick something
        // like a bitmap emoji font for the metrics or well end up
        // with crazy huge cells.
        // We do a sniff test based on the theoretical pixel height for
        // the supplied size+dpi.
        // If a given fallback slot deviates from the theoretical size
        // by too much we'll skip to the next slot.
        let theoretical_height = size * dpi as f64 / 72.0;
        let mut metrics_idx = 0;
        log::trace!(
            "compute metrics across these handles for size={}, dpi={},
             theoretical pixel height {}: {:?}",
            size,
            dpi,
            theoretical_height,
            self.handles
        );
        while let Ok(Some(mut pair)) = self.load_fallback(metrics_idx) {
            let selected_size = pair.face.set_font_size(size, dpi)?;
            let diff = (theoretical_height - selected_size.height).abs();
            let factor = diff / theoretical_height;
            if factor < 2.0 {
                log::trace!(
                    "idx {} cell_height is {}, which is {} away from theoretical
                     height (factor {}). Seems good enough",
                    metrics_idx,
                    selected_size.height,
                    diff,
                    factor
                );
                break;
            }
            log::trace!(
                "skip idx {} because diff={} factor={} theoretical_height={} cell_height={}",
                metrics_idx,
                diff,
                factor,
                theoretical_height,
                selected_size.height
            );
            metrics_idx += 1;
        }

        self.metrics_for_idx(metrics_idx, size, dpi)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::FontDatabase;
    use config::FontAttributes;

    #[test]
    fn ligatures() {
        let _ = pretty_env_logger::formatted_builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        let db = FontDatabase::with_built_in().unwrap();
        let handle = db
            .resolve(
                &FontAttributes {
                    family: "JetBrains Mono".into(),
                    stretch: Default::default(),
                    weight: Default::default(),
                    is_fallback: false,
                    is_synthetic: false,
                    italic: false,
                    freetype_load_flags: None,
                    freetype_load_target: None,
                    freetype_render_target: None,
                    harfbuzz_features: None,
                },
                14,
            )
            .unwrap()
            .clone();

        let config = config::configuration();

        let shaper = HarfbuzzShaper::new(&config, &[handle]).unwrap();
        {
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "abc",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "a",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 180,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "b",
        is_space: false,
        num_cells: 1,
        cluster: 1,
        font_idx: 0,
        glyph_pos: 205,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "c",
        is_space: false,
        num_cells: 1,
        cluster: 2,
        font_idx: 0,
        glyph_pos: 206,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }
        {
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "<",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "<",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 726,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }
        {
            // This is a ligatured sequence, but you wouldn't know
            // from this info :-/
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "<-",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "<",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 1212,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "-",
        is_space: false,
        num_cells: 1,
        cluster: 1,
        font_idx: 0,
        glyph_pos: 1065,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }
        {
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "<--",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "<",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 726,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "-",
        is_space: false,
        num_cells: 1,
        cluster: 1,
        font_idx: 0,
        glyph_pos: 1212,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "-",
        is_space: false,
        num_cells: 1,
        cluster: 2,
        font_idx: 0,
        glyph_pos: 623,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }

        {
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "x x",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "x",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 350,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: " ",
        is_space: true,
        num_cells: 1,
        cluster: 1,
        font_idx: 0,
        glyph_pos: 686,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "x",
        is_space: false,
        num_cells: 1,
        cluster: 2,
        font_idx: 0,
        glyph_pos: 350,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }

        {
            let mut no_glyphs = vec![];
            let info = shaper
                .shape(
                    "x\u{3000}x",
                    10.,
                    72,
                    &mut no_glyphs,
                    None,
                    Direction::LeftToRight,
                    None,
                )
                .unwrap();
            assert!(no_glyphs.is_empty(), "{:?}", no_glyphs);
            k9::snapshot!(
                info,
                r#"
[
    GlyphInfo {
        text: "x",
        is_space: false,
        num_cells: 1,
        cluster: 0,
        font_idx: 0,
        glyph_pos: 350,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "\u{3000}",
        is_space: false,
        num_cells: 2,
        cluster: 1,
        font_idx: 0,
        glyph_pos: 686,
        x_advance: 10.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
    GlyphInfo {
        text: "x",
        is_space: false,
        num_cells: 1,
        cluster: 4,
        font_idx: 0,
        glyph_pos: 350,
        x_advance: 6.0,
        y_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    },
]
"#
            );
        }
    }
}
