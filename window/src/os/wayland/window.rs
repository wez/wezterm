use super::copy_and_paste::*;
use super::frame::{ConceptConfig, ConceptFrame};
use super::pointer::*;
use crate::connection::ConnectionOps;
use crate::os::wayland::connection::WaylandConnection;
use crate::os::x11::keyboard::Keyboard;
use crate::{
    Clipboard, Connection, Dimensions, MouseCursor, Point, ScreenPoint, Window, WindowEvent,
    WindowEventReceiver, WindowEventSender, WindowOps,
};
use anyhow::{anyhow, bail, Context};
use async_io::Timer;
use async_trait::async_trait;
use config::ConfigHandle;
use filedescriptor::FileDescriptor;
use promise::{Future, Promise};
use raw_window_handle::unix::WaylandHandle;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::convert::TryInto;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use toolkit::get_surface_scale_factor;
use toolkit::reexports::client::protocol::wl_data_source::Event as DataSourceEvent;
use toolkit::reexports::client::protocol::wl_pointer::ButtonState;
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::window::{Event, State};
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeyState};
use wayland_egl::{is_available as egl_is_available, WlEglSurface};
use wezterm_font::FontConfiguration;
use wezterm_input_types::*;

#[derive(Debug)]
struct KeyRepeatState {
    when: Instant,
    key: KeyEvent,
}

impl KeyRepeatState {
    fn schedule(state: Arc<Mutex<Self>>, window_id: usize) {
        promise::spawn::spawn_into_main_thread(async move {
            let delay;
            let gap;
            {
                let conn = WaylandConnection::get().unwrap().wayland();
                delay = Duration::from_millis(*conn.key_repeat_delay.borrow() as u64);
                gap = Duration::from_millis(1000 / *conn.key_repeat_rate.borrow() as u64);
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

                    let inner = handle.borrow();

                    if inner.key_repeat.as_ref().map(|k| Arc::as_ptr(k))
                        != Some(Arc::as_ptr(&state))
                    {
                        // Key was released and/or some other key is doing
                        // its own repetition now
                        return;
                    }

                    let mut st = state.lock().unwrap();
                    let mut event = st.key.clone();

                    event.repeat_count = 1;

                    let mut elapsed = st.when.elapsed();
                    if initial {
                        elapsed -= delay;
                        initial = false;
                    }

                    // If our scheduling interval is longer than the repeat
                    // gap, we need to inflate the repeat count to match
                    // the intended rate
                    while elapsed >= gap {
                        event.repeat_count += 1;
                        elapsed -= gap;
                    }
                    inner.events.try_send(WindowEvent::KeyEvent(event)).ok();

                    st.when = Instant::now();
                }

                Timer::after(gap).await;
            }
        })
        .detach();
    }
}

pub struct WaylandWindowInner {
    window_id: usize,
    events: WindowEventSender,
    surface: WlSurface,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<toolkit::window::Window<ConceptFrame>>,
    dimensions: Dimensions,
    full_screen: bool,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    modifiers: Modifiers,
    key_repeat: Option<Arc<Mutex<KeyRepeatState>>>,
    pending_event: Arc<Mutex<PendingEvent>>,
    pending_mouse: Arc<Mutex<PendingMouse>>,
    pending_first_configure: Option<async_channel::Sender<()>>,
    // wegl_surface is listed before gl_state because it
    // must be dropped before gl_state otherwise the underlying
    // libraries will segfault on shutdown
    wegl_surface: Option<WlEglSurface>,
    gl_state: Option<Rc<glium::backend::Context>>,
}

#[derive(Default, Clone, Debug)]
struct PendingEvent {
    close: bool,
    had_configure_event: bool,
    refresh_decorations: bool,
    configure: Option<(u32, u32)>,
    dpi: Option<i32>,
    full_screen: Option<bool>,
}

impl PendingEvent {
    fn queue(&mut self, evt: Event) -> bool {
        match evt {
            Event::Close => {
                if !self.close {
                    self.close = true;
                    true
                } else {
                    false
                }
            }
            Event::Refresh => {
                if !self.refresh_decorations {
                    self.refresh_decorations = true;
                    true
                } else {
                    false
                }
            }
            Event::Configure { new_size, states } => {
                let mut changed;
                self.had_configure_event = true;
                if let Some(new_size) = new_size {
                    changed = self.configure.is_none();
                    self.configure.replace(new_size);
                } else {
                    changed = true;
                }
                let full_screen = states.contains(&State::Fullscreen);
                log::debug!(
                    "Config: self.full_screen={:?}, states:{:?} {:?}",
                    self.full_screen,
                    full_screen,
                    states
                );
                match (self.full_screen, full_screen) {
                    (None, false) => {}
                    _ => {
                        self.full_screen.replace(full_screen);
                        changed = true;
                    }
                }
                changed
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
    pub async fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        config: Option<&ConfigHandle>,
        font_config: Rc<FontConfiguration>,
    ) -> anyhow::Result<(Window, WindowEventReceiver)> {
        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                anyhow!(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .wayland();

        let window_id = conn.next_window_id();
        let pending_event = Arc::new(Mutex::new(PendingEvent::default()));
        let (events, receiver) = async_channel::unbounded();

        let (pending_first_configure, wait_configure) = async_channel::bounded(1);

        let surface = conn
            .environment
            .borrow_mut()
            .create_surface_with_scale_callback({
                let pending_event = Arc::clone(&pending_event);
                move |dpi, surface, _dispatch_data| {
                    pending_event.lock().unwrap().dpi.replace(dpi);
                    log::debug!(
                        "surface id={} dpi scale changed to {}",
                        surface.as_ref().id(),
                        dpi
                    );
                    WaylandConnection::with_window_inner(window_id, move |inner| {
                        inner.dispatch_pending_event();
                        Ok(())
                    });
                }
            });
        conn.surface_to_window_id
            .borrow_mut()
            .insert(surface.as_ref().id(), window_id);

        let dimensions = Dimensions {
            pixel_width: width,
            pixel_height: height,
            dpi: crate::DEFAULT_DPI as usize,
        };

        let theme_manager = None;

        let mut window = conn
            .environment
            .borrow()
            .create_window::<ConceptFrame, _>(
                surface.clone().detach(),
                theme_manager,
                (
                    dimensions.pixel_width as u32,
                    dimensions.pixel_height as u32,
                ),
                {
                    let pending_event = Arc::clone(&pending_event);
                    move |evt, mut _dispatch_data| {
                        if pending_event.lock().unwrap().queue(evt) {
                            WaylandConnection::with_window_inner(window_id, move |inner| {
                                inner.dispatch_pending_event();
                                Ok(())
                            });
                        }
                    }
                },
            )
            .context("Failed to create window")?;

        window.set_app_id(class_name.to_string());
        window.set_resizable(true);
        window.set_title(name.to_string());
        window.set_frame_config(ConceptConfig {
            font_config: Some(font_config),
            config: config.cloned(),
            ..Default::default()
        });

        window.set_min_size(Some((32, 32)));

        let copy_and_paste = CopyAndPaste::create();
        let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        conn.pointer.add_window(&surface, &pending_mouse);

        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            window_id,
            key_repeat: None,
            copy_and_paste,
            events,
            surface: surface.detach(),
            window: Some(window),
            dimensions,
            full_screen: false,
            last_mouse_coords: Point::new(0, 0),
            mouse_buttons: MouseButtons::NONE,
            modifiers: Modifiers::NONE,
            pending_event,
            pending_mouse,
            pending_first_configure: Some(pending_first_configure),
            gl_state: None,
            wegl_surface: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        wait_configure.recv().await?;

        Ok((window_handle, receiver))
    }
}

unsafe impl HasRawWindowHandle for WaylandWindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let conn = WaylandConnection::get().unwrap().wayland();
        let display = conn.display.borrow();
        RawWindowHandle::Wayland(WaylandHandle {
            surface: self.surface.as_ref().c_ptr() as *mut _,
            display: display.c_ptr() as *mut _,
            ..WaylandHandle::empty()
        })
    }
}

impl WaylandWindowInner {
    pub(crate) fn keyboard_event(&mut self, event: WlKeyboardEvent) {
        let conn = WaylandConnection::get().unwrap().wayland();
        let mut mapper = conn.keyboard_mapper.borrow_mut();
        let mapper = mapper.as_mut().expect("no keymap");

        match event {
            WlKeyboardEvent::Enter { keys, .. } => {
                // Keys is bytes, but is really u32 keysyms
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
                if let Some(event) = mapper.process_wayland_key(key, state == KeyState::Pressed) {
                    if event.key_is_down && mapper.wayland_key_repeats(key) {
                        let rep = Arc::new(Mutex::new(KeyRepeatState {
                            when: Instant::now(),
                            key: event.clone(),
                        }));
                        self.key_repeat.replace(Arc::clone(&rep));
                        KeyRepeatState::schedule(rep, self.window_id);
                    } else {
                        self.key_repeat.take();
                    }
                    self.events.try_send(WindowEvent::KeyEvent(event)).ok();
                } else {
                    self.key_repeat.take();
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
            }
            _ => {}
        }
    }

    fn emit_focus(&mut self, mapper: &mut Keyboard, focused: bool) {
        // Clear the modifiers when we change focus, otherwise weird
        // things can happen.  For instance, if we lost focus because
        // CTRL+SHIFT+N was pressed to spawn a new window, we'd be
        // left stuck with CTRL+SHIFT held down and the window would
        // be left in a broken state.

        self.modifiers = Modifiers::NONE;
        mapper.update_modifier_state(0, 0, 0, 0);
        self.key_repeat.take();
        self.events
            .try_send(WindowEvent::FocusChanged(focused))
            .ok();
    }

    pub(crate) fn dispatch_pending_mouse(&mut self) {
        // Dancing around the borrow checker and the call to self.refresh_frame()
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
            self.events.try_send(WindowEvent::MouseEvent(event)).ok();
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
            self.events.try_send(WindowEvent::MouseEvent(event)).ok();
        }

        if let Some((value_x, value_y)) = PendingMouse::scroll(&pending_mouse) {
            let factor = self.get_dpi_factor() as f64;
            let discrete_x = value_x.trunc() * factor;
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
                self.events.try_send(WindowEvent::MouseEvent(event)).ok();
            }

            let discrete_y = value_y.trunc() * factor;
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
                self.events.try_send(WindowEvent::MouseEvent(event)).ok();
            }
        }
    }

    fn get_dpi_factor(&self) -> i32 {
        self.dimensions.dpi as i32 / crate::DEFAULT_DPI as i32
    }

    fn surface_to_pixels(&self, surface: i32) -> i32 {
        surface * self.get_dpi_factor()
    }

    fn pixels_to_surface(&self, pixels: i32) -> i32 {
        // Take care to round up, otherwise we can lose a pixel
        // and that can effectively lose the final row of the
        // terminal
        ((pixels as f64) / (self.get_dpi_factor() as f64)).ceil() as i32
    }

    fn dispatch_pending_event(&mut self) {
        let mut pending;
        {
            let mut pending_events = self.pending_event.lock().unwrap();
            pending = pending_events.clone();
            *pending_events = PendingEvent::default();
        }
        if pending.close {
            if self.events.try_send(WindowEvent::CloseRequested).is_err() {
                self.window.take();
            }
        }

        if let Some(full_screen) = pending.full_screen.take() {
            log::debug!(
                "dispatch_pending_event self.full_screen={} pending:{}",
                self.full_screen,
                full_screen
            );
            self.full_screen = full_screen;
        }

        if pending.configure.is_none() && pending.dpi.is_some() {
            // Synthesize a pending configure event for the dpi change
            pending.configure.replace((
                self.pixels_to_surface(self.dimensions.pixel_width as i32) as u32,
                self.pixels_to_surface(self.dimensions.pixel_height as i32) as u32,
            ));
            log::debug!("synthesize configure with {:?}", pending.configure);
        }

        if let Some((w, h)) = pending.configure.take() {
            if self.window.is_some() {
                let factor = get_surface_scale_factor(&self.surface);

                let pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                let pixel_height = self.surface_to_pixels(h.try_into().unwrap());

                // Avoid blurring by matching the scaling factor of the
                // compositor; if it is going to double the size then
                // we render at double the size anyway and tell it that
                // the buffer is already doubled
                self.surface.set_buffer_scale(factor);

                // Update the window decoration size
                self.window.as_mut().unwrap().resize(w, h);

                // Compute the new pixel dimensions
                let new_dimensions = Dimensions {
                    pixel_width: pixel_width.try_into().unwrap(),
                    pixel_height: pixel_height.try_into().unwrap(),
                    dpi: factor as usize * crate::DEFAULT_DPI as usize,
                };
                // Only trigger a resize if the new dimensions are different;
                // this makes things more efficient and a little more smooth
                if new_dimensions != self.dimensions {
                    self.dimensions = new_dimensions;

                    self.events
                        .try_send(WindowEvent::Resized {
                            dimensions: self.dimensions,
                            is_full_screen: self.full_screen,
                        })
                        .ok();
                    if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                        wegl_surface.resize(pixel_width, pixel_height, 0, 0);
                    }
                }

                self.refresh_frame();
                self.do_paint().unwrap();
            }
        }
        if pending.refresh_decorations && self.window.is_some() {
            self.refresh_frame();
        }
        if pending.had_configure_event && self.window.is_some() {
            if let Some(notify) = self.pending_first_configure.take() {
                // Allow window creation to complete
                notify.try_send(()).ok();
            }
        }
    }

    fn refresh_frame(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window.refresh();
            window.surface().commit();
        }
    }

    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let wayland_conn = Connection::get().unwrap().wayland();
        let mut wegl_surface = None;

        let gl_state = if !egl_is_available() {
            Err(anyhow!("!egl_is_available"))
        } else {
            wegl_surface = Some(WlEglSurface::new(
                &self.surface,
                self.dimensions.pixel_width as i32,
                self.dimensions.pixel_height as i32,
            ));

            match wayland_conn.gl_connection.borrow().as_ref() {
                Some(glconn) => crate::egl::GlState::create_wayland_with_existing_connection(
                    glconn,
                    wegl_surface.as_ref().unwrap(),
                ),
                None => crate::egl::GlState::create_wayland(
                    Some(wayland_conn.display.borrow().get_display_ptr() as *const _),
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

    fn do_paint(&mut self) -> anyhow::Result<()> {
        self.events.try_send(WindowEvent::NeedRepaint).ok();
        Ok(())
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

#[async_trait(?Send)]
impl WindowOps for WaylandWindow {
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

    fn finish_frame(&self, frame: glium::Frame) -> anyhow::Result<()> {
        frame.finish()?;
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.refresh_frame();
            Ok(())
        });
        Ok(())
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        // If we're already on the correct thread, just queue it up
        if let Some(conn) = Connection::get() {
            let handle = match conn.wayland().window_by_id(self.0) {
                Some(h) => h,
                None => return,
            };
            let inner = handle.borrow();
            inner
                .events
                .try_send(WindowEvent::Notification(Box::new(t)))
                .ok();
        } else {
            // Otherwise, get into that thread and write to the queue
            WaylandConnection::with_window_inner(self.0, move |inner| {
                inner
                    .events
                    .try_send(WindowEvent::Notification(Box::new(t)))
                    .ok();
                Ok(())
            });
        }
    }

    fn close(&self) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn hide(&self) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        })
    }

    fn toggle_fullscreen(&self) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.toggle_fullscreen();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        })
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<Dimensions> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            Ok(inner.set_inner_size(width, height))
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn get_clipboard(&self, _clipboard: Clipboard) -> Future<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let promise = Arc::new(Mutex::new(promise));
        WaylandConnection::with_window_inner(self.0, move |inner| {
            let read = inner.copy_and_paste.lock().unwrap().get_clipboard_data()?;
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

    fn set_clipboard(&self, _clipboard: Clipboard, text: String) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            let text = text.clone();
            let conn = Connection::get().unwrap().wayland();

            let source = conn
                .environment
                .borrow()
                .require_global::<WlDataDeviceManager>()
                .create_data_source();
            source.quick_assign(move |_source, event, _dispatch_data| {
                if let DataSourceEvent::Send { fd, .. } = event {
                    let fd = unsafe { FileDescriptor::from_raw_fd(fd) };
                    if let Err(e) = write_pipe_with_timeout(fd, text.as_bytes()) {
                        log::error!("while sending paste to pipe: {}", e);
                    }
                }
            });
            source.offer(TEXT_MIME_TYPE.to_string());
            inner.copy_and_paste.lock().unwrap().set_selection(&source);

            Ok(())
        })
    }
}

fn write_pipe_with_timeout(mut file: FileDescriptor, data: &[u8]) -> anyhow::Result<()> {
    file.set_non_blocking(true)?;
    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };

    let mut buf = data;

    while !buf.is_empty() {
        if unsafe { libc::poll(&mut pfd, 1, 3000) == 1 } {
            match file.write(buf) {
                Ok(size) if size == 0 => {
                    bail!("zero byte write");
                }
                Ok(size) => {
                    buf = &buf[size..];
                }
                Err(e) => bail!("error writing to pipe: {}", e),
            }
        } else {
            bail!("timed out writing to pipe");
        }
    }

    Ok(())
}

fn read_pipe_with_timeout(mut file: FileDescriptor) -> anyhow::Result<String> {
    let mut result = Vec::new();

    file.set_non_blocking(true)?;
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

impl WaylandWindowInner {
    fn close(&mut self) {
        self.events.try_send(WindowEvent::Destroyed).ok();
        self.window.take();
    }

    fn hide(&mut self) {
        if let Some(window) = self.window.as_ref() {
            window.set_minimized();
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = self.window.as_ref() {
            if self.full_screen {
                window.unset_fullscreen();
            } else {
                window.set_fullscreen(None);
            }
        }
    }

    fn show(&mut self) {
        if self.window.is_none() {
            return;
        }
        // The window won't be visible until we've done our first paint,
        // so we unconditionally queue a NeedRepaint event
        self.do_paint().unwrap();
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        let cursor = match cursor {
            Some(MouseCursor::Arrow) => "arrow",
            Some(MouseCursor::Hand) => "hand",
            Some(MouseCursor::SizeUpDown) => "ns-resize",
            Some(MouseCursor::SizeLeftRight) => "ew-resize",
            Some(MouseCursor::Text) => "text",
            None => return,
        };
        let conn = Connection::get().unwrap().wayland();
        conn.pointer.set_cursor(cursor, None);
    }

    fn invalidate(&mut self) {
        self.do_paint().unwrap();
    }

    fn set_inner_size(&mut self, width: usize, height: usize) -> Dimensions {
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

        // and update the window decorations
        if let Some(window) = self.window.as_mut() {
            window.resize(surface_width, surface_height);
            // The resize must be followed by a refresh call.
            window.refresh();
            // In addition, resize doesn't take effect until
            // the suface is commited
            window.surface().commit();
        }

        let factor = get_surface_scale_factor(&self.surface);
        Dimensions {
            pixel_width: pixel_width as _,
            pixel_height: pixel_height as _,
            dpi: factor as usize * crate::DEFAULT_DPI as usize,
        }
    }

    fn set_window_position(&self, _coords: ScreenPoint) {}

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        if let Some(window) = self.window.as_ref() {
            window.set_title(title.to_string());
        }
        self.refresh_frame();
    }
}
