use egli;
use gl;
use glium;
use glium::backend::Backend;
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
                    //glium::debug::DebugCallbackBehavior::PrintAll
                    glium::debug::DebugCallbackBehavior::DebugMessageOnError
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
