use egli;
use gl;
use glium;
use glium::backend::Backend;
use libc;
use std::convert::From;
use std::io;
use std::mem;
use std::ops::Deref;
use std::os;
use std::ptr;
use std::rc::Rc;
use std::result;
use x11;
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
    pub display: *mut x11::xlib::Display,
    conn: xcb::Connection,
    screen_num: i32,
    pub atom_protocols: xcb::Atom,
    pub atom_delete: xcb::Atom,
    pub atom_utf8_string: xcb::Atom,
    pub atom_xsel_data: xcb::Atom,
    pub atom_targets: xcb::Atom,
    keysyms: *mut xcb_key_symbols_t,
    shm_available: bool,
    egl_display: Rc<egli::Display>,
    egl_config: egli::FrameBufferConfigRef,
}

impl Deref for Connection {
    type Target = xcb::Connection;

    fn deref(&self) -> &xcb::Connection {
        &self.conn
    }
}

#[link(name = "X11-xcb")]
extern "C" {
    fn XGetXCBConnection(display: *mut x11::xlib::Display) -> *mut xcb::ffi::xcb_connection_t;
    fn XSetEventQueueOwner(display: *mut x11::xlib::Display, owner: i32);
}

fn egli_err(err: egli::error::Error) -> Error {
    format_err!("egli error: {:?}", err)
}

impl Connection {
    pub fn new() -> Result<Connection> {
        let display = unsafe { x11::xlib::XOpenDisplay(ptr::null()) };
        if display.is_null() {
            bail!("failed to open display");
        }
        let screen_num = unsafe { x11::xlib::XDefaultScreen(display) };
        let conn = unsafe { xcb::Connection::from_raw_conn(XGetXCBConnection(display)) };
        unsafe { XSetEventQueueOwner(display, 1) };

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

        let egl_display = egli::Display::from_display_id(display as *mut _).map_err(
            egli_err,
        )?;

        let egl_version = egl_display.initialize_and_get_version().map_err(egli_err)?;
        println!("Using EGL {}", egl_version);

        let configs = egl_display
            .config_filter()
            .with_red_size(8)
            .with_green_size(8)
            .with_blue_size(8)
            .with_depth_size(24)
            .with_surface_type(
                egli::SurfaceType::WINDOW | egli::SurfaceType::PBUFFER | egli::SurfaceType::PIXMAP,
            )
            .with_renderable_type(egli::RenderableType::OPENGL_ES2)
            .with_conformant(egli::RenderableType::OPENGL_ES2)
            .choose_configs()
            .map_err(|e| format_err!("failed to get EGL config: {:?}", e))?;

        let first_config = *configs.first().ok_or(failure::err_msg(
            "no compatible EGL configuration was found",
        ))?;

        Ok(Connection {
            display,
            conn,
            screen_num,
            atom_protocols,
            atom_delete,
            keysyms,
            shm_available,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            egl_display: Rc::new(egl_display),
            egl_config: first_config,
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

struct GlState {
    display: Rc<egli::Display>,
    surface: egli::Surface,
    egl_context: egli::Context,
}

/// A Window!
pub struct Window<'a> {
    window_id: xcb::xproto::Window,
    conn: &'a Connection,
    gl: Rc<GlState>,
    glium_context: Rc<glium::backend::Context>,
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

        let surface = conn.egl_display
            .create_window_surface(conn.egl_config, window_id as *mut _)
            .map_err(egli_err)?;

        let egl_context = conn.egl_display
            .create_context_with_client_version(
                conn.egl_config,
                egli::ContextClientVersion::OpenGlEs2,
            )
            .map_err(egli_err)?;

        conn.egl_display
            .make_current(&surface, &surface, &egl_context)
            .map_err(egli_err)?;

        gl::load_with(
            |s| unsafe { mem::transmute(egli::egl::get_proc_address(s)) },
        );

        let gl_state = Rc::new(GlState {
            display: Rc::clone(&conn.egl_display),
            egl_context,
            surface,
        });

        let glium_context = unsafe {
            glium::backend::Context::new(
                Rc::clone(&gl_state),
                // we're single threaded, so no need to check contexts
                false,
                if cfg!(debug_assertions) {
                    glium::debug::DebugCallbackBehavior::PrintAll
                //glium::debug::DebugCallbackBehavior::DebugMessageOnError
                } else {
                    glium::debug::DebugCallbackBehavior::Ignore
                },
            )?
        };

        Ok(Window {
            conn,
            window_id,
            gl: gl_state,
            glium_context,
        })
    }

    /// Change the title for the window manager
    pub fn set_title(&self, title: &str) {
        xcb_util::icccm::set_wm_name(self.conn.conn(), self.window_id, title);
    }

    /// Display the window
    pub fn show(&self) {
        xcb::map_window(self.conn.conn(), self.window_id);
    }

    pub fn draw(&self) -> glium::Frame {
        glium::Frame::new(
            self.glium_context.clone(),
            self.gl.get_framebuffer_dimensions(),
        )
    }
}

impl<'a> Drop for Window<'a> {
    fn drop(&mut self) {
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
}

impl<'a> glium::backend::Facade for Window<'a> {
    fn get_context(&self) -> &Rc<glium::backend::Context> {
        &self.glium_context
    }
}

unsafe impl glium::backend::Backend for GlState {
    fn swap_buffers(&self) -> result::Result<(), glium::SwapBuffersError> {
        self.display.swap_buffers(&self.surface).map_err(|_| {
            // We're guessing that this is the case as the other option
            // that glium recognizes is threading related and we're
            // single threaded.
            glium::SwapBuffersError::AlreadySwapped
        })
    }

    unsafe fn get_proc_address(&self, symbol: &str) -> *const os::raw::c_void {
        mem::transmute(egli::egl::get_proc_address(symbol))
    }

    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        (
            self.surface.query_width().unwrap() as u32,
            self.surface.query_height().unwrap() as u32,
        )
    }

    fn is_current(&self) -> bool {
        true
    }

    unsafe fn make_current(&self) {
        self.display
            .make_current(&self.surface, &self.surface, &self.egl_context)
            .expect("make_current failed");
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
