#![allow(non_upper_case_globals, dead_code)]
use xcb::KeyPressEvent;
use xcb::ffi::xproto::xcb_keysym_t;

pub const XK_VoidSymbol: xcb_keysym_t = 0xffffff;
pub const XK_BackSpace: xcb_keysym_t = 0xff08;
pub const XK_Tab: xcb_keysym_t = 0xff09;
pub const XK_Linefeed: xcb_keysym_t = 0xff0a;
pub const XK_Clear: xcb_keysym_t = 0xff0b;
pub const XK_Return: xcb_keysym_t = 0xff0d;
pub const XK_Pause: xcb_keysym_t = 0xff13;
pub const XK_Scroll_Lock: xcb_keysym_t = 0xff14;
pub const XK_Sys_Req: xcb_keysym_t = 0xff15;
pub const XK_Escape: xcb_keysym_t = 0xff1b;
pub const XK_Delete: xcb_keysym_t = 0xffff;

pub const XK_Home: xcb_keysym_t = 0xff50;
pub const XK_Left: xcb_keysym_t = 0xff51;
pub const XK_Up: xcb_keysym_t = 0xff52;
pub const XK_Right: xcb_keysym_t = 0xff53;
pub const XK_Down: xcb_keysym_t = 0xff54;
pub const XK_Prior: xcb_keysym_t = 0xff55;
pub const XK_Page_Up: xcb_keysym_t = 0xff55;
pub const XK_Next: xcb_keysym_t = 0xff56;
pub const XK_Page_Down: xcb_keysym_t = 0xff56;
pub const XK_End: xcb_keysym_t = 0xff57;
pub const XK_Begin: xcb_keysym_t = 0xff58;

pub const XK_KP_Space: xcb_keysym_t = 0xff80;
pub const XK_KP_Tab: xcb_keysym_t = 0xff89;
pub const XK_KP_Enter: xcb_keysym_t = 0xff8d;
pub const XK_KP_F1: xcb_keysym_t = 0xff91;
pub const XK_KP_F2: xcb_keysym_t = 0xff92;
pub const XK_KP_F3: xcb_keysym_t = 0xff93;
pub const XK_KP_F4: xcb_keysym_t = 0xff94;
pub const XK_KP_Home: xcb_keysym_t = 0xff95;
pub const XK_KP_Left: xcb_keysym_t = 0xff96;
pub const XK_KP_Up: xcb_keysym_t = 0xff97;
pub const XK_KP_Right: xcb_keysym_t = 0xff98;
pub const XK_KP_Down: xcb_keysym_t = 0xff99;
pub const XK_KP_Prior: xcb_keysym_t = 0xff9a;
pub const XK_KP_Page_Up: xcb_keysym_t = 0xff9a;
pub const XK_KP_Next: xcb_keysym_t = 0xff9b;
pub const XK_KP_Page_Down: xcb_keysym_t = 0xff9b;
pub const XK_KP_End: xcb_keysym_t = 0xff9c;
pub const XK_KP_Begin: xcb_keysym_t = 0xff9d;
pub const XK_KP_Insert: xcb_keysym_t = 0xff9e;
pub const XK_KP_Delete: xcb_keysym_t = 0xff9f;
pub const XK_KP_Equal: xcb_keysym_t = 0xffbd;
pub const XK_KP_Multiply: xcb_keysym_t = 0xffaa;
pub const XK_KP_Add: xcb_keysym_t = 0xffab;
pub const XK_KP_Separator: xcb_keysym_t = 0xffac;
pub const XK_KP_Subtract: xcb_keysym_t = 0xffad;
pub const XK_KP_Decimal: xcb_keysym_t = 0xffae;
pub const XK_KP_Divide: xcb_keysym_t = 0xffaf;

pub const XK_KP_0: xcb_keysym_t = 0xffb0;
pub const XK_KP_1: xcb_keysym_t = 0xffb1;
pub const XK_KP_2: xcb_keysym_t = 0xffb2;
pub const XK_KP_3: xcb_keysym_t = 0xffb3;
pub const XK_KP_4: xcb_keysym_t = 0xffb4;
pub const XK_KP_5: xcb_keysym_t = 0xffb5;
pub const XK_KP_6: xcb_keysym_t = 0xffb6;
pub const XK_KP_7: xcb_keysym_t = 0xffb7;
pub const XK_KP_8: xcb_keysym_t = 0xffb8;
pub const XK_KP_9: xcb_keysym_t = 0xffb9;

pub const XK_F1: xcb_keysym_t = 0xffbe;
pub const XK_F2: xcb_keysym_t = 0xffbf;
pub const XK_F3: xcb_keysym_t = 0xffc0;
pub const XK_F4: xcb_keysym_t = 0xffc1;
pub const XK_F5: xcb_keysym_t = 0xffc2;
pub const XK_F6: xcb_keysym_t = 0xffc3;
pub const XK_F7: xcb_keysym_t = 0xffc4;
pub const XK_F8: xcb_keysym_t = 0xffc5;
pub const XK_F9: xcb_keysym_t = 0xffc6;
pub const XK_F10: xcb_keysym_t = 0xffc7;
pub const XK_F11: xcb_keysym_t = 0xffc8;
pub const XK_F12: xcb_keysym_t = 0xffc9;

pub const XK_Shift_L: xcb_keysym_t = 0xffe1;
pub const XK_Shift_R: xcb_keysym_t = 0xffe2;
pub const XK_Control_L: xcb_keysym_t = 0xffe3;
pub const XK_Control_R: xcb_keysym_t = 0xffe4;
pub const XK_Caps_Lock: xcb_keysym_t = 0xffe5;
pub const XK_Shift_Lock: xcb_keysym_t = 0xffe6;

pub const XK_Meta_L: xcb_keysym_t = 0xffe7;
pub const XK_Meta_R: xcb_keysym_t = 0xffe8;
pub const XK_Alt_L: xcb_keysym_t = 0xffe9;
pub const XK_Alt_R: xcb_keysym_t = 0xffea;
pub const XK_Super_L: xcb_keysym_t = 0xffeb;
pub const XK_Super_R: xcb_keysym_t = 0xffec;
pub const XK_Hyper_L: xcb_keysym_t = 0xffed;
pub const XK_Hyper_R: xcb_keysym_t = 0xffee;

pub const XK_space: xcb_keysym_t = 0x0020;
pub const XK_exclam: xcb_keysym_t = 0x0021;
pub const XK_quotedbl: xcb_keysym_t = 0x0022;
pub const XK_numbersign: xcb_keysym_t = 0x0023;
pub const XK_dollar: xcb_keysym_t = 0x0024;
pub const XK_percent: xcb_keysym_t = 0x0025;
pub const XK_ampersand: xcb_keysym_t = 0x0026;
pub const XK_apostrophe: xcb_keysym_t = 0x0027;
pub const XK_quoteright: xcb_keysym_t = 0x0027;
pub const XK_parenleft: xcb_keysym_t = 0x0028;
pub const XK_parenright: xcb_keysym_t = 0x0029;
pub const XK_asterisk: xcb_keysym_t = 0x002a;
pub const XK_plus: xcb_keysym_t = 0x002b;
pub const XK_comma: xcb_keysym_t = 0x002c;
pub const XK_minus: xcb_keysym_t = 0x002d;
pub const XK_period: xcb_keysym_t = 0x002e;
pub const XK_slash: xcb_keysym_t = 0x002f;
pub const XK_0: xcb_keysym_t = 0x0030;
pub const XK_1: xcb_keysym_t = 0x0031;
pub const XK_2: xcb_keysym_t = 0x0032;
pub const XK_3: xcb_keysym_t = 0x0033;
pub const XK_4: xcb_keysym_t = 0x0034;
pub const XK_5: xcb_keysym_t = 0x0035;
pub const XK_6: xcb_keysym_t = 0x0036;
pub const XK_7: xcb_keysym_t = 0x0037;
pub const XK_8: xcb_keysym_t = 0x0038;
pub const XK_9: xcb_keysym_t = 0x0039;
pub const XK_colon: xcb_keysym_t = 0x003a;
pub const XK_semicolon: xcb_keysym_t = 0x003b;
pub const XK_less: xcb_keysym_t = 0x003c;
pub const XK_equal: xcb_keysym_t = 0x003d;
pub const XK_greater: xcb_keysym_t = 0x003e;
pub const XK_question: xcb_keysym_t = 0x003f;
pub const XK_at: xcb_keysym_t = 0x0040;
pub const XK_A: xcb_keysym_t = 0x0041;
pub const XK_B: xcb_keysym_t = 0x0042;
pub const XK_C: xcb_keysym_t = 0x0043;
pub const XK_D: xcb_keysym_t = 0x0044;
pub const XK_E: xcb_keysym_t = 0x0045;
pub const XK_F: xcb_keysym_t = 0x0046;
pub const XK_G: xcb_keysym_t = 0x0047;
pub const XK_H: xcb_keysym_t = 0x0048;
pub const XK_I: xcb_keysym_t = 0x0049;
pub const XK_J: xcb_keysym_t = 0x004a;
pub const XK_K: xcb_keysym_t = 0x004b;
pub const XK_L: xcb_keysym_t = 0x004c;
pub const XK_M: xcb_keysym_t = 0x004d;
pub const XK_N: xcb_keysym_t = 0x004e;
pub const XK_O: xcb_keysym_t = 0x004f;
pub const XK_P: xcb_keysym_t = 0x0050;
pub const XK_Q: xcb_keysym_t = 0x0051;
pub const XK_R: xcb_keysym_t = 0x0052;
pub const XK_S: xcb_keysym_t = 0x0053;
pub const XK_T: xcb_keysym_t = 0x0054;
pub const XK_U: xcb_keysym_t = 0x0055;
pub const XK_V: xcb_keysym_t = 0x0056;
pub const XK_W: xcb_keysym_t = 0x0057;
pub const XK_X: xcb_keysym_t = 0x0058;
pub const XK_Y: xcb_keysym_t = 0x0059;
pub const XK_Z: xcb_keysym_t = 0x005a;
pub const XK_bracketleft: xcb_keysym_t = 0x005b;
pub const XK_backslash: xcb_keysym_t = 0x005c;
pub const XK_bracketright: xcb_keysym_t = 0x005d;
pub const XK_asciicircum: xcb_keysym_t = 0x005e;
pub const XK_underscore: xcb_keysym_t = 0x005f;
pub const XK_grave: xcb_keysym_t = 0x0060;
pub const XK_quoteleft: xcb_keysym_t = 0x0060;
pub const XK_a: xcb_keysym_t = 0x0061;
pub const XK_b: xcb_keysym_t = 0x0062;
pub const XK_c: xcb_keysym_t = 0x0063;
pub const XK_d: xcb_keysym_t = 0x0064;
pub const XK_e: xcb_keysym_t = 0x0065;
pub const XK_f: xcb_keysym_t = 0x0066;
pub const XK_g: xcb_keysym_t = 0x0067;
pub const XK_h: xcb_keysym_t = 0x0068;
pub const XK_i: xcb_keysym_t = 0x0069;
pub const XK_j: xcb_keysym_t = 0x006a;
pub const XK_k: xcb_keysym_t = 0x006b;
pub const XK_l: xcb_keysym_t = 0x006c;
pub const XK_m: xcb_keysym_t = 0x006d;
pub const XK_n: xcb_keysym_t = 0x006e;
pub const XK_o: xcb_keysym_t = 0x006f;
pub const XK_p: xcb_keysym_t = 0x0070;
pub const XK_q: xcb_keysym_t = 0x0071;
pub const XK_r: xcb_keysym_t = 0x0072;
pub const XK_s: xcb_keysym_t = 0x0073;
pub const XK_t: xcb_keysym_t = 0x0074;
pub const XK_u: xcb_keysym_t = 0x0075;
pub const XK_v: xcb_keysym_t = 0x0076;
pub const XK_w: xcb_keysym_t = 0x0077;
pub const XK_x: xcb_keysym_t = 0x0078;
pub const XK_y: xcb_keysym_t = 0x0079;
pub const XK_z: xcb_keysym_t = 0x007a;
pub const XK_braceleft: xcb_keysym_t = 0x007b;
pub const XK_bar: xcb_keysym_t = 0x007c;
pub const XK_braceright: xcb_keysym_t = 0x007d;
pub const XK_asciitilde: xcb_keysym_t = 0x007e;

use term::KeyCode;
use term::KeyModifiers;

impl From<xcb_keysym_t> for KeyCode {
    fn from(k: xcb_keysym_t) -> Self {
        match k {
            XK_space...XK_asciitilde => {
                // This range overlaps with ascii
                KeyCode::Char(k as u8 as char)
            }
            XK_BackSpace | XK_Tab | XK_Linefeed | XK_Return | XK_Escape => {
                KeyCode::Char((k & 0xff) as u8 as char)
            }
            XK_Control_L | XK_Control_R => KeyCode::Control,
            XK_Alt_L | XK_Alt_R => KeyCode::Alt,
            XK_Meta_L | XK_Meta_R => KeyCode::Meta,
            XK_Super_L | XK_Super_R => KeyCode::Super,
            XK_Hyper_L | XK_Hyper_R => KeyCode::Hyper,
            XK_Shift_L | XK_Shift_R => KeyCode::Shift,
            _ => KeyCode::Unknown,
        }
    }
}

pub fn modifiers(event: &KeyPressEvent) -> KeyModifiers {
    use xcb::xproto::*;

    let mut mods = KeyModifiers::default();
    let state = event.state() as u32;

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
