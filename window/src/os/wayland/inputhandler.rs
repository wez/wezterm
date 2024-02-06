//! Implements zwp_text_input_v3 for handling IME
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::Mutex;

use smithay_client_toolkit::globals::GlobalData;
use wayland_client::backend::ObjectId;
use wayland_client::globals::{BindError, GlobalList};
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    Event as TextInputEvent, ZwpTextInputV3,
};
use wezterm_input_types::{KeyCode, KeyEvent, KeyboardLedStatus, Modifiers};

use crate::{DeadKeyStatus, WindowEvent};

use super::state::WaylandState;

#[derive(Clone, Default, Debug)]
struct PendingState {
    pre_edit: Option<String>,
    commit: Option<String>,
}

pub(super) struct TextInputState {
    text_input_manager: ZwpTextInputManagerV3,
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    input_by_seat: HashMap<ObjectId, ZwpTextInputV3>,
    keyboard_to_seat: HashMap<ObjectId, ObjectId>,
    surface_to_keyboard: HashMap<ObjectId, ObjectId>,
    pending_state: HashMap<ObjectId, PendingState>,
}

impl TextInputState {
    pub(super) fn bind(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WaylandState>,
    ) -> Result<Self, BindError> {
        let text_input_manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self {
            text_input_manager,
            inner: Mutex::new(Inner::default()),
        })
    }

    pub fn get_text_input_for_keyboard(&self, keyboard: &WlKeyboard) -> Option<ZwpTextInputV3> {
        let inner = self.inner.lock().unwrap();
        let keyboard_id = keyboard.id();
        let seat_id = inner.keyboard_to_seat.get(&keyboard_id)?;
        inner.input_by_seat.get(&seat_id).cloned()
    }

    pub(super) fn get_text_input_for_surface(&self, surface: &WlSurface) -> Option<ZwpTextInputV3> {
        let inner = self.inner.lock().unwrap();
        let surface_id = surface.id();
        let keyboard_id = inner.surface_to_keyboard.get(&surface_id)?;
        let seat_id = inner.keyboard_to_seat.get(&keyboard_id)?;
        inner.input_by_seat.get(&seat_id).cloned()
    }

    fn get_text_input_for_seat(
        &self,
        seat: &WlSeat,
        qh: &QueueHandle<WaylandState>,
    ) -> Option<ZwpTextInputV3> {
        let mgr = &self.text_input_manager;
        let mut inner = self.inner.lock().unwrap();
        let seat_id = seat.id();
        let input = inner.input_by_seat.entry(seat_id).or_insert_with(|| {
            let input = mgr.get_text_input(seat, &qh, TextInputData::default());
            input.into()
        });
        Some(input.clone())
    }

    pub(super) fn advise_surface(&self, surface: &WlSurface, keyboard: &WlKeyboard) {
        let surface_id = surface.id();
        let keyboard_id = keyboard.id();
        self.inner
            .lock()
            .unwrap()
            .surface_to_keyboard
            .insert(surface_id, keyboard_id);
    }

    pub(super) fn advise_seat(
        &self,
        seat: &WlSeat,
        keyboard: &WlKeyboard,
        qh: &QueueHandle<WaylandState>,
    ) {
        self.get_text_input_for_seat(seat, qh);
        let keyboard_id = keyboard.id();
        let seat_id = seat.id();
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
}

impl Inner {
    fn disable_all(&mut self) {
        for input in self.input_by_seat.values() {
            input.disable();
            input.commit();
        }
    }
}

#[derive(Default)]
pub(super) struct TextInputData {
    // XXX: inner could probably be moved here
    _inner: Mutex<TextInputDataInner>,
}

#[derive(Default)]
pub(super) struct TextInputDataInner {}

impl Dispatch<ZwpTextInputManagerV3, GlobalData, WaylandState> for TextInputState {
    fn event(
        _state: &mut WaylandState,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &wayland_client::Connection,
        _qhandle: &QueueHandle<WaylandState>,
    ) {
        // No events from ZwpTextInputMangerV3
        unreachable!();
    }
}

impl Dispatch<ZwpTextInputV3, TextInputData, WaylandState> for TextInputState {
    fn event(
        state: &mut WaylandState,
        input: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        _data: &TextInputData,
        _conn: &wayland_client::Connection,
        _qhandle: &QueueHandle<WaylandState>,
    ) {
        log::trace!("ZwpTextInputEvent: {event:?}");
        let mut pending_state = {
            let text_input = state.text_input.as_mut().unwrap();
            let mut inner = text_input.inner.lock().unwrap();
            inner.pending_state.entry(input.id()).or_default().clone()
        };

        match event {
            TextInputEvent::PreeditString {
                text,
                cursor_begin: _,
                cursor_end: _,
            } => {
                pending_state.pre_edit = text;
            }
            TextInputEvent::CommitString { text } => {
                pending_state.commit = text;
                state.dispatch_to_focused_window(WindowEvent::AdviseDeadKeyStatus(
                    DeadKeyStatus::None,
                ));
            }
            TextInputEvent::Done { serial } => {
                *state.last_serial.borrow_mut() = serial;
                if let Some(text) = pending_state.commit.take() {
                    state.dispatch_to_focused_window(WindowEvent::KeyEvent(KeyEvent {
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
                state.dispatch_to_focused_window(WindowEvent::AdviseDeadKeyStatus(status));
            }
            _ => {}
        }

        state
            .text_input
            .as_ref()
            .unwrap()
            .inner
            .lock()
            .unwrap()
            .pending_state
            .insert(input.id(), pending_state);
    }
}

impl WaylandState {
    fn dispatch_to_focused_window(&self, event: WindowEvent) {
        if let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() {
            if let Some(win) = self.window_by_id(window_id) {
                let mut inner = win.borrow_mut();
                inner.events.dispatch(event);
            }
        }
    }
}

impl Drop for WaylandState {
    fn drop(&mut self) {
        if let Some(text_input) = self.text_input.as_mut() {
            text_input.shutdown();
        }
    }
}
