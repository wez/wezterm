use crate::color::Color;
use crate::{Operator, Point, Rect, Size};
#[cfg(feature = "opengl")]
use glium::texture::SrgbTexture2d;
use palette::LinSrgba;
use std::cell::RefCell;

pub mod atlas;

pub struct TextureUnit;
pub type TextureCoord = euclid::Point2D<f32, TextureUnit>;
pub type TextureRect = euclid::Rect<f32, TextureUnit>;
pub type TextureSize = euclid::Size2D<f32, TextureUnit>;

/// Represents a big endian bgra32 bitmap that may not be present
/// in local RAM, but may be addressable in eg: video RAM
pub trait Texture2d {
    /// Copy the bits from the source bitmap to the texture at the location
    /// specified by the rectangle.
    /// The dimensions of the rectangle must match the source image
    fn write(&self, rect: Rect, im: &dyn BitmapImage);

    /// Copy the bits from the texture at the location specified by the rectangle
    /// into the bitmap image.
    /// The dimensions of the rectangle must match the source image
    fn read(&self, rect: Rect, im: &mut dyn BitmapImage);

    /// Returns the width of the texture in pixels
    fn width(&self) -> usize;

    /// Returns the height of the texture in pixels
    fn height(&self) -> usize;

    /// Converts a rect in pixel coordinates to texture coordinates
    fn to_texture_coords(&self, coords: Rect) -> TextureRect {
        let coords = coords.to_f32();
        let width = self.width() as f32;
        let height = self.height() as f32;
        TextureRect::new(
            TextureCoord::new(coords.min_x() / width, coords.min_y() / height),
            TextureSize::new(coords.size.width / width, coords.size.height / height),
        )
    }
}

#[cfg(feature = "opengl")]
impl Texture2d for SrgbTexture2d {
    fn write(&self, rect: Rect, im: &dyn BitmapImage) {
        let (im_width, im_height) = im.image_dimensions();
        let source = glium::texture::RawImage2d {
            data: std::borrow::Cow::Borrowed(im.pixels()),
            width: im_width as u32,
            height: im_height as u32,
            format: glium::texture::ClientFormat::U8U8U8U8,
        };

        SrgbTexture2d::write(
            self,
            glium::Rect {
                left: rect.min_x() as u32,
                bottom: rect.min_y() as u32,
                width: rect.size.width as u32,
                height: rect.size.height as u32,
            },
            source,
        )
    }

    fn read(&self, _rect: Rect, _im: &mut dyn BitmapImage) {
        unimplemented!();
    }

    fn width(&self) -> usize {
        SrgbTexture2d::width(self) as usize
    }

    fn height(&self) -> usize {
        SrgbTexture2d::height(self) as usize
    }
}

#[cfg(target_arch = "x86_64")]
mod avx {
    use super::*;
    #[inline]
    fn align_lo(size: usize, align: usize) -> usize {
        size & !(align - 1)
    }

    #[allow(dead_code)]
    #[inline]
    fn is_aligned(size: usize, align: usize) -> bool {
        size == align_lo(size, align)
    }

    #[allow(clippy::cast_ptr_alignment)]
    pub unsafe fn fill_pixel(
        mut dest: *mut u8,
        stride_bytes: usize,
        width_pixels: usize,
        height_pixels: usize,
        color: Color,
    ) {
        // This holds 8 copies of the pixel value
        let bgra256 = std::arch::x86_64::_mm256_set1_epi32(color.0 as _);
        let aligned_width = align_lo(width_pixels, 8);

        if is_aligned(dest as usize, 32) && is_aligned(stride_bytes, 32) {
            for _row in 0..height_pixels {
                for col in (0..aligned_width).step_by(8) {
                    std::arch::x86_64::_mm256_store_si256(dest.add(4 * col) as *mut _, bgra256);
                }
                if width_pixels != aligned_width {
                    std::arch::x86_64::_mm256_storeu_si256(
                        dest.add(4 * (width_pixels - 8)) as *mut _,
                        bgra256,
                    );
                }
                dest = dest.add(stride_bytes);
            }
        } else {
            for _row in 0..height_pixels {
                for col in (0..aligned_width).step_by(8) {
                    std::arch::x86_64::_mm256_storeu_si256(dest.add(4 * col) as *mut _, bgra256);
                }
                if width_pixels != aligned_width {
                    std::arch::x86_64::_mm256_storeu_si256(
                        dest.add(4 * (width_pixels - 8)) as *mut _,
                        bgra256,
                    );
                }
                dest = dest.add(stride_bytes);
            }
        }
    }
}

/// A bitmap in big endian bgra32 color format with abstract
/// storage filled in by the trait implementation.
pub trait BitmapImage {
    /// Obtain a read only pointer to the pixel data
    unsafe fn pixel_data(&self) -> *const u8;

    /// Obtain a mutable pointer to the pixel data
    unsafe fn pixel_data_mut(&mut self) -> *mut u8;

    /// Return the pair (width, height) of the image, measured in pixels
    fn image_dimensions(&self) -> (usize, usize);

    #[inline]
    fn pixels(&self) -> &[u32] {
        let (width, height) = self.image_dimensions();
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let first = self.pixel_data() as *const u32;
            std::slice::from_raw_parts(first, width * height)
        }
    }

    #[inline]
    fn pixels_mut(&mut self) -> &mut [u32] {
        let (width, height) = self.image_dimensions();
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let first = self.pixel_data_mut() as *mut u32;
            std::slice::from_raw_parts_mut(first, width * height)
        }
    }

    #[inline]
    /// Obtain a mutable reference to the raw bgra pixel at the specified coordinates
    fn pixel_mut(&mut self, x: usize, y: usize) -> &mut u32 {
        let (width, height) = self.image_dimensions();
        debug_assert!(x < width && y < height);
        unsafe {
            let offset = (y * width * 4) + (x * 4);
            #[allow(clippy::cast_ptr_alignment)]
            &mut *(self.pixel_data_mut().add(offset) as *mut u32)
        }
    }

    #[inline]
    /// Read the raw bgra pixel at the specified coordinates
    fn pixel(&self, x: usize, y: usize) -> &u32 {
        let (width, height) = self.image_dimensions();
        debug_assert!(x < width && y < height);
        unsafe {
            let offset = (y * width * 4) + (x * 4);
            #[allow(clippy::cast_ptr_alignment)]
            &*(self.pixel_data().add(offset) as *const u32)
        }
    }

    #[inline]
    fn horizontal_pixel_range(&self, x1: usize, x2: usize, y: usize) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.pixel(x1, y), x2 - x1) }
    }

    #[inline]
    fn horizontal_pixel_range_mut(&mut self, x1: usize, x2: usize, y: usize) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.pixel_mut(x1, y), x2 - x1) }
    }

    /// Clear the entire image to the specific color
    fn clear(&mut self, color: Color) {
        let (width, height) = self.image_dimensions();

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx") && width >= 8 {
                unsafe {
                    avx::fill_pixel(self.pixel_data_mut(), width * 4, width, height, color);
                }
                return;
            }
        }

        for c in self.pixels_mut() {
            *c = color.0;
        }
    }

    fn clear_rect(&mut self, rect: Rect, color: Color) {
        let (dim_width, dim_height) = self.image_dimensions();
        let max_x = (rect.origin.x + rect.size.width as isize).min(dim_width as isize) as usize;
        let max_y = (rect.origin.y + rect.size.height as isize).min(dim_height as isize) as usize;

        let dest_x = rect.origin.x.max(0) as usize;
        let dest_y = rect.origin.y.max(0) as usize;

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx") && (max_x - dest_x) > 8 {
                unsafe {
                    avx::fill_pixel(
                        self.pixel_data_mut()
                            .add(4 * ((dest_y * dim_width) + dest_x)),
                        dim_width * 4,
                        max_x - dest_x,
                        max_y - dest_y,
                        color,
                    );
                }
                return;
            }
        }

        for y in dest_y..max_y {
            let range = self.horizontal_pixel_range_mut(dest_x, max_x, y);
            for c in range {
                *c = color.0;
            }
        }
    }

    /// Draw a line starting at `start` and ending at `end`.
    /// The line will be anti-aliased and applied to the surface using the
    /// specified Operator.
    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        let (dim_width, dim_height) = self.image_dimensions();
        let linear: LinSrgba = color.into();
        let (red, green, blue, alpha) = linear.into_components();

        for ((x, y), value) in line_drawing::XiaolinWu::<f32, isize>::new(
            (start.x as f32, start.y as f32),
            (end.x as f32, end.y as f32),
        ) {
            if y < 0 || x < 0 {
                continue;
            }
            if y >= dim_height as isize || x >= dim_width as isize {
                continue;
            }
            let pix = self.pixel_mut(x as usize, y as usize);

            let color: Color = LinSrgba::from_components((red, green, blue, alpha * value)).into();
            *pix = color.composite(Color(*pix), operator).0;
        }
    }

    /// Draw a 1-pixel wide rectangle
    fn draw_rect(&mut self, rect: Rect, color: Color, operator: Operator) {
        let bottom_right = rect.origin.add_size(&rect.size);

        // Draw the vertical lines down either side
        self.draw_line(
            rect.origin,
            Point::new(rect.origin.x, bottom_right.y),
            color,
            operator,
        );
        self.draw_line(
            Point::new(bottom_right.x, rect.origin.y),
            bottom_right,
            color,
            operator,
        );
        // And the horizontals for the top and bottom
        self.draw_line(
            rect.origin,
            Point::new(bottom_right.x, rect.origin.y),
            color,
            operator,
        );
        self.draw_line(
            Point::new(rect.origin.x, bottom_right.y),
            bottom_right,
            color,
            operator,
        );
    }

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        let (im_width, im_height) = im.image_dimensions();
        let src_rect = src_rect
            .unwrap_or_else(|| Rect::from_size(Size::new(im_width as isize, im_height as isize)));

        let (_dim_width, dim_height) = self.image_dimensions();
        debug_assert!(
            src_rect.size.width <= im_width as isize && src_rect.size.height <= im_height as isize
        );
        for y in src_rect.origin.y..src_rect.origin.y + src_rect.size.height {
            let dest_y = y as isize + dest_top_left.y - src_rect.origin.y as isize;
            if dest_y < 0 {
                continue;
            }
            if dest_y as usize >= dim_height {
                break;
            }

            let src_pixels = im.horizontal_pixel_range(
                src_rect.min_x() as usize,
                src_rect.max_x() as usize,
                y as usize,
            );
            let dest_pixels = self.horizontal_pixel_range_mut(
                dest_top_left.x.max(0) as usize,
                (dest_top_left.x + src_rect.size.width).max(0) as usize,
                dest_y as usize,
            );
            for (src_pix, dest_pix) in src_pixels.iter().zip(dest_pixels.iter_mut()) {
                *dest_pix = Color(*src_pix).composite(Color(*dest_pix), operator).0;
            }
        }
    }
}

/// A bitmap in big endian bgra32 color format, with storage
/// in a Vec<u8>.
pub struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
}

impl Into<Vec<u8>> for Image {
    fn into(self) -> Vec<u8> {
        self.data
    }
}

impl Image {
    /// Create a new bgra32 image buffer with the specified dimensions.
    /// The buffer is initialized to all zeroes.
    pub fn new(width: usize, height: usize) -> Image {
        let size = height * width * 4;
        let mut data = vec![0; size];
        data.resize(size, 0);
        Image {
            data,
            width,
            height,
        }
    }

    /// Create a new bgra32 image buffer with the specified dimensions.
    /// The buffer is populated with the source data in bgr24 format.
    pub fn with_bgr24(width: usize, height: usize, stride: usize, data: &[u8]) -> Image {
        let mut image = Image::new(width, height);
        for y in 0..height {
            let src_offset = y * stride;
            let dest_offset = y * width * 4;
            #[allow(clippy::identity_op)]
            for x in 0..width {
                let blue = data[src_offset + (x * 3) + 0];
                let green = data[src_offset + (x * 3) + 1];
                let red = data[src_offset + (x * 3) + 2];
                let alpha = red | green | blue;
                image.data[dest_offset + (x * 4) + 0] = blue;
                image.data[dest_offset + (x * 4) + 1] = green;
                image.data[dest_offset + (x * 4) + 2] = red;
                image.data[dest_offset + (x * 4) + 3] = alpha;
            }
        }
        image
    }

    /// Create a new bgra32 image buffer with the specified dimensions.
    /// The buffer is populated with the source data in argb32 format.
    pub fn with_bgra32(width: usize, height: usize, stride: usize, data: &[u8]) -> Image {
        let mut image = Image::new(width, height);
        for y in 0..height {
            let src_offset = y * stride;
            let dest_offset = y * width * 4;
            #[allow(clippy::identity_op)]
            for x in 0..width {
                let blue = data[src_offset + (x * 4) + 0];
                let green = data[src_offset + (x * 4) + 1];
                let red = data[src_offset + (x * 4) + 2];
                let alpha = data[src_offset + (x * 4) + 3];
                image.data[dest_offset + (x * 4) + 0] = blue;
                image.data[dest_offset + (x * 4) + 1] = green;
                image.data[dest_offset + (x * 4) + 2] = red;
                image.data[dest_offset + (x * 4) + 3] = alpha;
            }
        }
        image
    }

    /// Create a new bgra32 image buffer with the specified dimensions.
    /// The buffer is populated with the source data in rgba32 format.
    pub fn with_rgba32(width: usize, height: usize, stride: usize, data: &[u8]) -> Image {
        let mut image = Image::new(width, height);
        for y in 0..height {
            let src_offset = y * stride;
            let dest_offset = y * width * 4;
            #[allow(clippy::identity_op)]
            for x in 0..width {
                let red = data[src_offset + (x * 4) + 0];
                let green = data[src_offset + (x * 4) + 1];
                let blue = data[src_offset + (x * 4) + 2];
                let alpha = data[src_offset + (x * 4) + 3];
                image.data[dest_offset + (x * 4) + 0] = blue;
                image.data[dest_offset + (x * 4) + 1] = green;
                image.data[dest_offset + (x * 4) + 2] = red;
                image.data[dest_offset + (x * 4) + 3] = alpha;
            }
        }
        image
    }

    pub fn with_8bpp(width: usize, height: usize, stride: usize, data: &[u8]) -> Image {
        let mut image = Image::new(width, height);
        for y in 0..height {
            let src_offset = y * stride;
            let dest_offset = y * width * 4;
            #[allow(clippy::identity_op)]
            for x in 0..width {
                let gray = data[src_offset + x];
                image.data[dest_offset + (x * 4) + 0] = gray;
                image.data[dest_offset + (x * 4) + 1] = gray;
                image.data[dest_offset + (x * 4) + 2] = gray;
                image.data[dest_offset + (x * 4) + 3] = gray;
            }
        }
        image
    }

    /// Creates a new image with the contents of the current image, but
    /// resized to the specified dimensions.
    pub fn resize(&self, width: usize, height: usize) -> Image {
        let mut dest = Image::new(width, height);
        let algo = if (width * height) < (self.width * self.height) {
            resize::Type::Lanczos3
        } else {
            resize::Type::Mitchell
        };
        resize::new(
            self.width,
            self.height,
            width,
            height,
            resize::Pixel::RGBA,
            algo,
        )
        .resize(&self.data, &mut dest.data);
        dest
    }

    pub fn scale_by(&self, scale: f64) -> Image {
        let width = (self.width as f64 * scale) as usize;
        let height = (self.height as f64 * scale) as usize;
        self.resize(width, height)
    }
}

impl BitmapImage for Image {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.data.as_ptr()
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    fn image_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

pub struct ImageTexture {
    pub image: RefCell<Image>,
}

impl ImageTexture {
    pub fn new(width: usize, height: usize) -> Self {
        let im = Image::new(width, height);
        Self {
            image: RefCell::new(im),
        }
    }
}

impl Texture2d for ImageTexture {
    fn write(&self, rect: Rect, im: &dyn BitmapImage) {
        let mut image = self.image.borrow_mut();
        image.draw_image(rect.origin, None, im, Operator::Source);
    }

    fn read(&self, _rect: Rect, _im: &mut dyn BitmapImage) {
        unimplemented!();
    }

    /// Returns the width of the texture in pixels
    fn width(&self) -> usize {
        let (width, _height) = self.image.borrow().image_dimensions();
        width
    }

    /// Returns the height of the texture in pixels
    fn height(&self) -> usize {
        let (_width, height) = self.image.borrow().image_dimensions();
        height
    }
}
