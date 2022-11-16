use crate::bitmaps::{BitmapImage, Texture2d, TextureRect};
use crate::{Point, Rect, Size};
use anyhow::{ensure, Result as Fallible};
use guillotiere::{SimpleAtlasAllocator, Size as AtlasSize};
use std::convert::TryInto;
use std::rc::Rc;
use thiserror::*;

const PADDING: i32 = 1;

#[derive(Debug, Error)]
#[error("Texture Size exceeded, need {:?}", size)]
pub struct OutOfTextureSpace {
    pub size: Option<usize>,
    pub current_size: usize,
}

/// Atlases are bitmaps of srgba data that are sized as a power of 2.
/// We allocate sprites out of the available space, using AtlasAllocator
/// to manage the available rectangles.
pub struct Atlas {
    texture: Rc<dyn Texture2d>,

    allocator: SimpleAtlasAllocator,

    /// Dimensions of the texture
    side: usize,
}

impl Atlas {
    pub fn new(texture: &Rc<dyn Texture2d>) -> Fallible<Self> {
        ensure!(
            texture.width() == texture.height(),
            "texture must be square!"
        );
        let side = texture.width();
        let iside = side as isize;

        let image = crate::Image::new(side, side);
        let rect = Rect::new(Point::new(0, 0), Size::new(iside, iside));
        texture.write(rect, &image);

        let allocator =
            SimpleAtlasAllocator::new(AtlasSize::new(side.try_into()?, side.try_into()?));
        Ok(Self {
            texture: Rc::clone(texture),
            side,
            allocator,
        })
    }

    #[inline]
    pub fn texture(&self) -> Rc<dyn Texture2d> {
        Rc::clone(&self.texture)
    }

    /// Reserve space for a sprite of the given size
    pub fn allocate(&mut self, im: &dyn BitmapImage) -> Result<Sprite, OutOfTextureSpace> {
        self.allocate_with_padding(im, None)
    }

    pub fn allocate_with_padding(
        &mut self,
        im: &dyn BitmapImage,
        padding: Option<usize>,
    ) -> Result<Sprite, OutOfTextureSpace> {
        let (width, height) = im.image_dimensions();

        // If we can't convert the sizes to i32, then we'll never
        // be able to store this image
        let reserve_width: i32 = width.try_into().map_err(|_| OutOfTextureSpace {
            size: None,
            current_size: self.side,
        })?;
        let reserve_height: i32 = height.try_into().map_err(|_| OutOfTextureSpace {
            size: None,
            current_size: self.side,
        })?;

        // We pad each sprite reservation with blank space to avoid
        // surprising and unexpected artifacts when the texture is
        // interpolated on to the render surface.
        let reserve_width = reserve_width + padding.unwrap_or(0) as i32 + PADDING * 2;
        let reserve_height = reserve_height + padding.unwrap_or(0) as i32 + PADDING * 2;

        let start = std::time::Instant::now();
        let res = if let Some(allocation) = self
            .allocator
            .allocate(AtlasSize::new(reserve_width, reserve_height))
        {
            let left = allocation.min.x;
            let top = allocation.min.y;
            let rect = Rect::new(
                Point::new((left + PADDING) as isize, (top + PADDING) as isize),
                Size::new(width as isize, height as isize),
            );

            self.texture.write(rect, im);

            metrics::histogram!("window.atlas.allocate.success.rate", 1.);
            Ok(Sprite {
                texture: Rc::clone(&self.texture),
                coords: rect,
            })
        } else {
            // It's not possible to satisfy that request
            let size = (reserve_width.max(reserve_height) as usize).next_power_of_two();
            metrics::histogram!("window.atlas.allocate.failure.rate", 1.);
            Err(OutOfTextureSpace {
                size: Some((self.side * 2).max(size)),
                current_size: self.side,
            })
        };
        metrics::histogram!("window.atlas.allocate.latency", start.elapsed());

        res
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

pub struct Sprite {
    pub texture: Rc<dyn Texture2d>,
    pub coords: Rect,
}

impl std::fmt::Debug for Sprite {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("Sprite")
            .field("coords", &self.coords)
            .field("texture_width", &self.texture.width())
            .field("texture_height", &self.texture.height())
            .finish()
    }
}

impl Clone for Sprite {
    fn clone(&self) -> Self {
        Self {
            texture: Rc::clone(&self.texture),
            coords: self.coords,
        }
    }
}

impl Sprite {
    /// Returns the texture coordinates of the sprite
    pub fn texture_coords(&self) -> TextureRect {
        self.texture.to_texture_coords(self.coords)
    }
}
