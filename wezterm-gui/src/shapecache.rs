use crate::customglyph::BlockKey;
use crate::glyphcache::CachedGlyph;
use config::TextStyle;
use std::rc::Rc;
use wezterm_font::shaper::GlyphInfo;
use wezterm_font::units::*;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct ShapeCacheKey {
    pub style: TextStyle,
    pub text: String,
}

#[derive(Debug, PartialEq)]
pub struct GlyphPosition {
    pub glyph_idx: u32,
    pub num_cells: u8,
    pub x_offset: PixelLength,
    pub bearing_x: f32,
    pub bitmap_pixel_width: u32,
}

#[derive(Debug)]
pub struct ShapedInfo {
    pub glyph: Rc<CachedGlyph>,
    pub pos: GlyphPosition,
    pub block_key: Option<BlockKey>,
}

impl ShapedInfo {
    /// Process the results from the shaper, stitching together glyph
    /// and positioning information
    pub fn process(infos: &[GlyphInfo], glyphs: &[Rc<CachedGlyph>]) -> Vec<ShapedInfo> {
        let mut pos: Vec<ShapedInfo> = Vec::with_capacity(infos.len());

        for (info, glyph) in infos.iter().zip(glyphs.iter()) {
            pos.push(ShapedInfo {
                pos: GlyphPosition {
                    glyph_idx: info.glyph_pos,
                    bitmap_pixel_width: glyph
                        .texture
                        .as_ref()
                        .map_or(0, |t| t.coords.width() as u32),
                    num_cells: info.num_cells,
                    x_offset: info.x_offset,
                    bearing_x: glyph.bearing_x.get() as f32,
                },
                glyph: Rc::clone(glyph),
                block_key: info.only_char.and_then(BlockKey::from_char),
            });
        }
        pos
    }
}

/// We'd like to avoid allocating when resolving from the cache
/// so this is the borrowed version of ShapeCacheKey.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BorrowedShapeCacheKey<'a> {
    pub style: &'a TextStyle,
    pub text: &'a str,
}

impl<'a> BorrowedShapeCacheKey<'a> {
    pub fn to_owned(&self) -> ShapeCacheKey {
        ShapeCacheKey {
            style: self.style.clone(),
            text: self.text.to_owned(),
        }
    }
}

pub trait ShapeCacheKeyTrait: std::fmt::Debug {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k>;
}

impl ShapeCacheKeyTrait for ShapeCacheKey {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        BorrowedShapeCacheKey {
            style: &self.style,
            text: &self.text,
        }
    }
}

impl<'a> ShapeCacheKeyTrait for BorrowedShapeCacheKey<'a> {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        *self
    }
}

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for ShapeCacheKey {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        self
    }
}

impl<'a> PartialEq for (dyn ShapeCacheKeyTrait + 'a) {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(&other.key())
    }
}

impl<'a> Eq for (dyn ShapeCacheKeyTrait + 'a) {}

impl<'a> std::hash::Hash for (dyn ShapeCacheKeyTrait + 'a) {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}

#[cfg(test)]
mod test {
    use crate::glyphcache::GlyphCache;
    use crate::shapecache::{GlyphPosition, ShapedInfo};
    use crate::utilsprites::RenderMetrics;
    use config::{FontAttributes, TextStyle};
    use std::rc::Rc;
    use termwiz::cell::CellAttributes;
    use termwiz::surface::{Line, SEQ_ZERO};
    use wezterm_bidi::Direction;
    use wezterm_font::shaper::PresentationWidth;
    use wezterm_font::{FontConfiguration, LoadedFont};

    fn cluster_and_shape(
        render_metrics: &RenderMetrics,
        glyph_cache: &mut GlyphCache,
        style: &TextStyle,
        font: &Rc<LoadedFont>,
        text: &str,
    ) -> Vec<GlyphPosition> {
        let line = Line::from_text(text, &CellAttributes::default(), SEQ_ZERO, None);
        eprintln!("{:?}", line);
        let mut all_infos = vec![];
        let mut all_glyphs = vec![];

        for cluster in line.cluster(None) {
            let presentation_width = PresentationWidth::with_cluster(&cluster);
            let mut infos = font
                .shape(
                    &cluster.text,
                    || {},
                    |_| {},
                    None,
                    Direction::LeftToRight,
                    None,
                    Some(&presentation_width),
                )
                .unwrap();
            let mut glyphs = infos
                .iter()
                .map(|info| {
                    let cell_idx = cluster.byte_to_cell_idx(info.cluster as usize);
                    let num_cells = cluster.byte_to_cell_width(info.cluster as usize);

                    let followed_by_space = match line.get_cell(cell_idx + 1) {
                        Some(cell) => cell.str() == " ",
                        None => false,
                    };

                    glyph_cache
                        .cached_glyph(
                            info,
                            &style,
                            followed_by_space,
                            font,
                            render_metrics,
                            num_cells,
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>();

            all_infos.append(&mut infos);
            all_glyphs.append(&mut glyphs);
        }

        eprintln!("infos: {:#?}", all_infos);
        eprintln!("glyphs: {:#?}", all_glyphs);
        ShapedInfo::process(&all_infos, &all_glyphs)
            .into_iter()
            .map(|p| p.pos)
            .collect()
    }

    #[test]
    fn ligatures_fira() {
        config::use_test_configuration();
        let _ = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        let config = config::configuration();

        let mut config: config::Config = (*config).clone();
        config.font = TextStyle {
            font: vec![FontAttributes::new("Fira Code")],
            foreground: None,
        };
        config.font_rules.clear();
        config.compute_extra_defaults(None);
        config::use_this_configuration(config.clone());

        let fonts = Rc::new(
            FontConfiguration::new(
                None,
                config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
            )
            .unwrap(),
        );
        let render_metrics = RenderMetrics::new(&fonts).unwrap();
        let mut glyph_cache = GlyphCache::new_in_memory(&fonts, 128).unwrap();

        let style = TextStyle::default();
        let font = fonts.resolve_font(&style).unwrap();

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "a..."),
            "
[
    GlyphPosition {
        glyph_idx: 180,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 637,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -15.0,
        bitmap_pixel_width: 20,
    },
]
"
        );
    }

    #[test]
    fn bench_shaping() {
        config::use_test_configuration();

        // let mut glyph_cache = GlyphCache::new_in_memory(&fonts, 128, &render_metrics).unwrap();
        // let render_metrics = RenderMetrics::new(&fonts).unwrap();

        benchmarking::warm_up();

        for &n in &[100, 1000, 10_000] {
            let bench_result = benchmarking::measure_function(move |measurer| {
                let text: String = (0..n).map(|_| ' ').collect();

                let fonts = Rc::new(
                    FontConfiguration::new(
                        None,
                        config::configuration()
                            .dpi
                            .unwrap_or_else(|| ::window::default_dpi())
                            as usize,
                    )
                    .unwrap(),
                );
                let style = TextStyle::default();
                let font = fonts.resolve_font(&style).unwrap();
                let line = Line::from_text(&text, &CellAttributes::default(), SEQ_ZERO, None);
                let cell_clusters = line.cluster(None);
                let cluster = &cell_clusters[0];
                let presentation_width = PresentationWidth::with_cluster(&cluster);

                measurer.measure(|| {
                    let _x = font
                        .shape(
                            &cluster.text,
                            || {},
                            |_| {},
                            None,
                            Direction::LeftToRight,
                            None,
                            Some(&presentation_width),
                        )
                        .unwrap();
                    // println!("{:?}", &x[0..2]);
                });
            })
            .unwrap();
            println!("{}: {:?}", n, bench_result.elapsed());
        }
    }

    #[test]
    fn ligatures_jetbrains() {
        config::use_test_configuration();
        let _ = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();
        let config = config::configuration();

        let fonts = Rc::new(
            FontConfiguration::new(
                None,
                config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
            )
            .unwrap(),
        );
        let render_metrics = RenderMetrics::new(&fonts).unwrap();
        let mut glyph_cache = GlyphCache::new_in_memory(&fonts, 128).unwrap();

        let style = TextStyle::default();
        let font = fonts.resolve_font(&style).unwrap();

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "ab"),
            "
[
    GlyphPosition {
        glyph_idx: 180,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 205,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "a b"),
            "
[
    GlyphPosition {
        glyph_idx: 180,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 686,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 205,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "a..."),
            "
[
    GlyphPosition {
        glyph_idx: 180,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 637,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -15.0,
        bitmap_pixel_width: 20,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "e_or_"),
            "
[
    GlyphPosition {
        glyph_idx: 216,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 610,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 9,
    },
    GlyphPosition {
        glyph_idx: 279,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 308,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 610,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 9,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "a  b"),
            "
[
    GlyphPosition {
        glyph_idx: 180,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
    GlyphPosition {
        glyph_idx: 686,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 686,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 205,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 1.0,
        bitmap_pixel_width: 8,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "<-"),
            "
[
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1065,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -9.0,
        bitmap_pixel_width: 17,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "<>"),
            "
[
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1089,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -8.0,
        bitmap_pixel_width: 16,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "|=>"),
            "
[
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1040,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -18.0,
        bitmap_pixel_width: 27,
    },
]
"
        );

        let block_bottom_one_eighth = "\u{2581}";
        k9::snapshot!(
            cluster_and_shape(
                &render_metrics,
                &mut glyph_cache,
                &style,
                &font,
                block_bottom_one_eighth
            ),
            "
[
    GlyphPosition {
        glyph_idx: 790,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 10,
    },
]
"
        );

        let powerline_extra_honeycomb = "\u{e0cc}";
        k9::snapshot!(
            cluster_and_shape(
                &render_metrics,
                &mut glyph_cache,
                &style,
                &font,
                powerline_extra_honeycomb,
            ),
            "
[
    GlyphPosition {
        glyph_idx: 51,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 12,
    },
]
"
        );

        k9::snapshot!(
            cluster_and_shape(&render_metrics, &mut glyph_cache, &style, &font, "<!--"),
            "
[
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1212,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 0,
    },
    GlyphPosition {
        glyph_idx: 1071,
        num_cells: 1,
        x_offset: 0.0,
        bearing_x: -28.0,
        bitmap_pixel_width: 37,
    },
]
"
        );

        let deaf_man_medium_light_skin_tone = "\u{1F9CF}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}";
        println!(
            "deaf_man_medium_light_skin_tone: {}",
            deaf_man_medium_light_skin_tone
        );
        k9::snapshot!(
            cluster_and_shape(
                &render_metrics,
                &mut glyph_cache,
                &style,
                &font,
                deaf_man_medium_light_skin_tone
            ),
            "
[
    GlyphPosition {
        glyph_idx: 3249,
        num_cells: 2,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 17,
    },
]
"
        );

        let england_flag = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
        println!("england_flag: {}", england_flag);
        k9::snapshot!(
            cluster_and_shape(
                &render_metrics,
                &mut glyph_cache,
                &style,
                &font,
                england_flag
            ),
            "
[
    GlyphPosition {
        glyph_idx: 1857,
        num_cells: 2,
        x_offset: 0.0,
        bearing_x: 0.0,
        bitmap_pixel_width: 20,
    },
]
"
        );
    }
}
