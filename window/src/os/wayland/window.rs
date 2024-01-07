// TODO: change this
// TODO: change a lot of the pubstruct crates
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
use smithay_client_toolkit::compositor::SurfaceData;
use smithay_client_toolkit::compositor::SurfaceDataExt;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::shell::xdg::window::DecorationMode;
use smithay_client_toolkit::shell::xdg::window::Window as XdgWindow;
use smithay_client_toolkit::shell::xdg::window::WindowDecorations as Decorations;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_callback::WlCallback;
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
use crate::WindowState;

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
        let pending_event = Arc::new(Mutex::new(PendingEvent::default()));

        let (pending_first_configure, wait_configure) = async_channel::bounded(1);

        let qh = conn.event_queue.borrow().handle();
        let globals = conn.globals.borrow();

        let compositor = CompositorState::bind(&globals, &qh)?;
        let surface = compositor.create_surface(&qh);

        // We need user data so we can get the window_id => WaylandWindowInner during a handler
        let surface_data = SurfaceUserData {
            surface_data: SurfaceData::default(),
            window_id,
        };
        let surface = compositor.create_surface_with_data(&qh, surface_data);

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
            window_id,
            events: WindowEventSender::new(event_handler),

            invalidated: false,
            window: Some(window),

            window_state: WindowState::default(),

            pending_event,

            pending_first_configure: Some(pending_first_configure),
            frame_callback: None,

            title: None,

            wegl_surface: None,
            gl_state: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        inner
            .borrow_mut()
            .events
            .assign_window(window_handle.clone());

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        wait_configure.recv().await?;

        Ok(window_handle)
    }
}

#[async_trait(?Send)]
impl WindowOps for WaylandWindow {
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
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner
                .events
                .dispatch(WindowEvent::Notification(Box::new(t)));
            Ok(())
        });
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

    fn invalidate(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        });
    }

    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.set_title(title);
            Ok(())
        });
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
pub(crate) struct PendingEvent {
    pub(crate) close: bool,
    pub(crate) had_configure_event: bool,
    pub(crate) configure: Option<(u32, u32)>,
    pub(crate) dpi: Option<i32>,
    pub(crate) window_state: Option<WindowState>,
}

pub struct WaylandWindowInner {
    window_id: usize,
    pub(crate) events: WindowEventSender,
    // surface_factor: f64,
    // copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<XdgWindow>,
    // dimensions: Dimensions,
    // resize_increments: Option<(u16, u16)>,
    window_state: WindowState,
    // last_mouse_coords: Point,
    // mouse_buttons: MouseButtons,
    // hscroll_remainder: f64,
    // vscroll_remainder: f64,
    // modifiers: Modifiers,
    // leds: KeyboardLedStatus,
    // key_repeat: Option<(u32, Arc<Mutex<KeyRepeatState>>)>,
    pub(crate) pending_event: Arc<Mutex<PendingEvent>>,
    // pending_mouse: Arc<Mutex<PendingMouse>>,
    pending_first_configure: Option<async_channel::Sender<()>>,
    frame_callback: Option<WlCallback>,
    invalidated: bool,
    // font_config: Rc<FontConfiguration>,
    // text_cursor: Option<Rect>,
    // appearance: Appearance,
    // config: ConfigHandle,
    // // cache the title for comparison to avoid spamming
    // // the compositor with updates that don't actually change it
    title: Option<String>,
    // // wegl_surface is listed before gl_state because it
    // // must be dropped before gl_state otherwise the underlying
    // // libraries will segfault on shutdown
    wegl_surface: Option<WlEglSurface>,
    gl_state: Option<Rc<glium::backend::Context>>,
}

impl WaylandWindowInner {
    fn show(&mut self) {
        log::trace!("WaylandWindowInner show: {:?}", self.window);
        if self.window.is_none() {
            return;
        }

        self.do_paint().unwrap();
    }

    fn refresh_frame(&mut self) {
        if let Some(window) = self.window.as_mut() {
            // window.refresh();
            // window.surface().commit();
        }
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

    pub(crate) fn dispatch_pending_event(&mut self) {
        let mut pending;
        {
            let mut pending_events = self.pending_event.lock().unwrap();
            pending = pending_events.clone();
            *pending_events = PendingEvent::default();
        }

        if pending.close {
            self.events.dispatch(WindowEvent::CloseRequested);
        }

        if let Some(window_state) = pending.window_state.take() {
            log::debug!(
                "dispatch_pending_event self.window_state={:?}, pending:{:?}",
                window_state,
                window_state
            );
            self.window_state = window_state;
        }

        if pending.configure.is_none() {
            // TODO: Handle the DPI
        }

        if let Some((mut w, mut h)) = pending.configure.take() {
            log::trace!("Pending configure: w:{w}, h{h} -- {:?}", self.window);
            if self.window.is_some() {
                // TODO: here
                // let factor = get_surface_scale_factor(&self.surface) as f64;
                // let old_dimensions = self.dimensions;

                // FIXME: teach this how to resolve dpi_by_screen
                // let dpi = self.config.dpi.unwrap_or(factor * crate::DEFAULT_DPI) as usize;

                // Do this early because this affects surface_to_pixels/pixels_to_surface
                // self.dimensions.dpi = dpi;

                // let mut pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                // let mut pixel_height = self.surface_to_pixels(h.try_into().unwrap());
                //
                // if self.window_state.can_resize() {
                //     if let Some((x, y)) = self.resize_increments {
                //         let desired_pixel_width = pixel_width - (pixel_width % x as i32);
                //         let desired_pixel_height = pixel_height - (pixel_height % y as i32);
                //         w = self.pixels_to_surface(desired_pixel_width) as u32;
                //         h = self.pixels_to_surface(desired_pixel_height) as u32;
                //         pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                //         pixel_height = self.surface_to_pixels(h.try_into().unwrap());
                //     }
                // }
                //
                // // Update the window decoration size
                // self.window.as_mut().unwrap().resize(w, h);
                //
                // // Compute the new pixel dimensions
                // let new_dimensions = Dimensions {
                //     pixel_width: pixel_width.try_into().unwrap(),
                //     pixel_height: pixel_height.try_into().unwrap(),
                //     dpi,
                // };
                //
                // // Only trigger a resize if the new dimensions are different;
                // // this makes things more efficient and a little more smooth
                // if new_dimensions != old_dimensions {
                //     self.dimensions = new_dimensions;
                //
                //     self.events.dispatch(WindowEvent::Resized {
                //         dimensions: self.dimensions,
                //         window_state: self.window_state,
                //         // We don't know if we're live resizing or not, so
                //         // assume no.
                //         live_resizing: false,
                //     });
                //     // Avoid blurring by matching the scaling factor of the
                //     // compositor; if it is going to double the size then
                //     // we render at double the size anyway and tell it that
                //     // the buffer is already doubled.
                //     // Take care to detach the current buffer (managed by EGL),
                //     // so that the compositor doesn't get annoyed by it not
                //     // having dimensions that match the scale.
                //     // The wegl_surface.resize won't take effect until
                //     // we paint later on.
                //     // We do this only if the scale has actually changed,
                //     // otherwise interactive window resize will keep removing
                //     // the window contents!
                //     if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                //         wegl_surface.resize(pixel_width, pixel_height, 0, 0);
                //     }
                //     if self.surface_factor != factor {
                //         let wayland_conn = Connection::get().unwrap().wayland();
                //         let mut pool = wayland_conn.mem_pool.borrow_mut();
                //         // Make a "fake" buffer with the right dimensions, as
                //         // simply detaching the buffer can cause wlroots-derived
                //         // compositors consider the window to be unconfigured.
                //         if let Ok((_bytes, buffer)) = pool.buffer(
                //             factor as i32,
                //             factor as i32,
                //             (factor * 4.0) as i32,
                //             wayland_client::protocol::wl_shm::Format::Argb8888,
                //         ) {
                //             self.surface.attach(Some(&buffer), 0, 0);
                //             self.surface.set_buffer_scale(factor as i32);
                //             self.surface_factor = factor;
                //         }
                //     }
                // }
                // self.refresh_frame();
                // self.do_paint().unwrap();
            }
        }
        // if pending.refresh_decorations && self.window.is_some() {
        //     self.refresh_frame();
        // }
        if pending.had_configure_event && self.window.is_some() {
            if let Some(notify) = self.pending_first_configure.take() {
                // Allow window creation to complete
                notify.try_send(()).ok();
            }
        }
    }

    fn invalidate(&mut self) {
        if self.frame_callback.is_some() {
            self.invalidated = true;
            return;
        }
        self.do_paint().unwrap();
    }

    fn set_title(&mut self, title: String) {
        if let Some(last_title) = self.title.as_ref() {
            if last_title == &title {
                return;
            }
        }
        if let Some(window) = self.window.as_ref() {
            window.set_title(title.clone());
        }
        // TODO: self.refresh_frame();
        self.title = Some(title);
    }

    fn do_paint(&mut self) -> anyhow::Result<()> {
        if self.frame_callback.is_some() {
            // Painting now won't be productive, so skip it but
            // remember that we need to be painted so that when
            // the compositor is ready for us, we can paint then.
            self.invalidated = true;
            return Ok(());
        }

        self.invalidated = false;

        // Ask the compositor to wake us up when its time to paint the next frame,
        // note that this only happens _after_ the next commit
        let window_id = self.window_id;

        let conn = WaylandConnection::get().unwrap().wayland();
        let qh = conn.event_queue.borrow().handle();

        let callback = self.surface().frame(&qh, self.surface().clone());

        log::trace!("do_paint - callback: {:?}", callback);
        self.frame_callback.replace(callback);

        // The repaint has the side of effect of committing the surface,
        // which is necessary for the frame callback to get triggered.
        // Ordering the repaint after requesting the callback ensures that
        // we will get woken at the appropriate time.
        // <https://github.com/wez/wezterm/issues/3468>
        // <https://github.com/wez/wezterm/issues/3126>
        self.events.dispatch(WindowEvent::NeedRepaint);

        Ok(())
    }

    fn surface(&self) -> &WlSurface {
        self.window
            .as_ref()
            .expect("Window should exist")
            .wl_surface()
    }

    pub(crate) fn next_frame_is_ready(&mut self) {
        self.frame_callback.take();
        if self.invalidated {
            self.do_paint().ok();
        }
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
        let surface = self.surface();
        handle.surface = surface.id().as_ptr() as *mut _;
        RawWindowHandle::Wayland(handle)
    }
}

pub(crate) struct SurfaceUserData {
    surface_data: SurfaceData,
    pub(crate) window_id: usize,
}

impl SurfaceUserData {
    pub(crate) fn from_wl(wl: &WlSurface) -> &Self {
        wl.data()
            .expect("User data should be associated with WlSurface")
    }
}

impl SurfaceDataExt for SurfaceUserData {
    fn surface_data(&self) -> &smithay_client_toolkit::compositor::SurfaceData {
        &self.surface_data
    }
}
