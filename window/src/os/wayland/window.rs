use crate::bitmaps::BitmapImage;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::input::*;
use crate::os::xkeysyms::keysym_to_keycode;
use crate::{
    Connection, Dimensions, MouseCursor, Operator, PaintContext, Point, Rect, ScreenPoint,
    WindowCallbacks, WindowOps, WindowOpsMut,
};
use failure::Fallible;
use promise::Future;
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use toolkit::keyboard::{
    map_keyboard_auto_with_repeat, Event as KbEvent, KeyRepeatEvent, KeyRepeatKind, KeyState,
    ModifiersState,
};
use toolkit::reexports::client::protocol::wl_pointer::{
    self, Axis, AxisSource, Event as PointerEvent,
};
use toolkit::reexports::client::protocol::wl_seat::WlSeat;
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::reexports::client::NewProxy;
use toolkit::utils::MemPool;
use toolkit::window::Event;

fn modifier_keys(state: ModifiersState) -> Modifiers {
    let mut mods = Modifiers::NONE;
    if state.ctrl {
        mods |= Modifiers::CTRL;
    }
    if state.alt {
        mods |= Modifiers::ALT;
    }
    if state.shift {
        mods |= Modifiers::SHIFT;
    }
    if state.logo {
        mods |= Modifiers::SUPER;
    }
    mods
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
enum DebuggableButtonState {
    Released,
    Pressed,
}

impl From<wl_pointer::ButtonState> for DebuggableButtonState {
    fn from(state: wl_pointer::ButtonState) -> DebuggableButtonState {
        match state {
            wl_pointer::ButtonState::Released => Self::Released,
            wl_pointer::ButtonState::Pressed => Self::Pressed,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum SendablePointerEvent {
    Enter {
        serial: u32,
        surface_x: f64,
        surface_y: f64,
    },
    Leave {
        serial: u32,
    },
    Motion {
        time: u32,
        surface_x: f64,
        surface_y: f64,
    },
    Button {
        serial: u32,
        time: u32,
        button: u32,
        state: DebuggableButtonState,
    },
    Axis {
        time: u32,
        axis: Axis,
        value: f64,
    },
    Frame,
    AxisSource {
        axis_source: AxisSource,
    },
    AxisStop {
        time: u32,
        axis: Axis,
    },
    AxisDiscrete {
        axis: Axis,
        discrete: i32,
    },
}

impl From<PointerEvent> for SendablePointerEvent {
    fn from(event: PointerEvent) -> Self {
        match event {
            PointerEvent::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => SendablePointerEvent::Enter {
                serial,
                surface_x,
                surface_y,
            },
            PointerEvent::Leave { serial, .. } => SendablePointerEvent::Leave { serial },
            PointerEvent::Motion {
                time,
                surface_x,
                surface_y,
            } => SendablePointerEvent::Motion {
                time,
                surface_x,
                surface_y,
            },
            PointerEvent::Button {
                serial,
                time,
                button,
                state,
                ..
            } => SendablePointerEvent::Button {
                serial,
                time,
                button,
                state: state.into(),
            },
            PointerEvent::Axis { time, axis, value } => {
                SendablePointerEvent::Axis { time, axis, value }
            }
            PointerEvent::Frame => SendablePointerEvent::Frame,
            PointerEvent::AxisSource { axis_source, .. } => {
                SendablePointerEvent::AxisSource { axis_source }
            }
            PointerEvent::AxisStop { axis, time } => SendablePointerEvent::AxisStop { axis, time },
            PointerEvent::AxisDiscrete { axis, discrete } => {
                SendablePointerEvent::AxisDiscrete { axis, discrete }
            }
            _ => unreachable!(),
        }
    }
}

struct MyTheme;
use toolkit::window::ButtonState;

const DARK_PURPLE: [u8; 4] = [0xff, 0x2b, 0x20, 0x42];
const PURPLE: [u8; 4] = [0xff, 0x3b, 0x30, 0x52];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const GRAY: [u8; 4] = [0x80, 0x80, 0x80, 0x80];

impl toolkit::window::Theme for MyTheme {
    fn get_primary_color(&self, _active: bool) -> [u8; 4] {
        DARK_PURPLE
    }

    fn get_secondary_color(&self, _active: bool) -> [u8; 4] {
        DARK_PURPLE
    }

    fn get_close_button_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            PURPLE
        } else {
            DARK_PURPLE
        }
    }
    fn get_maximize_button_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            PURPLE
        } else {
            DARK_PURPLE
        }
    }
    fn get_minimize_button_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            PURPLE
        } else {
            DARK_PURPLE
        }
    }

    fn get_close_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            WHITE
        } else {
            GRAY
        }
    }
    fn get_maximize_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            WHITE
        } else {
            GRAY
        }
    }
    fn get_minimize_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        if let ButtonState::Hovered = status {
            WHITE
        } else {
            GRAY
        }
    }
}

pub struct WindowInner {
    window_id: usize,
    callbacks: Box<dyn WindowCallbacks>,
    surface: WlSurface,
    seat: WlSeat,
    window: Option<toolkit::window::Window<toolkit::window::ConceptFrame>>,
    pool: MemPool,
    dimensions: (u32, u32),
    need_paint: bool,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    modifiers: Modifiers,
}

#[derive(Clone, Debug)]
pub struct Window(usize);

impl Window {
    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> Fallible<Window> {
        let conn = Connection::get().ok_or_else(|| {
            failure::err_msg(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
        })?;

        let window_id = conn.next_window_id();

        let surface = conn
            .environment
            .borrow_mut()
            .create_surface(|dpi, _surface| {
                println!("surface dpi changed to {}", dpi);
            });

        let dimensions = (width as u32, height as u32);
        let mut window = toolkit::window::Window::<toolkit::window::ConceptFrame>::init_from_env(
            &*conn.environment.borrow(),
            surface.clone(),
            dimensions,
            move |evt| {
                Connection::with_window_inner(window_id, move |inner| {
                    inner.handle_event(evt.clone());
                    Ok(())
                });
            },
        )
        .map_err(|e| failure::format_err!("Failed to create window: {}", e))?;

        window.set_app_id(class_name.to_string());
        window.set_decorate(true);
        window.set_resizable(true);
        window.set_theme(MyTheme {});

        let pool = MemPool::new(&conn.environment.borrow().shm, || {})?;

        let seat = conn
            .environment
            .borrow()
            .manager
            .instantiate_range(1, 6, NewProxy::implement_dummy)
            .map_err(|_| failure::format_err!("Failed to create seat"))?;
        window.new_seat(&seat);

        seat.get_pointer(move |ptr| {
            ptr.implement_closure(
                move |evt, _| {
                    let evt: SendablePointerEvent = evt.into();
                    Connection::with_window_inner(window_id, move |inner| {
                        inner.handle_pointer(evt);
                        Ok(())
                    });
                },
                (),
            )
        })
        .map_err(|_| failure::format_err!("Failed to configure pointer callback"))?;

        map_keyboard_auto_with_repeat(
            &seat,
            KeyRepeatKind::System,
            move |event: KbEvent, _| match event {
                KbEvent::Key {
                    rawkey,
                    keysym,
                    state,
                    utf8,
                    ..
                } => {
                    Connection::with_window_inner(window_id, move |inner| {
                        inner.handle_key(state == KeyState::Pressed, rawkey, keysym, utf8.clone());
                        Ok(())
                    });
                }
                KbEvent::Modifiers { modifiers } => {
                    let mods = modifier_keys(modifiers);
                    Connection::with_window_inner(window_id, move |inner| {
                        inner.handle_modifiers(mods);
                        Ok(())
                    });
                }
                _ => {}
            },
            move |event: KeyRepeatEvent, _| {
                Connection::with_window_inner(window_id, move |inner| {
                    inner.handle_key(true, event.rawkey, event.keysym, event.utf8.clone());
                    Ok(())
                });
            },
        )
        .map_err(|_| failure::format_err!("Failed to configure keyboard callback"))?;

        let inner = Rc::new(RefCell::new(WindowInner {
            window_id,
            callbacks,
            surface,
            seat,
            window: Some(window),
            pool,
            dimensions,
            need_paint: true,
            last_mouse_coords: Point::new(0, 0),
            mouse_buttons: MouseButtons::NONE,
            modifiers: Modifiers::NONE,
        }));

        let window_handle = Window(window_id);

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        inner.borrow_mut().callbacks.created(&window_handle);

        Ok(window_handle)
    }
}

impl WindowInner {
    fn handle_key(&mut self, key_is_down: bool, rawkey: u32, keysym: u32, utf8: Option<String>) {
        let raw_key = keysym_to_keycode(keysym);
        let (key, raw_key) = match utf8 {
            Some(text) if text.chars().count() == 1 => {
                (KeyCode::Char(text.chars().nth(0).unwrap()), raw_key)
            }
            Some(text) => (KeyCode::Composed(text), raw_key),
            None => match raw_key {
                Some(key) => (key, None),
                None => {
                    println!("no mapping for keysym {:x} and rawkey {:x}", keysym, rawkey);
                    return;
                }
            },
        };
        // Avoid redundant key == raw_key
        let (key, raw_key) = match (key, raw_key) {
            // Avoid eg: \x01 when we can use CTRL-A
            (KeyCode::Char(c), Some(raw)) if c.is_ascii_control() => (raw.clone(), None),
            (key, Some(raw)) if key == raw => (key, None),
            pair => pair,
        };
        let key_event = KeyEvent {
            key_is_down,
            key,
            raw_key,
            modifiers: self.modifiers,
            repeat_count: 1,
        };
        self.callbacks
            .key_event(&key_event, &Window(self.window_id));
    }

    fn handle_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    fn handle_pointer(&mut self, evt: SendablePointerEvent) {
        match evt {
            SendablePointerEvent::Enter { .. } => {}
            SendablePointerEvent::Leave { .. } => {}
            SendablePointerEvent::AxisSource { .. } => {}
            SendablePointerEvent::AxisStop { .. } => {}
            SendablePointerEvent::AxisDiscrete { .. } => {}
            SendablePointerEvent::Frame => {}
            SendablePointerEvent::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                let factor = toolkit::surface::get_dpi_factor(&self.surface);
                let coords = Point::new(
                    surface_x as isize * factor as isize,
                    surface_y as isize * factor as isize,
                );
                self.last_mouse_coords = coords;
                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    coords,
                    screen_coords: ScreenPoint::new(
                        coords.x + self.dimensions.0 as isize,
                        coords.y + self.dimensions.1 as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.callbacks.mouse_event(&event, &Window(self.window_id));
            }
            SendablePointerEvent::Button { button, state, .. } => {
                fn linux_button(b: u32) -> Option<MousePress> {
                    // See BTN_LEFT and friends in <linux/input-event-codes.h>
                    match b {
                        0x110 => Some(MousePress::Left),
                        0x111 => Some(MousePress::Right),
                        0x112 => Some(MousePress::Middle),
                        _ => None,
                    }
                }
                let button = match linux_button(button) {
                    Some(button) => button,
                    None => return,
                };

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
                        self.last_mouse_coords.x + self.dimensions.0 as isize,
                        self.last_mouse_coords.y + self.dimensions.1 as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.callbacks.mouse_event(&event, &Window(self.window_id));
            }
            SendablePointerEvent::Axis { .. } => {}
        }
    }

    fn handle_event(&mut self, evt: Event) {
        match evt {
            Event::Close => {
                if self.callbacks.can_close() {
                    self.callbacks.destroy();
                    self.window.take();
                }
            }
            Event::Refresh => {
                self.do_paint().unwrap();
            }
            Event::Configure { new_size, .. } => {
                if self.window.is_none() {
                    return;
                }
                if let Some((w, h)) = new_size {
                    let factor = toolkit::surface::get_dpi_factor(&self.surface);
                    self.surface.set_buffer_scale(factor);
                    self.window.as_mut().unwrap().resize(w, h);
                    let w = w * factor as u32;
                    let h = h * factor as u32;
                    self.dimensions = (w, h);
                    self.callbacks.resize(Dimensions {
                        pixel_width: w as usize,
                        pixel_height: h as usize,
                        dpi: 96 * factor as usize,
                    });
                }
                self.window.as_mut().unwrap().refresh();
                self.do_paint().unwrap();
            }
        }
    }

    fn do_paint(&mut self) -> Fallible<()> {
        if self.pool.is_used() {
            // Buffer still in use by server; retry later
            return Ok(());
        }

        if self.window.is_none() {
            // Window has been closed; complete gracefully
            return Ok(());
        }

        self.pool
            .resize((4 * self.dimensions.0 * self.dimensions.1) as usize)?;

        let mut context = MmapImage {
            mmap: self.pool.mmap(),
            dimensions: (self.dimensions.0 as usize, self.dimensions.1 as usize),
        };
        self.callbacks.paint(&mut context);
        context.mmap.flush()?;

        let buffer = self.pool.buffer(
            0,
            self.dimensions.0 as i32,
            self.dimensions.1 as i32,
            4 * self.dimensions.0 as i32,
            toolkit::reexports::client::protocol::wl_shm::Format::Argb8888,
        );

        self.surface.attach(Some(&buffer), 0, 0);
        self.damage();

        self.surface.commit();
        self.window.as_mut().unwrap().refresh();
        self.need_paint = false;

        Ok(())
    }

    fn damage(&mut self) {
        if self.surface.as_ref().version() >= 4 {
            self.surface
                .damage_buffer(0, 0, self.dimensions.0 as i32, self.dimensions.1 as i32);
        } else {
            // Older versions use the surface size which is the pre-scaled
            // dimensions.  Since we store the scaled dimensions, we need
            // to compensate here.
            let factor = toolkit::surface::get_dpi_factor(&self.surface);
            self.surface.damage(
                0,
                0,
                self.dimensions.0 as i32 / factor,
                self.dimensions.1 as i32 / factor,
            );
        }
    }
}

struct MmapImage<'a> {
    mmap: &'a mut memmap::MmapMut,
    dimensions: (usize, usize),
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
            dpi: 96,
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

impl WindowOps for Window {
    fn close(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn hide(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        })
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps) -> Fallible<R>>(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);
            func(inner.callbacks.as_any(), &window)
        })
    }

    #[cfg(feature = "opengl")]
    fn enable_opengl<
        R,
        F: Send
            + 'static
            + Fn(
                &mut dyn Any,
                &dyn WindowOps,
                failure::Fallible<std::rc::Rc<glium::backend::Context>>,
            ) -> failure::Fallible<R>,
    >(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);

            /*
            let gl_state = crate::egl::GlState::create(
                Some(inner.conn.display as *const _),
                inner.window_id as *mut _,
            )
            .map(Rc::new)
            .and_then(|state| unsafe {
                Ok(glium::backend::Context::new(
                    Rc::clone(&state),
                    true,
                    if cfg!(debug_assertions) {
                        glium::debug::DebugCallbackBehavior::DebugMessageOnError
                    } else {
                        glium::debug::DebugCallbackBehavior::Ignore
                    },
                )?)
            });

            inner.gl_state = gl_state.as_ref().map(Rc::clone).ok();

            func(inner.callbacks.as_any(), &window, gl_state)
            */
            func(
                inner.callbacks.as_any(),
                &window,
                Err(failure::err_msg("no opengl")),
            )
        })
    }
}

impl WindowOpsMut for WindowInner {
    fn close(&mut self) {
        self.callbacks.destroy();
        self.window.take();
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        if self.window.is_none() {
            return;
        }
        let conn = Connection::get().unwrap();

        if !conn.environment.borrow().shell.needs_configure() {
            self.do_paint().unwrap();
        } else {
            self.window.as_mut().unwrap().refresh();
        }
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {}

    fn invalidate(&mut self) {
        self.need_paint = true;
        self.do_paint().unwrap();
    }

    fn set_inner_size(&self, width: usize, height: usize) {}

    fn set_window_position(&self, coords: ScreenPoint) {}

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        if let Some(window) = self.window.as_ref() {
            window.set_title(title.to_string());
        }
    }
}
