use libc;
use resize;
use std::convert::From;
use std::io;
use std::ops::Deref;
use std::ptr;
use std::result;
use xcb;
use xcb_util;
use xcb_util::ffi::keysyms::{xcb_key_press_lookup_keysym, xcb_key_symbols_alloc,
                             xcb_key_symbols_free, xcb_key_symbols_t};

use failure::{self, Error};
pub type Result<T> = result::Result<T, Error>;
pub use xkeysyms::*;

use super::term::color::RgbColor;
use palette::Blend;
use palette::pixel::Srgb;

pub struct Connection {
    conn: xcb::Connection,
    screen_num: i32,
    pub atom_protocols: xcb::Atom,
    pub atom_delete: xcb::Atom,
    pub atom_utf8_string: xcb::Atom,
    pub atom_xsel_data: xcb::Atom,
    pub atom_targets: xcb::Atom,
    keysyms: *mut xcb_key_symbols_t,
    shm_available: bool,
}

impl Deref for Connection {
    type Target = xcb::Connection;

    fn deref(&self) -> &xcb::Connection {
        &self.conn
    }
}

impl Connection {
    pub fn new() -> Result<Connection> {
        let (conn, screen_num) = xcb::Connection::connect(None)?;
        let atom_protocols = xcb::intern_atom(&conn, false, "WM_PROTOCOLS")
            .get_reply()?
            .atom();
        let atom_delete = xcb::intern_atom(&conn, false, "WM_DELETE_WINDOW")
            .get_reply()?
            .atom();
        let atom_utf8_string = xcb::intern_atom(&conn, false, "UTF8_STRING")
            .get_reply()?
            .atom();
        let atom_xsel_data = xcb::intern_atom(&conn, false, "XSEL_DATA")
            .get_reply()?
            .atom();
        let atom_targets = xcb::intern_atom(&conn, false, "TARGETS")
            .get_reply()?
            .atom();

        let keysyms = unsafe { xcb_key_symbols_alloc(conn.get_raw_conn()) };

        let reply = xcb::shm::query_version(&conn).get_reply()?;
        let shm_available = reply.shared_pixmaps();

        Ok(Connection {
            conn,
            screen_num,
            atom_protocols,
            atom_delete,
            keysyms,
            shm_available,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
        })
    }

    pub fn conn(&self) -> &xcb::Connection {
        &self.conn
    }

    pub fn screen_num(&self) -> i32 {
        self.screen_num
    }

    pub fn atom_delete(&self) -> xcb::Atom {
        self.atom_delete
    }

    pub fn lookup_keysym(&self, event: &xcb::KeyPressEvent, shifted: bool) -> xcb::Keysym {
        unsafe {
            let sym =
                xcb_key_press_lookup_keysym(self.keysyms, event.ptr, if shifted { 1 } else { 0 });
            if sym == 0 && shifted {
                xcb_key_press_lookup_keysym(self.keysyms, event.ptr, 0)
            } else {
                sym
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            xcb_key_symbols_free(self.keysyms);
        }
    }
}

/// The X protocol allows referencing a number of drawable
/// objects.  This trait marks those objects here in code.
pub trait Drawable {
    fn as_drawable(&self) -> xcb::xproto::Drawable;
    fn get_conn(&self) -> &Connection;
}

/// A Window!
pub struct Window<'a> {
    window_id: xcb::xproto::Window,
    conn: &'a Connection,
}

impl<'a> Drawable for Window<'a> {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.window_id
    }

    fn get_conn(&self) -> &Connection {
        self.conn
    }
}

impl<'a> Window<'a> {
    /// Create a new window on the specified screen with the specified dimensions
    pub fn new(conn: &Connection, width: u16, height: u16) -> Result<Window> {
        let setup = conn.conn().get_setup();
        let screen = setup.roots().nth(conn.screen_num() as usize).ok_or(
            failure::err_msg("no screen?"),
        )?;
        let window_id = conn.conn().generate_id();
        xcb::create_window_checked(
            conn.conn(),
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
                    xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_KEY_PRESS |
                        xcb::EVENT_MASK_BUTTON_PRESS |
                        xcb::EVENT_MASK_BUTTON_RELEASE |
                        xcb::EVENT_MASK_POINTER_MOTION |
                        xcb::EVENT_MASK_BUTTON_MOTION |
                        xcb::EVENT_MASK_KEY_RELEASE |
                        xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
            ],
        ).request_check()?;

        xcb::change_property(
            conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            conn.atom_protocols,
            4,
            32,
            &[conn.atom_delete],
        );


        Ok(Window { conn, window_id })
    }

    /// Change the title for the window manager
    pub fn set_title(&self, title: &str) {
        xcb_util::icccm::set_wm_name(self.conn.conn(), self.window_id, title);
    }

    /// Display the window
    pub fn show(&self) {
        xcb::map_window(self.conn.conn(), self.window_id);
    }
}

impl<'a> Drop for Window<'a> {
    fn drop(&mut self) {
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
}

pub struct Context<'a> {
    gc_id: xcb::xproto::Gcontext,
    conn: &'a Connection,
    drawable: xcb::xproto::Drawable,
}

impl<'a> Context<'a> {
    pub fn new(conn: &'a Connection, d: &Drawable) -> Context<'a> {
        let gc_id = conn.conn().generate_id();
        let drawable = d.as_drawable();
        xcb::create_gc(conn.conn(), gc_id, drawable, &[]);
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
            self.conn.conn(),
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
            self.conn.conn(),
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
        xcb::free_gc(self.conn.conn(), self.gc_id);
    }
}

/// A color stored as big endian bgra32
#[derive(Copy, Clone, Debug)]
pub struct Color(u32);

impl From<Srgb> for Color {
    #[inline]
    fn from(s: Srgb) -> Color {
        let b: [u8; 4] = s.to_pixel();
        Color::rgba(b[0], b[1], b[2], b[3])
    }
}

impl From<Color> for Srgb {
    #[inline]
    fn from(c: Color) -> Srgb {
        Srgb::from_pixel(&c.as_rgba())
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
                let src: Srgb = (*self).into();
                let dest: Srgb = dest.into();
                Srgb::from_linear(src.to_linear().over(dest.to_linear())).into()
            }

            &Operator::Source => *self,

            &Operator::Multiply => {
                let src: Srgb = (*self).into();
                let dest: Srgb = dest.into();
                let result: Color = Srgb::from_linear(src.to_linear().multiply(dest.to_linear()))
                    .into();
                result.into()
            }

            &Operator::MultiplyThenOver(ref tint) => {
                // First multiply by the tint color.  This colorizes the glyph.
                let src: Srgb = (*self).into();
                let tint: Srgb = (*tint).into();
                let mut tinted = src.to_linear().multiply(tint.to_linear());
                // We take the alpha from the source.  This is important because
                // we're using Multiply to tint the glyph and if we don't reset the
                // alpha we tend to end up with a background square of the tint color.
                tinted.alpha = src.alpha;

                // Then blend the tinted glyph over the destination background
                let dest: Srgb = dest.into();
                Srgb::from_linear(tinted.over(dest.to_linear())).into()
            }
        }
    }
}

impl From<RgbColor> for Color {
    #[inline]
    fn from(color: RgbColor) -> Self {
        Color::rgb(color.red, color.green, color.blue)
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
    /// Obtain a mutable reference to the raw bgra pixel at the specified coordinates
    fn pixel_mut(&mut self, x: usize, y: usize) -> &mut u32 {
        let (width, height) = self.image_dimensions();
        assert!(x < width && y < height);
        unsafe {
            let offset = (y * width * 4) + (x * 4);
            &mut *(self.pixel_data_mut().offset(offset as isize) as *mut u32)
        }
    }

    #[inline]
    /// Read the raw bgra pixel at the specified coordinates
    fn pixel(&self, x: usize, y: usize) -> u32 {
        let (width, height) = self.image_dimensions();
        assert!(x < width && y < height);
        unsafe {
            let offset = (y * width * 4) + (x * 4);
            *(self.pixel_data().offset(offset as isize) as *const u32)
        }
    }

    /// Clear the entire image to the specific color
    fn clear(&mut self, color: Color) {
        let (width, height) = self.image_dimensions();
        self.clear_rect(0, 0, width, height, color);
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
        for y in 0..height {
            let dest_y = y as isize + dest_y;
            if dest_y < 0 {
                continue;
            }
            if dest_y as usize >= dim_height {
                break;
            }
            for x in 0..width {
                let dest_x = x as isize + dest_x;
                if dest_x < 0 {
                    continue;
                }
                if dest_x as usize >= dim_width {
                    break;
                }

                *self.pixel_mut(dest_x as usize, dest_y as usize) = color.0;
            }
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
        let (dim_width, dim_height) = self.image_dimensions();
        // Draw the vertical lines down either side
        for y in 0..height {
            let dest_y = y as isize + dest_y;
            if dest_y < 0 {
                continue;
            }
            if dest_y >= dim_height as isize {
                break;
            }

            if dest_x >= 0 && dest_x < dim_width as isize {
                let left = self.pixel_mut(dest_x as usize, dest_y as usize);
                *left = color.composite(Color(*left), &operator).0;
            }

            let right_x = dest_x + width as isize - 1;
            if right_x >= 0 && right_x < dim_width as isize {
                let right = self.pixel_mut(right_x as usize, dest_y as usize);
                *right = color.composite(Color(*right), &operator).0;
            }
        }

        // And the horizontals for the top and bottom
        for x in 0..width {
            let dest_x = x as isize + dest_x;

            if dest_x < 0 {
                continue;
            }
            if dest_x >= dim_width as isize {
                break;
            }

            if dest_y >= 0 && dest_y < dim_height as isize {
                let top = self.pixel_mut(dest_x as usize, dest_y as usize);
                *top = color.composite(Color(*top), &operator).0;
            }

            let bot_y = dest_y + height as isize - 1;
            if bot_y >= 0 && bot_y < dim_height as isize {
                let bottom = self.pixel_mut(dest_x as usize, bot_y as usize);
                *bottom = color.composite(Color(*bottom), &operator).0;
            }
        }
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
        assert!(width <= dest_width && height <= dest_height);
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

                let src = Color(im.pixel(x, y));
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
        ).resize(&self.data, &mut dest.data);
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

/// Holder for a shared memory segment id.
/// We hold on to the id only until the server has attached
/// (or failed to attach) to the segment.
/// The id is removed on Drop.
struct ShmId {
    id: libc::c_int,
}

/// Holder for a shared memory mapping.
/// The mapping is removed on Drop.
struct ShmData {
    /// the base address of the mapping
    data: *mut u8,
}

impl ShmId {
    /// Create a new private shared memory segment of the specified size
    fn new(size: usize) -> Result<ShmId> {
        let id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };

        if id == -1 {
            bail!(
                "shmget failed for {} bytes: {:?}",
                size,
                io::Error::last_os_error()
            );
        }

        Ok(ShmId { id })
    }

    /// Attach the segment to our address space
    fn attach(&self) -> Result<ShmData> {
        let data = unsafe { libc::shmat(self.id, ptr::null(), 0) };
        if data as usize == !0 {
            bail!("shmat failed: {:?} {}", data, io::Error::last_os_error());
        }
        Ok(ShmData { data: data as *mut u8 })
    }
}

impl Drop for ShmId {
    fn drop(&mut self) {
        unsafe {
            libc::shmctl(self.id, libc::IPC_RMID, ptr::null_mut());
        }
    }
}

impl Drop for ShmData {
    fn drop(&mut self) {
        unsafe {
            libc::shmdt(self.data as *const _);
        }
    }
}

/// An image implementation backed by shared memory.
/// This also has an associated pixmap on the server side,
/// so we implement both BitmapImage and Drawable.
pub struct ShmImage<'a> {
    data: ShmData,
    seg_id: xcb::shm::Seg,
    draw_id: u32,
    conn: &'a Connection,
    width: usize,
    height: usize,
}

impl<'a> ShmImage<'a> {
    pub fn new(
        conn: &Connection,
        drawable: xcb::xproto::Drawable,
        width: usize,
        height: usize,
    ) -> Result<ShmImage> {
        if !conn.shm_available {
            bail!("SHM not available");
        }

        // Allocate and attach memory of the desired size
        let id = ShmId::new(width * height * 4)?;
        let data = id.attach()?;

        // Tell the server to attach to it
        let seg_id = conn.generate_id();
        xcb::shm::attach_checked(conn, seg_id, id.id as u32, false)
            .request_check()?;

        // Now create a pixmap that references it
        let draw_id = conn.generate_id();
        xcb::shm::create_pixmap_checked(
            conn,
            draw_id,
            drawable,
            width as u16,
            height as u16,
            24, // TODO: we need to get this from the conn->screen
            seg_id,
            0,
        ).request_check()?;

        Ok(ShmImage {
            data,
            seg_id,
            draw_id,
            conn,
            width,
            height,
        })
    }
}

impl<'a> BitmapImage for ShmImage<'a> {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.data.data as *const u8
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.data.data
    }

    fn image_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

impl<'a> Drop for ShmImage<'a> {
    fn drop(&mut self) {
        xcb::free_pixmap(self.conn, self.draw_id);
        xcb::shm::detach(self.conn, self.seg_id);
    }
}

impl<'a> Drawable for ShmImage<'a> {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.draw_id
    }

    fn get_conn(&self) -> &Connection {
        self.conn
    }
}
