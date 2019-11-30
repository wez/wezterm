use crate::bitmaps::BitmapImage;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::input::*;
use crate::os::wayland::connection::WaylandConnection;
use crate::os::xkeysyms::keysym_to_keycode;
use crate::{
    Connection, Dimensions, MouseCursor, Operator, PaintContext, Point, Rect, ScreenPoint, Window,
    WindowCallbacks, WindowOps, WindowOpsMut,
};
use failure::Fallible;
use filedescriptor::{FileDescriptor, Pipe};
use promise::{Future, Promise};
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use toolkit::keyboard::{
    map_keyboard_auto_with_repeat, Event as KbEvent, KeyRepeatEvent, KeyRepeatKind, KeyState,
    ModifiersState,
};
use toolkit::reexports::client::protocol::wl_data_device::{
    Event as DataDeviceEvent, WlDataDevice,
};
use toolkit::reexports::client::protocol::wl_data_offer::{Event as DataOfferEvent, WlDataOffer};
use toolkit::reexports::client::protocol::wl_data_source::{
    Event as DataSourceEvent, WlDataSource,
};
use toolkit::reexports::client::protocol::wl_pointer::{
    self, Axis, AxisSource, Event as PointerEvent,
};
use toolkit::reexports::client::protocol::wl_seat::{Event as SeatEvent, WlSeat};
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::utils::MemPool;
use toolkit::window::Event;
use wayland_client::egl::{is_available as egl_is_available, WlEglSurface};

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

const DARK_GRAY: [u8; 4] = [0xff, 0x35, 0x35, 0x35];
const DARK_PURPLE: [u8; 4] = [0xff, 0x2b, 0x20, 0x42];
const PURPLE: [u8; 4] = [0xff, 0x3b, 0x30, 0x52];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const GRAY: [u8; 4] = [0x80, 0x80, 0x80, 0x80];

impl toolkit::window::Theme for MyTheme {
    fn get_primary_color(&self, active: bool) -> [u8; 4] {
        if active {
            DARK_PURPLE
        } else {
            DARK_GRAY
        }
    }

    fn get_secondary_color(&self, active: bool) -> [u8; 4] {
        self.get_primary_color(active)
    }

    fn get_close_button_color(&self, status: ButtonState) -> [u8; 4] {
        match status {
            ButtonState::Hovered => PURPLE,
            ButtonState::Idle => DARK_PURPLE,
            ButtonState::Disabled => DARK_GRAY,
        }
    }
    fn get_maximize_button_color(&self, status: ButtonState) -> [u8; 4] {
        self.get_close_button_color(status)
    }
    fn get_minimize_button_color(&self, status: ButtonState) -> [u8; 4] {
        self.get_close_button_color(status)
    }

    fn get_close_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        match status {
            ButtonState::Hovered => WHITE,
            ButtonState::Idle => GRAY,
            ButtonState::Disabled => DARK_GRAY,
        }
    }
    fn get_maximize_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        self.get_close_button_icon_color(status)
    }
    fn get_minimize_button_icon_color(&self, status: ButtonState) -> [u8; 4] {
        self.get_close_button_icon_color(status)
    }
}

struct CopyAndPaste {
    data_offer: Option<WlDataOffer>,
    last_serial: u32,
    data_device: Option<WlDataDevice>,
}

const TEXT_MIME_TYPE: &str = "text/plain;charset=utf-8";

impl CopyAndPaste {
    fn update_last_serial(&mut self, serial: u32) {
        if serial != 0 {
            self.last_serial = serial;
        }
    }

    fn get_clipboard_data(&mut self) -> Fallible<FileDescriptor> {
        let offer = self
            .data_offer
            .as_ref()
            .ok_or_else(|| failure::err_msg("no data offer"))?;
        let pipe = Pipe::new()?;
        offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
        Ok(pipe.read)
    }

    fn handle_data_offer(&mut self, event: DataOfferEvent, offer: WlDataOffer) {
        match event {
            DataOfferEvent::Offer { mime_type } => {
                if mime_type == TEXT_MIME_TYPE {
                    offer.accept(self.last_serial, Some(mime_type));
                    self.data_offer.replace(offer);
                } else {
                    // Refuse other mime types
                    offer.accept(self.last_serial, None);
                }
            }
            DataOfferEvent::SourceActions { source_actions } => {
                log::error!("Offer source_actions {}", source_actions);
            }
            DataOfferEvent::Action { dnd_action } => {
                log::error!("Offer dnd_action {}", dnd_action);
            }
            _ => {}
        }
    }

    fn confirm_selection(&mut self, offer: WlDataOffer) {
        self.data_offer.replace(offer);
    }

    fn set_selection(&mut self, source: WlDataSource) {
        if let Some(dev) = self.data_device.as_ref() {
            dev.set_selection(Some(&source), self.last_serial);
        }
    }
}

pub struct WaylandWindowInner {
    window_id: usize,
    callbacks: Box<dyn WindowCallbacks>,
    surface: WlSurface,
    #[allow(dead_code)]
    seat: WlSeat,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    window: Option<toolkit::window::Window<toolkit::window::ConceptFrame>>,
    pool: MemPool,
    dimensions: (u32, u32),
    need_paint: bool,
    last_mouse_coords: Point,
    mouse_buttons: MouseButtons,
    modifiers: Modifiers,
    pending_event: Arc<Mutex<PendingEvent>>,
    pending_mouse: Arc<Mutex<PendingMouse>>,
    // wegl_surface is listed before gl_state because it
    // must be dropped before gl_state otherwise the underlying
    // libraries will segfault on shutdown
    #[cfg(feature = "opengl")]
    wegl_surface: Option<WlEglSurface>,
    #[cfg(feature = "opengl")]
    gl_state: Option<Rc<glium::backend::Context>>,
}

#[derive(Clone)]
struct PendingMouse {
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    surface_coords: Option<(f64, f64)>,
    button: Vec<(MousePress, DebuggableButtonState)>,
    scroll: Option<(f64, f64)>,
}

impl PendingMouse {
    // Return true if we need to queue up a call to act on the event,
    // false if we think there is already a pending event
    fn queue(&mut self, evt: SendablePointerEvent) -> bool {
        match evt {
            SendablePointerEvent::Enter { serial, .. } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
                false
            }
            SendablePointerEvent::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let changed = self.surface_coords.is_none();
                self.surface_coords.replace((surface_x, surface_y));
                changed
            }
            SendablePointerEvent::Button {
                button,
                state,
                serial,
                ..
            } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
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
                    None => return false,
                };
                let changed = self.button.is_empty();
                self.button.push((button, state));
                changed
            }
            SendablePointerEvent::Axis {
                axis: Axis::VerticalScroll,
                value,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll.replace((x, y + value));
                changed
            }
            SendablePointerEvent::Axis {
                axis: Axis::HorizontalScroll,
                value,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll.replace((x + value, y));
                changed
            }
            _ => false,
        }
    }

    fn next_button(pending: &Arc<Mutex<Self>>) -> Option<(MousePress, DebuggableButtonState)> {
        let mut pending = pending.lock().unwrap();
        if pending.button.is_empty() {
            None
        } else {
            Some(pending.button.remove(0))
        }
    }

    fn coords(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().surface_coords.take()
    }

    fn scroll(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().scroll.take()
    }
}

#[derive(Default, Clone, Debug)]
struct PendingEvent {
    close: bool,
    refresh: bool,
    configure: Option<(u32, u32)>,
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
                if !self.refresh {
                    self.refresh = true;
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
                    changed = !self.refresh;
                    self.refresh = true;
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
    ) -> Fallible<Window> {
        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                failure::err_msg(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .wayland();

        let window_id = conn.next_window_id();

        let surface = conn
            .environment
            .borrow_mut()
            .create_surface(|dpi, _surface| {
                println!("surface dpi changed to {}", dpi);
            });

        let dimensions = (width as u32, height as u32);
        let pending_event = Arc::new(Mutex::new(PendingEvent::default()));
        let mut window = toolkit::window::Window::<toolkit::window::ConceptFrame>::init_from_env(
            &*conn.environment.borrow(),
            surface.clone(),
            dimensions,
            {
                let pending_event = Arc::clone(&pending_event);
                move |evt| {
                    if pending_event.lock().unwrap().queue(evt) {
                        WaylandConnection::with_window_inner(window_id, move |inner| {
                            inner.dispatch_pending_event();
                            Ok(())
                        });
                    }
                }
            },
        )
        .map_err(|e| failure::format_err!("Failed to create window: {}", e))?;

        window.set_app_id(class_name.to_string());
        window.set_decorate(true);
        window.set_resizable(true);
        window.set_title(name.to_string());
        window.set_theme(MyTheme {});

        let pool = MemPool::new(&conn.environment.borrow().shm, || {})?;

        let seat = conn
            .environment
            .borrow()
            .manager
            .instantiate_range(1, 6, move |seat| {
                seat.implement_closure(
                    move |event, _seat| {
                        if let SeatEvent::Name { name } = event {
                            log::error!("seat name is {}", name);
                        }
                    },
                    (),
                )
            })
            .map_err(|_| failure::format_err!("Failed to create seat"))?;

        window.new_seat(&seat);

        let copy_and_paste = Arc::new(Mutex::new(CopyAndPaste {
            data_offer: None,
            last_serial: 0,
            data_device: None,
        }));

        let pending_mouse = Arc::new(Mutex::new(PendingMouse {
            copy_and_paste: Arc::clone(&copy_and_paste),
            button: vec![],
            scroll: None,
            surface_coords: None,
        }));

        let data_device = conn
            .environment
            .borrow()
            .data_device_manager
            .get_data_device(&seat, {
                let copy_and_paste = Arc::clone(&copy_and_paste);
                move |device| {
                    device.implement_closure(
                        {
                            let copy_and_paste = Arc::clone(&copy_and_paste);
                            move |event, _device| match event {
                                DataDeviceEvent::DataOffer { id } => {
                                    id.implement_closure(
                                        {
                                            let copy_and_paste = Arc::clone(&copy_and_paste);
                                            move |event, offer| {
                                                copy_and_paste
                                                    .lock()
                                                    .unwrap()
                                                    .handle_data_offer(event, offer);
                                            }
                                        },
                                        (),
                                    );
                                }
                                DataDeviceEvent::Enter { .. }
                                | DataDeviceEvent::Leave { .. }
                                | DataDeviceEvent::Motion { .. }
                                | DataDeviceEvent::Drop => {}

                                DataDeviceEvent::Selection { id } => {
                                    if let Some(offer) = id {
                                        copy_and_paste.lock().unwrap().confirm_selection(offer);
                                    }
                                }
                                _ => {}
                            }
                        },
                        (),
                    )
                }
            })
            .map_err(|_| failure::format_err!("Failed to configure data_device"))?;

        copy_and_paste
            .lock()
            .unwrap()
            .data_device
            .replace(data_device);

        seat.get_pointer({
            let pending_mouse = Arc::clone(&pending_mouse);
            move |ptr| {
                ptr.implement_closure(
                    {
                        let pending_mouse = Arc::clone(&pending_mouse);
                        move |evt, _| {
                            let evt: SendablePointerEvent = evt.into();
                            if pending_mouse.lock().unwrap().queue(evt) {
                                WaylandConnection::with_window_inner(window_id, move |inner| {
                                    inner.dispatch_pending_mouse();
                                    Ok(())
                                });
                            }
                        }
                    },
                    (),
                )
            }
        })
        .map_err(|_| failure::format_err!("Failed to configure pointer callback"))?;

        map_keyboard_auto_with_repeat(
            &seat,
            KeyRepeatKind::System,
            {
                let copy_and_paste = Arc::clone(&copy_and_paste);
                move |event: KbEvent, _| match event {
                    KbEvent::Enter { serial, .. } => {
                        copy_and_paste.lock().unwrap().update_last_serial(serial);
                    }
                    KbEvent::Key {
                        rawkey,
                        keysym,
                        state,
                        utf8,
                        serial,
                        ..
                    } => {
                        WaylandConnection::with_window_inner(window_id, move |inner| {
                            inner.handle_key(
                                serial,
                                state == KeyState::Pressed,
                                rawkey,
                                keysym,
                                utf8.clone(),
                            );
                            Ok(())
                        });
                    }
                    KbEvent::Modifiers { modifiers } => {
                        let mods = modifier_keys(modifiers);
                        WaylandConnection::with_window_inner(window_id, move |inner| {
                            inner.handle_modifiers(mods);
                            Ok(())
                        });
                    }
                    _ => {}
                }
            },
            move |event: KeyRepeatEvent, _| {
                WaylandConnection::with_window_inner(window_id, move |inner| {
                    inner.handle_key(0, true, event.rawkey, event.keysym, event.utf8.clone());
                    Ok(())
                });
            },
        )
        .map_err(|_| failure::format_err!("Failed to configure keyboard callback"))?;

        let inner = Rc::new(RefCell::new(WaylandWindowInner {
            copy_and_paste,
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
            pending_event,
            pending_mouse,
            #[cfg(feature = "opengl")]
            gl_state: None,
            #[cfg(feature = "opengl")]
            wegl_surface: None,
        }));

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        inner.borrow_mut().callbacks.created(&window_handle);

        Ok(window_handle)
    }
}

impl WaylandWindowInner {
    fn handle_key(
        &mut self,
        serial: u32,
        key_is_down: bool,
        rawkey: u32,
        keysym: u32,
        utf8: Option<String>,
    ) {
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
            .key_event(&key_event, &Window::Wayland(WaylandWindow(self.window_id)));
    }

    fn handle_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    fn dispatch_pending_mouse(&mut self) {
        // Dancing around the borrow checker and the call to self.refresh_frame()
        let pending_mouse = Arc::clone(&self.pending_mouse);

        if let Some((x, y)) = PendingMouse::coords(&pending_mouse) {
            let factor = toolkit::surface::get_dpi_factor(&self.surface);
            let coords = Point::new(x as isize * factor as isize, y as isize * factor as isize);
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
                    self.last_mouse_coords.x + self.dimensions.0 as isize,
                    self.last_mouse_coords.y + self.dimensions.1 as isize,
                ),
                mouse_buttons: self.mouse_buttons,
                modifiers: self.modifiers,
            };
            self.callbacks
                .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
        }

        if let Some((value_x, value_y)) = PendingMouse::scroll(&pending_mouse) {
            let discrete_x = value_x.trunc();
            if discrete_x != 0. {
                let event = MouseEvent {
                    kind: MouseEventKind::HorzWheel(-discrete_x as i16),
                    coords: self.last_mouse_coords,
                    screen_coords: ScreenPoint::new(
                        self.last_mouse_coords.x + self.dimensions.0 as isize,
                        self.last_mouse_coords.y + self.dimensions.1 as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.callbacks
                    .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
            }

            let discrete_y = value_y.trunc();
            if discrete_y != 0. {
                let event = MouseEvent {
                    kind: MouseEventKind::VertWheel(-discrete_y as i16),
                    coords: self.last_mouse_coords,
                    screen_coords: ScreenPoint::new(
                        self.last_mouse_coords.x + self.dimensions.0 as isize,
                        self.last_mouse_coords.y + self.dimensions.1 as isize,
                    ),
                    mouse_buttons: self.mouse_buttons,
                    modifiers: self.modifiers,
                };
                self.callbacks
                    .mouse_event(&event, &Window::Wayland(WaylandWindow(self.window_id)));
            }
        }
    }

    fn dispatch_pending_event(&mut self) {
        let mut pending;
        {
            let mut pending_events = self.pending_event.lock().unwrap();
            pending = pending_events.clone();
            *pending_events = PendingEvent::default();
        }
        if pending.close {
            if self.callbacks.can_close() {
                self.callbacks.destroy();
                self.window.take();
            }
        }
        if let Some((w, h)) = pending.configure.take() {
            if self.window.is_some() {
                let factor = toolkit::surface::get_dpi_factor(&self.surface);
                self.surface.set_buffer_scale(factor);
                self.window.as_mut().unwrap().resize(w, h);
                let w = w * factor as u32;
                let h = h * factor as u32;
                self.dimensions = (w, h);
                #[cfg(feature = "opengl")]
                {
                    if let Some(wegl_surface) = self.wegl_surface.as_mut() {
                        wegl_surface.resize(w as i32, h as i32, 0, 0);
                    }
                }
                self.callbacks.resize(Dimensions {
                    pixel_width: w as usize,
                    pixel_height: h as usize,
                    dpi: 96 * factor as usize,
                });
                self.refresh_frame();
                pending.refresh = true;
            }
        }
        if pending.refresh {
            if self.window.is_some() {
                self.do_paint().unwrap();
            }
        }
    }

    fn refresh_frame(&mut self) {
        if let Some(window) = self.window.as_mut() {
            window.refresh();
        }
    }

    fn do_paint(&mut self) -> Fallible<()> {
        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = self.gl_state.as_ref() {
                let mut frame = glium::Frame::new(
                    Rc::clone(&gl_context),
                    (u32::from(self.dimensions.0), u32::from(self.dimensions.1)),
                );

                self.callbacks.paint_opengl(&mut frame);
                frame.finish()?;
                // self.damage();
                self.surface.commit();
                self.refresh_frame();
                self.need_paint = false;
                return Ok(());
            }
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
        self.refresh_frame();
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
            let _ = inner.set_cursor(cursor);
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

    fn apply<R, F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps) -> Fallible<R>>(
        &self,
        func: F,
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
        WaylandConnection::with_window_inner(self.0, move |inner| {
            let window = Window::Wayland(WaylandWindow(inner.window_id));
            let wayland_conn = Connection::get().unwrap().wayland();
            let mut wegl_surface = None;

            let gl_state = if !egl_is_available() {
                Err(failure::err_msg("!egl_is_available"))
            } else {
                wegl_surface = Some(WlEglSurface::new(
                    &inner.surface,
                    inner.dimensions.0 as i32,
                    inner.dimensions.1 as i32,
                ));

                crate::egl::GlState::create_wayland(
                    Some(wayland_conn.display.borrow().get_display_ptr() as *const _),
                    wegl_surface.as_ref().unwrap(),
                )
            }
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
            inner.wegl_surface = wegl_surface;

            func(inner.callbacks.as_any(), &window, gl_state)
        })
    }

    fn get_clipboard(&self) -> Future<String> {
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
                        promise.ok(result);
                    }
                    Err(e) => {
                        log::error!("while reading clipboard: {}", e);
                        promise.err(failure::format_err!("{}", e));
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
                .data_device_manager
                .create_data_source(move |source| {
                    source.implement_closure(
                        move |event, _source| match event {
                            DataSourceEvent::Send { fd, .. } => {
                                let fd = unsafe { FileDescriptor::from_raw_fd(fd) };
                                if let Err(e) = write_pipe_with_timeout(fd, text.as_bytes()) {
                                    log::error!("while sending paste to pipe: {}", e);
                                }
                            }
                            _ => {}
                        },
                        (),
                    )
                })
                .map_err(|_| failure::format_err!("failed to create data source"))?;
            source.offer(TEXT_MIME_TYPE.to_string());
            inner.copy_and_paste.lock().unwrap().set_selection(source);

            Ok(())
        })
    }
}

fn write_pipe_with_timeout(mut file: FileDescriptor, data: &[u8]) -> Fallible<()> {
    let on: libc::c_int = 1;
    unsafe {
        libc::ioctl(file.as_raw_fd(), libc::FIONBIO, &on);
    }
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
                    failure::bail!("zero byte write");
                }
                Ok(size) => {
                    buf = &buf[size..];
                }
                Err(e) => failure::bail!("error writing to pipe: {}", e),
            }
        } else {
            failure::bail!("timed out writing to pipe");
        }
    }

    Ok(())
}

fn read_pipe_with_timeout(mut file: FileDescriptor) -> Fallible<String> {
    let mut result = Vec::new();

    let on: libc::c_int = 1;
    unsafe {
        libc::ioctl(file.as_raw_fd(), libc::FIONBIO, &on);
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
                Err(e) => failure::bail!("error reading from pipe: {}", e),
            }
        } else {
            failure::bail!("timed out reading from pipe");
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

        if !conn.environment.borrow().shell.needs_configure() {
            self.do_paint().unwrap();
        } else {
            self.refresh_frame();
        }
    }

    fn set_cursor(&mut self, _cursor: Option<MouseCursor>) {}

    fn invalidate(&mut self) {
        self.need_paint = true;
        self.do_paint().unwrap();
    }

    fn set_inner_size(&self, _width: usize, _height: usize) {}

    fn set_window_position(&self, _coords: ScreenPoint) {}

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        if let Some(window) = self.window.as_ref() {
            window.set_title(title.to_string());
        }
        self.refresh_frame();
    }
}
