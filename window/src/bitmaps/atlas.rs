use crate::bitmaps::{BitmapImage, Texture2d};
use crate::{Point, Rect, Size};
use failure::{ensure, Fallible};
use failure_derive::*;
use std::rc::Rc;

pub const TEX_SIZE: u32 = 4096;

#[derive(Debug, Fail)]
#[fail(display = "Texture Size exceeded, need {}", size)]
pub struct OutOfTextureSpace {
    pub size: usize,
}

/// Atlases are bitmaps of srgba data that are sized as a power of 2.
/// We allocate sprites out of the available space, starting from the
/// bottom left corner and working to the right until we run out of
/// space, then we move up to the logical row above.  Since sprites can
/// have varying height the height of the rows can also vary.
pub struct Atlas {
    texture: Rc<dyn Texture2d>,

    /// Dimensions of the texture
    side: usize,

    /// The bottom of the available space.
    bottom: usize,

    /// The height of the tallest sprite allocated on the current row
    tallest: usize,

    /// How far along the current row we've progressed
    left: usize,
}

impl Atlas {
    pub fn new(texture: &Rc<dyn Texture2d>) -> Fallible<Self> {
        ensure!(
            texture.width() == texture.height(),
            "texture must be square!"
        );
        Ok(Self {
            texture: Rc::clone(texture),
            side: texture.width(),
            bottom: 0,
            tallest: 0,
            left: 0,
        })
    }

    #[inline]
    pub fn texture(&self) -> Rc<dyn Texture2d> {
        Rc::clone(&self.texture)
    }

    /// Reserve space for a sprite of the given size
    pub fn allocate(&mut self, im: &dyn BitmapImage) -> Result<Sprite, OutOfTextureSpace> {
        let (width, height) = im.image_dimensions();

        // We pad each sprite reservation with blank space to avoid
        // surprising and unexpected artifacts when the texture is
        // interpolated on to the render surface.
        // In addition, we need to ensure that the bottom left pixel
        // is blank as we use that for whitespace glyphs.
        let reserve_width = width + 2;
        let reserve_height = height + 2;

        if reserve_width > self.side || reserve_height > self.side {
            // It's not possible to satisfy that request
            return Err(OutOfTextureSpace {
                size: reserve_width.max(reserve_height).next_power_of_two(),
            });
        }
        let x_left = self.side - self.left;
        if x_left < reserve_width {
            // Bump up to next row
            self.bottom += self.tallest;
            self.left = 0;
            self.tallest = 0;
        }

        // Do we have vertical space?
        let y_left = self.side - self.bottom;
        if y_left < reserve_height {
            // No room at the inn.
            return Err(OutOfTextureSpace {
                size: (self.side + reserve_width.max(reserve_height)).next_power_of_two(),
            });
        }

        let rect = Rect::new(
            Point::new(self.left as isize + 1, self.bottom as isize + 1),
            Size::new(width as isize, height as isize),
        );

        self.texture.write(rect, im);

        self.left += reserve_width;
        self.tallest = self.tallest.max(reserve_height);

        Ok(Sprite {
            texture: Rc::clone(&self.texture),
            coords: rect,
        })
    }
}

pub struct Sprite {
    pub texture: Rc<dyn Texture2d>,
    pub coords: Rect,
}

/// Represents a vertical slice through a sprite.
/// These are used to handle multi-cell wide glyphs.
/// Each cell is nominally `cell_width` wide but font metrics
/// may result in the glyphs being wider than this.
pub struct SpriteSlice {
    /// This is glyph X out of num_cells
    pub cell_idx: usize,
    /// How many cells comprise this glyph
    pub num_cells: usize,
    /// The nominal width of each cell
    pub cell_width: usize,
    /// The glyph will be scaled from sprite pixels down to
    /// cell pixels by this factor.
    pub scale: f32,
    /// The font metrics will adjust the left-most pixel
    /// by this amount.  This causes the width of cell 0
    /// to be adjusted by this same amount.
    pub left_offset: f32,
}

impl Sprite {
    /// Returns the scaled offset to the left most pixel in a slice.
    /// This is 0 for the first slice and increases by the slice_width
    /// as we work through the slices.
    pub fn left_pix(&self, slice: &SpriteSlice) -> f32 {
        let width = self.coords.size.width as f32 * slice.scale;
        if slice.num_cells == 1 || slice.cell_idx == 0 {
            0.0
        } else {
            // Width of the first cell
            let cell_0 = width.min((slice.cell_width as f32) - slice.left_offset);

            if slice.cell_idx == slice.num_cells - 1 {
                // Width of all the other cells
                let middle = slice.cell_width * (slice.num_cells - 2);
                cell_0 + middle as f32
            } else {
                // Width of all the preceding cells
                let prev = slice.cell_width * slice.cell_idx;
                cell_0 + prev as f32
            }
        }
    }

    /// Returns the (scaled) pixel width of a slice.
    /// This is nominally the cell_width but can be modified by being the first
    /// or last in a sequence of potentially oversized sprite slices.
    pub fn slice_width(&self, slice: &SpriteSlice) -> f32 {
        let width = self.coords.size.width as f32 * slice.scale;

        if slice.num_cells == 1 {
            width
        } else if slice.cell_idx == 0 {
            // The first slice can extend (or recede) to the left based
            // on the slice.left_offset value.
            width.min((slice.cell_width as f32) - slice.left_offset)
        } else if slice.cell_idx == slice.num_cells - 1 {
            width - self.left_pix(slice)
        } else {
            // somewhere in the middle of the sequence, the width is
            // simply the cell_width
            slice.cell_width as f32
        }
    }

    /// Returns the left coordinate for a slice in texture coordinate space
    #[inline]
    pub fn left(&self, slice: &SpriteSlice) -> f32 {
        let left = self.coords.min_x() as f32 + (self.left_pix(slice) / slice.scale);
        left / self.texture.width() as f32
    }

    /// Returns the right coordinate for a slice in texture coordinate space
    #[inline]
    pub fn right(&self, slice: &SpriteSlice) -> f32 {
        let right = self.coords.min_x() as f32
            + ((self.left_pix(slice) + self.slice_width(slice)) as f32 / slice.scale);
        right / self.texture.width() as f32
    }

    /// Returns the top coordinate for a slice in texture coordinate space
    #[inline]
    pub fn top(&self, _slice: &SpriteSlice) -> f32 {
        self.coords.max_y() as f32 / self.texture.height() as f32
    }

    /// Returns the bottom coordinate for a slice in texture coordinate space
    #[inline]
    pub fn bottom(&self, _slice: &SpriteSlice) -> f32 {
        (self.coords.max_y() + self.coords.size.height as isize) as f32
            / self.texture.height() as f32
    }

    /// Returns the top-left coordinate for a slice in texture coordinate space
    #[inline]
    pub fn top_left(&self, slice: &SpriteSlice) -> (f32, f32) {
        (self.left(slice), self.top(slice))
    }

    /// Returns the bottom-left coordinate for a slice in texture coordinate
    /// space
    #[inline]
    pub fn bottom_left(&self, slice: &SpriteSlice) -> (f32, f32) {
        (self.left(slice), self.bottom(slice))
    }

    /// Returns the bottom-right coordinate for a slice in texture coordinate
    /// space
    #[inline]
    pub fn bottom_right(&self, slice: &SpriteSlice) -> (f32, f32) {
        (self.right(slice), self.bottom(slice))
    }

    /// Returns the top-right coordinate for a slice in texture coordinate space
    #[inline]
    pub fn top_right(&self, slice: &SpriteSlice) -> (f32, f32) {
        (self.right(slice), self.top(slice))
    }
}
