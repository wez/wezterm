// TODO: change this
#![allow(dead_code, unused)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::anyhow;
use async_trait::async_trait;
use config::configuration;
use config::ConfigHandle;
use promise::Future;
use raw_window_handle::HasRawDisplayHandle;
use raw_window_handle::HasRawWindowHandle;
use raw_window_handle::RawDisplayHandle;
use raw_window_handle::RawWindowHandle;
use raw_window_handle::WaylandDisplayHandle;
use raw_window_handle::WaylandWindowHandle;
use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::shell::xdg::window::DecorationMode;
use smithay_client_toolkit::shell::xdg::window::Window as XdgWindow;
use smithay_client_toolkit::shell::xdg::window::WindowDecorations as Decorations;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy;
use wayland_egl::{is_available as egl_is_available, WlEglSurface};
use wezterm_font::FontConfiguration;
use wezterm_input_types::KeyboardLedStatus;
use wezterm_input_types::Modifiers;
use wezterm_input_types::MouseButtons;
use wezterm_input_types::WindowDecorations;

use crate::wayland::WaylandConnection;
use crate::Clipboard;
use crate::Connection;
use crate::ConnectionOps;
use crate::MouseCursor;
use crate::RequestedWindowGeometry;
use crate::Window;
use crate::WindowEvent;
use crate::WindowEventSender;
use crate::WindowOps;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
        config: Option<&ConfigHandle>,
        font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        log::trace!("Creating a window");
        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };

        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                anyhow!(
                    "new_window must be called on the gui thread after Connection:init has succeed",
                )
            })?
            .wayland();

        let window_id = conn.next_window_id();
        // let pending_event = Arc::new(Mutex::new(PendingEvent::default()));

        // let (pending_first_configure, wait_configure) = async_channel::bounded(1);

        let qh = conn.event_queue.borrow().handle();
        let globals = conn.globals.borrow();

        let compositor = CompositorState::bind(&globals, &qh)?;
        let surface = compositor.create_surface(&qh);

        let xdg_shell = XdgShell::bind(&globals, &qh)?;
        let window = xdg_shell.create_window(surface.clone(), Decorations::RequestServer, &qh);

        window.set_app_id(class_name.to_string());
        // TODO: investigate the resizable thing
        // window.set_resizable(true);
        window.set_title(name.to_string());
        let decorations = config.window_decorations;

        let decor_mode = if decorations == WindowDecorations::NONE {
            None
        } else if decorations == WindowDecorations::default() {
            Some(DecorationMode::Server)
        } else {
            Some(DecorationMode::Client)
        };
        window.request_decoration_mode(decor_mode);

        // TODO: I don't know anything about the frame thing
        //         window.set_frame_config(ConceptConfig {

        window.set_min_size(Some((32, 32)));

        window.commit();
        //
        // TODO:
        // let copy_and_paste = CopyAndPaste::create();
        // let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        // conn.pointer.borrow().add_window(&surface, &pending_mouse);

        // TODO: WindowInner
        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            events: WindowEventSender::new(event_handler),
            window: Some(window),

            wegl_surface: None,
            gl_state: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        // TODO: assign window inner
        inner
            .borrow_mut()
            .events
            .assign_window(window_handle.clone());

        // window.set_decorate(if decorations == WindowDecorations::NONE {
        //     Decorations::None
        // } else if decorations == WindowDecorations::default() {
        //     Decorations::FollowServer
        // } else {
        //     // SCTK/Wayland don't allow more nuance than "decorations are hidden",
        //     // so if we have a mixture of things, then we need to force our
        //     // client side decoration rendering.
        //     Decorations::ClientSide
        // });
        conn.windows.borrow_mut().insert(window_id, inner.clone());
        log::trace!("Return from commiting window");

        Ok(window_handle)
    }
}

#[async_trait(?Send)]
impl WindowOps for WaylandWindow {
    #[doc = r" Show a hidden window"]
    fn show(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        });
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        todo!()
    }

    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let window = self.0;
        promise::spawn::spawn(async move {
            if let Some(handle) = Connection::get().unwrap().wayland().window_by_id(window) {
                let mut inner = handle.borrow_mut();
                inner.enable_opengl()
            } else {
                anyhow::bail!("invalid window");
            }
        })
        .await
    }

    #[doc = r" Hide a visible window"]
    fn hide(&self) {
        todo!()
    }

    #[doc = r" Schedule the window to be closed"]
    fn close(&self) {
        todo!()
    }

    #[doc = r" Change the cursor"]
    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        todo!()
    }

    #[doc = r" Invalidate the window so that the entire client area will"]
    #[doc = r" be repainted shortly"]
    fn invalidate(&self) {
        todo!()
    }

    #[doc = r" Change the titlebar text for the window"]
    fn set_title(&self, title: &str) {
        todo!()
    }

    #[doc = r" Resize the inner or client area of the window"]
    fn set_inner_size(&self, width: usize, height: usize) {
        todo!()
    }

    #[doc = r" Initiate textual transfer from the clipboard"]
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        todo!()
    }

    #[doc = r" Set some text in the clipboard"]
    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        todo!()
    }
}

unsafe impl HasRawDisplayHandle for WaylandWindow {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut handle = WaylandDisplayHandle::empty();
        let conn = WaylandConnection::get().unwrap().wayland();
        handle.display = conn.connection.borrow().backend().display_ptr() as *mut _;
        RawDisplayHandle::Wayland(handle)
    }
}

unsafe impl HasRawWindowHandle for WaylandWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let conn = Connection::get().expect("raw_window_handle only callable on main thread");
        let handle = conn
            .wayland()
            .window_by_id(self.0)
            .expect("window handle invalid!?");

        let inner = handle.borrow();
        inner.raw_window_handle()
    }
}

#[derive(Default, Clone, Debug)]
struct PendingEvent {
    close: bool,
    had_configure_event: bool,
    configure: Option<(u32, u32)>,
    dpi: Option<i32>,
    // window_state: Option<WindowState>,
}

pub struct WaylandWindowInner {
    //     window_id: usize,
    pub(crate) events: WindowEventSender,
    // surface_factor: f64,
    // copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<XdgWindow>,
    // dimensions: Dimensions,
    // resize_increments: Option<(u16, u16)>,
    // window_state: WindowState,
    // last_mouse_coords: Point,
    // mouse_buttons: MouseButtons,
    // hscroll_remainder: f64,
    // vscroll_remainder: f64,
    // modifiers: Modifiers,
    // leds: KeyboardLedStatus,
    // key_repeat: Option<(u32, Arc<Mutex<KeyRepeatState>>)>,
    // pending_event: Arc<Mutex<PendingEvent>>,
    // pending_mouse: Arc<Mutex<PendingMouse>>,
    // pending_first_configure: Option<async_channel::Sender<()>>,
    // frame_callback: Option<Main<WlCallback>>,
    // invalidated: bool,
    // font_config: Rc<FontConfiguration>,
    // text_cursor: Option<Rect>,
    // appearance: Appearance,
    // config: ConfigHandle,
    // // cache the title for comparison to avoid spamming
    // // the compositor with updates that don't actually change it
    // title: Option<String>,
    // // wegl_surface is listed before gl_state because it
    // // must be dropped before gl_state otherwise the underlying
    // // libraries will segfault on shutdown
    wegl_surface: Option<WlEglSurface>,
    gl_state: Option<Rc<glium::backend::Context>>,
}

impl WaylandWindowInner {
    fn show(&mut self) {
        // TODO: Need to implement show
        if self.window.is_none() {}
    }

    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let wayland_conn = Connection::get().unwrap().wayland();
        let mut wegl_surface = None;

        log::trace!("Enable opengl");

        let gl_state = if !egl_is_available() {
            Err(anyhow!("!egl_is_available"))
        } else {
            let window = self
                .window
                .as_ref()
                .ok_or(anyhow!("Window does not exist"))?;
            let object_id = window.wl_surface().id();

            wegl_surface = Some(WlEglSurface::new(
                object_id, // TODO: remove the hardcoded stuff
                100, 100,
            )?);

            log::trace!("WEGL Surface here {:?}", wegl_surface);

            match wayland_conn.gl_connection.borrow().as_ref() {
                Some(glconn) => crate::egl::GlState::create_wayland_with_existing_connection(
                    glconn,
                    wegl_surface.as_ref().unwrap(),
                ),
                None => crate::egl::GlState::create_wayland(
                    Some(wayland_conn.connection.borrow().backend().display_ptr() as *const _),
                    wegl_surface.as_ref().unwrap(),
                ),
            }
        };
        let gl_state = gl_state.map(Rc::new).and_then(|state| unsafe {
            wayland_conn
                .gl_connection
                .borrow_mut()
                .replace(Rc::clone(state.get_connection()));
            Ok(glium::backend::Context::new(
                Rc::clone(&state),
                true,
                if cfg!(debug_assertions) {
                    glium::debug::DebugCallbackBehavior::DebugMessageOnError
                } else {
                    glium::debug::DebugCallbackBehavior::Ignore
                },
            )?)
        })?;

        self.gl_state.replace(gl_state.clone());
        self.wegl_surface = wegl_surface;

        Ok(gl_state)
    }
}

unsafe impl HasRawDisplayHandle for WaylandWindowInner {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        // let mut handle = WaylandDisplayHandle::empty();
        // let conn = WaylandConnection::get().unwrap().wayland();
        // handle.display = conn.display.borrow().c_ptr() as _;
        // RawDisplayHandle::Wayland(handle)
        todo!()
    }
}

unsafe impl HasRawWindowHandle for WaylandWindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = WaylandWindowHandle::empty();
        let surface = self
            .window
            .as_ref()
            .expect("Window should exist")
            .wl_surface();
        handle.surface = surface.id().as_ptr() as *mut _;
        RawWindowHandle::Wayland(handle)
    }
}
