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
                run.pos.x_offset = info.x_advance - PixelLength::new(run.pos.bearing_x as f64);
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
