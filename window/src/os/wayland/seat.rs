use std::borrow::BorrowMut;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, QueueHandle};

use super::state::WaylandState;
use super::KeyRepeatState;

impl SeatHandler for WaylandState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {
        todo!()
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: smithay_client_toolkit::seat::Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            log::trace!("Setting keyboard capability");
            let keyboard = self
                .seat
                .get_keyboard(qh, &seat, None)
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        _capability: smithay_client_toolkit::seat::Capability,
    ) {
        todo!()
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {
        todo!()
    }
}

impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[u32],
    ) {
        // *self.active_surface_id.borrow_mut() = Some(surface.id());
        // *self.last_serial.borrow_mut() = serial;
        // if let Some(sud) = SurfaceUserData::try_from_wl(surface) {
        //     let window_id = sud.window_id;
        //     self.keyboard_window_id.borrow_mut().replace(window_id);
        //     // TODO: env with inner seems to IME stuff
        // } else {
        //     log::warn!("{:?}, no known surface", "WlKeyboardEnter");
        // }
        //
        // if let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() {
        //     // if let Some(win) = self.
        // }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        serial: u32,
    ) {
        // TODO: inner input
        *self.last_serial.borrow_mut() = serial;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        *self.last_serial.borrow_mut() = serial;
        let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() else {
            return;
        };
        let Some(mut win) = self.window_by_id(window_id) else {
            return;
        };
        let inner = win.borrow_mut();

        let events = &mut inner.as_ref().borrow_mut().events;
        let mapper = self.keyboard_mapper.borrow_mut();
        let mapper = mapper.as_mut().expect("no keymap");

        // TODO: not sure if i should use keysym vs rawcode
        let key = event.keysym;
        let key_repeat = &mut inner.as_ref().borrow_mut().key_repeat;
        if let Some(event) = mapper.process_wayland_key(key, true, events) {
            let rep = Arc::new(Mutex::new(KeyRepeatState {
                when: Instant::now(),
                event,
            }));

            let kp = &mut inner.as_ref().borrow_mut().key_repeat;
            kp.replace((key, Arc::clone(&rep)));

            KeyRepeatState::schedule(rep, window_id);
        } else if let Some((cur_key, _)) = key_repeat {
            if *cur_key == key {
                key_repeat.take();
            }
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // TODO: copy paste of press_key except process is false
        *self.last_serial.borrow_mut() = serial;
        let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() else {
            return;
        };
        let Some(mut win) = self.window_by_id(window_id) else {
            return;
        };
        let inner = win.borrow_mut();

        let events = &mut inner.as_ref().borrow_mut().events;
        let mapper = self.keyboard_mapper.borrow_mut();
        let mapper = mapper.as_mut().expect("no keymap");

        // TODO: not sure if i should use keysym vs rawcode
        let key = event.keysym;
        let key_repeat = &mut inner.as_ref().borrow_mut().key_repeat;
        if let Some(event) = mapper.process_wayland_key(key, false, events) {
            let rep = Arc::new(Mutex::new(KeyRepeatState {
                when: Instant::now(),
                event,
            }));

            let kp = &mut inner.as_ref().borrow_mut().key_repeat;
            kp.replace((key, Arc::clone(&rep)));

            KeyRepeatState::schedule(rep, window_id);
        } else if let Some((cur_key, _)) = key_repeat {
            if *cur_key == key {
                key_repeat.take();
            }
        }
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        serial: u32,
        _modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
    ) {
        *self.last_serial.borrow_mut() = serial;
    }

    fn update_keymap(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _keymap: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
    }
}
