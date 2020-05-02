use crate::input::*;
use crate::os::wayland::connection::WaylandConnection;
use anyhow::anyhow;
use smithay_client_toolkit as toolkit;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use toolkit::reexports::calloop::LoopHandle;
use toolkit::seat::keyboard::{
    map_keyboard_repeat, Event as KbEvent, KeyState, ModifiersState, RepeatKind,
};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Attached;

#[derive(Default)]
struct Inner {
    active_surface_id: u32,
    surface_to_window_id: HashMap<u32, usize>,
}

impl Inner {
    fn handle_event(&mut self, evt: KbEvent) {
        // Track the most recently entered window surface.
        // We manually filter to the keys of surface_to_window_id
        // because we may have auxilliary surfaces on our connection
        // that were created by the window decorations and we don't
        // want to suppress keyboard input if the user clicked in
        // the titlebar.
        if let KbEvent::Enter { surface, .. } = &evt {
            let id = surface.as_ref().id();
            if self.surface_to_window_id.contains_key(&id) {
                self.active_surface_id = id;
            }
        }

        if let Some(event) = KeyboardEvent::from_event(evt) {
            self.dispatch_to_window(event);
        }
    }

    fn dispatch_to_window(&mut self, evt: KeyboardEvent) {
        if let Some(window_id) = self.surface_to_window_id.get(&self.active_surface_id) {
            let mut evt = Some(evt);
            WaylandConnection::with_window_inner(*window_id, move |inner| {
                inner.handle_keyboard_event(evt.take().unwrap());
                Ok(())
            });
        }
    }
}

#[derive(Clone)]
pub struct KeyboardDispatcher {
    inner: Arc<Mutex<Inner>>,
}

impl KeyboardDispatcher {
    pub fn new() -> Self {
        let inner = Arc::new(Mutex::new(Inner::default()));
        Self { inner }
    }

    pub fn register(
        &self,
        loop_handle: LoopHandle<()>,
        seat: &Attached<WlSeat>,
    ) -> anyhow::Result<()> {
        let inner = Arc::clone(&self.inner);
        let (_kbd, _source) = map_keyboard_repeat(
            loop_handle,
            &seat,
            None,
            RepeatKind::System,
            move |evt: KbEvent, _, _| {
                inner.lock().unwrap().handle_event(evt);
            },
        )
        .map_err(|e| anyhow!("Failed to configure keyboard callback: {:?}", e))?;

        Ok(())
    }

    pub fn add_window(&self, window_id: usize, surface: &WlSurface) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .surface_to_window_id
            .insert(surface.as_ref().id(), window_id);
    }
}

#[derive(Clone, Debug)]
pub enum KeyboardEvent {
    Enter {
        serial: u32,
    },
    Leave {
        serial: u32,
    },
    Key {
        rawkey: u32,
        keysym: u32,
        is_down: bool,
        serial: u32,
        utf8: Option<String>,
    },
    Modifiers {
        modifiers: Modifiers,
    },
}

impl KeyboardEvent {
    fn from_event(evt: KbEvent) -> Option<Self> {
        Some(match evt {
            KbEvent::Enter { serial, .. } => KeyboardEvent::Enter { serial },
            KbEvent::Leave { serial, .. } => KeyboardEvent::Leave { serial },
            KbEvent::Key {
                rawkey,
                keysym,
                state,
                serial,
                utf8,
                ..
            } => KeyboardEvent::Key {
                rawkey,
                keysym,
                is_down: state == KeyState::Pressed,
                serial,
                utf8,
            },
            KbEvent::Repeat {
                rawkey,
                keysym,
                utf8,
                ..
            } => KeyboardEvent::Key {
                rawkey,
                keysym,
                is_down: true,
                serial: 0,
                utf8,
            },
            KbEvent::Modifiers { modifiers } => KeyboardEvent::Modifiers {
                modifiers: modifier_keys(modifiers),
            },
        })
    }
}

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
