use super::copy_and_paste::*;
use super::keyboard::KeyboardEvent;
use super::pointer::*;
use crate::bitmaps::BitmapImage;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::os::wayland::connection::WaylandConnection;
use crate::os::xkeysyms::keysym_to_keycode;
use crate::{
    Clipboard, Connection, Dimensions, MouseCursor, Operator, PaintContext, Point, Rect,
    ScreenPoint, Window, WindowCallbacks, WindowOps, WindowOpsMut,
};
use anyhow::{anyhow, bail, Context};
use filedescriptor::FileDescriptor;
use promise::{Future, Promise};
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::convert::TryInto;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use toolkit::get_surface_scale_factor;
use toolkit::reexports::client::protocol::wl_data_source::Event as DataSourceEvent;
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::shm::MemPool;
use toolkit::window::{ButtonColorSpec, ColorSpec, ConceptConfig, ConceptFrame, Event};
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_egl::{is_available as egl_is_available, WlEglSurface};
use wezterm_input_types::*;

const DARK_GRAY: [u8; 4] = [0xff, 0x35, 0x35, 0x35];
const DARK_PURPLE: [u8; 4] = [0xff, 0x2b, 0x20, 0x42];
const PURPLE: [u8; 4] = [0xff, 0x3b, 0x30, 0x52];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const SILVER: [u8; 4] = [0xcc, 0xcc, 0xcc, 0xcc];

fn frame_config() -> ConceptConfig {
    let icon = ButtonColorSpec {
        hovered: ColorSpec::identical(WHITE.into()),
        idle: ColorSpec {
            active: PURPLE.into(),
            inactive: SILVER.into(),
        },
        disabled: ColorSpec::invisible(),
    };

    let close = Some((
        icon,
        ButtonColorSpec {
            hovered: ColorSpec::identical(PURPLE.into()),
            idle: ColorSpec {
                active: DARK_PURPLE.into(),
                inactive: DARK_GRAY.into(),
            },
            disabled: ColorSpec::invisible(),
        },
    ));

    ConceptConfig {
        primary_color: ColorSpec {
            active: DARK_PURPLE.into(),
            inactive: DARK_GRAY.into(),
        },

        secondary_color: ColorSpec {
            active: DARK_PURPLE.into(),
            inactive: DARK_GRAY.into(),
        },

        close_button: close,
        maximize_button: close,
        minimize_button: close,
        title_font: Some(("sans".into(), 17.0)),
        title_color: ColorSpec {
            active: WHITE.into(),
            inactive: SILVER.into(),
        },
    }
}

pub struct WaylandWindowInner {
    window_id: usize,
    callbacks: Box<dyn WindowCallbacks>,
    surface: WlSurface,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<toolkit::window::Window<ConceptFrame>>,
    pool: MemPool,
    dimensions: Dimensions,
    need_paint: bool,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    modifiers: Modifiers,
    pending_event: Arc<Mutex<PendingEvent>>,
    pending_mouse: Arc<Mutex<PendingMouse>>,
    // wegl_surface is listed before gl_state because it
    // must be dropped before gl_state otherwise the underlying
    // libraries will segfault on shutdown
    wegl_surface: Option<WlEglSurface>,
    gl_state: Option<Rc<glium::backend::Context>>,
}

#[derive(Default, Clone, Debug)]
struct PendingEvent {
    close: bool,
    refresh_decorations: bool,
    configure: Option<(u32, u32)>,
    dpi: Option<i32>,
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
            Event::Configure { new_size, .. } => {
                let changed;
                if let Some(new_size) = new_size {
                    changed = self.configure.is_none();
                    self.configure.replace(new_size);
                } else {
                    changed = !self.refresh_decorations;
                    self.refresh_decorations = true;
                }
                changed
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> anyhow::Result<Window> {
        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                anyhow!(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .wayland();

        let window_id = conn.next_window_id();
        let pending_event = Arc::new(Mutex::new(PendingEvent::default()));

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

        let dimensions = Dimensions {
            pixel_width: width,
            pixel_height: height,
            dpi: crate::DEFAULT_DPI as usize,
        };

        let mut window = conn
            .environment
            .borrow()
            .create_window::<ConceptFrame, _>(
                surface.clone().detach(),
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
        window.set_frame_config(frame_config());

        let pool = MemPool::new(conn.environment.borrow().require_global(), |_| {})?;

        // window.new_seat(&conn.seat);
        conn.keyboard.add_window(window_id, &surface);

        let copy_and_paste = CopyAndPaste::create();
        let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        conn.pointer.add_window(&surface, &pending_mouse);

        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            copy_and_paste,
            window_id,
            callbacks,
            surface: surface.detach(),
            window: Some(window),
            pool,
            dimensions,
            need_paint: true,
            last_mouse_coords: Point::new(0, 0),
            mouse_buttons: MouseButtons::NONE,
            modifiers: Modifiers::NONE,
            pending_event,
            pending_mouse,
            gl_state: None,
            wegl_surface: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        inner.borrow_mut().enable_opengl()?;

        Ok(window_handle)
    }
}

impl WaylandWindowInner {
    pub(crate) fn handle_keyboard_event(&mut self, evt: KeyboardEvent) {
        match evt {
            KeyboardEvent::Key {
                keysym,
                is_down,
                utf8,
                serial,
                rawkey: raw_code,
            } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
                let raw_key = keysym_to_keycode(keysym);
                let (key, raw_key) = match utf8 {
                    Some(text) if text.chars().count() == 1 => {
                        (KeyCode::Char(text.chars().nth(0).unwrap()), raw_key)
                    }
                    Some(text) => (KeyCode::Composed(text), raw_key),
                    None => match raw_key {
                        Some(key) => (key, None),
                        None => return,
                    },
                };
                let (key, raw_key) = match (key, raw_key) {
                    // Avoid eg: \x01 when we can use CTRL-A
                    (KeyCode::Char(c), Some(raw)) if c.is_ascii_control() => (raw, None),
                    // Avoid redundant key == raw_key
                    (key, Some(raw)) if key == raw => (key, None),
                    pair => pair,
                };

                let (modifiers, raw_modifiers) = if raw_key.is_some() {
                    (Modifiers::NONE, self.modifiers)
                } else {
                    (self.modifiers, Modifiers::NONE)
                };

                let key_event = KeyEvent {
                    key_is_down: is_down,
                    key,
                    raw_key,
                    modifiers,
                    raw_modifiers,
                    raw_code: Some(raw_code),
                    repeat_count: 1,
                }
                .normalize_shift();
                self.callbacks
                    .key_event(&key_event, &Window::Wayland(WaylandWindow(self.window_id)));
            }
            KeyboardEvent::Modifiers { modifiers } => self.modifiers = modifiers,
            // Clear the modifiers when we change focus, otherwise weird
            // things can happen.  For instance, if we lost focus because
            // CTRL+SHIFT+N was pressed to spawn a new window, we'd be
            // left stuck with CTRL+SHIFT held down and the window would
            // be left in a broken state.
            KeyboardEvent::Enter { .. } => {
                self.modifiers = Modifiers::NONE;
                self.callbacks.focus_change(true)
            }
            KeyboardEvent::Leave { .. } => {
                self.modifiers = Modifiers::NONE;
                self.callbacks.focus_change(false)
            }
        }
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
            self.callbacks
                .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
            self.refresh_frame();
        }

        while let Some((button, state)) = PendingMouse::next_button(&pending_mouse) {
            let button_mask = match button {
                MousePress::Left => MouseButtons::LEFT,
                MousePress::Right => MouseButtons::RIGHT,
                MousePress::Middle => MouseButtons::MIDDLE,
            };

            if state == DebuggableButtonState::Pressed {
                self.mouse_buttons |= button_mask;
            } else {
                self.mouse_buttons -= button_mask;
            }

            let event = MouseEvent {
                kind: match state {
                    DebuggableButtonState::Pressed => MouseEventKind::Press(button),
                    DebuggableButtonState::Released => MouseEventKind::Release(button),
                },
                coords: self.last_mouse_coords,
                screen_coords: ScreenPoint::new(
                    self.last_mouse_coords.x + self.dimensions.pixel_width as isize,
                    self.last_mouse_coords.y + self.dimensions.pixel_height as isize,
                ),
                mouse_buttons: self.mouse_buttons,
                modifiers: self.modifiers,
            };
            self.callbacks
                .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
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
                self.callbacks
                    .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
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
                self.callbacks
                    .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
            }
        }
    }

    fn get_dpi_factor(&self) -> i32 {
        self.dimensions.dpi as i32 / crate::DEFAULT_DPI as i32
    }

    fn get_dpi(&self) -> usize {
        self.dimensions.dpi
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
        if pending.close && self.callbacks.can_close() {
            self.callbacks.destroy();
            self.window.take();
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

                    self.callbacks.resize(self.dimensions);
                    if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                        wegl_surface.resize(pixel_width, pixel_height, 0, 0);
                    }
                }

                self.refresh_frame();
                self.need_paint = true;
                self.do_paint().unwrap();
            }
        }
        if pending.refresh_decorations && self.window.is_some() {
            self.refresh_frame();
        }
    }

    fn refresh_frame(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window.refresh();
            self.surface.commit();
        }
    }

    fn enable_opengl(&mut self) -> anyhow::Result<()> {
        let window = Window::Wayland(WaylandWindow(self.window_id));
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

        self.callbacks.created(&window, gl_state)
    }

    fn do_paint(&mut self) -> anyhow::Result<()> {
        if let Some(gl_context) = self.gl_state.as_ref() {
            if gl_context.is_context_lost() {
                log::error!("opengl context was lost; should reinit");
                drop(self.gl_state.take());
                self.enable_opengl()?;
                return self.do_paint();
            }

            let mut frame = glium::Frame::new(
                Rc::clone(&gl_context),
                (
                    self.dimensions.pixel_width as u32,
                    self.dimensions.pixel_height as u32,
                ),
            );

            self.callbacks.paint_opengl(&mut frame);
            frame.finish()?;
            // self.damage();
            self.refresh_frame();
            self.need_paint = false;
            return Ok(());
        }

        if self.pool.is_used() {
            // Buffer still in use by server; retry later
            return Ok(());
        }

        if self.window.is_none() {
            // Window has been closed; complete gracefully
            return Ok(());
        }

        self.pool
            .resize(4 * self.dimensions.pixel_width * self.dimensions.pixel_height)?;

        let dpi = self.get_dpi();
        let mut context = MmapImage {
            mmap: self.pool.mmap(),
            dimensions: (self.dimensions.pixel_width, self.dimensions.pixel_height),
            dpi,
        };
        self.callbacks.paint(&mut context);
        context.mmap.flush()?;

        let buffer = self.pool.buffer(
            0,
            self.dimensions.pixel_width as i32,
            self.dimensions.pixel_height as i32,
            4 * self.dimensions.pixel_width as i32,
            toolkit::reexports::client::protocol::wl_shm::Format::Argb8888,
        );

        self.surface.attach(Some(&buffer), 0, 0);
        self.damage();

        self.surface.commit();
        self.refresh_frame();
        self.need_paint = false;

        Ok(())
    }

    fn damage(&mut self) {
        if self.surface.as_ref().version() >= 4 {
            self.surface.damage_buffer(
                0,
                0,
                self.dimensions.pixel_width as i32,
                self.dimensions.pixel_height as i32,
            );
        } else {
            // Older versions use the surface size which is the pre-scaled
            // dimensions.  Since we store the scaled dimensions, we need
            // to compensate here.
            self.surface.damage(
                0,
                0,
                self.pixels_to_surface(self.dimensions.pixel_width as i32),
                self.pixels_to_surface(self.dimensions.pixel_height as i32),
            );
        }
    }
}

struct MmapImage<'a> {
    mmap: &'a mut memmap::MmapMut,
    dimensions: (usize, usize),
    dpi: usize,
}

impl<'a> BitmapImage for MmapImage<'a> {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.mmap.as_ptr()
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.mmap.as_mut_ptr()
    }

    fn image_dimensions(&self) -> (usize, usize) {
        self.dimensions
    }
}

impl<'a> PaintContext for MmapImage<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        BitmapImage::clear_rect(self, rect, color)
    }

    fn clear(&mut self, color: Color) {
        BitmapImage::clear(self, color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: self.dpi,
        }
    }

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        BitmapImage::draw_image(self, dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        BitmapImage::draw_line(self, start, end, color, operator);
    }
}

impl WindowOps for WaylandWindow {
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

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + FnMut(&mut dyn Any, &dyn WindowOps) -> anyhow::Result<R>>(
        &self,
        mut func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        WaylandConnection::with_window_inner(self.0, move |inner| {
            let window = Window::Wayland(WaylandWindow(inner.window_id));
            func(inner.callbacks.as_any(), &window)
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

    fn set_clipboard(&self, text: String) -> Future<()> {
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

impl WindowOpsMut for WaylandWindowInner {
    fn close(&mut self) {
        self.callbacks.destroy();
        self.window.take();
    }

    fn hide(&mut self) {
        if let Some(window) = self.window.as_ref() {
            window.set_minimized();
        }
    }

    fn show(&mut self) {
        if self.window.is_none() {
            return;
        }
        let conn = Connection::get().unwrap().wayland();

        if !conn
            .environment
            .borrow()
            .get_shell()
            .unwrap()
            .needs_configure()
        {
            self.do_paint().unwrap();
        } else {
            self.refresh_frame();
        }
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
        self.need_paint = true;
        self.do_paint().unwrap();
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

        // and update the window decorations
        if let Some(window) = self.window.as_mut() {
            window.resize(surface_width, surface_height);
            // The resize must be followed by a refresh call.
            window.refresh();
            // In addition, resize doesn't take effect until
            // the suface is commited
            self.surface.commit();
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
