#![allow(non_upper_case_globals)]
#![allow(dead_code)]
use std::collections::HashMap;
use wezterm_input_types::PhysKeyCode;

fn build_map() -> HashMap<u16, PhysKeyCode> {
    [
        (kVK_ANSI_A, PhysKeyCode::A),
        (kVK_ANSI_S, PhysKeyCode::S),
        (kVK_ANSI_D, PhysKeyCode::D),
        (kVK_ANSI_F, PhysKeyCode::F),
        (kVK_ANSI_H, PhysKeyCode::H),
        (kVK_ANSI_G, PhysKeyCode::G),
        (kVK_ANSI_Z, PhysKeyCode::Z),
        (kVK_ANSI_X, PhysKeyCode::X),
        (kVK_ANSI_C, PhysKeyCode::C),
        (kVK_ANSI_V, PhysKeyCode::V),
        (kVK_ANSI_B, PhysKeyCode::B),
        (kVK_ANSI_Q, PhysKeyCode::Q),
        (kVK_ANSI_W, PhysKeyCode::W),
        (kVK_ANSI_E, PhysKeyCode::E),
        (kVK_ANSI_R, PhysKeyCode::R),
        (kVK_ANSI_Y, PhysKeyCode::Y),
        (kVK_ANSI_T, PhysKeyCode::T),
        (kVK_ANSI_1, PhysKeyCode::K1),
        (kVK_ANSI_2, PhysKeyCode::K2),
        (kVK_ANSI_3, PhysKeyCode::K3),
        (kVK_ANSI_4, PhysKeyCode::K4),
        (kVK_ANSI_6, PhysKeyCode::K6),
        (kVK_ANSI_5, PhysKeyCode::K5),
        (kVK_ANSI_Equal, PhysKeyCode::Equal),
        (kVK_ANSI_9, PhysKeyCode::K9),
        (kVK_ANSI_7, PhysKeyCode::K7),
        (kVK_ANSI_Minus, PhysKeyCode::Minus),
        (kVK_ANSI_8, PhysKeyCode::K8),
        (kVK_ANSI_0, PhysKeyCode::K0),
        (kVK_ANSI_RightBracket, PhysKeyCode::RightBracket),
        (kVK_ANSI_O, PhysKeyCode::O),
        (kVK_ANSI_U, PhysKeyCode::U),
        (kVK_ANSI_LeftBracket, PhysKeyCode::LeftBracket),
        (kVK_ANSI_I, PhysKeyCode::I),
        (kVK_ANSI_P, PhysKeyCode::P),
        (kVK_ANSI_L, PhysKeyCode::L),
        (kVK_ANSI_J, PhysKeyCode::J),
        (kVK_ANSI_Quote, PhysKeyCode::Quote),
        (kVK_ANSI_K, PhysKeyCode::K),
        (kVK_ANSI_Semicolon, PhysKeyCode::Semicolon),
        (kVK_ANSI_Backslash, PhysKeyCode::Backslash),
        (kVK_ANSI_Comma, PhysKeyCode::Comma),
        (kVK_ANSI_Slash, PhysKeyCode::Slash),
        (kVK_ANSI_N, PhysKeyCode::N),
        (kVK_ANSI_M, PhysKeyCode::M),
        (kVK_ANSI_Period, PhysKeyCode::Period),
        (kVK_ANSI_Grave, PhysKeyCode::Grave),
        (kVK_ANSI_KeypadDecimal, PhysKeyCode::KeypadDecimal),
        (kVK_ANSI_KeypadMultiply, PhysKeyCode::KeypadMultiply),
        (kVK_ANSI_KeypadPlus, PhysKeyCode::KeypadAdd),
        (kVK_ANSI_KeypadClear, PhysKeyCode::KeypadClear),
        (kVK_ANSI_KeypadDivide, PhysKeyCode::KeypadDivide),
        (kVK_ANSI_KeypadEnter, PhysKeyCode::KeypadEnter),
        (kVK_ANSI_KeypadMinus, PhysKeyCode::KeypadSubtract),
        (kVK_ANSI_KeypadEquals, PhysKeyCode::KeypadEquals),
        (kVK_ANSI_Keypad0, PhysKeyCode::Keypad0),
        (kVK_ANSI_Keypad1, PhysKeyCode::Keypad1),
        (kVK_ANSI_Keypad2, PhysKeyCode::Keypad2),
        (kVK_ANSI_Keypad3, PhysKeyCode::Keypad3),
        (kVK_ANSI_Keypad4, PhysKeyCode::Keypad4),
        (kVK_ANSI_Keypad5, PhysKeyCode::Keypad5),
        (kVK_ANSI_Keypad6, PhysKeyCode::Keypad6),
        (kVK_ANSI_Keypad7, PhysKeyCode::Keypad7),
        (kVK_ANSI_Keypad8, PhysKeyCode::Keypad8),
        (kVK_ANSI_Keypad9, PhysKeyCode::Keypad9),
        (kVK_Return, PhysKeyCode::Return),
        (kVK_Tab, PhysKeyCode::Tab),
        (kVK_Space, PhysKeyCode::Space),
        (kVK_Delete, PhysKeyCode::Backspace),
        (kVK_Escape, PhysKeyCode::Escape),
        (kVK_Command, PhysKeyCode::LeftWindows),
        (kVK_Shift, PhysKeyCode::LeftShift),
        (kVK_CapsLock, PhysKeyCode::CapsLock),
        (kVK_Option, PhysKeyCode::LeftAlt),
        (kVK_Control, PhysKeyCode::LeftControl),
        (kVK_RightCommand, PhysKeyCode::RightWindows),
        (kVK_RightShift, PhysKeyCode::RightShift),
        (kVK_RightOption, PhysKeyCode::RightAlt),
        (kVK_RightControl, PhysKeyCode::RightControl),
        (kVK_Function, PhysKeyCode::Function),
        (kVK_F17, PhysKeyCode::F17),
        (kVK_VolumeUp, PhysKeyCode::VolumeUp),
        (kVK_VolumeDown, PhysKeyCode::VolumeDown),
        (kVK_Mute, PhysKeyCode::VolumeMute),
        (kVK_F18, PhysKeyCode::F18),
        (kVK_F19, PhysKeyCode::F19),
        (kVK_F20, PhysKeyCode::F20),
        (kVK_F5, PhysKeyCode::F5),
        (kVK_F6, PhysKeyCode::F6),
        (kVK_F7, PhysKeyCode::F7),
        (kVK_F3, PhysKeyCode::F3),
        (kVK_F8, PhysKeyCode::F8),
        (kVK_F9, PhysKeyCode::F9),
        (kVK_F11, PhysKeyCode::F11),
        (kVK_F13, PhysKeyCode::F13),
        (kVK_F16, PhysKeyCode::F16),
        (kVK_F14, PhysKeyCode::F14),
        (kVK_F10, PhysKeyCode::F10),
        (kVK_F12, PhysKeyCode::F12),
        (kVK_F15, PhysKeyCode::F15),
        (kVK_Help, PhysKeyCode::Help),
        (kVK_Home, PhysKeyCode::Home),
        (kVK_PageUp, PhysKeyCode::PageUp),
        (kVK_ForwardDelete, PhysKeyCode::Delete),
        (kVK_F4, PhysKeyCode::F4),
        (kVK_End, PhysKeyCode::End),
        (kVK_F2, PhysKeyCode::F2),
        (kVK_PageDown, PhysKeyCode::PageDown),
        (kVK_F1, PhysKeyCode::F1),
        (kVK_LeftArrow, PhysKeyCode::LeftArrow),
        (kVK_RightArrow, PhysKeyCode::RightArrow),
        (kVK_DownArrow, PhysKeyCode::DownArrow),
        (kVK_UpArrow, PhysKeyCode::UpArrow),
    ]
    .iter()
    .map(|&tuple| tuple)
    .collect()
}

lazy_static::lazy_static! {
    static ref MAP: HashMap<u16, PhysKeyCode> = build_map();
}

pub fn vkey_to_phys(vkey: u16) -> Option<PhysKeyCode> {
    MAP.get(&vkey).copied()
}

pub const kVK_ANSI_A: u16 = 0x00;
pub const kVK_ANSI_S: u16 = 0x01;
pub const kVK_ANSI_D: u16 = 0x02;
pub const kVK_ANSI_F: u16 = 0x03;
pub const kVK_ANSI_H: u16 = 0x04;
pub const kVK_ANSI_G: u16 = 0x05;
pub const kVK_ANSI_Z: u16 = 0x06;
pub const kVK_ANSI_X: u16 = 0x07;
pub const kVK_ANSI_C: u16 = 0x08;
pub const kVK_ANSI_V: u16 = 0x09;
pub const kVK_ANSI_B: u16 = 0x0B;
pub const kVK_ANSI_Q: u16 = 0x0C;
pub const kVK_ANSI_W: u16 = 0x0D;
pub const kVK_ANSI_E: u16 = 0x0E;
pub const kVK_ANSI_R: u16 = 0x0F;
pub const kVK_ANSI_Y: u16 = 0x10;
pub const kVK_ANSI_T: u16 = 0x11;
pub const kVK_ANSI_1: u16 = 0x12;
pub const kVK_ANSI_2: u16 = 0x13;
pub const kVK_ANSI_3: u16 = 0x14;
pub const kVK_ANSI_4: u16 = 0x15;
pub const kVK_ANSI_6: u16 = 0x16;
pub const kVK_ANSI_5: u16 = 0x17;
pub const kVK_ANSI_Equal: u16 = 0x18;
pub const kVK_ANSI_9: u16 = 0x19;
pub const kVK_ANSI_7: u16 = 0x1A;
pub const kVK_ANSI_Minus: u16 = 0x1B;
pub const kVK_ANSI_8: u16 = 0x1C;
pub const kVK_ANSI_0: u16 = 0x1D;
pub const kVK_ANSI_RightBracket: u16 = 0x1E;
pub const kVK_ANSI_O: u16 = 0x1F;
pub const kVK_ANSI_U: u16 = 0x20;
pub const kVK_ANSI_LeftBracket: u16 = 0x21;
pub const kVK_ANSI_I: u16 = 0x22;
pub const kVK_ANSI_P: u16 = 0x23;
pub const kVK_ANSI_L: u16 = 0x25;
pub const kVK_ANSI_J: u16 = 0x26;
pub const kVK_ANSI_Quote: u16 = 0x27;
pub const kVK_ANSI_K: u16 = 0x28;
pub const kVK_ANSI_Semicolon: u16 = 0x29;
pub const kVK_ANSI_Backslash: u16 = 0x2A;
pub const kVK_ANSI_Comma: u16 = 0x2B;
pub const kVK_ANSI_Slash: u16 = 0x2C;
pub const kVK_ANSI_N: u16 = 0x2D;
pub const kVK_ANSI_M: u16 = 0x2E;
pub const kVK_ANSI_Period: u16 = 0x2F;
pub const kVK_ANSI_Grave: u16 = 0x32;
pub const kVK_ANSI_KeypadDecimal: u16 = 0x41;
pub const kVK_ANSI_KeypadMultiply: u16 = 0x43;
pub const kVK_ANSI_KeypadPlus: u16 = 0x45;
pub const kVK_ANSI_KeypadClear: u16 = 0x47;
pub const kVK_ANSI_KeypadDivide: u16 = 0x4B;
pub const kVK_ANSI_KeypadEnter: u16 = 0x4C;
pub const kVK_ANSI_KeypadMinus: u16 = 0x4E;
pub const kVK_ANSI_KeypadEquals: u16 = 0x51;
pub const kVK_ANSI_Keypad0: u16 = 0x52;
pub const kVK_ANSI_Keypad1: u16 = 0x53;
pub const kVK_ANSI_Keypad2: u16 = 0x54;
pub const kVK_ANSI_Keypad3: u16 = 0x55;
pub const kVK_ANSI_Keypad4: u16 = 0x56;
pub const kVK_ANSI_Keypad5: u16 = 0x57;
pub const kVK_ANSI_Keypad6: u16 = 0x58;
pub const kVK_ANSI_Keypad7: u16 = 0x59;
pub const kVK_ANSI_Keypad8: u16 = 0x5B;
pub const kVK_ANSI_Keypad9: u16 = 0x5C;

pub const kVK_Return: u16 = 0x24;
pub const kVK_Tab: u16 = 0x30;
pub const kVK_Space: u16 = 0x31;
pub const kVK_Delete: u16 = 0x33;
pub const kVK_Escape: u16 = 0x35;
pub const kVK_Command: u16 = 0x37;
pub const kVK_Shift: u16 = 0x38;
pub const kVK_CapsLock: u16 = 0x39;
pub const kVK_Option: u16 = 0x3A;
pub const kVK_Control: u16 = 0x3B;
pub const kVK_RightCommand: u16 = 0x36;
pub const kVK_RightShift: u16 = 0x3C;
pub const kVK_RightOption: u16 = 0x3D;
pub const kVK_RightControl: u16 = 0x3E;
pub const kVK_Function: u16 = 0x3F;
pub const kVK_F17: u16 = 0x40;
pub const kVK_VolumeUp: u16 = 0x48;
pub const kVK_VolumeDown: u16 = 0x49;
pub const kVK_Mute: u16 = 0x4A;
pub const kVK_F18: u16 = 0x4F;
pub const kVK_F19: u16 = 0x50;
pub const kVK_F20: u16 = 0x5A;
pub const kVK_F5: u16 = 0x60;
pub const kVK_F6: u16 = 0x61;
pub const kVK_F7: u16 = 0x62;
pub const kVK_F3: u16 = 0x63;
pub const kVK_F8: u16 = 0x64;
pub const kVK_F9: u16 = 0x65;
pub const kVK_F11: u16 = 0x67;
pub const kVK_F13: u16 = 0x69;
pub const kVK_F16: u16 = 0x6A;
pub const kVK_F14: u16 = 0x6B;
pub const kVK_F10: u16 = 0x6D;
pub const kVK_F12: u16 = 0x6F;
pub const kVK_F15: u16 = 0x71;
pub const kVK_Help: u16 = 0x72;
pub const kVK_Home: u16 = 0x73;
pub const kVK_PageUp: u16 = 0x74;
pub const kVK_ForwardDelete: u16 = 0x75;
pub const kVK_F4: u16 = 0x76;
pub const kVK_End: u16 = 0x77;
pub const kVK_F2: u16 = 0x78;
pub const kVK_PageDown: u16 = 0x79;
pub const kVK_F1: u16 = 0x7A;
pub const kVK_LeftArrow: u16 = 0x7B;
pub const kVK_RightArrow: u16 = 0x7C;
pub const kVK_DownArrow: u16 = 0x7D;
pub const kVK_UpArrow: u16 = 0x7E;
