#![allow(non_upper_case_globals, dead_code)]
use xcb::ffi::xproto::xcb_keysym_t;
use xcb::KeyPressEvent;

use term::KeyCode;
use term::KeyModifiers;

pub fn modifiers_from_state(state: u16) -> KeyModifiers {
    use xcb::xproto::*;

    let mut mods = KeyModifiers::default();
    let state = u32::from(state);

    if state & MOD_MASK_SHIFT != 0 {
        mods |= KeyModifiers::SHIFT;
    }
    if state & MOD_MASK_CONTROL != 0 {
        mods |= KeyModifiers::CTRL;
    }
    if state & MOD_MASK_1 != 0 {
        mods |= KeyModifiers::ALT;
    }
    if state & MOD_MASK_4 != 0 {
        mods |= KeyModifiers::SUPER;
    }

    mods
}

pub fn modifiers(event: &KeyPressEvent) -> KeyModifiers {
    modifiers_from_state(event.state())
}
