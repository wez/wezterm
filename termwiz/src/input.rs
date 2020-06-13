//! This module provides an InputParser struct to help with parsing
//! input received from a terminal.
use crate::escape::csi::MouseReport;
use crate::escape::parser::Parser;
use crate::escape::{Action, CSI};
use crate::keymap::{Found, KeyMap};
use crate::readbuf::ReadBuffer;
use bitflags::bitflags;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use std;

#[cfg(windows)]
use winapi::um::wincon::{
    INPUT_RECORD, KEY_EVENT, KEY_EVENT_RECORD, MOUSE_EVENT, MOUSE_EVENT_RECORD,
    WINDOW_BUFFER_SIZE_EVENT, WINDOW_BUFFER_SIZE_RECORD,
};

bitflags! {
    #[cfg_attr(feature="use_serde", derive(Serialize, Deserialize))]
    #[derive(Default)]
    pub struct Modifiers: u8 {
        const NONE = 0;
        const SHIFT = 1<<1;
        const ALT = 1<<2;
        const CTRL = 1<<3;
        const SUPER = 1<<4;
    }
}
bitflags! {
    #[cfg_attr(feature="use_serde", derive(Serialize, Deserialize))]
    #[derive(Default)]
    pub struct MouseButtons: u8 {
        const NONE = 0;
        const LEFT = 1<<1;
        const RIGHT = 1<<2;
        const MIDDLE = 1<<3;
        const VERT_WHEEL = 1<<4;
        const HORZ_WHEEL = 1<<5;
        /// if set then the wheel movement was in the positive
        /// direction, else the negative direction
        const WHEEL_POSITIVE = 1<<6;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    /// Detected that the user has resized the terminal
    Resized {
        cols: usize,
        rows: usize,
    },
    /// For terminals that support Bracketed Paste mode,
    /// pastes are collected and reported as this variant.
    Paste(String),
    /// The program has woken the input thread.
    Wake,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub mouse_buttons: MouseButtons,
    pub modifiers: Modifiers,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which key was pressed
    pub key: KeyCode,

    /// Which modifiers are down
    pub modifiers: Modifiers,
}

/// Which key is pressed.  Not all of these are probable to appear
/// on most systems.  A lot of this list is @wez trawling docs and
/// making an entry for things that might be possible in this first pass.
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// The decoded unicode character
    Char(char),

    Hyper,
    Super,
    Meta,

    /// Ctrl-break on windows
    Cancel,
    Backspace,
    Tab,
    Clear,
    Enter,
    Shift,
    Escape,
    LeftShift,
    RightShift,
    Control,
    LeftControl,
    RightControl,
    Alt,
    LeftAlt,
    RightAlt,
    Menu,
    LeftMenu,
    RightMenu,
    Pause,
    CapsLock,
    PageUp,
    PageDown,
    End,
    Home,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    Select,
    Print,
    Execute,
    PrintScreen,
    Insert,
    Delete,
    Help,
    LeftWindows,
    RightWindows,
    Applications,
    Sleep,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Multiply,
    Add,
    Separator,
    Subtract,
    Decimal,
    Divide,
    /// F1-F24 are possible
    Function(u8),
    NumLock,
    ScrollLock,
    BrowserBack,
    BrowserForward,
    BrowserRefresh,
    BrowserStop,
    BrowserSearch,
    BrowserFavorites,
    BrowserHome,
    VolumeMute,
    VolumeDown,
    VolumeUp,
    MediaNextTrack,
    MediaPrevTrack,
    MediaStop,
    MediaPlayPause,
    ApplicationLeftArrow,
    ApplicationRightArrow,
    ApplicationUpArrow,
    ApplicationDownArrow,

    #[doc(hidden)]
    InternalPasteStart,
    #[doc(hidden)]
    InternalPasteEnd,
}

impl KeyCode {
    /// if SHIFT is held and we have KeyCode::Char('c') we want to normalize
    /// that keycode to KeyCode::Char('C'); that is what this function does.
    /// In theory we should give the same treatment to keys like `[` -> `{`
    /// but that assumes something about the keyboard layout and is probably
    /// better done in the gui frontend rather than this layer.
    /// In fact, this function might be better off if it lived elsewhere.
    pub fn normalize_shift_to_upper_case(self, modifiers: Modifiers) -> KeyCode {
        if modifiers.contains(Modifiers::SHIFT) {
            match self {
                KeyCode::Char(c) if c.is_ascii_lowercase() => KeyCode::Char(c.to_ascii_uppercase()),
                _ => self,
            }
        } else {
            self
        }
    }

    /// Return true if the key represents a modifier key.
    pub fn is_modifier(self) -> bool {
        match self {
            Self::Hyper
            | Self::Super
            | Self::Meta
            | Self::Shift
            | Self::LeftShift
            | Self::RightShift
            | Self::Control
            | Self::LeftControl
            | Self::RightControl
            | Self::Alt
            | Self::LeftAlt
            | Self::RightAlt
            | Self::LeftWindows
            | Self::RightWindows => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputState {
    Normal,
    EscapeMaybeAlt,
    Pasting(usize),
}

#[derive(Debug)]
pub struct InputParser {
    key_map: KeyMap<InputEvent>,
    buf: ReadBuffer,
    state: InputState,
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std;
    use winapi::um::winuser;

    fn modifiers_from_ctrl_key_state(state: u32) -> Modifiers {
        use winapi::um::wincon::*;

        let mut mods = Modifiers::NONE;

        if (state & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED)) != 0 {
            mods |= Modifiers::ALT;
        }

        if (state & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) != 0 {
            mods |= Modifiers::CTRL;
        }

        if (state & SHIFT_PRESSED) != 0 {
            mods |= Modifiers::SHIFT;
        }

        // TODO: we could report caps lock, numlock and scrolllock

        mods
    }
    impl InputParser {
        fn decode_key_record<F: FnMut(InputEvent)>(
            &mut self,
            event: &KEY_EVENT_RECORD,
            callback: &mut F,
        ) {
            // TODO: do we want downs instead of ups?
            if event.bKeyDown == 0 {
                return;
            }

            let key_code = match std::char::from_u32(*unsafe { event.uChar.UnicodeChar() } as u32) {
                Some(unicode) if unicode > '\x00' => {
                    let mut buf = [0u8; 4];
                    self.buf
                        .extend_with(unicode.encode_utf8(&mut buf).as_bytes());
                    self.process_bytes(callback, true);
                    return;
                }
                _ => match event.wVirtualKeyCode as i32 {
                    winuser::VK_CANCEL => KeyCode::Cancel,
                    winuser::VK_BACK => KeyCode::Backspace,
                    winuser::VK_TAB => KeyCode::Tab,
                    winuser::VK_CLEAR => KeyCode::Clear,
                    winuser::VK_RETURN => KeyCode::Enter,
                    winuser::VK_SHIFT => KeyCode::Shift,
                    winuser::VK_CONTROL => KeyCode::Control,
                    winuser::VK_MENU => KeyCode::Menu,
                    winuser::VK_PAUSE => KeyCode::Pause,
                    winuser::VK_CAPITAL => KeyCode::CapsLock,
                    winuser::VK_ESCAPE => KeyCode::Escape,
                    winuser::VK_PRIOR => KeyCode::PageUp,
                    winuser::VK_NEXT => KeyCode::PageDown,
                    winuser::VK_END => KeyCode::End,
                    winuser::VK_HOME => KeyCode::Home,
                    winuser::VK_LEFT => KeyCode::LeftArrow,
                    winuser::VK_RIGHT => KeyCode::RightArrow,
                    winuser::VK_UP => KeyCode::UpArrow,
                    winuser::VK_DOWN => KeyCode::DownArrow,
                    winuser::VK_SELECT => KeyCode::Select,
                    winuser::VK_PRINT => KeyCode::Print,
                    winuser::VK_EXECUTE => KeyCode::Execute,
                    winuser::VK_SNAPSHOT => KeyCode::PrintScreen,
                    winuser::VK_INSERT => KeyCode::Insert,
                    winuser::VK_DELETE => KeyCode::Delete,
                    winuser::VK_HELP => KeyCode::Help,
                    winuser::VK_LWIN => KeyCode::LeftWindows,
                    winuser::VK_RWIN => KeyCode::RightWindows,
                    winuser::VK_APPS => KeyCode::Applications,
                    winuser::VK_SLEEP => KeyCode::Sleep,
                    winuser::VK_NUMPAD0 => KeyCode::Numpad0,
                    winuser::VK_NUMPAD1 => KeyCode::Numpad1,
                    winuser::VK_NUMPAD2 => KeyCode::Numpad2,
                    winuser::VK_NUMPAD3 => KeyCode::Numpad3,
                    winuser::VK_NUMPAD4 => KeyCode::Numpad4,
                    winuser::VK_NUMPAD5 => KeyCode::Numpad5,
                    winuser::VK_NUMPAD6 => KeyCode::Numpad6,
                    winuser::VK_NUMPAD7 => KeyCode::Numpad7,
                    winuser::VK_NUMPAD8 => KeyCode::Numpad8,
                    winuser::VK_NUMPAD9 => KeyCode::Numpad9,
                    winuser::VK_MULTIPLY => KeyCode::Multiply,
                    winuser::VK_ADD => KeyCode::Add,
                    winuser::VK_SEPARATOR => KeyCode::Separator,
                    winuser::VK_SUBTRACT => KeyCode::Subtract,
                    winuser::VK_DECIMAL => KeyCode::Decimal,
                    winuser::VK_DIVIDE => KeyCode::Divide,
                    winuser::VK_F1 => KeyCode::Function(1),
                    winuser::VK_F2 => KeyCode::Function(2),
                    winuser::VK_F3 => KeyCode::Function(3),
                    winuser::VK_F4 => KeyCode::Function(4),
                    winuser::VK_F5 => KeyCode::Function(5),
                    winuser::VK_F6 => KeyCode::Function(6),
                    winuser::VK_F7 => KeyCode::Function(7),
                    winuser::VK_F8 => KeyCode::Function(8),
                    winuser::VK_F9 => KeyCode::Function(9),
                    winuser::VK_F10 => KeyCode::Function(10),
                    winuser::VK_F11 => KeyCode::Function(11),
                    winuser::VK_F12 => KeyCode::Function(12),
                    winuser::VK_F13 => KeyCode::Function(13),
                    winuser::VK_F14 => KeyCode::Function(14),
                    winuser::VK_F15 => KeyCode::Function(15),
                    winuser::VK_F16 => KeyCode::Function(16),
                    winuser::VK_F17 => KeyCode::Function(17),
                    winuser::VK_F18 => KeyCode::Function(18),
                    winuser::VK_F19 => KeyCode::Function(19),
                    winuser::VK_F20 => KeyCode::Function(20),
                    winuser::VK_F21 => KeyCode::Function(21),
                    winuser::VK_F22 => KeyCode::Function(22),
                    winuser::VK_F23 => KeyCode::Function(23),
                    winuser::VK_F24 => KeyCode::Function(24),
                    winuser::VK_NUMLOCK => KeyCode::NumLock,
                    winuser::VK_SCROLL => KeyCode::ScrollLock,
                    winuser::VK_LSHIFT => KeyCode::LeftShift,
                    winuser::VK_RSHIFT => KeyCode::RightShift,
                    winuser::VK_LCONTROL => KeyCode::LeftControl,
                    winuser::VK_RCONTROL => KeyCode::RightControl,
                    winuser::VK_LMENU => KeyCode::LeftMenu,
                    winuser::VK_RMENU => KeyCode::RightMenu,
                    winuser::VK_BROWSER_BACK => KeyCode::BrowserBack,
                    winuser::VK_BROWSER_FORWARD => KeyCode::BrowserForward,
                    winuser::VK_BROWSER_REFRESH => KeyCode::BrowserRefresh,
                    winuser::VK_BROWSER_STOP => KeyCode::BrowserStop,
                    winuser::VK_BROWSER_SEARCH => KeyCode::BrowserSearch,
                    winuser::VK_BROWSER_FAVORITES => KeyCode::BrowserFavorites,
                    winuser::VK_BROWSER_HOME => KeyCode::BrowserHome,
                    winuser::VK_VOLUME_MUTE => KeyCode::VolumeMute,
                    winuser::VK_VOLUME_DOWN => KeyCode::VolumeDown,
                    winuser::VK_VOLUME_UP => KeyCode::VolumeUp,
                    winuser::VK_MEDIA_NEXT_TRACK => KeyCode::MediaNextTrack,
                    winuser::VK_MEDIA_PREV_TRACK => KeyCode::MediaPrevTrack,
                    winuser::VK_MEDIA_STOP => KeyCode::MediaStop,
                    winuser::VK_MEDIA_PLAY_PAUSE => KeyCode::MediaPlayPause,
                    _ => return,
                },
            };
            let mut modifiers = modifiers_from_ctrl_key_state(event.dwControlKeyState);

            let key_code = key_code.normalize_shift_to_upper_case(modifiers);
            if let KeyCode::Char(c) = key_code {
                if c.is_ascii_uppercase() {
                    modifiers.remove(Modifiers::SHIFT);
                }
            }

            let input_event = InputEvent::Key(KeyEvent {
                key: key_code,
                modifiers,
            });
            for _ in 0..event.wRepeatCount {
                callback(input_event.clone());
            }
        }

        fn decode_mouse_record<F: FnMut(InputEvent)>(
            &self,
            event: &MOUSE_EVENT_RECORD,
            callback: &mut F,
        ) {
            use winapi::um::wincon::*;
            let mut buttons = MouseButtons::NONE;

            if (event.dwButtonState & FROM_LEFT_1ST_BUTTON_PRESSED) != 0 {
                buttons |= MouseButtons::LEFT;
            }
            if (event.dwButtonState & RIGHTMOST_BUTTON_PRESSED) != 0 {
                buttons |= MouseButtons::RIGHT;
            }
            if (event.dwButtonState & FROM_LEFT_2ND_BUTTON_PRESSED) != 0 {
                buttons |= MouseButtons::MIDDLE;
            }

            let modifiers = modifiers_from_ctrl_key_state(event.dwControlKeyState);

            if (event.dwEventFlags & MOUSE_WHEELED) != 0 {
                buttons |= MouseButtons::VERT_WHEEL;
                if (event.dwButtonState >> 8) != 0 {
                    buttons |= MouseButtons::WHEEL_POSITIVE;
                }
            } else if (event.dwEventFlags & MOUSE_HWHEELED) != 0 {
                buttons |= MouseButtons::HORZ_WHEEL;
                if (event.dwButtonState >> 8) != 0 {
                    buttons |= MouseButtons::WHEEL_POSITIVE;
                }
            }

            let mouse = InputEvent::Mouse(MouseEvent {
                x: event.dwMousePosition.X as u16,
                y: event.dwMousePosition.Y as u16,
                mouse_buttons: buttons,
                modifiers,
            });

            if (event.dwEventFlags & DOUBLE_CLICK) != 0 {
                callback(mouse.clone());
            }
            callback(mouse);
        }

        fn decode_resize_record<F: FnMut(InputEvent)>(
            &self,
            event: &WINDOW_BUFFER_SIZE_RECORD,
            callback: &mut F,
        ) {
            callback(InputEvent::Resized {
                rows: event.dwSize.Y as usize,
                cols: event.dwSize.X as usize,
            });
        }

        pub fn decode_input_records<F: FnMut(InputEvent)>(
            &mut self,
            records: &[INPUT_RECORD],
            callback: &mut F,
        ) {
            for record in records {
                match record.EventType {
                    KEY_EVENT => {
                        self.decode_key_record(unsafe { record.Event.KeyEvent() }, callback)
                    }
                    MOUSE_EVENT => {
                        self.decode_mouse_record(unsafe { record.Event.MouseEvent() }, callback)
                    }
                    WINDOW_BUFFER_SIZE_EVENT => self.decode_resize_record(
                        unsafe { record.Event.WindowBufferSizeEvent() },
                        callback,
                    ),
                    _ => {}
                }
            }
            self.process_bytes(callback, false);
        }
    }
}

impl Default for InputParser {
    fn default() -> Self {
        Self::new()
    }
}

impl InputParser {
    pub fn new() -> Self {
        Self {
            key_map: Self::build_basic_key_map(),
            buf: ReadBuffer::new(),
            state: InputState::Normal,
        }
    }

    fn build_basic_key_map() -> KeyMap<InputEvent> {
        let mut map = KeyMap::new();

        let modifier_combos = &[
            ("", Modifiers::NONE),
            (";1", Modifiers::NONE),
            (";2", Modifiers::SHIFT),
            (";3", Modifiers::ALT),
            (";4", Modifiers::ALT | Modifiers::SHIFT),
            (";5", Modifiers::CTRL),
            (";6", Modifiers::CTRL | Modifiers::SHIFT),
            (";7", Modifiers::CTRL | Modifiers::ALT),
            (";8", Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT),
        ];
        // Meta is theoretically a distinct modifier of its own, but modern systems don't
        // have a dedicated Meta key and use the Alt/Option key instead.  The mapping
        // below is reproduced from the xterm documentation from a time where it was
        // possible to hold both Alt and Meta down as modifiers.  Since we define meta to
        // ALT, the use of `meta | ALT` in the table below appears to be redundant,
        // but makes it easier to see that the mapping matches xterm when viewing
        // its documentation.
        let meta = Modifiers::ALT;
        let meta_modifier_combos = &[
            (";9", meta),
            (";10", meta | Modifiers::SHIFT),
            (";11", meta | Modifiers::ALT),
            (";12", meta | Modifiers::ALT | Modifiers::SHIFT),
            (";13", meta | Modifiers::CTRL),
            (";14", meta | Modifiers::CTRL | Modifiers::SHIFT),
            (";15", meta | Modifiers::CTRL | Modifiers::ALT),
            (
                ";16",
                meta | Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
            ),
        ];

        let modifier_combos_including_meta =
            || modifier_combos.iter().chain(meta_modifier_combos.iter());

        for alpha in b'A'..=b'Z' {
            // Ctrl-[A..=Z] are sent as 1..=26
            let ctrl = [alpha & 0x1f];
            map.insert(
                &ctrl,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(alpha as char),
                    modifiers: Modifiers::CTRL,
                }),
            );

            // ALT A-Z is often sent with a leading ESC
            let alt = [0x1b, alpha];
            map.insert(
                &alt,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(alpha as char),
                    modifiers: Modifiers::ALT,
                }),
            );
        }

        // `CSI u` encodings for the ascii range;
        // see http://www.leonerd.org.uk/hacks/fixterms/
        for c in 0..=0x7fu8 {
            for (suffix, modifiers) in modifier_combos {
                let key = format!("\x1b[{}{}u", c, suffix);
                map.insert(
                    key,
                    InputEvent::Key(KeyEvent {
                        key: KeyCode::Char(c as char),
                        modifiers: *modifiers,
                    }),
                );
            }
        }

        // Common arrow keys
        for (keycode, dir) in &[
            (KeyCode::UpArrow, b'A'),
            (KeyCode::DownArrow, b'B'),
            (KeyCode::RightArrow, b'C'),
            (KeyCode::LeftArrow, b'D'),
            (KeyCode::Home, b'H'),
            (KeyCode::End, b'F'),
        ] {
            // Arrow keys in normal mode encoded using CSI
            let arrow = [0x1b, b'[', *dir];
            map.insert(
                &arrow,
                InputEvent::Key(KeyEvent {
                    key: *keycode,
                    modifiers: Modifiers::NONE,
                }),
            );
            for (suffix, modifiers) in modifier_combos_including_meta() {
                let key = format!("\x1b[1{}{}", suffix, *dir as char);
                map.insert(
                    key,
                    InputEvent::Key(KeyEvent {
                        key: *keycode,
                        modifiers: *modifiers,
                    }),
                );
            }
        }
        for &(keycode, dir) in &[
            (KeyCode::UpArrow, b'a'),
            (KeyCode::DownArrow, b'b'),
            (KeyCode::RightArrow, b'c'),
            (KeyCode::LeftArrow, b'd'),
        ] {
            // rxvt-specific modified arrows.
            for &(seq, mods) in &[
                ([0x1b, b'[', dir], Modifiers::SHIFT),
                ([0x1b, b'O', dir], Modifiers::CTRL),
            ] {
                map.insert(
                    &seq,
                    InputEvent::Key(KeyEvent {
                        key: keycode,
                        modifiers: mods,
                    }),
                );
            }
        }

        for (keycode, dir) in &[
            (KeyCode::ApplicationUpArrow, b'A'),
            (KeyCode::ApplicationDownArrow, b'B'),
            (KeyCode::ApplicationRightArrow, b'C'),
            (KeyCode::ApplicationLeftArrow, b'D'),
        ] {
            // Arrow keys in application cursor mode encoded using SS3
            let app = [0x1b, b'O', *dir];
            map.insert(
                &app,
                InputEvent::Key(KeyEvent {
                    key: *keycode,
                    modifiers: Modifiers::NONE,
                }),
            );
            for (suffix, modifiers) in modifier_combos {
                let key = format!("\x1bO1{}{}", suffix, *dir as char);
                map.insert(
                    key,
                    InputEvent::Key(KeyEvent {
                        key: *keycode,
                        modifiers: *modifiers,
                    }),
                );
            }
        }

        // Function keys 1-4 with no modifiers encoded using SS3
        for (keycode, c) in &[
            (KeyCode::Function(1), b'P'),
            (KeyCode::Function(2), b'Q'),
            (KeyCode::Function(3), b'R'),
            (KeyCode::Function(4), b'S'),
        ] {
            let key = [0x1b, b'O', *c];
            map.insert(
                &key,
                InputEvent::Key(KeyEvent {
                    key: *keycode,
                    modifiers: Modifiers::NONE,
                }),
            );
        }

        // Function keys 1-4 with modifiers
        for (keycode, c) in &[
            (KeyCode::Function(1), b'P'),
            (KeyCode::Function(2), b'Q'),
            (KeyCode::Function(3), b'R'),
            (KeyCode::Function(4), b'S'),
        ] {
            for (suffix, modifiers) in modifier_combos_including_meta() {
                let key = format!("\x1b[1{suffix}{code}", code = *c as char, suffix = suffix);
                map.insert(
                    key,
                    InputEvent::Key(KeyEvent {
                        key: *keycode,
                        modifiers: *modifiers,
                    }),
                );
            }
        }

        // Function keys with modifiers encoded using CSI.
        // http://aperiodic.net/phil/archives/Geekery/term-function-keys.html
        for (range, offset) in &[
            // F1-F5 encoded as 11-15
            (1..=5, 10),
            // F6-F10 encoded as 17-21
            (6..=10, 11),
            // F11-F14 encoded as 23-26
            (11..=14, 12),
            // F15-F16 encoded as 28-29
            (15..=16, 13),
            // F17-F20 encoded as 31-34
            (17..=20, 14),
        ] {
            for n in range.clone() {
                for (suffix, modifiers) in modifier_combos_including_meta() {
                    let key = format!("\x1b[{code}{suffix}~", code = n + offset, suffix = suffix);
                    map.insert(
                        key,
                        InputEvent::Key(KeyEvent {
                            key: KeyCode::Function(n),
                            modifiers: *modifiers,
                        }),
                    );
                }
            }
        }

        for (keycode, c) in &[
            (KeyCode::Insert, b'2'),
            (KeyCode::Delete, b'3'),
            (KeyCode::Home, b'1'),
            (KeyCode::End, b'4'),
            (KeyCode::PageUp, b'5'),
            (KeyCode::PageDown, b'6'),
            // rxvt
            (KeyCode::Home, b'7'),
            (KeyCode::End, b'8'),
        ] {
            for (suffix, modifiers) in &[
                (b'~', Modifiers::NONE),
                (b'$', Modifiers::SHIFT),
                (b'^', Modifiers::CTRL),
                (b'@', Modifiers::SHIFT | Modifiers::CTRL),
            ] {
                let key = [0x1b, b'[', *c, *suffix];
                map.insert(
                    key,
                    InputEvent::Key(KeyEvent {
                        key: *keycode,
                        modifiers: *modifiers,
                    }),
                );
            }
        }

        map.insert(
            &[0x7f],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Backspace,
                modifiers: Modifiers::NONE,
            }),
        );

        map.insert(
            &[0x8],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Backspace,
                modifiers: Modifiers::NONE,
            }),
        );

        map.insert(
            &[0x1b],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                modifiers: Modifiers::NONE,
            }),
        );

        map.insert(
            &[b'\t'],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Tab,
                modifiers: Modifiers::NONE,
            }),
        );

        map.insert(
            &[b'\r'],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                modifiers: Modifiers::NONE,
            }),
        );
        map.insert(
            &[b'\n'],
            InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                modifiers: Modifiers::NONE,
            }),
        );

        map.insert(
            b"\x1b[200~",
            InputEvent::Key(KeyEvent {
                key: KeyCode::InternalPasteStart,
                modifiers: Modifiers::NONE,
            }),
        );
        map.insert(
            b"\x1b[201~",
            InputEvent::Key(KeyEvent {
                key: KeyCode::InternalPasteEnd,
                modifiers: Modifiers::NONE,
            }),
        );

        map
    }

    /// Returns the first char from a str and the length of that char
    /// in *bytes*.
    fn first_char_and_len(s: &str) -> (char, usize) {
        let mut iter = s.chars();
        let c = iter.next().unwrap();
        (c, c.len_utf8())
    }

    /// This is a horrible function to pull off the first unicode character
    /// from the sequence of bytes and return it and the remaining slice.
    fn decode_one_char(bytes: &[u8]) -> Option<(char, usize)> {
        // This has the potential to be an ugly hotspot since the complexity
        // is a function of the length of the entire buffer rather than the length
        // of the first char component.  A simple mitigation might be to slice off
        // the first 4 bytes.  We pick 4 bytes because the docs for str::len_utf8()
        // state that the maximum expansion for a `char` is 4 bytes.
        let bytes = &bytes[..bytes.len().min(4)];
        match std::str::from_utf8(bytes) {
            Ok(s) => {
                let (c, len) = Self::first_char_and_len(s);
                Some((c, len))
            }
            Err(err) => {
                let (valid, _after_valid) = bytes.split_at(err.valid_up_to());
                if !valid.is_empty() {
                    let s = unsafe { std::str::from_utf8_unchecked(valid) };
                    let (c, len) = Self::first_char_and_len(s);
                    Some((c, len))
                } else {
                    None
                }
            }
        }
    }

    fn dispatch_callback<F: FnMut(InputEvent)>(&mut self, mut callback: F, event: InputEvent) {
        match (self.state, event) {
            (
                InputState::Normal,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::InternalPasteStart,
                    ..
                }),
            ) => {
                self.state = InputState::Pasting(0);
            }
            (
                InputState::EscapeMaybeAlt,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::InternalPasteStart,
                    ..
                }),
            ) => {
                // The prior ESC was not part of an ALT sequence, so emit
                // it before we start collecting for paste.
                callback(InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    modifiers: Modifiers::NONE,
                }));
                self.state = InputState::Pasting(0);
            }
            (InputState::EscapeMaybeAlt, InputEvent::Key(KeyEvent { key, modifiers })) => {
                // Treat this as ALT-key
                self.state = InputState::Normal;
                callback(InputEvent::Key(KeyEvent {
                    key,
                    modifiers: modifiers | Modifiers::ALT,
                }));
            }
            (InputState::EscapeMaybeAlt, event) => {
                // The prior ESC was not part of an ALT sequence, so emit
                // both it and the current event
                callback(InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    modifiers: Modifiers::NONE,
                }));
                callback(event);
            }
            (_, event) => callback(event),
        }
    }

    fn process_bytes<F: FnMut(InputEvent)>(&mut self, mut callback: F, maybe_more: bool) {
        while !self.buf.is_empty() {
            match self.state {
                InputState::Pasting(offset) => {
                    let end_paste = b"\x1b[201~";
                    if let Some(idx) = self.buf.find_subsequence(offset, end_paste) {
                        let pasted =
                            String::from_utf8_lossy(&self.buf.as_slice()[0..idx]).to_string();
                        self.buf.advance(pasted.len() + end_paste.len());
                        callback(InputEvent::Paste(pasted));
                        self.state = InputState::Normal;
                    } else {
                        self.state = InputState::Pasting(self.buf.len() - end_paste.len());
                        return;
                    }
                }
                InputState::EscapeMaybeAlt | InputState::Normal => {
                    if self.state == InputState::Normal && self.buf.as_slice()[0] == b'\x1b' {
                        // This feels a bit gross because we have two different parsers at play
                        // here.  We want to re-use the escape sequence parser to crack the
                        // parameters out from things like mouse reports.  The keymap tree doesn't
                        // know how to grok this.
                        let mut parser = Parser::new();
                        if let Some((Action::CSI(CSI::Mouse(mouse)), len)) =
                            parser.parse_first(self.buf.as_slice())
                        {
                            self.buf.advance(len);

                            match mouse {
                                MouseReport::SGR1006 {
                                    x,
                                    y,
                                    button,
                                    modifiers,
                                } => {
                                    callback(InputEvent::Mouse(MouseEvent {
                                        x,
                                        y,
                                        mouse_buttons: button.into(),
                                        modifiers,
                                    }));
                                }
                            }
                            continue;
                        }
                    }

                    match (self.key_map.lookup(self.buf.as_slice()), maybe_more) {
                        // If we got an unambiguous ESC and we have more data to
                        // follow, then this is likely the Meta version of the
                        // following keypress.  Buffer up the escape key and
                        // consume it from the input.  dispatch_callback() will
                        // emit either the ESC or the ALT modified following key.
                        (
                            Found::Exact(
                                len,
                                InputEvent::Key(KeyEvent {
                                    key: KeyCode::Escape,
                                    modifiers: Modifiers::NONE,
                                }),
                            ),
                            _,
                        ) if self.state == InputState::Normal && self.buf.len() > len => {
                            self.state = InputState::EscapeMaybeAlt;
                            self.buf.advance(len);
                        }
                        (Found::Exact(len, event), _) | (Found::Ambiguous(len, event), false) => {
                            self.dispatch_callback(&mut callback, event.clone());
                            self.buf.advance(len);
                        }
                        (Found::Ambiguous(_, _), true) | (Found::NeedData, true) => {
                            return;
                        }
                        (Found::None, _) | (Found::NeedData, false) => {
                            // No pre-defined key, so pull out a unicode character
                            if let Some((c, len)) = Self::decode_one_char(self.buf.as_slice()) {
                                self.buf.advance(len);
                                self.dispatch_callback(
                                    &mut callback,
                                    InputEvent::Key(KeyEvent {
                                        key: KeyCode::Char(c),
                                        modifiers: Modifiers::NONE,
                                    }),
                                );
                            } else {
                                // We need more data to recognize the input, so
                                // yield the remainder of the slice
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Push a sequence of bytes into the parser.
    /// Each time input is recognized, the provided `callback` will be passed
    /// the decoded `InputEvent`.
    /// If not enough data are available to fully decode a sequence, the
    /// remaining data will be buffered until the next call.
    /// The `maybe_more` flag controls how ambiguous partial sequences are
    /// handled. The intent is that `maybe_more` should be set to true if
    /// you believe that you will be able to provide more data momentarily.
    /// This will cause the parser to defer judgement on partial prefix
    /// matches. You should attempt to read and pass the new data in
    /// immediately afterwards. If you have attempted a read and no data is
    /// immediately available, you should follow up with a call to parse
    /// with an empty slice and `maybe_more=false` to allow the partial
    /// data to be recognized and processed.
    pub fn parse<F: FnMut(InputEvent)>(&mut self, bytes: &[u8], callback: F, maybe_more: bool) {
        self.buf.extend_with(bytes);
        self.process_bytes(callback, maybe_more);
    }

    pub fn parse_as_vec(&mut self, bytes: &[u8]) -> Vec<InputEvent> {
        let mut result = Vec::new();
        self.parse(bytes, |event| result.push(event), false);
        result
    }

    #[cfg(windows)]
    pub fn decode_input_records_as_vec(&mut self, records: &[INPUT_RECORD]) -> Vec<InputEvent> {
        let mut result = Vec::new();
        self.decode_input_records(records, &mut |event| result.push(event));
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple() {
        let mut p = InputParser::new();
        let inputs = p.parse_as_vec(b"hello");
        assert_eq!(
            vec![
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('h'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('e'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('l'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('l'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('o'),
                }),
            ],
            inputs
        );
    }

    #[test]
    fn control_characters() {
        let mut p = InputParser::new();
        let inputs = p.parse_as_vec(b"\x03\x1bJ\x7f");
        assert_eq!(
            vec![
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::CTRL,
                    key: KeyCode::Char('C'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::ALT,
                    key: KeyCode::Char('J'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Backspace,
                }),
            ],
            inputs
        );
    }

    #[test]
    fn arrow_keys() {
        let mut p = InputParser::new();
        let inputs = p.parse_as_vec(b"\x1bOA\x1bOB\x1bOC\x1bOD");
        assert_eq!(
            vec![
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::ApplicationUpArrow,
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::ApplicationDownArrow,
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::ApplicationRightArrow,
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::ApplicationLeftArrow,
                }),
            ],
            inputs
        );
    }

    #[test]
    fn partial() {
        let mut p = InputParser::new();
        let mut inputs = Vec::new();
        // Fragment this F-key sequence across two different pushes
        p.parse(b"\x1b[11", |evt| inputs.push(evt), true);
        p.parse(b"~", |evt| inputs.push(evt), true);
        // make sure we recognize it as just the F-key
        assert_eq!(
            vec![InputEvent::Key(KeyEvent {
                modifiers: Modifiers::NONE,
                key: KeyCode::Function(1),
            })],
            inputs
        );
    }

    #[test]
    fn partial_ambig() {
        let mut p = InputParser::new();

        assert_eq!(
            vec![InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                modifiers: Modifiers::NONE,
            })],
            p.parse_as_vec(b"\x1b")
        );

        let mut inputs = Vec::new();
        // Fragment this F-key sequence across two different pushes
        p.parse(b"\x1b[11", |evt| inputs.push(evt), true);
        p.parse(b"", |evt| inputs.push(evt), false);
        // make sure we recognize it as just the F-key
        assert_eq!(
            vec![
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    modifiers: Modifiers::NONE,
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('['),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('1'),
                }),
                InputEvent::Key(KeyEvent {
                    modifiers: Modifiers::NONE,
                    key: KeyCode::Char('1'),
                }),
            ],
            inputs
        );
    }
}
