//! Implements zwp_text_input_v3 for handling IME
use crate::connection::ConnectionOps;
use crate::os::wayland::{wl_id, WaylandConnection};
use crate::{DeadKeyStatus, KeyCode, KeyEvent, Modifiers, WindowEvent};
use smithay_client_toolkit::environment::GlobalHandler;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Attached, DispatchData, Main};
use wayland_protocols::unstable::text_input::v3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::unstable::text_input::v3::client::zwp_text_input_v3::{
    Event, ZwpTextInputV3,
};
use wezterm_input_types::KeyboardLedStatus;

#[derive(Default, Debug)]
struct PendingState {
    pre_edit: Option<String>,
    commit: Option<String>,
}

#[derive(Debug, Default)]
struct Inner {
    input_by_seat: HashMap<u32, Attached<ZwpTextInputV3>>,
    keyboard_to_seat: HashMap<u32, u32>,
    surface_to_keyboard: HashMap<u32, u32>,
    pending_state: HashMap<u32, PendingState>,
}

impl Inner {
    fn handle_event(
        &mut self,
        input: Main<ZwpTextInputV3>,
        event: Event,
        _ddata: DispatchData,
        _inner: &Arc<Mutex<Self>>,
    ) {
        log::trace!("{event:?}");
        let conn = WaylandConnection::get().unwrap().wayland();
        let pending_state = self.pending_state.entry(wl_id(&**input)).or_default();
        match event {
            Event::PreeditString {
                text,
                cursor_begin: _,
                cursor_end: _,
            } => {
                pending_state.pre_edit = text;
            }
            Event::CommitString { text } => {
                pending_state.commit = text;
                conn.dispatch_to_focused_window(WindowEvent::AdviseDeadKeyStatus(
                    DeadKeyStatus::None,
                ));
            }
            Event::Done { serial } => {
                *conn.last_serial.borrow_mut() = serial;
                if let Some(text) = pending_state.commit.take() {
                    conn.dispatch_to_focused_window(WindowEvent::KeyEvent(KeyEvent {
                        key: KeyCode::composed(&text),
                        modifiers: Modifiers::NONE,
                        leds: KeyboardLedStatus::empty(),
                        repeat_count: 1,
                        key_is_down: true,
                        raw: None,
                    }));
                }
                let status = if let Some(text) = pending_state.pre_edit.take() {
                    DeadKeyStatus::Composing(text)
                } else {
                    DeadKeyStatus::None
                };
                conn.dispatch_to_focused_window(WindowEvent::AdviseDeadKeyStatus(status));
            }
            _ => {}
        }
    }

    fn disable_all(&mut self) {
        for input in self.input_by_seat.values() {
            input.disable();
            input.commit();
        }
    }
}

pub struct InputHandler {
    mgr: Option<Attached<ZwpTextInputManagerV3>>,
    inner: Arc<Mutex<Inner>>,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            mgr: None,
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    pub fn get_text_input_for_keyboard(
        &self,
        keyboard: &WlKeyboard,
    ) -> Option<Attached<ZwpTextInputV3>> {
        let inner = self.inner.lock().unwrap();
        let keyboard_id = wl_id(keyboard);
        let seat_id = inner.keyboard_to_seat.get(&keyboard_id)?;
        inner.input_by_seat.get(&seat_id).cloned()
    }

    pub fn get_text_input_for_surface(
        &self,
        surface: &WlSurface,
    ) -> Option<Attached<ZwpTextInputV3>> {
        let inner = self.inner.lock().unwrap();
        let surface_id = wl_id(surface);
        let keyboard_id = inner.surface_to_keyboard.get(&surface_id)?;
        let seat_id = inner.keyboard_to_seat.get(&keyboard_id)?;
        inner.input_by_seat.get(&seat_id).cloned()
    }

    pub fn get_text_input_for_seat(&self, seat: &WlSeat) -> Option<Attached<ZwpTextInputV3>> {
        let mgr = self.mgr.as_ref()?;
        let mut inner = self.inner.lock().unwrap();
        let seat_id = wl_id(seat);
        let input = inner.input_by_seat.entry(seat_id).or_insert_with(|| {
            let input = mgr.get_text_input(seat);
            let inner = Arc::clone(&self.inner);

            input.quick_assign(move |input, event, ddat| {
                inner
                    .lock()
                    .unwrap()
                    .handle_event(input, event, ddat, &inner);
            });

            input.into()
        });
        Some(input.clone())
    }

    pub fn advise_surface(&self, surface: &WlSurface, keyboard: &WlKeyboard) {
        let surface_id = wl_id(surface);
        let keyboard_id = wl_id(keyboard);
        self.inner
            .lock()
            .unwrap()
            .surface_to_keyboard
            .insert(surface_id, keyboard_id);
    }

    pub fn advise_seat(&self, seat: &WlSeat, keyboard: &WlKeyboard) {
        self.get_text_input_for_seat(seat);
        let keyboard_id = wl_id(keyboard);
        let seat_id = wl_id(seat);
        self.inner
            .lock()
            .unwrap()
            .keyboard_to_seat
            .insert(keyboard_id, seat_id);
    }

    /// Workaround for <https://gitlab.gnome.org/GNOME/gnome-shell/-/issues/4776>
    /// If we make sure to disable things before we close the app,
    /// mutter is less likely to get in a bad state
    pub fn shutdown(&self) {
        self.inner.lock().unwrap().disable_all();
    }

    pub fn seat_defunct(&self, seat: &WlSeat) {
        let seat_id = wl_id(seat);
        self.inner.lock().unwrap().input_by_seat.remove(&seat_id);
    }
}

impl GlobalHandler<ZwpTextInputManagerV3> for InputHandler {
    fn created(
        &mut self,
        registry: Attached<WlRegistry>,
        id: u32,
        version: u32,
        _ddata: DispatchData,
    ) {
        log::debug!("created ZwpTextInputV3 {id} {version}");
        let mgr = registry.bind::<ZwpTextInputManagerV3>(1, id);
        self.mgr.replace(mgr.into());
    }

    fn get(&self) -> std::option::Option<Attached<ZwpTextInputManagerV3>> {
        self.mgr.clone()
    }
}
