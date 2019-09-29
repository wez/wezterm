use crate::bitmaps::{BitmapImage, Texture2d, TextureRect};
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
pub struct Atlas<T>
where
    T: Texture2d,
{
    texture: Rc<T>,

    /// Dimensions of the texture
    side: usize,

    /// The bottom of the available space.
    bottom: usize,

    /// The height of the tallest sprite allocated on the current row
    tallest: usize,

    /// How far along the current row we've progressed
    left: usize,
}

impl<T> Atlas<T>
where
    T: Texture2d,
{
    pub fn new(texture: &Rc<T>) -> Fallible<Self> {
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
    pub fn texture(&self) -> Rc<T> {
        Rc::clone(&self.texture)
    }

    /// Reserve space for a sprite of the given size
    pub fn allocate(&mut self, im: &dyn BitmapImage) -> Result<Sprite<T>, OutOfTextureSpace> {
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

    pub fn size(&self) -> usize {
        self.side
    }
}

pub struct Sprite<T>
where
    T: Texture2d,
{
    pub texture: Rc<T>,
    pub coords: Rect,
}

impl<T> Sprite<T>
where
    T: Texture2d,
{
    /// Returns the texture coordinates of the sprite
    pub fn texture_coords(&self) -> TextureRect {
        self.texture.to_texture_coords(self.coords)
    }
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

impl SpriteSlice {
    pub fn pixel_rect<T: Texture2d>(&self, sprite: &Sprite<T>) -> Rect {
        let width = self.slice_width(sprite) as isize;
        let left = self.left_pix(sprite) as isize;

        Rect::new(
            Point::new(sprite.coords.origin.x + left, sprite.coords.origin.y),
            Size::new(width, sprite.coords.size.height),
        )
    }

    /// Returns the scaled offset to the left most pixel in a slice.
    /// This is 0 for the first slice and increases by the slice_width
    /// as we work through the slices.
    pub fn left_pix<T: Texture2d>(&self, sprite: &Sprite<T>) -> f32 {
        let width = sprite.coords.size.width as f32 * self.scale;
        if self.num_cells == 1 || self.cell_idx == 0 {
            0.0
        } else {
            // Width of the first cell
            let cell_0 = width.min((self.cell_width as f32) - self.left_offset);

            if self.cell_idx == self.num_cells - 1 {
                // Width of all the other cells
                let middle = self.cell_width * (self.num_cells - 2);
                cell_0 + middle as f32
            } else {
                // Width of all the preceding cells
                let prev = self.cell_width * self.cell_idx;
                cell_0 + prev as f32
            }
        }
    }

    /// Returns the (scaled) pixel width of a slice.
    /// This is nominally the cell_width but can be modified by being the first
    /// or last in a sequence of potentially oversized sprite slices.
    pub fn slice_width<T: Texture2d>(&self, sprite: &Sprite<T>) -> f32 {
        let width = sprite.coords.size.width as f32 * self.scale;

        if self.num_cells == 1 {
            width
        } else if self.cell_idx == 0 {
            // The first slice can extend (or recede) to the left based
            // on the slice.left_offset value.
            width.min((self.cell_width as f32) - self.left_offset)
        } else if self.cell_idx == self.num_cells - 1 {
            width - self.left_pix(sprite)
        } else {
            // somewhere in the middle of the sequence, the width is
            // simply the cell_width
            self.cell_width as f32
        }
    }
}
