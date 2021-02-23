use crate::gui::glyphcache::CachedGlyph;
use ::window::bitmaps::Texture2d;
use config::TextStyle;
use std::rc::Rc;
use termwiz::cellcluster::CellCluster;
use wezterm_font::shaper::GlyphInfo;
use wezterm_font::units::*;

#[derive(PartialEq, Eq, Hash)]
pub struct ShapeCacheKey {
    pub style: TextStyle,
    pub text: String,
}

#[derive(Debug, PartialEq)]
pub struct GlyphPosition {
    pub glyph_idx: u32,
    pub cluster: u32,
    pub num_cells: u8,
    pub x_offset: PixelLength,
    pub bearing_x: f32,
    pub bitmap_pixel_width: u32,
}

#[derive(Debug)]
pub struct ShapedInfo<T>
where
    T: Texture2d,
    T: std::fmt::Debug,
{
    pub glyph: Rc<CachedGlyph<T>>,
    pub pos: GlyphPosition,
}

impl<T> ShapedInfo<T>
where
    T: Texture2d,
    T: std::fmt::Debug,
{
    /// Process the results from the shaper.
    /// Ideally this would not be needed, but the shaper doesn't
    /// merge certain forms of ligatured cluster, and won't merge
    /// certain combining sequences for which no glyph could be
    /// found for the resultant grapheme.
    /// This function's goal is to handle those two cases.
    pub fn process(
        cluster: &CellCluster,
        infos: &[GlyphInfo],
        glyphs: &[Rc<CachedGlyph<T>>],
    ) -> Vec<ShapedInfo<T>> {
        let mut pos = vec![];
        let mut run = None;
        for (info, glyph) in infos.iter().zip(glyphs.iter()) {
            if !info.is_space && glyph.texture.is_none() {
                if run.is_none() {
                    run.replace(ShapedInfo {
                        pos: GlyphPosition {
                            glyph_idx: info.glyph_pos,
                            cluster: info.cluster,
                            num_cells: info.num_cells,
                            x_offset: info.x_advance,
                            bearing_x: 0.,
                            bitmap_pixel_width: 0,
                        },
                        glyph: Rc::clone(glyph),
                    });
                    continue;
                }

                let run = run.as_mut().unwrap();
                run.pos.num_cells += info.num_cells;
                run.pos.x_offset += info.x_advance;
                continue;
            }

            if let Some(mut run) = run.take() {
                run.glyph = Rc::clone(glyph);
                run.pos.glyph_idx = info.glyph_pos;
                run.pos.num_cells += info.num_cells;
                run.pos.bitmap_pixel_width = glyph.texture.as_ref().unwrap().coords.width() as u32;
                run.pos.bearing_x = (run.pos.x_offset.get() + glyph.bearing_x.get() as f64) as f32;
                run.pos.x_offset = PixelLength::new(0.);
                pos.push(run);
            } else {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                if let Some(prior) = pos.last() {
                    let prior_cell_idx = cluster.byte_to_cell_idx[prior.pos.cluster as usize];
                    if cell_idx <= prior_cell_idx {
                        // This is a tricky case: if we have a cluster such as
                        // 1F470 1F3FF 200D 2640 (woman with veil: dark skin tone)
                        // and the font doesn't define a glyph for it, the shaper
                        // may give us a sequence of three output clusters, each
                        // comprising: veil, skin tone and female respectively.
                        // Those all have the same info.cluster which
                        // means that they all resolve to the same cell_idx.
                        // In this case, the cluster is logically a single cell,
                        // and the best presentation is of the veil, so we pick
                        // that one and ignore the rest of the glyphs that map to
                        // this same cell.
                        // Ideally we'd overlay this with a "something is broken"
                        // glyph in the corner.
                        continue;
                    }
                }
                pos.push(ShapedInfo {
                    pos: GlyphPosition {
                        glyph_idx: info.glyph_pos,
                        bitmap_pixel_width: glyph
                            .texture
                            .as_ref()
                            .map_or(0, |t| t.coords.width() as u32),
                        cluster: info.cluster,
                        num_cells: info.num_cells,
                        x_offset: info.x_offset,
                        bearing_x: glyph.bearing_x.get() as f32,
                    },
                    glyph: Rc::clone(glyph),
                });
            }
        }
        pos
    }
}

/// We'd like to avoid allocating when resolving from the cache
/// so this is the borrowed version of ShapeCacheKey.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
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

pub trait ShapeCacheKeyTrait {
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

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for lru::KeyRef<ShapeCacheKey> {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        let k: &ShapeCacheKey = self.borrow();
        k
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
    use super::*;
    use crate::gui::glyphcache::GlyphCache;
    use crate::gui::shapecache::GlyphPosition;
    use crate::gui::shapecache::ShapedInfo;
    use crate::gui::utilsprites::RenderMetrics;
    use config::TextStyle;
    use k9::assert_equal as assert_eq;
    use std::rc::Rc;
    use termwiz::cell::CellAttributes;
    use termwiz::surface::Line;
    use wezterm_font::FontConfiguration;
    use wezterm_font::LoadedFont;

    fn cluster_and_shape<T>(
        glyph_cache: &mut GlyphCache<T>,
        style: &TextStyle,
        font: &Rc<LoadedFont>,
        text: &str,
    ) -> Vec<GlyphPosition>
    where
        T: Texture2d,
        T: std::fmt::Debug,
    {
        let line = Line::from_text(text, &CellAttributes::default());
        eprintln!("{:?}", line);
        let cell_clusters = line.cluster();
        assert_eq!(cell_clusters.len(), 1);
        let cluster = &cell_clusters[0];
        let infos = font.shape(&cluster.text).unwrap();
        let glyphs = infos
            .iter()
            .map(|info| {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];

                let followed_by_space = match line.cells().get(cell_idx + 1) {
                    Some(cell) => cell.str() == " ",
                    None => false,
                };

                glyph_cache
                    .cached_glyph(info, &style, followed_by_space)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        eprintln!("infos: {:#?}", infos);
        eprintln!("glyphs: {:#?}", glyphs);
        ShapedInfo::process(&cluster, &infos, &glyphs)
            .into_iter()
            .map(|p| p.pos)
            .collect()
    }

    #[test]
    fn ligatures() {
        config::use_test_configuration();
        let _ = pretty_env_logger::formatted_builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();

        let fonts = Rc::new(FontConfiguration::new().unwrap());
        let render_metrics = RenderMetrics::new(&fonts).unwrap();
        let mut glyph_cache = GlyphCache::new_in_memory(&fonts, 128, &render_metrics).unwrap();

        let style = TextStyle::default();
        let font = fonts.resolve_font(&style).unwrap();

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "ab"),
            vec![
                GlyphPosition {
                    glyph_idx: 180,
                    cluster: 0,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 7,
                },
                GlyphPosition {
                    glyph_idx: 205,
                    cluster: 1,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 1.0,
                    bitmap_pixel_width: 6,
                },
            ]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "a b"),
            vec![
                GlyphPosition {
                    glyph_idx: 180,
                    cluster: 0,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 7,
                },
                GlyphPosition {
                    glyph_idx: 686,
                    cluster: 1,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 0,
                },
                GlyphPosition {
                    glyph_idx: 205,
                    cluster: 2,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 1.0,
                    bitmap_pixel_width: 6,
                },
            ]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "a  b"),
            vec![
                GlyphPosition {
                    glyph_idx: 180,
                    cluster: 0,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 7,
                },
                GlyphPosition {
                    glyph_idx: 686,
                    cluster: 1,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 0,
                },
                GlyphPosition {
                    glyph_idx: 686,
                    cluster: 2,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 0.0,
                    bitmap_pixel_width: 0,
                },
                GlyphPosition {
                    glyph_idx: 205,
                    cluster: 3,
                    num_cells: 1,
                    x_offset: PixelLength::new(0.0),
                    bearing_x: 1.0,
                    bitmap_pixel_width: 6,
                },
            ]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "<-"),
            vec![GlyphPosition {
                glyph_idx: 1065,
                cluster: 0,
                num_cells: 2,
                x_offset: PixelLength::new(0.0),
                bearing_x: 1.0,
                bitmap_pixel_width: 14,
            }]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "<>"),
            vec![GlyphPosition {
                glyph_idx: 1089,
                cluster: 0,
                num_cells: 2,
                x_offset: PixelLength::new(0.0),
                bearing_x: 2.0,
                bitmap_pixel_width: 12,
            }]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "|=>"),
            vec![GlyphPosition {
                glyph_idx: 1040,
                cluster: 0,
                num_cells: 3,
                x_offset: PixelLength::new(0.0),
                bearing_x: 2.0,
                bitmap_pixel_width: 21,
            }]
        );

        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, "<!--"),
            vec![GlyphPosition {
                glyph_idx: 1071,
                cluster: 0,
                num_cells: 4,
                x_offset: PixelLength::new(0.0),
                bearing_x: 1.0,
                bitmap_pixel_width: 30,
            }]
        );

        let deaf_man_medium_light_skin_tone = "\u{1F9CF}\u{1F3FC}\u{200D}\u{2642}\u{FE0F}";
        println!(
            "deaf_man_medium_light_skin_tone: {}",
            deaf_man_medium_light_skin_tone
        );
        assert_eq!(
            cluster_and_shape(
                &mut glyph_cache,
                &style,
                &font,
                deaf_man_medium_light_skin_tone
            ),
            vec![GlyphPosition {
                glyph_idx: 298,
                cluster: 0,
                num_cells: 2,
                x_offset: PixelLength::new(0.0),
                bearing_x: 1.0666667,
                bitmap_pixel_width: 14,
            }]
        );

        let england_flag = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
        println!("england_flag: {}", england_flag);
        assert_eq!(
            cluster_and_shape(&mut glyph_cache, &style, &font, england_flag),
            vec![GlyphPosition {
                glyph_idx: 1583,
                cluster: 0,
                num_cells: 2,
                x_offset: PixelLength::new(0.0),
                bearing_x: 0.,
                bitmap_pixel_width: 14,
            }]
        );
    }
}
