#![cfg(all(unix, not(target_os = "macos")))]

use crate::{KeyCode, Modifiers};

pub fn modifiers_from_state(state: u32) -> Modifiers {
    let mut mods = Modifiers::default();

    if (state & xcb::x::ModMask::SHIFT.bits()) != 0 {
        mods |= Modifiers::SHIFT;
    }
    if (state & xcb::x::ModMask::CONTROL.bits()) != 0 {
        mods |= Modifiers::CTRL;
    }
    if (state & xcb::x::ModMask::N1.bits()) != 0 {
        mods |= Modifiers::ALT;
    }
    if (state & xcb::x::ModMask::N4.bits()) != 0 {
        mods |= Modifiers::SUPER;
    }

    mods
}

/// Translates non-printable X11 keysym to KeyCode
/// for missing keys, look into `/usr/include/X11/keysymdef.h`
/// and/or define them in KeyCode.
pub fn keysym_to_keycode(keysym: u32) -> Option<KeyCode> {
    let utf32 = xkbcommon::xkb::keysym_to_utf32(keysym.into());
    if utf32 >= 0x20 {
        // Unsafety: this is ok because we trust that keysym_to_utf32
        // is only going to return valid utf32 codepoints.
        // Note that keysym_to_utf32 returns 0 for no match.
        unsafe {
            return Some(KeyCode::Char(std::char::from_u32_unchecked(utf32)));
        }
    }

    use xkbcommon::xkb::keysyms::*;
    #[allow(non_upper_case_globals)]
    Some(match keysym {
        KEY_Escape => KeyCode::Char('\u{1b}'),
        KEY_Tab => KeyCode::Char('\t'),
        KEY_ISO_Left_Tab => KeyCode::Char('\t'),

        KEY_BackSpace => KeyCode::Char('\u{8}'),
        KEY_Return => KeyCode::Char('\r'),
        KEY_Insert => KeyCode::Insert,
        KEY_Delete => KeyCode::Char('\u{7f}'),
        KEY_Clear => KeyCode::Clear,
        KEY_Pause => KeyCode::Pause,
        KEY_Print => KeyCode::Print,

        // latin-1
        i @ KEY_space..=KEY_ydiaeresis => KeyCode::Char(i as u8 as char),

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

        KEY_VoidSymbol => KeyCode::VoidSymbol,

        i @ KEY_F1..=KEY_F24 => KeyCode::Function((1 + i - KEY_F1) as u8),

        // numeric and function keypad keys
        KEY_KP_Enter => KeyCode::Char(0xdu8 as char),
        KEY_KP_Delete => KeyCode::Char('\u{7f}'),
        KEY_KP_Home => KeyCode::KeyPadHome,
        KEY_KP_End => KeyCode::KeyPadEnd,
        KEY_KP_Page_Up => KeyCode::KeyPadPageUp,
        KEY_KP_Page_Down => KeyCode::KeyPadPageDown,
        KEY_KP_Begin => KeyCode::KeyPadBegin,
        KEY_KP_Multiply => KeyCode::Multiply,
        KEY_KP_Add => KeyCode::Add,
        KEY_KP_Divide => KeyCode::Divide,
        KEY_KP_Subtract => KeyCode::Subtract,
        KEY_KP_Decimal => KeyCode::Decimal,
        KEY_KP_Separator => KeyCode::Separator,
        KEY_KP_Space => KeyCode::Char(' '),
        KEY_KP_Tab => KeyCode::Char('\t'),
        KEY_KP_Left => KeyCode::ApplicationLeftArrow,
        KEY_KP_Up => KeyCode::ApplicationUpArrow,
        KEY_KP_Right => KeyCode::ApplicationRightArrow,
        KEY_KP_Down => KeyCode::ApplicationDownArrow,
        KEY_KP_Insert => KeyCode::Insert,
        KEY_KP_Equal => KeyCode::Char('='),

        i @ KEY_KP_0..=KEY_KP_9 => KeyCode::Numpad((i - KEY_KP_0) as u8),

        KEY_XF86Copy => KeyCode::Copy,
        KEY_XF86Cut => KeyCode::Cut,
        KEY_XF86Paste => KeyCode::Paste,

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
