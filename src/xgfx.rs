use resize;
use std::result;
use xcb;
use xcb_util;

use failure::{self, Error};
pub type Result<T> = result::Result<T, Error>;

/// The X protocol allows referencing a number of drawable
/// objects.  This trait marks those objects here in code.
pub trait Drawable {
    fn as_drawable(&self) -> xcb::xproto::Drawable;
    fn get_conn(&self) -> &xcb::Connection;
}

/// A Window!
pub struct Window<'a> {
    window_id: xcb::xproto::Window,
    conn: &'a xcb::Connection,
}

impl<'a> Drawable for Window<'a> {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.window_id
    }

    fn get_conn(&self) -> &xcb::Connection {
        self.conn
    }
}

impl<'a> Window<'a> {
    /// Create a new window on the specified screen with the specified dimensions
    pub fn new(conn: &xcb::Connection, screen_num: i32, width: u16, height: u16) -> Result<Window> {
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).ok_or(
            failure::err_msg(
                "no screen?",
            ),
        )?;
        let window_id = conn.generate_id();
        xcb::create_window_checked(
            &conn,
            xcb::COPY_FROM_PARENT as u8,
            window_id,
            screen.root(),
            // x, y
            0,
            0,
            // width, height
            width,
            height,
            // border width
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &[
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_KEY_PRESS | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
            ],
        ).request_check()?;
        Ok(Window { conn, window_id })
    }

    /// Change the title for the window manager
    pub fn set_title(&self, title: &str) {
        xcb_util::icccm::set_wm_name(&self.conn, self.window_id, title);
    }

    /// Display the window
    pub fn show(&self) {
        xcb::map_window(self.conn, self.window_id);
    }
}

impl<'a> Drop for Window<'a> {
    fn drop(&mut self) {
        xcb::destroy_window(self.conn, self.window_id);
    }
}

pub struct Pixmap<'a> {
    pixmap_id: xcb::xproto::Pixmap,
    conn: &'a xcb::Connection,
}

impl<'a> Drawable for Pixmap<'a> {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.pixmap_id
    }

    fn get_conn(&self) -> &xcb::Connection {
        self.conn
    }
}

impl<'a> Pixmap<'a> {
    pub fn new(drawable: &Drawable, depth: u8, width: u16, height: u16) -> Result<Pixmap> {
        let conn = drawable.get_conn();
        let pixmap_id = conn.generate_id();
        xcb::create_pixmap(
            &conn,
            depth,
            pixmap_id,
            drawable.as_drawable(),
            width,
            height,
        ).request_check()?;
        Ok(Pixmap { conn, pixmap_id })
    }
}

impl<'a> Drop for Pixmap<'a> {
    fn drop(&mut self) {
        xcb::free_pixmap(self.conn, self.pixmap_id);
    }
}

pub struct Context<'a> {
    gc_id: xcb::xproto::Gcontext,
    conn: &'a xcb::Connection,
    drawable: xcb::xproto::Drawable,
}

impl<'a> Context<'a> {
    pub fn new(conn: &'a xcb::Connection, d: &Drawable) -> Context<'a> {
        let gc_id = conn.generate_id();
        let drawable = d.as_drawable();
        xcb::create_gc(&conn, gc_id, drawable, &[]);
        Context {
            gc_id,
            conn,
            drawable,
        }
    }

    /// Copy an area from one drawable to another using the settings
    /// defined in this context.
    pub fn copy_area(
        &self,
        src: &Drawable,
        src_x: i16,
        src_y: i16,
        dest: &Drawable,
        dest_x: i16,
        dest_y: i16,
        width: u16,
        height: u16,
    ) -> xcb::VoidCookie {
        xcb::copy_area(
            self.conn,
            src.as_drawable(),
            dest.as_drawable(),
            self.gc_id,
            src_x,
            src_y,
            dest_x,
            dest_y,
            width,
            height,
        )
    }

    /// Send image bytes and render them into the drawable that was used to
    /// create this context.
    pub fn put_image(&self, dest_x: i16, dest_y: i16, im: &Image) -> xcb::VoidCookie {
        debug!(
            "put_image @{},{} x {},{}",
            dest_x,
            dest_y,
            im.width,
            im.height
        );
        xcb::put_image(
            self.conn,
            xcb::xproto::IMAGE_FORMAT_Z_PIXMAP as u8,
            self.drawable,
            self.gc_id,
            im.width as u16,
            im.height as u16,
            dest_x,
            dest_y,
            0,
            24,
            &im.data,
        )
    }
}

impl<'a> Drop for Context<'a> {
    fn drop(&mut self) {
        xcb::free_gc(self.conn, self.gc_id);
    }
}

/// A color stored as big endian bgra32
#[derive(Copy, Clone)]
pub struct Color(u32);

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
                let (src_r, src_g, src_b, src_a) = self.as_rgba();
                let (dst_r, dst_g, dst_b, _dst_a) = dest.as_rgba();

                // Alpha blending per https://stackoverflow.com/a/12016968/149111
                let inv_alpha = 256u16 - src_a as u16;
                let alpha = src_a as u16 + 1;

                Color::rgb(
                    ((alpha * src_r as u16 + inv_alpha * dst_r as u16) >> 8) as u8,
                    ((alpha * src_g as u16 + inv_alpha * dst_g as u16) >> 8) as u8,
                    ((alpha * src_b as u16 + inv_alpha * dst_b as u16) >> 8) as u8,
                )
            }

            &Operator::Dest => dest,

            &Operator::Source => *self,

            &Operator::Multiply => {
                let (src_r, src_g, src_b, src_a) = self.as_rgba();
                let (dst_r, dst_g, dst_b, _dst_a) = dest.as_rgba();
                let r = ((src_r as u16 * dst_r as u16) >> 8) as u8;
                let g = ((src_g as u16 * dst_g as u16) >> 8) as u8;
                let b = ((src_b as u16 * dst_b as u16) >> 8) as u8;

                Color::rgba(r, g, b, src_a)
            }

            &Operator::MultiplyThenOver(ref tint) => {
                self.composite(*tint, &Operator::Multiply).composite(
                    dest,
                    &Operator::Over,
                )
            }
        }
    }
}

/// Compositing operator.
/// We implement a small subset of possible compositing operators.
/// More information on these and their temrinology can be found
/// in the Cairo documentation here:
/// https://www.cairographics.org/operators/
pub enum Operator {
    /// Apply the alpha channel of src and combine src with dest,
    /// according to the classic OVER composite operator
    Over,
    /// Ignore src; leave dest untouched
    Dest,
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

/// A bitmap in big endian bgra32 color format
pub struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
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
        ).resize(&self.data, &mut dest.data);
        dest
    }

    pub fn scale_by(&self, scale: f64) -> Image {
        let width = (self.width as f64 * scale) as usize;
        let height = (self.height as f64 * scale) as usize;
        self.resize(width, height)
    }

    #[inline]
    /// Obtain a mutable reference to the raw bgra pixel at the specified coordinates
    pub fn pixel_mut(&mut self, x: usize, y: usize) -> &mut u32 {
        assert!(x < self.width && y < self.height);
        unsafe {
            let offset = (y * self.width * 4) + (x * 4);
            &mut *(self.data.as_mut_ptr().offset(offset as isize) as *mut u32)
        }
    }

    #[inline]
    /// Read the raw bgra pixel at the specified coordinates
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        assert!(x < self.width && y < self.height);
        unsafe {
            let offset = (y * self.width * 4) + (x * 4);
            *(self.data.as_ptr().offset(offset as isize) as *const u32)
        }
    }

    /// Clear the entire image to the specific color
    pub fn clear(&mut self, color: Color) {
        let width = self.width;
        let height = self.height;
        self.clear_rect(0, 0, width, height, color);
    }

    pub fn clear_rect(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        height: usize,
        color: Color,
    ) {
        for y in 0..height {
            let dest_y = y as isize + dest_y;
            if dest_y < 0 {
                continue;
            }
            if dest_y as usize >= self.height {
                break;
            }
            for x in 0..width {
                let dest_x = x as isize + dest_x;
                if dest_x < 0 {
                    continue;
                }
                if dest_x as usize >= self.width {
                    break;
                }

                *self.pixel_mut(dest_x as usize, dest_y as usize) = color.0;
            }
        }
    }

    pub fn draw_image(&mut self, dest_x: isize, dest_y: isize, im: &Image, operator: Operator) {
        self.draw_image_subset(dest_x, dest_y, 0, 0, im.width, im.height, im, operator)
    }

    pub fn draw_image_subset(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        im: &Image,
        operator: Operator,
    ) {
        assert!(width <= im.width && height <= im.height);
        assert!(src_x < im.width && src_y < im.height);
        for y in src_y..src_y + height {
            let dest_y = y as isize + dest_y - src_y as isize;
            if dest_y < 0 {
                continue;
            }
            if dest_y as usize >= self.height {
                break;
            }
            for x in src_x..src_x + width {
                let dest_x = x as isize + dest_x - src_x as isize;
                if dest_x < 0 {
                    continue;
                }
                if dest_x as usize >= self.width {
                    break;
                }

                let src = Color(im.pixel(x, y));
                let dst = self.pixel_mut(dest_x as usize, dest_y as usize);
                *dst = src.composite(Color(*dst), &operator).0;
            }
        }
    }
}
