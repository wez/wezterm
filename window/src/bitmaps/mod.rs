use palette::{Blend, Srgb, Srgba};

/// A color stored as big endian bgra32
#[derive(Copy, Clone, Debug)]
pub struct Color(u32);

impl From<Srgb> for Color {
    #[inline]
    fn from(s: Srgb) -> Color {
        let b: Srgb<u8> = s.into_format();
        let b = b.into_components();
        Color::rgb(b.0, b.1, b.2)
    }
}

impl From<Srgba> for Color {
    #[inline]
    fn from(s: Srgba) -> Color {
        let b: Srgba<u8> = s.into_format();
        let b = b.into_components();
        Color::rgba(b.0, b.1, b.2, b.3)
    }
}

impl From<Color> for Srgb {
    #[inline]
    fn from(c: Color) -> Srgb {
        let c = c.as_rgba();
        let s = Srgb::<u8>::new(c.0, c.1, c.2);
        s.into_format()
    }
}

impl From<Color> for Srgba {
    #[inline]
    fn from(c: Color) -> Srgba {
        let c = c.as_rgba();
        let s = Srgba::<u8>::new(c.0, c.1, c.2, c.3);
        s.into_format()
    }
}

impl Color {
    #[inline]
    pub fn rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::rgba(red, green, blue, 0xff)
    }

    #[inline]
    pub fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        let word = (blue as u32) << 24 | (green as u32) << 16 | (red as u32) << 8 | alpha as u32;
        Color(word.to_be())
    }

    #[inline]
    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let host = u32::from_be(self.0);
        (
            (host >> 8) as u8,
            (host >> 16) as u8,
            (host >> 24) as u8,
            (host & 0xff) as u8,
        )
    }

    /// Compute the composite of two colors according to the supplied operator.
    /// self is the src operand, dest is the dest operand.
    #[inline]
    pub fn composite(&self, dest: Color, operator: &Operator) -> Color {
        match operator {
            &Operator::Over => {
                let src: Srgba = (*self).into();
                let dest: Srgba = dest.into();
                Srgba::from_linear(src.into_linear().over(dest.into_linear())).into()
            }
            &Operator::Source => *self,
            &Operator::Multiply => {
                let src: Srgba = (*self).into();
                let dest: Srgba = dest.into();
                let result: Color =
                    Srgba::from_linear(src.into_linear().multiply(dest.into_linear())).into();
                result.into()
            }
            &Operator::MultiplyThenOver(ref tint) => {
                // First multiply by the tint color.  This colorizes the glyph.
                let src: Srgba = (*self).into();
                let tint: Srgba = (*tint).into();
                let mut tinted = src.into_linear().multiply(tint.into_linear());
                // We take the alpha from the source.  This is important because
                // we're using Multiply to tint the glyph and if we don't reset the
                // alpha we tend to end up with a background square of the tint color.
                tinted.alpha = src.alpha;
                // Then blend the tinted glyph over the destination background
                let dest: Srgba = dest.into();
                Srgba::from_linear(tinted.over(dest.into_linear())).into()
            }
        }
    }
}

/// Compositing operator.
/// We implement a small subset of possible compositing operators.
/// More information on these and their temrinology can be found
/// in the Cairo documentation here:
/// https://www.cairographics.org/operators/
#[derive(Debug, Clone, Copy)]
pub enum Operator {
    /// Apply the alpha channel of src and combine src with dest,
    /// according to the classic OVER composite operator
    Over,
    /// Ignore dest; take src as the result of the operation
    Source,
    /// Multiply src x dest.  The result is at least as dark as
    /// the darker of the two input colors.  This is used to
    /// apply a color tint.
    Multiply,
    /// Multiply src with the provided color, then apply the
    /// Over operator on the result with the dest as the dest.
    /// This is used to colorize the src and then blend the
    /// result into the destination.
    MultiplyThenOver(Color),
}

/// A bitmap in big endian bgra32 color format, with storage
/// in a Vec<u8>.
pub struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
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
            let first = self.pixel_data() as *const u32;
            std::slice::from_raw_parts(first, width * height)
        }
    }

    #[inline]
    fn pixels_mut(&mut self) -> &mut [u32] {
        let (width, height) = self.image_dimensions();
        unsafe {
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
            &mut *(self.pixel_data_mut().offset(offset as isize) as *mut u32)
        }
    }

    #[inline]
    /// Read the raw bgra pixel at the specified coordinates
    fn pixel(&self, x: usize, y: usize) -> &u32 {
        let (width, height) = self.image_dimensions();
        debug_assert!(x < width && y < height);
        unsafe {
            let offset = (y * width * 4) + (x * 4);
            &*(self.pixel_data().offset(offset as isize) as *const u32)
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
        for c in self.pixels_mut() {
            *c = color.0;
        }
    }

    fn clear_rect(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        height: usize,
        color: Color,
    ) {
        let (dim_width, dim_height) = self.image_dimensions();
        let max_x = (dest_x + width as isize).min(dim_width as isize) as usize;
        let max_y = (dest_y + height as isize).min(dim_height as isize) as usize;

        let dest_x = dest_x.max(0) as usize;
        let dest_y = dest_y.max(0) as usize;

        for y in dest_y..max_y {
            let range = self.horizontal_pixel_range_mut(dest_x, max_x, y);
            for c in range {
                *c = color.0;
            }
        }
    }

    fn draw_vertical_line(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        height: usize,
        color: Color,
        operator: Operator,
    ) {
        let (dim_width, dim_height) = self.image_dimensions();
        if dest_x < 0 || dest_x >= dim_width as isize {
            return;
        }
        for y in 0..height {
            let dest_y = y as isize + dest_y;
            if dest_y < 0 {
                continue;
            }
            if dest_y >= dim_height as isize {
                break;
            }
            let pix = self.pixel_mut(dest_x as usize, dest_y as usize);
            *pix = color.composite(Color(*pix), &operator).0;
        }
    }

    fn draw_horizontal_line(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        color: Color,
        operator: Operator,
    ) {
        let (dim_width, dim_height) = self.image_dimensions();
        if dest_y < 0 || dest_y >= dim_height as isize {
            return;
        }
        for x in 0..width {
            let dest_x = x as isize + dest_x;
            if dest_x < 0 {
                continue;
            }
            if dest_x >= dim_width as isize {
                break;
            }
            let pix = self.pixel_mut(dest_x as usize, dest_y as usize);
            *pix = color.composite(Color(*pix), &operator).0;
        }
    }

    /// Draw a 1-pixel wide rectangle
    fn draw_rect(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        height: usize,
        color: Color,
        operator: Operator,
    ) {
        // Draw the vertical lines down either side
        self.draw_vertical_line(dest_x, dest_y, height, color, operator);
        self.draw_vertical_line(dest_x + width as isize, dest_y, height, color, operator);
        // And the horizontals for the top and bottom
        self.draw_horizontal_line(dest_x, dest_y, width, color, operator);
        self.draw_horizontal_line(dest_x, dest_y + height as isize, width, color, operator);
    }

    fn draw_image(&mut self, dest_x: isize, dest_y: isize, im: &BitmapImage, operator: Operator) {
        let (dest_width, dest_height) = im.image_dimensions();
        self.draw_image_subset(dest_x, dest_y, 0, 0, dest_width, dest_height, im, operator)
    }

    fn draw_image_subset(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        im: &BitmapImage,
        operator: Operator,
    ) {
        let (dest_width, dest_height) = im.image_dimensions();
        let (dim_width, dim_height) = self.image_dimensions();
        debug_assert!(width <= dest_width && height <= dest_height);
        for y in src_y..src_y + height {
            let dest_y = y as isize + dest_y - src_y as isize;
            if dest_y < 0 {
                continue;
            }
            if dest_y as usize >= dim_height {
                break;
            }
            for x in src_x..src_x + width {
                let dest_x = x as isize + dest_x - src_x as isize;
                if dest_x < 0 {
                    continue;
                }
                if dest_x as usize >= dim_width {
                    break;
                }
                let src = Color(*im.pixel(x, y));
                let dst = self.pixel_mut(dest_x as usize, dest_y as usize);
                *dst = src.composite(Color(*dst), &operator).0;
            }
        }
    }
}

impl Image {
    /// Create a new bgra32 image buffer with the specified dimensions.
    /// The buffer is initialized to all zeroes.
    pub fn new(width: usize, height: usize) -> Image {
        let size = height * width * 4;
        let mut data = Vec::with_capacity(size);
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

    pub fn with_8bpp(width: usize, height: usize, stride: usize, data: &[u8]) -> Image {
        let mut image = Image::new(width, height);
        for y in 0..height {
            let src_offset = y * stride;
            let dest_offset = y * width * 4;
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
