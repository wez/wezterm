use crate::{KeyCode, Modifiers};

pub fn modifiers_from_state(state: u16) -> Modifiers {
    use xcb::xproto::*;

    let mut mods = Modifiers::default();
    let state = u32::from(state);

    if state & MOD_MASK_SHIFT != 0 {
        mods |= Modifiers::SHIFT;
    }
    if state & MOD_MASK_CONTROL != 0 {
        mods |= Modifiers::CTRL;
    }
    if state & MOD_MASK_1 != 0 {
        mods |= Modifiers::ALT;
    }
    if state & MOD_MASK_4 != 0 {
        mods |= Modifiers::SUPER;
    }

    mods
}

/// Translates non-printable X11 keysym to KeyCode
/// for missing keys, look into `/usr/include/X11/keysymdef.h`
/// and/or define them in KeyCode.
pub fn keysym_to_keycode(keysym: u32) -> Option<KeyCode> {
    use xkbcommon::xkb::keysyms::*;
    #[allow(non_upper_case_globals)]
    Some(match keysym {
        KEY_Escape => KeyCode::Char('\u{1b}'),
        KEY_Tab => KeyCode::Char('\t'),

        KEY_BackSpace => KeyCode::Char('\u{8}'),
        KEY_Return => KeyCode::Char('\r'),
        KEY_Insert => KeyCode::Insert,
        KEY_Delete => KeyCode::Char('\u{7f}'),
        KEY_Clear => KeyCode::Clear,
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
        KEY_Menu => KeyCode::Applications,
        KEY_Help => KeyCode::Help,

        i @ KEY_F1..=KEY_F12 => KeyCode::Function((1 + i - KEY_F1) as u8),

        // numeric and function keypad keys
        KEY_KP_Enter => KeyCode::Char(0xdu8 as char),
        KEY_KP_Delete => KeyCode::Char('\u{7f}'),
        KEY_KP_Home => KeyCode::Home,
        KEY_KP_Page_Up => KeyCode::PageUp,
        KEY_KP_Page_Down => KeyCode::PageDown,
        KEY_KP_Multiply => KeyCode::Multiply,
        KEY_KP_Add => KeyCode::Add,
        KEY_KP_Divide => KeyCode::Divide,
        KEY_KP_Subtract => KeyCode::Subtract,
        KEY_KP_Decimal => KeyCode::Decimal,
        KEY_KP_Separator => KeyCode::Separator,

        i @ KEY_KP_0..=KEY_KP_9 => KeyCode::Numpad((i - KEY_KP_0) as u8),

        KEY_XF86Back => KeyCode::BrowserBack,
        KEY_XF86Forward => KeyCode::BrowserForward,
        KEY_XF86Stop => KeyCode::BrowserStop,
        KEY_XF86Refresh => KeyCode::BrowserRefresh,
        KEY_XF86Favorites => KeyCode::BrowserFavorites,
        KEY_XF86HomePage => KeyCode::BrowserHome,

        KEY_XF86AudioLowerVolume => KeyCode::VolumeDown,
        KEY_XF86AudioMute => KeyCode::VolumeMute,
        KEY_XF86AudioRaiseVolume => KeyCode::VolumeUp,
        _ => return None,
    })
}
