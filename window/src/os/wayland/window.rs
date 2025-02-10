use std::any::Any;
use std::cell::RefCell;
use std::cmp::max;
use std::convert::TryInto;
use std::io::Read;
use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use async_io::Timer;
use async_trait::async_trait;
use config::ConfigHandle;
use promise::{Future, Promise};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::compositor::{CompositorHandler, SurfaceData, SurfaceDataExt};
use smithay_client_toolkit::data_device_manager::ReadPipe;
use smithay_client_toolkit::reexports::csd_frame::{
    DecorationsFrame, FrameAction, ResizeEdge, WindowState as SCTKWindowState,
};
use smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge as XdgResizeEdge;
use smithay_client_toolkit::seat::pointer::CursorIcon;
use smithay_client_toolkit::shell::xdg::fallback_frame::FallbackFrame;
use smithay_client_toolkit::shell::xdg::window::{
    DecorationMode, Window as XdgWindow, WindowConfigure, WindowDecorations as Decorations,
    WindowHandler,
};
use smithay_client_toolkit::shell::xdg::XdgSurface;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeyState};
use wayland_client::protocol::wl_pointer::{ButtonState, WlPointer};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection as WConnection, Proxy};
use wayland_egl::{is_available as egl_is_available, WlEglSurface};
use wezterm_font::FontConfiguration;
use wezterm_input_types::{
    KeyboardLedStatus, Modifiers, MouseButtons, MouseEvent, MouseEventKind, MousePress,
    ScreenPoint, WindowDecorations,
};

use crate::wayland::WaylandConnection;
use crate::x11::KeyboardWithFallback;
use crate::{
    Appearance, Clipboard, Connection, ConnectionOps, Dimensions, MouseCursor, Point, Rect,
    RequestedWindowGeometry, ResizeIncrement, ResolvedGeometry, Window, WindowEvent,
    WindowEventSender, WindowKeyEvent, WindowOps, WindowState,
};

use super::copy_and_paste::CopyAndPaste;
use super::pointer::{PendingMouse, PointerUserData};
use super::state::WaylandState;

#[derive(Debug)]
pub(super) struct KeyRepeatState {
    pub(super) when: Instant,
    pub(super) event: WindowKeyEvent,
}

impl KeyRepeatState {
    pub(super) fn schedule(state: Arc<Mutex<Self>>, window_id: usize) {
        promise::spawn::spawn_into_main_thread(async move {
            let delay;
            let gap;
            {
                let conn = WaylandConnection::get().unwrap().wayland();
                let (rate, ddelay) = {
                    let wstate = conn.wayland_state.borrow();
                    (
                        wstate.key_repeat_rate as u64,
                        wstate.key_repeat_delay as u64,
                    )
                };
                if rate == 0 {
                    return;
                }
                delay = Duration::from_millis(ddelay);
                gap = Duration::from_millis(1000 / rate);
            }

            let mut initial = true;
            Timer::after(delay).await;
            loop {
                {
                    let handle = {
                        let conn = WaylandConnection::get().unwrap().wayland();
                        match conn.window_by_id(window_id) {
                            Some(handle) => handle,
                            None => return,
                        }
                    };

                    let mut inner = handle.borrow_mut();

                    if inner.key_repeat.as_ref().map(|(_, k)| Arc::as_ptr(k))
                        != Some(Arc::as_ptr(&state))
                    {
                        // Key was released and/or some other key is doing
                        // its own repetition now
                        return;
                    }

                    let mut st = state.lock().unwrap();

                    let mut repeat_count = 1;

                    let mut elapsed = st.when.elapsed();
                    if initial {
                        elapsed -= delay;
                        initial = false;
                    }

                    // If our scheduling interval is longer than the repeat
                    // gap, we need to inflate the repeat count to match
                    // the intended rate
                    while elapsed >= gap {
                        repeat_count += 1;
                        elapsed -= gap;
                    }

                    let event = match st.event.clone() {
                        WindowKeyEvent::KeyEvent(mut key) => {
                            key.repeat_count = repeat_count;
                            WindowEvent::KeyEvent(key)
                        }
                        WindowKeyEvent::RawKeyEvent(mut raw) => {
                            raw.repeat_count = repeat_count;
                            WindowEvent::RawKeyEvent(raw)
                        }
                    };

                    inner.events.dispatch(event);

                    st.when = Instant::now();
                }

                Timer::after(gap).await;
            }
        })
        .detach();
    }
}

enum WaylandWindowEvent {
    Close,
    Request(WindowConfigure),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
        config: Option<&ConfigHandle>,
        _font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
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

        // We need user data so we can get the window_id => WaylandWindowInner during a handler
        let surface_data = SurfaceUserData {
            surface_data: SurfaceData::default(),
            window_id,
        };
        let surface = {
            let compositor = &conn.wayland_state.borrow().compositor;
            compositor.create_surface_with_data(&qh, surface_data)
        };

        let ResolvedGeometry {
            x: _,
            y: _,
            width,
            height,
        } = conn.resolve_geometry(geometry);

        let dimensions = Dimensions {
            pixel_width: width,
            pixel_height: height,
            dpi: config.dpi.unwrap_or(crate::DEFAULT_DPI) as usize,
        };

        let window = {
            let xdg_shell = &conn.wayland_state.borrow().xdg;
            xdg_shell.create_window(surface.clone(), Decorations::RequestServer, &qh)
        };

        window.set_app_id(class_name.to_string());
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

        let mut window_frame = {
            let wayland_state = &conn.wayland_state.borrow();
            let shm = &wayland_state.shm;
            let subcompositor = wayland_state.subcompositor.clone();
            FallbackFrame::new(&window, shm, subcompositor, qh.clone())
                .expect("failed to create csd frame")
        };
        let hidden = match decor_mode {
            Some(DecorationMode::Client) => false,
            _ => true,
        };
        window_frame.set_hidden(hidden);
        if !hidden {
            window_frame.resize(
                NonZeroU32::new(dimensions.pixel_width as u32)
                    .ok_or_else(|| anyhow!("dimensions {dimensions:?} are invalid"))?,
                NonZeroU32::new(dimensions.pixel_height as u32)
                    .ok_or_else(|| anyhow!("dimensions {dimensions:?} are invalid"))?,
            );
        }

        window.set_min_size(Some((32, 32)));
        let (x, y) = window_frame.location();
        window.xdg_surface().set_window_geometry(
            x,
            y,
            dimensions.pixel_width as i32,
            dimensions.pixel_height as i32,
        );
        window.commit();

        let copy_and_paste = CopyAndPaste::create();
        let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        {
            let surface_to_pending = &mut conn.wayland_state.borrow_mut().surface_to_pending;
            surface_to_pending.insert(surface.id(), Arc::clone(&pending_mouse));
        }

        let appearance = conn.get_appearance();

        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            events: WindowEventSender::new(event_handler),
            surface_factor: 1.0,
            copy_and_paste,
            invalidated: false,
            window: Some(window),
            window_frame,
            dimensions,
            resize_increments: None,
            window_state: WindowState::default(),
            last_mouse_coords: Point::new(0, 0),
            mouse_buttons: MouseButtons::NONE,
            hscroll_remainder: 0.0,
            vscroll_remainder: 0.0,

            modifiers: Modifiers::NONE,
            leds: KeyboardLedStatus::empty(),

            key_repeat: None,
            pending_event,
            pending_mouse,

            pending_first_configure: Some(pending_first_configure),
            frame_callback: None,

            text_cursor: None,
            appearance,

            config,

            title: None,

            wegl_surface: None,
            gl_state: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        inner
            .borrow_mut()
            .events
            .assign_window(window_handle.clone());

        {
            let windows = &conn.wayland_state.borrow().windows;
            windows.borrow_mut().insert(window_id, inner.clone());
        };

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

    fn hide(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.window.as_ref().unwrap().set_minimized();
            Ok(())
        });
    }

    fn close(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        });
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_cursor(cursor);
            Ok(())
        });
    }

    fn invalidate(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        });
    }

    fn set_text_cursor_position(&self, cursor: Rect) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
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

    fn set_inner_size(&self, width: usize, height: usize) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        });
    }

    fn set_resize_increments(&self, incr: ResizeIncrement) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_resize_increments(incr)
        });
    }

    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let promise = Arc::new(Mutex::new(promise));
        WaylandConnection::with_window_inner(self.0, move |inner| {
            let read = inner
                .copy_and_paste
                .lock()
                .unwrap()
                .get_clipboard_data(clipboard)?;
            let promise = Arc::clone(&promise);
            std::thread::spawn(move || {
                let mut promise = promise.lock().unwrap();
                match read_pipe_with_timeout(read) {
                    Ok(result) => {
                        // Normalize the text to unix line endings, otherwise
                        // copying from eg: firefox inserts a lot of blank
                        // lines, and that is super annoying.
                        promise.ok(result.replace("\r\n", "\n"));
                    }
                    Err(e) => {
                        log::error!("while reading clipboard: {}", e);
                        promise.err(anyhow!("{}", e));
                    }
                };
            });
            Ok(())
        });
        future
    }

    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner
                .copy_and_paste
                .lock()
                .unwrap()
                .set_clipboard_data(clipboard, text);
            Ok(())
        });
    }

    fn toggle_fullscreen(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            if inner.window_state.contains(WindowState::FULL_SCREEN) {
                inner.window.as_ref().unwrap().unset_fullscreen();
            } else {
                inner.window.as_ref().unwrap().set_fullscreen(None);
            }
            Ok(())
        });
    }

    fn maximize(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| Ok(inner.maximize()));
    }

    fn restore(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| Ok(inner.restore()));
    }
}
#[derive(Default, Clone, Debug)]
pub(crate) struct PendingEvent {
    pub(crate) close: bool,
    pub(crate) had_configure_event: bool,
    refresh_decorations: bool,
    // XXX: configure and window_configure could probably be combined, but right now configure only
    // queues a new size, so it can be out of sync. Example would be maximizing and minimizing winodw
    pub(crate) configure: Option<(u32, u32)>,
    pub(crate) window_configure: Option<WindowConfigure>,
    pub(crate) dpi: Option<i32>,
    pub(crate) window_state: Option<WindowState>,
}

pub(crate) fn read_pipe_with_timeout(mut file: ReadPipe) -> anyhow::Result<String> {
    let mut result = Vec::new();

    // set non-blocking I/O on the pipe
    // (adapted from FileDescriptor::set_non_blocking_impl in /filedescriptor/src/unix.rs)
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } != 0 {
        bail!(
            "failed to change non-blocking mode: {}",
            std::io::Error::last_os_error()
        )
    }

    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };

    let mut buf = [0u8; 8192];

    loop {
        if unsafe { libc::poll(&mut pfd, 1, 3000) == 1 } {
            match file.read(&mut buf) {
                Ok(size) if size == 0 => {
                    break;
                }
                Ok(size) => {
                    result.extend_from_slice(&buf[..size]);
                }
                Err(e) => bail!("error reading from pipe: {}", e),
            }
        } else {
            bail!("timed out reading from pipe");
        }
    }

    Ok(String::from_utf8(result)?)
}

pub struct WaylandWindowInner {
    pub(crate) events: WindowEventSender,
    surface_factor: f64,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<XdgWindow>,
    pub(super) window_frame: FallbackFrame<WaylandState>,
    dimensions: Dimensions,
    resize_increments: Option<ResizeIncrement>,
    window_state: WindowState,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    hscroll_remainder: f64,
    vscroll_remainder: f64,
    modifiers: Modifiers,
    leds: KeyboardLedStatus,
    pub(super) key_repeat: Option<(u32, Arc<Mutex<KeyRepeatState>>)>,
    pub(super) pending_event: Arc<Mutex<PendingEvent>>,
    pub(super) pending_mouse: Arc<Mutex<PendingMouse>>,
    pending_first_configure: Option<async_channel::Sender<()>>,
    frame_callback: Option<WlCallback>,
    invalidated: bool,
    // font_config: Rc<FontConfiguration>,
    text_cursor: Option<Rect>,
    appearance: Appearance,
    config: ConfigHandle,
    // cache the title for comparison to avoid spamming
    // the compositor with updates that don't actually change it
    title: Option<String>,
    // wegl_surface is listed before gl_state because it
    // must be dropped before gl_state otherwise the underlying
    // libraries will segfault on shutdown
    wegl_surface: Option<WlEglSurface>,
    gl_state: Option<Rc<glium::backend::Context>>,
}

impl WaylandWindowInner {
    fn close(&mut self) {
        self.events.dispatch(WindowEvent::Destroyed);
        self.window.take();
    }

    fn show(&mut self) {
        log::trace!("WaylandWindowInner show: {:?}", self.window);
        if self.window.is_none() {
            return;
        }

        // If the do_paint function has been called previously, calling it again will not
        // send the NeedRepaint event. This results in the window not being displayed
        // correctly.
        // Therefore, when frame_callback is set to some, we need to send the NeedRepaint
        // event again to ensure the window is displayed.
        // Fix: https://github.com/wezterm/wezterm/issues/5103
        if self.frame_callback.is_some() {
            self.events.dispatch(WindowEvent::NeedRepaint);
        }

        self.do_paint().unwrap();
    }

    fn refresh_frame(&mut self) {
        if self.window_frame.is_dirty() && !self.window_frame.is_hidden() {
            self.window_frame.draw();
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
                object_id,
                self.dimensions.pixel_width as i32,
                self.dimensions.pixel_height as i32,
            )?);

            log::trace!("WEGL Surface here {:?}", wegl_surface);

            match wayland_conn.gl_connection.borrow().as_ref() {
                Some(glconn) => crate::egl::GlState::create_wayland_with_existing_connection(
                    glconn,
                    wegl_surface.as_ref().unwrap(),
                ),
                None => crate::egl::GlState::create_wayland(
                    Some(wayland_conn.connection.backend().display_ptr() as *const _),
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

    fn get_dpi_factor(&self) -> f64 {
        self.dimensions.dpi as f64 / crate::DEFAULT_DPI as f64
    }

    fn surface_to_pixels(&self, surface: i32) -> i32 {
        (surface as f64 * self.get_dpi_factor()).ceil() as i32
    }

    fn pixels_to_surface(&self, pixels: i32) -> i32 {
        // Take care to round up, otherwise we can lose a pixel
        // and that can effectively lose the final row of the
        // terminal
        ((pixels as f64) / self.get_dpi_factor()).ceil() as i32
    }

    pub(super) fn dispatch_dropped_files(&mut self, paths: Vec<PathBuf>) {
        self.events.dispatch(WindowEvent::DroppedFile(paths));
    }

    pub(crate) fn dispatch_pending_mouse(&mut self) {
        let pending_mouse = Arc::clone(&self.pending_mouse);

        if let Some((x, y)) = PendingMouse::coords(&pending_mouse) {
            let coords = Point::new(
                self.surface_to_pixels(x as i32) as isize,
                self.surface_to_pixels(y as i32) as isize,
            );
            self.last_mouse_coords = coords;
            let event = MouseEvent {
                kind: MouseEventKind::Move,
                coords,
                screen_coords: ScreenPoint::new(
                    coords.x + self.dimensions.pixel_width as isize,
                    coords.y + self.dimensions.pixel_height as isize,
                ),
                mouse_buttons: self.mouse_buttons,
                modifiers: self.modifiers,
            };
            self.events.dispatch(WindowEvent::MouseEvent(event));
            self.refresh_frame();
        }

        while let Some((button, state)) = PendingMouse::next_button(&pending_mouse) {
            let button_mask = match button {
                MousePress::Left => MouseButtons::LEFT,
                MousePress::Right => MouseButtons::RIGHT,
                MousePress::Middle => MouseButtons::MIDDLE,
            };

            if state == ButtonState::Pressed {
                self.mouse_buttons |= button_mask;
            } else {
                self.mouse_buttons -= button_mask;
            }

            let event = MouseEvent {
                kind: match state {
                    ButtonState::Pressed => MouseEventKind::Press(button),
                    ButtonState::Released => MouseEventKind::Release(button),
                    _ => continue,
                },
                coords: self.last_mouse_coords,
                screen_coords: ScreenPoint::new(
                    self.last_mouse_coords.x + self.dimensions.pixel_width as isize,
                    self.last_mouse_coords.y + self.dimensions.pixel_height as isize,
                ),
                mouse_buttons: self.mouse_buttons,
                modifiers: self.modifiers,
            };
            self.events.dispatch(WindowEvent::MouseEvent(event));
        }

        if let Some((value_x, value_y)) = PendingMouse::scroll(&pending_mouse) {
            let factor = self.get_dpi_factor() as f64;

            if value_x.signum() != self.hscroll_remainder.signum() {
                // reset accumulator when changing scroll direction
                self.hscroll_remainder = 0.0;
            }
            let scaled_x = (value_x * factor) + self.hscroll_remainder;
            let discrete_x = scaled_x.trunc();
            self.hscroll_remainder = scaled_x - discrete_x;
            if discrete_x != 0. {
                let event = MouseEvent {
                    kind: MouseEventKind::HorzWheel(-discrete_x as i16),
                    coords: self.last_mouse_coords,
                    screen_coords: ScreenPoint::new(
                        self.last_mouse_coords.x + self.dimensions.pixel_width as isize,
                        self.last_mouse_coords.y + self.dimensions.pixel_height as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.events.dispatch(WindowEvent::MouseEvent(event));
            }

            if value_y.signum() != self.vscroll_remainder.signum() {
                self.vscroll_remainder = 0.0;
            }
            let scaled_y = (value_y * factor) + self.vscroll_remainder;
            let discrete_y = scaled_y.trunc();
            self.vscroll_remainder = scaled_y - discrete_y;
            if discrete_y != 0. {
                let event = MouseEvent {
                    kind: MouseEventKind::VertWheel(-discrete_y as i16),
                    coords: self.last_mouse_coords,
                    screen_coords: ScreenPoint::new(
                        self.last_mouse_coords.x + self.dimensions.pixel_width as isize,
                        self.last_mouse_coords.y + self.dimensions.pixel_height as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.events.dispatch(WindowEvent::MouseEvent(event));
            }
        }

        if !PendingMouse::in_window(&pending_mouse) {
            self.events.dispatch(WindowEvent::MouseLeave);
            self.refresh_frame();
        }
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
                self.window_state,
                window_state
            );
            self.window_state = window_state;
        }

        if pending.configure.is_none() {
            if pending.dpi.is_some() {
                // Synthesize a pending configure event for the dpi change
                pending.configure.replace((
                    self.pixels_to_surface(self.dimensions.pixel_width as i32) as u32,
                    self.pixels_to_surface(self.dimensions.pixel_height as i32) as u32,
                ));
                log::debug!("synthesize configure with {:?}", pending.configure);
            }
        }

        if let Some(ref window_config) = pending.window_configure {
            self.window_frame.update_state(window_config.state);
            self.window_frame
                .update_wm_capabilities(window_config.capabilities);
        }

        if let Some((mut w, mut h)) = pending.configure.take() {
            log::trace!("Pending configure: w:{w}, h{h} -- {:?}", self.window);
            if self.window.is_some() {
                let surface_udata = SurfaceUserData::from_wl(self.surface());
                let factor = surface_udata.surface_data.scale_factor() as f64;
                let old_dimensions = self.dimensions;

                // FIXME: teach this how to resolve dpi_by_screen
                let dpi = self.config.dpi.unwrap_or(factor * crate::DEFAULT_DPI) as usize;

                // Do this early because this affects surface_to_pixels/pixels_to_surface
                self.dimensions.dpi = dpi;

                let mut pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                let mut pixel_height = self.surface_to_pixels(h.try_into().unwrap());

                if self.window_state.can_resize() {
                    self.window_frame.set_resizable(true);
                    if let Some(incr) = self.resize_increments {
                        let min_width = incr.base_width + incr.x;
                        let min_height = incr.base_height + incr.y;
                        let extra_width = (pixel_width - incr.base_width as i32) % incr.x as i32;
                        let extra_height = (pixel_height - incr.base_height as i32) % incr.y as i32;
                        let desired_pixel_width = max(pixel_width - extra_width, min_width as i32);
                        let desired_pixel_height =
                            max(pixel_height - extra_height, min_height as i32);
                        w = self.pixels_to_surface(desired_pixel_width) as u32;
                        h = self.pixels_to_surface(desired_pixel_height) as u32;
                        pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                        pixel_height = self.surface_to_pixels(h.try_into().unwrap());
                    }
                }

                log::trace!("Resizing frame");
                if !self.window_frame.is_hidden() {
                    // Clamp the size to at least one surface heigh/width.
                    let width = NonZeroU32::new(w).unwrap_or(NonZeroU32::new(1).unwrap());
                    let height = NonZeroU32::new(h).unwrap_or(NonZeroU32::new(1).unwrap());
                    self.window_frame.resize(width, height);
                    pending.refresh_decorations = true
                }
                let (x, y) = self.window_frame.location();
                self.window
                    .as_mut()
                    .unwrap()
                    .xdg_surface()
                    .set_window_geometry(x, y, pixel_width, pixel_height);
                // Compute the new pixel dimensions
                let new_dimensions = Dimensions {
                    pixel_width: pixel_width.try_into().unwrap(),
                    pixel_height: pixel_height.try_into().unwrap(),
                    dpi,
                };

                // Only trigger a resize if the new dimensions are different;
                // this makes things more efficient and a little more smooth
                if new_dimensions != old_dimensions {
                    self.dimensions = new_dimensions;

                    self.events.dispatch(WindowEvent::Resized {
                        dimensions: self.dimensions,
                        window_state: self.window_state,
                        // We don't know if we're live resizing or not, so
                        // assume no.
                        live_resizing: false,
                    });
                    // Avoid blurring by matching the scaling factor of the
                    // compositor; if it is going to double the size then
                    // we render at double the size anyway and tell it that
                    // the buffer is already doubled.
                    // Take care to detach the current buffer (managed by EGL),
                    // so that the compositor doesn't get annoyed by it not
                    // having dimensions that match the scale.
                    // The wegl_surface.resize won't take effect until
                    // we paint later on.
                    // We do this only if the scale has actually changed,
                    // otherwise interactive window resize will keep removing
                    // the window contents!
                    if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                        wegl_surface.resize(pixel_width, pixel_height, 0, 0);
                    }
                    if self.surface_factor != factor {
                        let wayland_conn = Connection::get().unwrap().wayland();
                        let wayland_state = wayland_conn.wayland_state.borrow();
                        let mut pool = wayland_state.mem_pool.borrow_mut();

                        // Make a "fake" buffer with the right dimensions, as
                        // simply detaching the buffer can cause wlroots-derived
                        // compositors consider the window to be unconfigured.
                        if let Ok((buffer, _bytes)) = pool.create_buffer(
                            factor as i32,
                            factor as i32,
                            (factor * 4.0) as i32,
                            wayland_client::protocol::wl_shm::Format::Argb8888,
                        ) {
                            self.surface().attach(Some(buffer.wl_buffer()), 0, 0);
                            self.surface().set_buffer_scale(factor as i32);
                            self.surface().commit();

                            self.surface_factor = factor;
                        }
                    }
                }
                self.do_paint().unwrap();
            }
        }
        if pending.refresh_decorations && self.window.is_some() {
            self.refresh_frame();
        }
        if pending.had_configure_event && self.window.is_some() {
            log::debug!("Had configured an event");
            if let Some(notify) = self.pending_first_configure.take() {
                // Allow window creation to complete
                notify.try_send(()).ok();
            }
        }
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        if !PendingMouse::in_window(&self.pending_mouse) {
            return;
        }

        let conn = Connection::get().unwrap().wayland();
        let state = conn.wayland_state.borrow_mut();
        let pointer = match &state.pointer {
            Some(pointer) => pointer,
            None => return,
        };

        match cursor {
            Some(cursor) => {
                if let Err(err) = pointer.set_cursor(
                    &conn.connection,
                    match cursor {
                        MouseCursor::Arrow => CursorIcon::Default,
                        MouseCursor::Hand => CursorIcon::Pointer,
                        MouseCursor::SizeUpDown => CursorIcon::NsResize,
                        MouseCursor::SizeLeftRight => CursorIcon::EwResize,
                        MouseCursor::Text => CursorIcon::Text,
                    },
                ) {
                    log::error!("set_cursor: {}", err);
                }
            }
            None => {
                if let Err(err) = pointer.hide_cursor() {
                    log::error!("hide_cursor: {}", err)
                }
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

    fn set_text_cursor_position(&mut self, rect: Rect) {
        let conn = WaylandConnection::get().unwrap().wayland();
        let state = conn.wayland_state.borrow();
        let surface = self.surface().clone();
        let active_surface_id = state.active_surface_id.borrow();
        let surface_id = surface.id();

        if let Some(active_surface_id) = active_surface_id.as_ref() {
            if surface_id == active_surface_id.clone() {
                if self.text_cursor.map(|prior| prior != rect).unwrap_or(true) {
                    self.text_cursor.replace(rect);

                    let surface_udata = SurfaceUserData::from_wl(&surface);
                    let factor = surface_udata.surface_data().scale_factor();

                    if let Some(text_input) = &state.text_input {
                        if let Some(input) = text_input.get_text_input_for_surface(&surface) {
                            input.set_cursor_rectangle(
                                rect.min_x() as i32 / factor,
                                rect.min_y() as i32 / factor,
                                rect.width() as i32 / factor,
                                rect.height() as i32 / factor,
                            );
                            input.commit();
                        }
                    }
                }
            }
        }
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
        self.refresh_frame();
        self.title = Some(title);
    }

    fn set_resize_increments(&mut self, incr: ResizeIncrement) -> anyhow::Result<()> {
        self.resize_increments.replace(incr);
        Ok(())
    }

    fn set_inner_size(&mut self, width: usize, height: usize) {
        let pixel_width = width as i32;
        let pixel_height = height as i32;
        let surface_width = self.pixels_to_surface(pixel_width) as u32;
        let surface_height = self.pixels_to_surface(pixel_height) as u32;
        // window.resize() doesn't generate a configure event,
        // so we're going to fake one up, otherwise the window
        // contents don't reflect the real size until eg:
        // the focus is changed.
        self.pending_event
            .lock()
            .unwrap()
            .configure
            .replace((surface_width, surface_height));
        // apply the synthetic configure event to the inner surfaces
        self.dispatch_pending_event();

        self.events.dispatch(WindowEvent::SetInnerSizeCompleted);
    }

    fn do_paint(&mut self) -> anyhow::Result<()> {
        if self.window.is_none() {
            // We're likely in the middle of closing/destroying
            // the window; we've nothing to do here.
            return Ok(());
        }

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
        let conn = WaylandConnection::get().unwrap().wayland();
        let qh = conn.event_queue.borrow().handle();

        let callback = self.surface().frame(&qh, self.surface().clone());

        log::trace!("do_paint - callback: {:?}", callback);
        self.frame_callback.replace(callback);

        // The repaint has the side of effect of committing the surface,
        // which is necessary for the frame callback to get triggered.
        // Ordering the repaint after requesting the callback ensures that
        // we will get woken at the appropriate time.
        // <https://github.com/wezterm/wezterm/issues/3468>
        // <https://github.com/wezterm/wezterm/issues/3126>
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

    pub(crate) fn emit_focus(&mut self, mapper: &mut KeyboardWithFallback, focused: bool) {
        // Clear the modifiers when we change focus, otherwise weird
        // things can happen.  For instance, if we lost focus because
        // CTRL+SHIFT+N was pressed to spawn a new window, we'd be
        // left stuck with CTRL+SHIFT held down and the window would
        // be left in a broken state.

        self.modifiers = Modifiers::NONE;
        mapper.update_modifier_state(0, 0, 0, 0);
        self.key_repeat.take();
        self.events.dispatch(WindowEvent::FocusChanged(focused));
        self.text_cursor.take();
    }

    pub(crate) fn appearance_changed(&mut self, appearance: Appearance) {
        if appearance != self.appearance {
            self.appearance = appearance;
            self.events
                .dispatch(WindowEvent::AppearanceChanged(appearance));
        }
    }

    pub(super) fn keyboard_event(
        &mut self,
        mapper: &mut KeyboardWithFallback,
        event: WlKeyboardEvent,
    ) {
        match event {
            WlKeyboardEvent::Enter { keys, .. } => {
                let key_codes = keys
                    .chunks_exact(4)
                    .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
                    .collect::<Vec<_>>();
                log::trace!("keyboard event: Enter with keys: {:?}", key_codes);
                self.emit_focus(mapper, true);
            }
            WlKeyboardEvent::Leave { .. } => {
                self.emit_focus(mapper, false);
            }
            WlKeyboardEvent::Key { key, state, .. } => {
                if let Some(event) = mapper.process_wayland_key(
                    key,
                    state.into_result().unwrap() == KeyState::Pressed,
                    &mut self.events,
                ) {
                    let rep = Arc::new(Mutex::new(KeyRepeatState {
                        when: Instant::now(),
                        event,
                    }));
                    self.key_repeat.replace((key, Arc::clone(&rep)));
                    let window_id = SurfaceUserData::from_wl(
                        self.window
                            .as_ref()
                            .expect("window should exist")
                            .wl_surface(),
                    )
                    .window_id;
                    KeyRepeatState::schedule(rep, window_id);
                } else if let Some((cur_key, _)) = self.key_repeat.as_ref() {
                    // important to check that it's the same key, because the release of the previously
                    // repeated key can come right after the press of the newly held key
                    if *cur_key == key {
                        self.key_repeat.take();
                    }
                }
            }
            WlKeyboardEvent::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                mapper.update_modifier_state(mods_depressed, mods_latched, mods_locked, group);

                let mods = mapper.get_key_modifiers();
                let leds = mapper.get_led_status();

                let changed = (mods != self.modifiers) || (leds != self.leds);

                self.modifiers = mapper.get_key_modifiers();
                self.leds = mapper.get_led_status();

                if changed {
                    self.events
                        .dispatch(WindowEvent::AdviseModifiersLedStatus(mods, leds));
                }
            }
            _ => {}
        }
    }

    pub(super) fn frame_action(&mut self, pointer: &WlPointer, serial: u32, action: FrameAction) {
        let pointer_data = pointer.data::<PointerUserData>().unwrap();
        let seat = pointer_data.pdata.seat();
        match action {
            FrameAction::Close => self.events.dispatch(WindowEvent::CloseRequested),
            FrameAction::Minimize => self.window.as_ref().unwrap().set_minimized(),
            FrameAction::Maximize => self.window.as_ref().unwrap().set_maximized(),
            FrameAction::UnMaximize => self.window.as_ref().unwrap().unset_maximized(),
            FrameAction::ShowMenu(x, y) => {
                self.window
                    .as_ref()
                    .unwrap()
                    .show_window_menu(seat, serial, (x, y))
            }
            FrameAction::Resize(edge) => {
                let edge = match edge {
                    ResizeEdge::None => XdgResizeEdge::None,
                    ResizeEdge::Top => XdgResizeEdge::Top,
                    ResizeEdge::Bottom => XdgResizeEdge::Bottom,
                    ResizeEdge::Left => XdgResizeEdge::Left,
                    ResizeEdge::TopLeft => XdgResizeEdge::TopLeft,
                    ResizeEdge::BottomLeft => XdgResizeEdge::BottomLeft,
                    ResizeEdge::Right => XdgResizeEdge::Right,
                    ResizeEdge::TopRight => XdgResizeEdge::TopRight,
                    ResizeEdge::BottomRight => XdgResizeEdge::BottomRight,
                    _ => return, // Realistically, there probably won't be any new edges added.
                };
                self.window.as_ref().unwrap().resize(seat, serial, edge)
            }
            FrameAction::Move => self.window.as_ref().unwrap().move_(seat, serial),
            _ => log::warn!("unhandled FrameAction: {:?}", action),
        }
    }

    fn maximize(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window.set_maximized();
        }
    }

    fn restore(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window.unset_maximized();
        }
    }
}

impl WaylandState {
    pub(super) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WaylandWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    fn handle_window_event(&self, window: &XdgWindow, event: WaylandWindowEvent) {
        let surface_data = SurfaceUserData::from_wl(window.wl_surface());
        let window_id = surface_data.window_id;

        let window_inner = self
            .window_by_id(window_id)
            .expect("Inner Window should exist");

        let p = window_inner.borrow().pending_event.clone();
        let mut pending_event = p.lock().unwrap();

        let changed = match event {
            WaylandWindowEvent::Close => {
                // TODO: This should the new queue function
                // p.queue_close()
                if !pending_event.close {
                    pending_event.close = true;
                    true
                } else {
                    false
                }
            }
            WaylandWindowEvent::Request(configure) => {
                pending_event.window_configure.replace(configure.clone());
                // TODO: This should the new queue function
                // p.queue_configure(&configure)
                //
                let mut changed;
                pending_event.had_configure_event = true;
                if let (Some(w), Some(h)) = configure.new_size {
                    changed = pending_event.configure.is_none();
                    pending_event.configure.replace((w.get(), h.get()));
                } else {
                    changed = true;
                }

                let mut state = WindowState::default();
                if configure.state.contains(SCTKWindowState::FULLSCREEN) {
                    state |= WindowState::FULL_SCREEN;
                }
                if configure.state.contains(SCTKWindowState::MAXIMIZED) {
                    state |= WindowState::MAXIMIZED;
                }

                log::debug!(
                    "Config: self.window_state={:?}, states: {:?} {:?}",
                    pending_event.window_state,
                    state,
                    configure.state
                );

                if pending_event.window_state.is_none() && state != WindowState::default() {
                    changed = true;
                }

                pending_event.window_state.replace(state);
                changed
            }
        };
        if changed {
            WaylandConnection::with_window_inner(window_id, move |inner| {
                inner.dispatch_pending_event();
                Ok(())
            });
        }
    }
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // We do nothing, we get the scale_factor from surface_data
    }

    fn frame(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        _time: u32,
    ) {
        log::trace!("frame: CompositorHandler");
        let surface_data = SurfaceUserData::from_wl(surface);
        let window_id = surface_data.window_id;

        WaylandConnection::with_window_inner(window_id, |inner| {
            inner.next_frame_is_ready();
            Ok(())
        });
    }

    fn transform_changed(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {
        // TODO: do we need to do anything here?
    }

    fn surface_enter(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _output: &wayland_client::protocol::wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _output: &wayland_client::protocol::wl_output::WlOutput,
    ) {
    }
}

impl WindowHandler for WaylandState {
    fn request_close(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        window: &XdgWindow,
    ) {
        self.handle_window_event(window, WaylandWindowEvent::Close);
    }

    fn configure(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        window: &XdgWindow,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        self.handle_window_event(window, WaylandWindowEvent::Request(configure));
    }
}

pub(super) struct SurfaceUserData {
    surface_data: SurfaceData,
    pub(super) window_id: usize,
}

impl SurfaceUserData {
    pub(super) fn from_wl(wl: &WlSurface) -> &Self {
        wl.data()
            .expect("User data should be associated with WlSurface")
    }
    pub(super) fn try_from_wl(wl: &WlSurface) -> Option<&SurfaceUserData> {
        wl.data()
    }
}

impl SurfaceDataExt for SurfaceUserData {
    fn surface_data(&self) -> &SurfaceData {
        &self.surface_data
    }
}

impl HasDisplayHandle for WaylandWindowInner {
    fn display_handle(&self) -> Result<DisplayHandle, HandleError> {
        let conn = WaylandConnection::get().unwrap().wayland();
        let backend = conn.connection.backend();
        let handle = backend.display_handle()?;
        Ok(unsafe { DisplayHandle::borrow_raw(handle.as_raw()) })
    }
}

impl HasWindowHandle for WaylandWindowInner {
    fn window_handle(&self) -> Result<WindowHandle, HandleError> {
        let handle = WaylandWindowHandle::new(
            NonNull::new(self.surface().id().as_ptr() as _).expect("non-null"),
        );
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle))) }
    }
}

impl HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<DisplayHandle, HandleError> {
        let conn = WaylandConnection::get().unwrap().wayland();
        let backend = conn.connection.backend();
        let handle = backend.display_handle()?;
        Ok(unsafe { DisplayHandle::borrow_raw(handle.as_raw()) })
    }
}

impl HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle, HandleError> {
        let conn = Connection::get().expect("raw_window_handle only callable on main thread");
        let handle = conn
            .wayland()
            .window_by_id(self.0)
            .expect("window handle invalid!?");

        let inner = handle.borrow();
        let handle = inner.window_handle()?;
        unsafe { Ok(WindowHandle::borrow_raw(handle.as_raw())) }
    }
}
