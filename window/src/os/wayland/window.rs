use super::copy_and_paste::*;
use super::frame::{ConceptConfig, ConceptFrame};
use super::pointer::*;
use crate::connection::ConnectionOps;
use crate::os::wayland::connection::WaylandConnection;
use crate::os::wayland::wl_id;
use crate::os::x11::keyboard::Keyboard;
use crate::{
    Appearance, Clipboard, Connection, Dimensions, MouseCursor, Point, Rect,
    RequestedWindowGeometry, ResolvedGeometry, ScreenPoint, Window, WindowEvent, WindowEventSender,
    WindowKeyEvent, WindowOps, WindowState,
};
use anyhow::{anyhow, bail, Context};
use async_io::Timer;
use async_trait::async_trait;
use config::ConfigHandle;
use filedescriptor::FileDescriptor;
use promise::{Future, Promise};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, WaylandWindowHandle};
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::convert::TryInto;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use toolkit::get_surface_scale_factor;
use toolkit::reexports::client::protocol::wl_pointer::ButtonState;
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::window::{Decorations, Event as SCTKWindowEvent, State};
use wayland_client::protocol::wl_callback::WlCallback;
use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeyState};
use wayland_client::{Attached, Main};
use wayland_egl::{is_available as egl_is_available, WlEglSurface};
use wezterm_font::FontConfiguration;
use wezterm_input_types::*;

#[derive(Debug)]
struct KeyRepeatState {
    when: Instant,
    event: WindowKeyEvent,
}

impl KeyRepeatState {
    fn schedule(state: Arc<Mutex<Self>>, window_id: usize) {
        promise::spawn::spawn_into_main_thread(async move {
            let delay;
            let gap;
            {
                let conn = WaylandConnection::get().unwrap().wayland();
                let rate = *conn.key_repeat_rate.borrow() as u64;
                if rate == 0 {
                    return;
                }
                delay = Duration::from_millis(*conn.key_repeat_delay.borrow() as u64);
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

pub struct WaylandWindowInner {
    window_id: usize,
    pub(crate) events: WindowEventSender,
    surface: Attached<WlSurface>,
    surface_factor: i32,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<toolkit::window::Window<ConceptFrame>>,
    dimensions: Dimensions,
    resize_increments: Option<(u16, u16)>,
    window_state: WindowState,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    hscroll_remainder: f64,
    vscroll_remainder: f64,
    modifiers: Modifiers,
    key_repeat: Option<(u32, Arc<Mutex<KeyRepeatState>>)>,
    pending_event: Arc<Mutex<PendingEvent>>,
    pending_mouse: Arc<Mutex<PendingMouse>>,
    pending_first_configure: Option<async_channel::Sender<()>>,
    frame_callback: Option<Main<WlCallback>>,
    invalidated: bool,
    font_config: Rc<FontConfiguration>,
    text_cursor: Option<Rect>,
    appearance: Appearance,
    config: Option<ConfigHandle>,
    // cache the title for comparison to avoid spamming
    // the compositor with updates that don't actually change it
    title: Option<String>,
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
    window_state: Option<WindowState>,
}

impl PendingEvent {
    fn queue(&mut self, evt: SCTKWindowEvent) -> bool {
        match evt {
            SCTKWindowEvent::Close => {
                if !self.close {
                    self.close = true;
                    true
                } else {
                    false
                }
            }
            SCTKWindowEvent::Refresh => {
                if !self.refresh_decorations {
                    self.refresh_decorations = true;
                    true
                } else {
                    false
                }
            }
            SCTKWindowEvent::Configure { new_size, states } => {
                let mut changed;
                self.had_configure_event = true;
                if let Some(new_size) = new_size {
                    changed = self.configure.is_none();
                    self.configure.replace(new_size);
                } else {
                    changed = true;
                }
                let mut state = WindowState::default();
                for s in &states {
                    match s {
                        State::Fullscreen => {
                            state |= WindowState::FULL_SCREEN;
                        }
                        State::Maximized
                        | State::TiledLeft
                        | State::TiledRight
                        | State::TiledTop
                        | State::TiledBottom => {
                            state |= WindowState::MAXIMIZED;
                        }
                        _ => {}
                    }
                }
                log::debug!(
                    "Config: self.window_state={:?}, states:{:?} {:?}",
                    self.window_state,
                    state,
                    states
                );
                if self.window_state.is_none() && state != WindowState::default() {
                    changed = true;
                }
                // Always set it to avoid losing non-default -> default transitions
                self.window_state.replace(state);
                changed
            }
        }
    }
}

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
        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                anyhow!(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .wayland();

        let window_id = conn.next_window_id();
        let pending_event = Arc::new(Mutex::new(PendingEvent::default()));

        let (pending_first_configure, wait_configure) = async_channel::bounded(1);

        let surface = conn.environment.create_surface_with_scale_callback({
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

        let ResolvedGeometry {
            x: _,
            y: _,
            width,
            height,
        } = conn.resolve_geometry(geometry);

        let dimensions = Dimensions {
            pixel_width: width,
            pixel_height: height,
            dpi: crate::DEFAULT_DPI as usize,
        };

        let theme_manager = None;

        let mut window = conn
            .environment
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
        let decorations = config
            .as_ref()
            .map(|c| c.window_decorations)
            .unwrap_or(WindowDecorations::default());

        window.set_decorate(if decorations == WindowDecorations::NONE {
            Decorations::None
        } else if decorations == WindowDecorations::default() {
            Decorations::FollowServer
        } else {
            // SCTK/Wayland don't allow more nuance than "decorations are hidden",
            // so if we have a mixture of things, then we need to force our
            // client side decoration rendering.
            Decorations::ClientSide
        });

        window.set_frame_config(ConceptConfig {
            font_config: Some(Rc::clone(&font_config)),
            config: config.cloned(),
            ..Default::default()
        });

        window.set_min_size(Some((32, 32)));

        let copy_and_paste = CopyAndPaste::create();
        let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        conn.pointer.borrow().add_window(&surface, &pending_mouse);

        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            window_id,
            font_config,
            config: config.cloned(),
            key_repeat: None,
            copy_and_paste,
            events: WindowEventSender::new(event_handler),
            surface,
            surface_factor: 1,
            invalidated: false,
            window: Some(window),
            dimensions,
            resize_increments: None,
            window_state: WindowState::default(),
            last_mouse_coords: Point::new(0, 0),
            mouse_buttons: MouseButtons::NONE,
            hscroll_remainder: 0.0,
            vscroll_remainder: 0.0,
            modifiers: Modifiers::NONE,
            pending_event,
            pending_mouse,
            pending_first_configure: Some(pending_first_configure),
            frame_callback: None,
            title: None,
            gl_state: None,
            wegl_surface: None,
            text_cursor: None,
            appearance: Appearance::Light,
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

unsafe impl HasRawWindowHandle for WaylandWindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = WaylandWindowHandle::empty();
        handle.surface = self.surface.as_ref().c_ptr() as *mut _;
        RawWindowHandle::Wayland(handle)
    }
}

impl WaylandWindowInner {
    pub(crate) fn appearance_changed(&mut self, appearance: Appearance) {
        if appearance != self.appearance {
            self.appearance = appearance;
            self.events
                .dispatch(WindowEvent::AppearanceChanged(appearance));
        }
    }

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
                if let Some(event) =
                    mapper.process_wayland_key(key, state == KeyState::Pressed, &mut self.events)
                {
                    let rep = Arc::new(Mutex::new(KeyRepeatState {
                        when: Instant::now(),
                        event,
                    }));
                    self.key_repeat.replace((key, Arc::clone(&rep)));
                    KeyRepeatState::schedule(rep, self.window_id);
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
                self.modifiers = mapper.get_key_modifiers();
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
        self.events.dispatch(WindowEvent::FocusChanged(focused));
        self.text_cursor.take();
    }

    pub(crate) fn dispatch_dropped_files(&mut self, paths: Vec<PathBuf>) {
        self.events.dispatch(WindowEvent::DroppedFile(paths));
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
            self.events.dispatch(WindowEvent::CloseRequested);
        }

        if let Some(window_state) = pending.window_state.take() {
            log::debug!(
                "dispatch_pending_event self.window_state={:?} pending:{:?}",
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

        if let Some((mut w, mut h)) = pending.configure.take() {
            if self.window.is_some() {
                let factor = get_surface_scale_factor(&self.surface);

                // Do this early because this affects surface_to_pixels/pixels_to_surface below!
                self.dimensions.dpi = factor as usize * crate::DEFAULT_DPI as usize;

                let mut pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                let mut pixel_height = self.surface_to_pixels(h.try_into().unwrap());

                if self.window_state.can_resize() {
                    if let Some((x, y)) = self.resize_increments {
                        let desired_pixel_width = pixel_width - (pixel_width % x as i32);
                        let desired_pixel_height = pixel_height - (pixel_height % y as i32);
                        w = self.pixels_to_surface(desired_pixel_width) as u32;
                        h = self.pixels_to_surface(desired_pixel_height) as u32;
                        pixel_width = self.surface_to_pixels(w.try_into().unwrap());
                        pixel_height = self.surface_to_pixels(h.try_into().unwrap());
                    }
                }

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

                    self.events.dispatch(WindowEvent::Resized {
                        dimensions: self.dimensions,
                        window_state: self.window_state,
                        // We don't know if we're live resizing or not, so
                        // assume no.
                        live_resizing: false,
                    });
                    if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                        wegl_surface.resize(pixel_width, pixel_height, 0, 0);
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
                        if self.surface_factor != factor {
                            let wayland_conn = Connection::get().unwrap().wayland();
                            let mut pool = wayland_conn.mem_pool.borrow_mut();
                            // Make a "fake" buffer with the right dimensions, as
                            // simply detaching the buffer can cause wlroots-derived
                            // compositors consider the window to be unconfigured.
                            if let Ok((_bytes, buffer)) = pool.buffer(
                                factor,
                                factor,
                                factor * 4,
                                wayland_client::protocol::wl_shm::Format::Argb8888,
                            ) {
                                self.surface.attach(Some(&buffer), 0, 0);
                                self.surface.set_buffer_scale(factor);
                                self.surface_factor = factor;
                            }
                        }
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

    fn next_frame_is_ready(&mut self) {
        self.frame_callback.take();
        if self.invalidated {
            self.do_paint().ok();
        }
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
        self.events.dispatch(WindowEvent::NeedRepaint);

        // Ask the compositor to wake us up when its time to paint
        // the next frame
        let window_id = self.window_id;
        let callback = self.surface.frame();
        callback.quick_assign(move |_source, _event, _data| {
            WaylandConnection::with_window_inner(window_id, |inner| {
                inner.next_frame_is_ready();
                Ok(())
            });
        });
        self.frame_callback.replace(callback);

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
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner
                .events
                .dispatch(WindowEvent::Notification(Box::new(t)));
            Ok(())
        });
    }

    fn close(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        });
    }

    fn hide(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        });
    }

    fn toggle_fullscreen(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.toggle_fullscreen();
            Ok(())
        });
    }

    fn config_did_change(&self, config: &ConfigHandle) {
        let config = config.clone();
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.config_did_change(&config);
            Ok(())
        });
    }

    fn show(&self) {
        WaylandConnection::with_window_inner(self.0, |inner| {
            inner.show();
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
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_title(title);
            Ok(())
        });
    }

    fn maximize(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| Ok(inner.maximize()));
    }

    fn restore(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| Ok(inner.restore()));
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            Ok(inner.set_inner_size(width, height))
        });
    }

    fn request_drag_move(&self) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.request_drag_move();
            Ok(())
        });
    }

    fn set_resize_increments(&self, x: u16, y: u16) {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            Ok(inner.set_resize_increments(x, y))
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
}

pub(crate) fn read_pipe_with_timeout(mut file: FileDescriptor) -> anyhow::Result<String> {
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
        self.events.dispatch(WindowEvent::Destroyed);
        self.window.take();
    }

    fn hide(&mut self) {
        if let Some(window) = self.window.as_ref() {
            window.set_minimized();
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = self.window.as_ref() {
            if self.window_state.contains(WindowState::FULL_SCREEN) {
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
            Some(MouseCursor::Text) => "xterm",
            None => return,
        };
        let conn = Connection::get().unwrap().wayland();
        conn.pointer.borrow().set_cursor(cursor, None);
    }

    fn invalidate(&mut self) {
        if self.frame_callback.is_some() {
            self.invalidated = true;
            return;
        }
        self.do_paint().unwrap();
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

    fn request_drag_move(&self) {
        if let Some(window) = self.window.as_ref() {
            let serial = self.copy_and_paste.lock().unwrap().last_serial;
            let conn = Connection::get().unwrap().wayland();
            window.start_interactive_move(&conn.pointer.borrow().seat, serial);
        }
    }

    fn set_text_cursor_position(&mut self, rect: Rect) {
        let surface_id = wl_id(&*self.surface);
        let conn = Connection::get().unwrap().wayland();
        if surface_id == *conn.active_surface_id.borrow() {
            if self.text_cursor.map(|prior| prior != rect).unwrap_or(true) {
                self.text_cursor.replace(rect);
                let factor = get_surface_scale_factor(&self.surface);

                conn.environment.with_inner(|env| {
                    if let Some(input) = env
                        .input_handler()
                        .get_text_input_for_surface(&self.surface)
                    {
                        input.set_cursor_rectangle(
                            rect.min_x() as i32 / factor,
                            rect.min_y() as i32 / factor,
                            rect.width() as i32 / factor,
                            rect.height() as i32 / factor,
                        );
                        input.commit();
                    }
                });
            }
        }
    }

    /// Change the title for the window manager
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

    fn set_resize_increments(&mut self, x: u16, y: u16) {
        self.resize_increments = Some((x, y));
    }

    fn config_did_change(&mut self, config: &ConfigHandle) {
        self.config.replace(config.clone());
        if let Some(window) = self.window.as_mut() {
            window.set_frame_config(ConceptConfig {
                font_config: Some(Rc::clone(&self.font_config)),
                config: Some(config.clone()),
                ..Default::default()
            });
            // I tried re-applying the config to window.set_decorate
            // here, but it crashed weston.  I figure that users
            // would prefer to manually close wezterm to change
            // this setting!
        }
    }
}
