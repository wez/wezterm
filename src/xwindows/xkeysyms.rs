#![allow(non_upper_case_globals)]
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

/// Translates non-printable X11 keysym to termwiz::KeyCode
/// for missing keys, look into ```/usr/include/X11/keysymdef.h``` and/or define them in KeyCode.
/// If we can find a unicode representation of the input key then this function is skipped.
pub fn keysym_to_keycode(keysym: u32) -> Option<KeyCode> {
    use xkbcommon::xkb::keysyms::*;
    let res = match keysym {
        KEY_Escape => KeyCode::Escape,
        KEY_Tab => KeyCode::Tab,

        KEY_BackSpace => KeyCode::Backspace,
        KEY_Return => KeyCode::Char(0xdu8 as char),
        KEY_Insert => KeyCode::Insert,
        KEY_Delete => KeyCode::Delete,
        KEY_Clear => KeyCode::Delete,
        KEY_Pause => KeyCode::Pause,
        KEY_Print => KeyCode::Print,

        // cursor movement
        KEY_Home => KeyCode::Home,
        KEY_End => KeyCode::End,
        KEY_Left => KeyCode::LeftArrow,
        KEY_Up => KeyCode::UpArrow,
        KEY_Right => KeyCode::RightArrow,
        KEY_Down => KeyCode::DownArrow,
        KEY_Page_Up => KeyCode::PageUp,
        KEY_Page_Down => KeyCode::PageDown,

        // modifiers
        KEY_Shift_L => KeyCode::Shift,
        KEY_Shift_R => KeyCode::Shift,

        KEY_Control_L => KeyCode::Control,
        KEY_Control_R => KeyCode::Control,
        KEY_Alt_L => KeyCode::Alt,
        KEY_Alt_R => KeyCode::Alt,
        KEY_Caps_Lock => KeyCode::CapsLock,
        KEY_Num_Lock => KeyCode::NumLock,
        KEY_Scroll_Lock => KeyCode::ScrollLock,
        KEY_Super_L => KeyCode::Super,
        KEY_Super_R => KeyCode::Super,
        KEY_Menu => KeyCode::Menu,
        KEY_Help => KeyCode::Help,

        KEY_F1 => KeyCode::Function(1),
        KEY_F2 => KeyCode::Function(2),
        KEY_F3 => KeyCode::Function(3),
        KEY_F4 => KeyCode::Function(4),
        KEY_F5 => KeyCode::Function(5),
        KEY_F6 => KeyCode::Function(6),
        KEY_F7 => KeyCode::Function(7),
        KEY_F8 => KeyCode::Function(8),
        KEY_F9 => KeyCode::Function(9),
        KEY_F10 => KeyCode::Function(10),
        KEY_F11 => KeyCode::Function(11),
        KEY_F12 => KeyCode::Function(12),

        // numeric and function keypad keys
        KEY_KP_Enter => KeyCode::Char(0xdu8 as char),
        KEY_KP_Delete => KeyCode::Delete,
        KEY_KP_Home => KeyCode::Home,
        KEY_KP_Page_Up => KeyCode::PageUp,
        KEY_KP_Page_Down => KeyCode::PageDown,
        KEY_KP_Multiply => KeyCode::Multiply,
        KEY_KP_Add => KeyCode::Add,
        KEY_KP_Divide => KeyCode::Divide,
        KEY_KP_Subtract => KeyCode::Subtract,
        KEY_KP_Decimal => KeyCode::Decimal,
        KEY_KP_Separator => KeyCode::Separator,

        KEY_KP_0 => KeyCode::Numpad0,
        KEY_KP_1 => KeyCode::Numpad1,
        KEY_KP_2 => KeyCode::Numpad2,
        KEY_KP_3 => KeyCode::Numpad3,
        KEY_KP_4 => KeyCode::Numpad4,
        KEY_KP_6 => KeyCode::Numpad6,
        KEY_KP_7 => KeyCode::Numpad7,
        KEY_KP_8 => KeyCode::Numpad8,
        KEY_KP_9 => KeyCode::Numpad9,

        KEY_XF86Back => KeyCode::BrowserBack,
        KEY_XF86Forward => KeyCode::BrowserForward,
        KEY_XF86Stop => KeyCode::BrowserStop,
        KEY_XF86Refresh => KeyCode::BrowserRefresh,
        KEY_XF86Favorites => KeyCode::BrowserFavorites,
        KEY_XF86HomePage => KeyCode::BrowserHome,

        KEY_XF86AudioLowerVolume => KeyCode::VolumeDown,
        KEY_XF86AudioMute => KeyCode::VolumeMute,
        KEY_XF86AudioRaiseVolume => KeyCode::VolumeUp,
        _ => {
            return None;
        }
    };
    Some(res)
}
