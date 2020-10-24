use crate::bitmaps::{BitmapImage, Texture2d, TextureRect};
use crate::{Point, Rect, Size};
use anyhow::{ensure, Result as Fallible};
use guillotiere::{AtlasAllocator, Size as AtlasSize};
use std::convert::TryInto;
use std::rc::Rc;
use thiserror::*;

const PADDING: i32 = 1;

#[derive(Debug, Error)]
#[error("Texture Size exceeded, need {:?}", size)]
pub struct OutOfTextureSpace {
    pub size: Option<usize>,
}

/// Atlases are bitmaps of srgba data that are sized as a power of 2.
/// We allocate sprites out of the available space, using AtlasAllocator
/// to manage the available rectangles.
pub struct Atlas<T>
where
    T: Texture2d,
{
    texture: Rc<T>,

    allocator: AtlasAllocator,

    /// Dimensions of the texture
    side: usize,
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
        let side = texture.width();
        let iside = side as isize;

        let image = crate::Image::new(side, side);
        let rect = Rect::new(Point::new(0, 0), Size::new(iside, iside));
        texture.write(rect, &image);

        let allocator = AtlasAllocator::new(AtlasSize::new(side.try_into()?, side.try_into()?));
        Ok(Self {
            texture: Rc::clone(texture),
            side,
            allocator,
        })
    }

    #[inline]
    pub fn texture(&self) -> Rc<T> {
        Rc::clone(&self.texture)
    }

    /// Reserve space for a sprite of the given size
    pub fn allocate(&mut self, im: &dyn BitmapImage) -> Result<Sprite<T>, OutOfTextureSpace> {
        self.allocate_with_padding(im, None)
    }

    pub fn allocate_with_padding(
        &mut self,
        im: &dyn BitmapImage,
        padding: Option<usize>,
    ) -> Result<Sprite<T>, OutOfTextureSpace> {
        let (width, height) = im.image_dimensions();

        // If we can't convert the sizes to i32, then we'll never
        // be able to store this image
        let reserve_width: i32 = width
            .try_into()
            .map_err(|_| OutOfTextureSpace { size: None })?;
        let reserve_height: i32 = height
            .try_into()
            .map_err(|_| OutOfTextureSpace { size: None })?;

        // We pad each sprite reservation with blank space to avoid
        // surprising and unexpected artifacts when the texture is
        // interpolated on to the render surface.
        let reserve_width = reserve_width + padding.unwrap_or(0) as i32 + PADDING * 2;
        let reserve_height = reserve_height + padding.unwrap_or(0) as i32 + PADDING * 2;

        if let Some(allocation) = self
            .allocator
            .allocate(AtlasSize::new(reserve_width, reserve_height))
        {
            let left = allocation.rectangle.min.x;
            let top = allocation.rectangle.min.y;
            let rect = Rect::new(
                Point::new((left + PADDING) as isize, (top + PADDING) as isize),
                Size::new(width as isize, height as isize),
            );

            self.texture.write(rect, im);

            Ok(Sprite {
                texture: Rc::clone(&self.texture),
                coords: rect,
            })
        } else {
            // It's not possible to satisfy that request
            let size = (reserve_width.max(reserve_height) as usize).next_power_of_two();
            Err(OutOfTextureSpace {
                size: Some((self.side * 2).max(size)),
            })
        }
    }

    pub fn size(&self) -> usize {
        self.side
    }

    /// Zero out the texture, and forget all allocated regions
    pub fn clear(&mut self) {
        let iside = self.side as isize;
        let image = crate::Image::new(self.side, self.side);
        let rect = Rect::new(Point::new(0, 0), Size::new(iside, iside));
        self.texture.write(rect, &image);
        self.allocator.clear();
    }
}

pub struct Sprite<T>
where
    T: Texture2d,
{
    pub texture: Rc<T>,
    pub coords: Rect,
}

impl<T: Texture2d> std::fmt::Debug for Sprite<T> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("Sprite")
            .field("coords", &self.coords)
            .field("texture_width", &self.texture.width())
            .field("texture_height", &self.texture.height())
            .finish()
    }
}

impl<T> Clone for Sprite<T>
where
    T: Texture2d,
{
    fn clone(&self) -> Self {
        Self {
            texture: Rc::clone(&self.texture),
            coords: self.coords,
        }
    }
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
#[derive(Debug)]
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
                let middle = self.cell_width * (self.num_cells - (PADDING as usize) * 2);
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
