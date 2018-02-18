//! Keeping track of sprite textures

use failure::Error;
use glium::{self, Rect};
use glium::backend::Facade;
use glium::texture::{SrgbTexture2d, Texture2dDataSource};
use std::rc::Rc;

/// Textures are bitmaps of srgba data that are sized as a power of 2.
/// We allocate sprites out of the available space, starting from the
/// bottom left corner and working to the right until we run out of
/// space, then we move up to the logical row above.  Since sprites can
/// have varying height the height of the rows can also vary.
#[derive(Debug)]
pub struct Texture {
    texture: Rc<SrgbTexture2d>,

    // Dimensions of the texture
    width: u32,
    height: u32,

    /// The bottom of the available space.
    bottom: u32,

    /// The height of the tallest sprite allocated on the current row
    tallest: u32,

    /// How far along the current row we've progressed
    left: u32,
}

impl Texture {
    fn new<F: Facade>(facade: &F, width: u32, height: u32) -> Result<Self, Error> {
        let texture = Rc::new(SrgbTexture2d::empty_with_format(
            facade,
            glium::texture::SrgbFormat::U8U8U8U8,
            glium::texture::MipmapsOption::NoMipmap,
            width,
            height,
        )?);
        Ok(Self {
            texture,
            width,
            height,
            bottom: 0,
            tallest: 0,
            left: 0,
        })
    }

    /// Reserve space for a sprite of the given size
    fn reserve<'a, T: Texture2dDataSource<'a>>(
        &mut self,
        width: u32,
        height: u32,
        data: T,
    ) -> Result<Rect, T> {
        if width > self.width || height > self.height {
            // It's not possible to satisfy that request
            return Err(data);
        }
        let x_left = self.width - self.left;
        if x_left < width {
            // Bump up to next row
            self.bottom += self.tallest;
            self.left = 0;
            self.tallest = 0;
        }

        // Do we have vertical space?
        let y_left = self.height - self.bottom;
        if y_left < height {
            // No room at the inn.
            return Err(data);
        }

        let rect = Rect {
            left: self.left,
            bottom: self.bottom,
            width,
            height,
        };

        self.texture.write(rect, data);

        self.left += width;
        self.tallest = self.tallest.max(height);

        Ok(rect)
    }
}

#[derive(Debug)]
pub struct Sprite {
    pub texture: Rc<SrgbTexture2d>,
    pub coords: Rect,
}

impl Sprite {
    pub fn top_left(&self) -> (f32, f32) {
        (
            self.coords.left as f32 / self.texture.width() as f32,
            self.coords.bottom as f32 / self.texture.height() as f32,
        )
    }

    pub fn bottom_left(&self) -> (f32, f32) {
        (
            self.coords.left as f32 / self.texture.width() as f32,
            (self.coords.bottom + self.coords.height) as f32 / self.texture.height() as f32,
        )
    }

    pub fn bottom_right(&self) -> (f32, f32) {
        (
            (self.coords.left + self.coords.width) as f32 / self.texture.width() as f32,
            (self.coords.bottom + self.coords.height) as f32 / self.texture.height() as f32,
        )
    }

    pub fn top_right(&self) -> (f32, f32) {
        (
            (self.coords.left + self.coords.width) as f32 / self.texture.width() as f32,
            self.coords.bottom as f32 / self.texture.height() as f32,
        )
    }
}

#[derive(Debug, Default)]
pub struct Atlas {
    textures: Vec<Texture>,
}

const TEX_SIZE: u32 = 2048;

impl Atlas {
    pub fn new<'a, F: Facade>(facade: &F) -> Result<Self, Error> {
        let tex = Texture::new(facade, TEX_SIZE, TEX_SIZE)?;
        Ok(Self { textures: vec![tex] })
    }

    // TODO: this is gross, need to tweak this API
    pub fn texture(&self) -> Rc<SrgbTexture2d> {
        Rc::clone(&self.textures[0].texture)
    }

    pub fn allocate<'a, F: Facade, T: Texture2dDataSource<'a>>(
        &mut self,
        facade: &F,
        width: u32,
        height: u32,
        mut data: T,
    ) -> Result<Sprite, Error> {
        for tex in self.textures.iter_mut() {
            match tex.reserve(width, height, data) {
                Ok(rect) => {
                    return Ok(Sprite {
                        texture: Rc::clone(&tex.texture),
                        coords: rect,
                    });
                }
                Err(dat) => data = dat,
            }
        }

        // Round up to a reasonable size
        let size = width.max(height).max(TEX_SIZE).next_power_of_two();

        let mut tex = Texture::new(facade, size, size)?;
        let rect = match tex.reserve(width, height, data) {
            Ok(rect) => rect,
            _ => unreachable!("impossible for Texture::reserve to fail on a fresh instance"),
        };

        let sprite = Sprite {
            texture: Rc::clone(&tex.texture),
            coords: rect,
        };

        self.textures.push(tex);

        Ok(sprite)
    }
}
