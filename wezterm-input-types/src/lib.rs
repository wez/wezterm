use bitflags::*;
#[cfg(feature = "serde")]
use serde::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use wezterm_dynamic::{FromDynamic, ToDynamic};

pub struct PixelUnit;
pub struct ScreenPixelUnit;
pub type Point = euclid::Point2D<isize, PixelUnit>;
pub type PointF = euclid::Point2D<f32, PixelUnit>;
pub type ScreenPoint = euclid::Point2D<isize, ScreenPixelUnit>;

/// Which key is pressed.  Not all of these are probable to appear
/// on most systems.  A lot of this list is @wez trawling docs and
/// making an entry for things that might be possible in this first pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, FromDynamic, ToDynamic)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyCode {
    /// The decoded unicode character
    Char(char),
    Composed(String),
    RawCode(u32),
    Physical(PhysKeyCode),

    Hyper,
    Super,
    Meta,

    /// Ctrl-break on windows
    Cancel,
    // There is no `Backspace`; use `Char('\u{8}') instead

    // There is no `Tab`; use `Char('\t')` instead
    Clear,
    // There is no `Enter`; use `Char('\r')` instead
    Shift,
    // There is no `Escape`; use `Char('\u{1b}') instead
    LeftShift,
    RightShift,
    Control,
    LeftControl,
    RightControl,
    Alt,
    LeftAlt,
    RightAlt,
    Pause,
    CapsLock,
    VoidSymbol,
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
    // There is no `Delete`; use `Char('\u{7f}')` instead
    Help,
    LeftWindows,
    RightWindows,
    Applications,
    Sleep,
    /// Numeric keypad digits 0-9
    Numpad(u8),
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
    Copy,
    Cut,
    Paste,
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
    KeyPadHome,
    KeyPadEnd,
    KeyPadPageUp,
    KeyPadPageDown,
    KeyPadBegin,
}

impl KeyCode {
    /// Return true if the key represents a modifier key.
    pub fn is_modifier(&self) -> bool {
        match self {
            Self::Hyper
            | Self::CapsLock
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

    pub fn normalize_shift(&self, modifiers: Modifiers) -> (KeyCode, Modifiers) {
        normalize_shift(self.clone(), modifiers)
    }

    pub fn composed(s: &str) -> Self {
        // Prefer to send along a single Char when the string
        // is just a single char, as the keymapping layer cannot
        // bind to composed key sequences
        let mut iter = s.chars();
        let first_char = iter.next();
        let next_char = iter.next();
        match (first_char, next_char) {
            (Some(c), None) => Self::Char(c),
            _ => Self::Composed(s.to_string()),
        }
    }

    /// Convert to a PhysKeyCode.
    /// Note that by the nature of PhysKeyCode being defined in terms
    /// of a US ANSI standard layout, essentially "latinizes" the keycode,
    /// so the results may not make as much sense for non-latin keyboards.
    /// It also loses the shifted state of alphabetical characters.
    pub fn to_phys(&self) -> Option<PhysKeyCode> {
        Some(match self {
            Self::Char('a') | Self::Char('A') => PhysKeyCode::A,
            Self::Char('b') | Self::Char('B') => PhysKeyCode::B,
            Self::Char('c') | Self::Char('C') => PhysKeyCode::C,
            Self::Char('d') | Self::Char('D') => PhysKeyCode::D,
            Self::Char('e') | Self::Char('E') => PhysKeyCode::E,
            Self::Char('f') | Self::Char('F') => PhysKeyCode::F,
            Self::Char('g') | Self::Char('G') => PhysKeyCode::G,
            Self::Char('h') | Self::Char('H') => PhysKeyCode::H,
            Self::Char('i') | Self::Char('I') => PhysKeyCode::I,
            Self::Char('j') | Self::Char('J') => PhysKeyCode::J,
            Self::Char('k') | Self::Char('K') => PhysKeyCode::K,
            Self::Char('l') | Self::Char('L') => PhysKeyCode::L,
            Self::Char('m') | Self::Char('M') => PhysKeyCode::M,
            Self::Char('n') | Self::Char('N') => PhysKeyCode::N,
            Self::Char('o') | Self::Char('O') => PhysKeyCode::O,
            Self::Char('p') | Self::Char('P') => PhysKeyCode::P,
            Self::Char('q') | Self::Char('Q') => PhysKeyCode::Q,
            Self::Char('r') | Self::Char('R') => PhysKeyCode::R,
            Self::Char('s') | Self::Char('S') => PhysKeyCode::S,
            Self::Char('t') | Self::Char('T') => PhysKeyCode::T,
            Self::Char('u') | Self::Char('U') => PhysKeyCode::U,
            Self::Char('v') | Self::Char('V') => PhysKeyCode::V,
            Self::Char('w') | Self::Char('W') => PhysKeyCode::W,
            Self::Char('x') | Self::Char('X') => PhysKeyCode::X,
            Self::Char('y') | Self::Char('Y') => PhysKeyCode::Y,
            Self::Char('z') | Self::Char('Z') => PhysKeyCode::Z,
            Self::Char('0') => PhysKeyCode::K0,
            Self::Char('1') => PhysKeyCode::K1,
            Self::Char('2') => PhysKeyCode::K2,
            Self::Char('3') => PhysKeyCode::K3,
            Self::Char('4') => PhysKeyCode::K4,
            Self::Char('5') => PhysKeyCode::K5,
            Self::Char('6') => PhysKeyCode::K6,
            Self::Char('7') => PhysKeyCode::K7,
            Self::Char('8') => PhysKeyCode::K8,
            Self::Char('9') => PhysKeyCode::K9,
            Self::Char('\\') => PhysKeyCode::Backslash,
            Self::Char(',') => PhysKeyCode::Comma,
            Self::Char('\u{8}') => PhysKeyCode::Backspace,
            Self::Char('\u{7f}') => PhysKeyCode::Delete,
            Self::Char('=') => PhysKeyCode::Equal,
            Self::Char('\u{1b}') => PhysKeyCode::Escape,
            Self::Char('`') => PhysKeyCode::Grave,
            Self::Char('\r') => PhysKeyCode::Return,
            Self::Char('[') => PhysKeyCode::LeftBracket,
            Self::Char(']') => PhysKeyCode::RightBracket,
            Self::Char('-') => PhysKeyCode::Minus,
            Self::Char('.') => PhysKeyCode::Period,
            Self::Char('\'') => PhysKeyCode::Quote,
            Self::Char(';') => PhysKeyCode::Semicolon,
            Self::Char('/') => PhysKeyCode::Slash,
            Self::Char(' ') => PhysKeyCode::Space,
            Self::Char('\t') => PhysKeyCode::Tab,
            Self::Numpad(0) => PhysKeyCode::Keypad0,
            Self::Numpad(1) => PhysKeyCode::Keypad1,
            Self::Numpad(2) => PhysKeyCode::Keypad2,
            Self::Numpad(3) => PhysKeyCode::Keypad3,
            Self::Numpad(4) => PhysKeyCode::Keypad4,
            Self::Numpad(5) => PhysKeyCode::Keypad5,
            Self::Numpad(6) => PhysKeyCode::Keypad6,
            Self::Numpad(7) => PhysKeyCode::Keypad7,
            Self::Numpad(8) => PhysKeyCode::Keypad8,
            Self::Numpad(9) => PhysKeyCode::Keypad9,
            Self::Function(1) => PhysKeyCode::F1,
            Self::Function(2) => PhysKeyCode::F2,
            Self::Function(3) => PhysKeyCode::F3,
            Self::Function(4) => PhysKeyCode::F4,
            Self::Function(5) => PhysKeyCode::F5,
            Self::Function(6) => PhysKeyCode::F6,
            Self::Function(7) => PhysKeyCode::F7,
            Self::Function(8) => PhysKeyCode::F8,
            Self::Function(9) => PhysKeyCode::F9,
            Self::Function(10) => PhysKeyCode::F10,
            Self::Function(11) => PhysKeyCode::F11,
            Self::Function(12) => PhysKeyCode::F12,
            Self::Function(13) => PhysKeyCode::F13,
            Self::Function(14) => PhysKeyCode::F14,
            Self::Function(15) => PhysKeyCode::F15,
            Self::Function(16) => PhysKeyCode::F16,
            Self::Function(17) => PhysKeyCode::F17,
            Self::Function(18) => PhysKeyCode::F18,
            Self::Function(19) => PhysKeyCode::F19,
            Self::Function(20) => PhysKeyCode::F20,
            Self::Physical(p) => *p,
            Self::Shift | Self::LeftShift => PhysKeyCode::LeftShift,
            Self::RightShift => PhysKeyCode::RightShift,
            Self::Alt | Self::LeftAlt => PhysKeyCode::LeftAlt,
            Self::RightAlt => PhysKeyCode::RightAlt,
            Self::LeftWindows => PhysKeyCode::LeftWindows,
            Self::RightWindows => PhysKeyCode::RightWindows,
            Self::Control | Self::LeftControl => PhysKeyCode::LeftControl,
            Self::RightControl => PhysKeyCode::RightControl,
            Self::CapsLock => PhysKeyCode::CapsLock,
            Self::PageUp => PhysKeyCode::PageUp,
            Self::PageDown => PhysKeyCode::PageDown,
            Self::Home => PhysKeyCode::Home,
            Self::End => PhysKeyCode::End,
            Self::LeftArrow => PhysKeyCode::LeftArrow,
            Self::RightArrow => PhysKeyCode::RightArrow,
            Self::UpArrow => PhysKeyCode::UpArrow,
            Self::DownArrow => PhysKeyCode::DownArrow,
            Self::Insert => PhysKeyCode::Insert,
            Self::Help => PhysKeyCode::Help,
            Self::Multiply => PhysKeyCode::KeypadMultiply,
            Self::Clear => PhysKeyCode::KeypadClear,
            Self::Decimal => PhysKeyCode::KeypadDecimal,
            Self::Divide => PhysKeyCode::KeypadDivide,
            Self::Add => PhysKeyCode::KeypadAdd,
            Self::Subtract => PhysKeyCode::KeypadSubtract,
            Self::NumLock => PhysKeyCode::NumLock,
            Self::VolumeUp => PhysKeyCode::VolumeUp,
            Self::VolumeDown => PhysKeyCode::VolumeDown,
            Self::VolumeMute => PhysKeyCode::VolumeMute,
            Self::ApplicationLeftArrow
            | Self::ApplicationRightArrow
            | Self::ApplicationUpArrow
            | Self::ApplicationDownArrow
            | Self::KeyPadHome
            | Self::KeyPadEnd
            | Self::KeyPadPageUp
            | Self::KeyPadPageDown
            | Self::KeyPadBegin
            | Self::MediaNextTrack
            | Self::MediaPrevTrack
            | Self::MediaStop
            | Self::MediaPlayPause
            | Self::Copy
            | Self::Cut
            | Self::Paste
            | Self::BrowserBack
            | Self::BrowserForward
            | Self::BrowserRefresh
            | Self::BrowserStop
            | Self::BrowserSearch
            | Self::BrowserFavorites
            | Self::BrowserHome
            | Self::ScrollLock
            | Self::Separator
            | Self::Sleep
            | Self::Applications
            | Self::Execute
            | Self::PrintScreen
            | Self::Print
            | Self::Select
            | Self::VoidSymbol
            | Self::Pause
            | Self::Cancel
            | Self::Hyper
            | Self::Super
            | Self::Meta
            | Self::Composed(_)
            | Self::RawCode(_)
            | Self::Char(_)
            | Self::Numpad(_)
            | Self::Function(_) => return None,
        })
    }
}

impl TryFrom<&str> for KeyCode {
    type Error = String;
    fn try_from(s: &str) -> std::result::Result<Self, String> {
        macro_rules! m {
            ($($val:ident),* $(,)?) => {
                match s {
                $(
                    stringify!($val) => return Ok(Self::$val),
                )*
                    _ => {}
                }
            }
        }

        m!(
            Hyper,
            Super,
            Meta,
            Cancel,
            Clear,
            Shift,
            LeftShift,
            RightShift,
            Control,
            LeftControl,
            RightControl,
            Alt,
            LeftAlt,
            RightAlt,
            Pause,
            CapsLock,
            VoidSymbol,
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
            Help,
            LeftWindows,
            RightWindows,
            Applications,
            Sleep,
            Multiply,
            Add,
            Separator,
            Subtract,
            Decimal,
            Divide,
            NumLock,
            ScrollLock,
            Copy,
            Cut,
            Paste,
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
        );

        match s {
            "Backspace" => return Ok(KeyCode::Char('\u{8}')),
            "Tab" => return Ok(KeyCode::Char('\t')),
            "Return" | "Enter" => return Ok(KeyCode::Char('\r')),
            "Escape" => return Ok(KeyCode::Char('\u{1b}')),
            "Delete" => return Ok(KeyCode::Char('\u{7f}')),
            _ => {}
        };

        if let Some(n) = s.strip_prefix("Numpad") {
            let n: u8 = n
                .parse()
                .map_err(|err| format!("parsing Numpad<NUMBER>: {:#}", err))?;
            if n > 9 {
                return Err("Numpad numbers must be in range 0-9".to_string());
            }
            return Ok(KeyCode::Numpad(n));
        }

        // Don't consider "F" to be an invalid F key!
        if s.len() > 1 {
            if let Some(n) = s.strip_prefix("F") {
                let n: u8 = n
                    .parse()
                    .map_err(|err| format!("parsing F<NUMBER>: {:#}", err))?;
                if n == 0 || n > 24 {
                    return Err("Function key numbers must be in range 1-24".to_string());
                }
                return Ok(KeyCode::Function(n));
            }
        }

        let chars: Vec<char> = s.chars().collect();
        if chars.len() == 1 {
            let k = KeyCode::Char(chars[0]);
            Ok(k)
        } else {
            Err(format!("invalid KeyCode string {}", s))
        }
    }
}

impl ToString for KeyCode {
    fn to_string(&self) -> String {
        match self {
            Self::RawCode(n) => format!("raw:{}", n),
            Self::Char(c) => format!("mapped:{}", c),
            Self::Physical(phys) => phys.to_string(),
            Self::Composed(s) => s.to_string(),
            Self::Numpad(n) => format!("Numpad{}", n),
            Self::Function(n) => format!("F{}", n),
            other => format!("{:?}", other),
        }
    }
}

bitflags! {
    #[derive(Default, FromDynamic, ToDynamic)]
    pub struct KeyboardLedStatus: u8 {
        const CAPS_LOCK = 1<<1;
        const NUM_LOCK = 1<<2;
    }
}

impl ToString for KeyboardLedStatus {
    fn to_string(&self) -> String {
        let mut s = String::new();
        if self.contains(Self::CAPS_LOCK) {
            s.push_str("CAPS_LOCK");
        }
        if self.contains(Self::NUM_LOCK) {
            if !s.is_empty() {
                s.push('|');
            }
            s.push_str("NUM_LOCK");
        }
        s
    }
}

bitflags! {
    #[cfg_attr(feature="serde", derive(Serialize, Deserialize))]
    #[derive(Default, FromDynamic, ToDynamic)]
    #[dynamic(into="String", try_from="String")]
    pub struct Modifiers: u16 {
        const NONE = 0;
        const SHIFT = 1<<1;
        const ALT = 1<<2;
        const CTRL = 1<<3;
        const SUPER = 1<<4;
        const LEFT_ALT = 1<<5;
        const RIGHT_ALT = 1<<6;
        /// This is a virtual modifier used by wezterm
        const LEADER = 1<<7;
        const LEFT_CTRL = 1<<8;
        const RIGHT_CTRL = 1<<9;
        const LEFT_SHIFT = 1<<10;
        const RIGHT_SHIFT = 1<<11;
        const ENHANCED_KEY = 1<<12;
    }
}

impl TryFrom<String> for Modifiers {
    type Error = String;

    fn try_from(s: String) -> Result<Modifiers, String> {
        let mut mods = Modifiers::NONE;
        for ele in s.split('|') {
            // Allow for whitespace; debug printing Modifiers includes spaces
            // around the `|` so it is desirable to be able to reverse that
            // encoding here.
            let ele = ele.trim();
            if ele == "SHIFT" {
                mods |= Modifiers::SHIFT;
            } else if ele == "ALT" || ele == "OPT" || ele == "META" {
                mods |= Modifiers::ALT;
            } else if ele == "CTRL" {
                mods |= Modifiers::CTRL;
            } else if ele == "SUPER" || ele == "CMD" || ele == "WIN" {
                mods |= Modifiers::SUPER;
            } else if ele == "LEADER" {
                mods |= Modifiers::LEADER;
            } else if ele == "NONE" || ele == "" {
                mods |= Modifiers::NONE;
            } else {
                return Err(format!("invalid modifier name {} in {}", ele, s));
            }
        }
        Ok(mods)
    }
}

impl From<&Modifiers> for String {
    fn from(val: &Modifiers) -> Self {
        val.to_string()
    }
}

pub struct ModifierToStringArgs<'a> {
    /// How to join two modifier keys. Can be empty.
    pub separator: &'a str,
    /// Whether to output NONE when no modifiers are present
    pub want_none: bool,
    /// How to render the keycaps for the UI
    pub ui_key_cap_rendering: Option<UIKeyCapRendering>,
}

impl Modifiers {
    pub fn encode_xterm(self) -> u8 {
        let mut number = 0;
        if self.contains(Self::SHIFT) {
            number |= 1;
        }
        if self.contains(Self::ALT) {
            number |= 2;
        }
        if self.contains(Self::CTRL) {
            number |= 4;
        }
        number
    }

    #[allow(non_upper_case_globals)]
    pub fn to_string_with_separator(&self, args: ModifierToStringArgs) -> String {
        let mut s = String::new();
        if args.want_none && *self == Self::NONE {
            s.push_str("NONE");
        }

        // The unicode escapes here are nerdfont symbols; we use those because
        // we're guaranteed to have them available, and the symbols are
        // very legible
        const md_apple_keyboard_command: &str = "\u{f0633}"; // 󰘳
        const md_apple_keyboard_control: &str = "\u{f0634}"; // 󰘴
        const md_apple_keyboard_option: &str = "\u{f0635}"; // 󰘵
        const md_apple_keyboard_shift: &str = "\u{f0636}"; // 󰘶
        const md_microsoft_windows: &str = "\u{f05b3}"; // 󰖳

        for (value, label, unix, emacs, apple, windows, win_sym) in [
            (
                Self::SHIFT,
                "SHIFT",
                "Shift",
                "S",
                md_apple_keyboard_shift,
                "Shift",
                "Shift",
            ),
            (
                Self::ALT,
                "ALT",
                "Alt",
                "M",
                md_apple_keyboard_option,
                "Alt",
                "Alt",
            ),
            (
                Self::CTRL,
                "CTRL",
                "Ctrl",
                "C",
                md_apple_keyboard_control,
                "Ctrl",
                "Ctrl",
            ),
            (
                Self::SUPER,
                "SUPER",
                "Super",
                "Super",
                md_apple_keyboard_command,
                "Win",
                md_microsoft_windows,
            ),
            (
                Self::LEFT_ALT,
                "LEFT_ALT",
                "Alt",
                "M",
                md_apple_keyboard_option,
                "Alt",
                "Alt",
            ),
            (
                Self::RIGHT_ALT,
                "RIGHT_ALT",
                "Alt",
                "M",
                md_apple_keyboard_option,
                "Alt",
                "Alt",
            ),
            (
                Self::LEADER,
                "LEADER",
                "Leader",
                "Leader",
                "Leader",
                "Leader",
                "Leader",
            ),
            (
                Self::LEFT_CTRL,
                "LEFT_CTRL",
                "Ctrl",
                "C",
                md_apple_keyboard_control,
                "Ctrl",
                "Ctrl",
            ),
            (
                Self::RIGHT_CTRL,
                "RIGHT_CTRL",
                "Ctrl",
                "C",
                md_apple_keyboard_control,
                "Ctrl",
                "Ctrl",
            ),
            (
                Self::LEFT_SHIFT,
                "LEFT_SHIFT",
                "Shift",
                "S",
                md_apple_keyboard_shift,
                "Shift",
                "Shift",
            ),
            (
                Self::RIGHT_SHIFT,
                "RIGHT_SHIFT",
                "Shift",
                "S",
                md_apple_keyboard_shift,
                "Shift",
                "Shift",
            ),
            (
                Self::ENHANCED_KEY,
                "ENHANCED_KEY",
                "ENHANCED_KEY",
                "ENHANCED_KEY",
                "ENHANCED_KEY",
                "ENHANCED_KEY",
                "ENHANCED_KEY",
            ),
        ] {
            if !self.contains(value) {
                continue;
            }
            if !s.is_empty() {
                s.push_str(args.separator);
            }
            s.push_str(match args.ui_key_cap_rendering {
                Some(UIKeyCapRendering::UnixLong) => unix,
                Some(UIKeyCapRendering::Emacs) => emacs,
                Some(UIKeyCapRendering::AppleSymbols) => apple,
                Some(UIKeyCapRendering::WindowsLong) => windows,
                Some(UIKeyCapRendering::WindowsSymbols) => win_sym,
                None => label,
            });
        }

        s
    }
}

impl ToString for Modifiers {
    fn to_string(&self) -> String {
        self.to_string_with_separator(ModifierToStringArgs {
            separator: "|",
            want_none: true,
            ui_key_cap_rendering: None,
        })
    }
}

impl Modifiers {
    /// Remove positional and other "supplemental" bits that
    /// are used to carry around implementation details, but that
    /// are not bits that should be matched when matching key
    /// assignments.
    pub fn remove_positional_mods(self) -> Self {
        self - (Self::LEFT_ALT
            | Self::RIGHT_ALT
            | Self::LEFT_CTRL
            | Self::RIGHT_CTRL
            | Self::LEFT_SHIFT
            | Self::RIGHT_SHIFT
            | Self::ENHANCED_KEY)
    }
}

/// These keycodes identify keys based on their physical
/// position on an ANSI-standard US keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, Ord, PartialOrd, FromDynamic, ToDynamic)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PhysKeyCode {
    A,
    B,
    Backslash,
    C,
    CapsLock,
    Comma,
    D,
    Backspace,
    DownArrow,
    E,
    End,
    Equal,
    Escape,
    F,
    F1,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F2,
    F20,
    F21,
    F22,
    F23,
    F24,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    Delete,
    Function,
    G,
    Grave,
    H,
    Help,
    Home,
    I,
    Insert,
    J,
    K,
    K0,
    K1,
    K2,
    K3,
    K4,
    K5,
    K6,
    K7,
    K8,
    K9,
    Keypad0,
    Keypad1,
    Keypad2,
    Keypad3,
    Keypad4,
    Keypad5,
    Keypad6,
    Keypad7,
    Keypad8,
    Keypad9,
    KeypadClear,
    KeypadDecimal,
    KeypadDelete,
    KeypadDivide,
    KeypadEnter,
    KeypadEquals,
    KeypadSubtract,
    KeypadMultiply,
    KeypadAdd,
    L,
    LeftAlt,
    LeftArrow,
    LeftBracket,
    LeftControl,
    LeftShift,
    LeftWindows,
    M,
    Minus,
    VolumeMute,
    N,
    NumLock,
    O,
    P,
    PageDown,
    PageUp,
    Period,
    Q,
    Quote,
    R,
    Return,
    RightAlt,
    RightArrow,
    RightBracket,
    RightControl,
    RightShift,
    RightWindows,
    S,
    Semicolon,
    Slash,
    Space,
    T,
    Tab,
    U,
    UpArrow,
    V,
    VolumeDown,
    VolumeUp,
    W,
    X,
    Y,
    Z,
}

impl PhysKeyCode {
    pub fn is_modifier(&self) -> bool {
        match self {
            Self::LeftShift
            | Self::LeftControl
            | Self::LeftWindows
            | Self::LeftAlt
            | Self::RightShift
            | Self::RightControl
            | Self::RightWindows
            | Self::RightAlt => true,
            _ => false,
        }
    }

    pub fn to_key_code(self) -> KeyCode {
        match self {
            Self::LeftShift => KeyCode::LeftShift,
            Self::LeftControl => KeyCode::LeftControl,
            Self::LeftWindows => KeyCode::LeftWindows,
            Self::LeftAlt => KeyCode::LeftAlt,
            Self::RightShift => KeyCode::RightShift,
            Self::RightControl => KeyCode::RightControl,
            Self::RightWindows => KeyCode::RightWindows,
            Self::RightAlt => KeyCode::RightAlt,
            Self::LeftArrow => KeyCode::LeftArrow,
            Self::RightArrow => KeyCode::RightArrow,
            Self::UpArrow => KeyCode::UpArrow,
            Self::DownArrow => KeyCode::DownArrow,
            Self::CapsLock => KeyCode::CapsLock,
            Self::F1 => KeyCode::Function(1),
            Self::F2 => KeyCode::Function(2),
            Self::F3 => KeyCode::Function(3),
            Self::F4 => KeyCode::Function(4),
            Self::F5 => KeyCode::Function(5),
            Self::F6 => KeyCode::Function(6),
            Self::F7 => KeyCode::Function(7),
            Self::F8 => KeyCode::Function(8),
            Self::F9 => KeyCode::Function(9),
            Self::F10 => KeyCode::Function(10),
            Self::F11 => KeyCode::Function(11),
            Self::F12 => KeyCode::Function(12),
            Self::F13 => KeyCode::Function(13),
            Self::F14 => KeyCode::Function(14),
            Self::F15 => KeyCode::Function(15),
            Self::F16 => KeyCode::Function(16),
            Self::F17 => KeyCode::Function(17),
            Self::F18 => KeyCode::Function(18),
            Self::F19 => KeyCode::Function(19),
            Self::F20 => KeyCode::Function(20),
            Self::F21 => KeyCode::Function(21),
            Self::F22 => KeyCode::Function(22),
            Self::F23 => KeyCode::Function(23),
            Self::F24 => KeyCode::Function(24),
            Self::Keypad0 => KeyCode::Numpad(0),
            Self::Keypad1 => KeyCode::Numpad(1),
            Self::Keypad2 => KeyCode::Numpad(2),
            Self::Keypad3 => KeyCode::Numpad(3),
            Self::Keypad4 => KeyCode::Numpad(4),
            Self::Keypad5 => KeyCode::Numpad(5),
            Self::Keypad6 => KeyCode::Numpad(6),
            Self::Keypad7 => KeyCode::Numpad(7),
            Self::Keypad8 => KeyCode::Numpad(8),
            Self::Keypad9 => KeyCode::Numpad(9),
            Self::KeypadClear => KeyCode::Clear,
            Self::KeypadMultiply => KeyCode::Multiply,
            Self::KeypadDecimal => KeyCode::Decimal,
            Self::KeypadDivide => KeyCode::Divide,
            Self::KeypadAdd => KeyCode::Add,
            Self::KeypadSubtract => KeyCode::Subtract,
            Self::A => KeyCode::Char('a'),
            Self::B => KeyCode::Char('b'),
            Self::C => KeyCode::Char('c'),
            Self::D => KeyCode::Char('d'),
            Self::E => KeyCode::Char('e'),
            Self::F => KeyCode::Char('f'),
            Self::G => KeyCode::Char('g'),
            Self::H => KeyCode::Char('h'),
            Self::I => KeyCode::Char('i'),
            Self::J => KeyCode::Char('j'),
            Self::K => KeyCode::Char('k'),
            Self::L => KeyCode::Char('l'),
            Self::M => KeyCode::Char('m'),
            Self::N => KeyCode::Char('n'),
            Self::O => KeyCode::Char('o'),
            Self::P => KeyCode::Char('p'),
            Self::Q => KeyCode::Char('q'),
            Self::R => KeyCode::Char('r'),
            Self::S => KeyCode::Char('s'),
            Self::T => KeyCode::Char('t'),
            Self::U => KeyCode::Char('u'),
            Self::V => KeyCode::Char('v'),
            Self::W => KeyCode::Char('w'),
            Self::X => KeyCode::Char('x'),
            Self::Y => KeyCode::Char('y'),
            Self::Z => KeyCode::Char('z'),
            Self::Backslash => KeyCode::Char('\\'),
            Self::Comma => KeyCode::Char(','),
            Self::Backspace => KeyCode::Char('\u{8}'),
            Self::KeypadDelete | Self::Delete => KeyCode::Char('\u{7f}'),
            Self::End => KeyCode::End,
            Self::Home => KeyCode::Home,
            Self::KeypadEquals | Self::Equal => KeyCode::Char('='),
            Self::Escape => KeyCode::Char('\u{1b}'),
            Self::Function => KeyCode::Physical(self),
            Self::Grave => KeyCode::Char('`'),
            Self::Help => KeyCode::Help,
            Self::Insert => KeyCode::Insert,
            Self::K0 => KeyCode::Char('0'),
            Self::K1 => KeyCode::Char('1'),
            Self::K2 => KeyCode::Char('2'),
            Self::K3 => KeyCode::Char('3'),
            Self::K4 => KeyCode::Char('4'),
            Self::K5 => KeyCode::Char('5'),
            Self::K6 => KeyCode::Char('6'),
            Self::K7 => KeyCode::Char('7'),
            Self::K8 => KeyCode::Char('8'),
            Self::K9 => KeyCode::Char('9'),
            Self::Return | Self::KeypadEnter => KeyCode::Char('\r'),
            Self::LeftBracket => KeyCode::Char('['),
            Self::RightBracket => KeyCode::Char(']'),
            Self::Minus => KeyCode::Char('-'),
            Self::VolumeMute => KeyCode::VolumeMute,
            Self::VolumeUp => KeyCode::VolumeUp,
            Self::VolumeDown => KeyCode::VolumeDown,
            Self::NumLock => KeyCode::NumLock,
            Self::PageUp => KeyCode::PageUp,
            Self::PageDown => KeyCode::PageDown,
            Self::Period => KeyCode::Char('.'),
            Self::Quote => KeyCode::Char('\''),
            Self::Semicolon => KeyCode::Char(';'),
            Self::Slash => KeyCode::Char('/'),
            Self::Space => KeyCode::Char(' '),
            Self::Tab => KeyCode::Char('\t'),
        }
    }

    fn make_map() -> HashMap<String, Self> {
        let mut map = HashMap::new();

        macro_rules! m {
            ($($val:ident),* $(,)?) => {
                $(
                    let key = stringify!($val).to_string();
                    if key.len() == 1 {
                        map.insert(key.to_ascii_lowercase(), PhysKeyCode::$val);
                    }
                    map.insert(key, PhysKeyCode::$val);
                )*
            }
        }

        m!(
            A,
            B,
            Backslash,
            C,
            CapsLock,
            Comma,
            D,
            Backspace,
            DownArrow,
            E,
            End,
            Equal,
            Escape,
            F,
            F1,
            F10,
            F11,
            F12,
            F13,
            F14,
            F15,
            F16,
            F17,
            F18,
            F19,
            F2,
            F20,
            F3,
            F4,
            F5,
            F6,
            F7,
            F8,
            F9,
            Delete,
            Function,
            G,
            Grave,
            H,
            Help,
            Home,
            I,
            Insert,
            J,
            K,
            Keypad0,
            Keypad1,
            Keypad2,
            Keypad3,
            Keypad4,
            Keypad5,
            Keypad6,
            Keypad7,
            Keypad8,
            Keypad9,
            KeypadClear,
            KeypadDecimal,
            KeypadDelete,
            KeypadDivide,
            KeypadEnter,
            KeypadEquals,
            KeypadSubtract,
            KeypadMultiply,
            KeypadAdd,
            L,
            LeftAlt,
            LeftArrow,
            LeftBracket,
            LeftControl,
            LeftShift,
            LeftWindows,
            M,
            Minus,
            VolumeMute,
            N,
            NumLock,
            O,
            P,
            PageDown,
            PageUp,
            Period,
            Q,
            Quote,
            R,
            Return,
            RightAlt,
            RightArrow,
            RightBracket,
            RightControl,
            RightShift,
            RightWindows,
            S,
            Semicolon,
            Slash,
            Space,
            T,
            Tab,
            U,
            UpArrow,
            V,
            VolumeDown,
            VolumeUp,
            W,
            X,
            Y,
            Z,
        );

        map.insert("0".to_string(), PhysKeyCode::K0);
        map.insert("1".to_string(), PhysKeyCode::K1);
        map.insert("2".to_string(), PhysKeyCode::K2);
        map.insert("3".to_string(), PhysKeyCode::K3);
        map.insert("4".to_string(), PhysKeyCode::K4);
        map.insert("5".to_string(), PhysKeyCode::K5);
        map.insert("6".to_string(), PhysKeyCode::K6);
        map.insert("7".to_string(), PhysKeyCode::K7);
        map.insert("8".to_string(), PhysKeyCode::K8);
        map.insert("9".to_string(), PhysKeyCode::K9);

        map
    }

    fn make_inv_map() -> HashMap<Self, String> {
        let mut map = HashMap::new();
        for (k, v) in PHYSKEYCODE_MAP.iter() {
            map.insert(*v, k.clone());
        }
        map
    }
}

lazy_static::lazy_static! {
    static ref PHYSKEYCODE_MAP: HashMap<String, PhysKeyCode> = PhysKeyCode::make_map();
    static ref INV_PHYSKEYCODE_MAP: HashMap<PhysKeyCode, String> = PhysKeyCode::make_inv_map();
}

impl TryFrom<&str> for PhysKeyCode {
    type Error = String;
    fn try_from(s: &str) -> std::result::Result<PhysKeyCode, String> {
        if let Some(code) = PHYSKEYCODE_MAP.get(s) {
            Ok(*code)
        } else {
            Err(format!("invalid PhysKeyCode '{}'", s))
        }
    }
}

impl ToString for PhysKeyCode {
    fn to_string(&self) -> String {
        if let Some(s) = INV_PHYSKEYCODE_MAP.get(self) {
            s.to_string()
        } else {
            format!("{:?}", self)
        }
    }
}

bitflags! {
    #[derive(Default)]
    pub struct MouseButtons: u8 {
        const NONE = 0;
        #[allow(clippy::identity_op)]
        const LEFT = 1<<0;
        const RIGHT = 1<<1;
        const MIDDLE = 1<<2;
        const X1 = 1<<3;
        const X2 = 1<<4;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MousePress {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    Move,
    Press(MousePress),
    Release(MousePress),
    VertWheel(i16),
    HorzWheel(i16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    /// Coordinates of the mouse relative to the top left of the window
    pub coords: Point,
    /// The mouse position in screen coordinates
    pub screen_coords: crate::ScreenPoint,
    pub mouse_buttons: MouseButtons,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone)]
pub struct Handled(Arc<AtomicBool>);

impl Handled {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn set_handled(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_handled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl PartialEq for Handled {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Handled {}

/// A key event prior to any dead key or IME composition
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RawKeyEvent {
    pub key: KeyCode,
    pub modifiers: Modifiers,
    pub leds: KeyboardLedStatus,

    /// The physical location of the key on an ANSI-Standard US layout
    pub phys_code: Option<PhysKeyCode>,
    /// The OS and hardware dependent key code for the key
    pub raw_code: u32,

    /// The *other* OS and hardware dependent key code for the key
    #[cfg(windows)]
    pub scan_code: u32,

    /// How many times this key repeats
    pub repeat_count: u16,

    /// If true, this is a key down rather than a key up event
    pub key_is_down: bool,
    pub handled: Handled,
}

impl RawKeyEvent {
    /// Mark the event as handled, in order to prevent additional
    /// processing.
    pub fn set_handled(&self) {
        self.handled.set_handled();
    }

    /// <https://sw.kovidgoyal.net/kitty/keyboard-protocol/#functional-key-definitions>
    #[deny(warnings)]
    fn kitty_function_code(&self) -> Option<u32> {
        use KeyCode::*;
        Some(match self.key {
            // Tab => 9,
            // Backspace => 127,
            // CapsLock => 57358,
            // ScrollLock => 57359,
            // NumLock => 57360,
            // PrintScreen => 57361,
            // Pause => 57362,
            // Menu => 57363,
            Function(n) if n >= 13 && n <= 35 => 57376 + n as u32 - 13,
            Numpad(n) => n as u32 + 57399,
            Decimal => 57409,
            Divide => 57410,
            Multiply => 57411,
            Subtract => 57412,
            Add => 57413,
            // KeypadEnter => 57414,
            // KeypadEquals => 57415,
            Separator => 57416,
            ApplicationLeftArrow => 57417,
            ApplicationRightArrow => 57418,
            ApplicationUpArrow => 57419,
            ApplicationDownArrow => 57420,
            KeyPadHome => 57423,
            KeyPadEnd => 57424,
            KeyPadBegin => 57427,
            KeyPadPageUp => 57421,
            KeyPadPageDown => 57422,
            Insert => 57425,
            // KeypadDelete => 57426,
            MediaPlayPause => 57430,
            MediaStop => 57432,
            MediaNextTrack => 57435,
            MediaPrevTrack => 57436,
            VolumeDown => 57436,
            VolumeUp => 57439,
            VolumeMute => 57440,
            LeftShift => 57441,
            LeftControl => 57442,
            LeftAlt => 57443,
            LeftWindows => 57444,
            RightShift => 57447,
            RightControl => 57448,
            RightAlt => 57449,
            RightWindows => 57450,
            _ => match &self.phys_code {
                Some(phys) => {
                    use PhysKeyCode::*;

                    match *phys {
                        Escape => 27,
                        Return => 13,
                        Tab => 9,
                        Backspace => 127,
                        CapsLock => 57358,
                        // ScrollLock => 57359,
                        NumLock => 57360,
                        // PrintScreen => 57361,
                        // Pause => 57362,
                        // Menu => 57363,
                        F13 => 57376,
                        F14 => 57377,
                        F15 => 57378,
                        F16 => 57379,
                        F17 => 57380,
                        F18 => 57381,
                        F19 => 57382,
                        F20 => 57383,
                        F21 => 57384,
                        F22 => 57385,
                        F23 => 57386,
                        F24 => 57387,
                        /*
                        F25 => 57388,
                        F26 => 57389,
                        F27 => 57390,
                        F28 => 57391,
                        F29 => 57392,
                        F30 => 57393,
                        F31 => 57394,
                        F32 => 57395,
                        F33 => 57396,
                        F34 => 57397,
                        */
                        Keypad0 => 57399,
                        Keypad1 => 57400,
                        Keypad2 => 57401,
                        Keypad3 => 57402,
                        Keypad4 => 57403,
                        Keypad5 => 57404,
                        Keypad6 => 57405,
                        Keypad7 => 57406,
                        Keypad8 => 57407,
                        Keypad9 => 57408,
                        KeypadDecimal => 57409,
                        KeypadDivide => 57410,
                        KeypadMultiply => 57411,
                        KeypadSubtract => 57412,
                        KeypadAdd => 57413,
                        KeypadEnter => 57414,
                        KeypadEquals => 57415,
                        // KeypadSeparator => 57416,
                        // ApplicationLeftArrow => 57417,
                        // ApplicationRightArrow => 57418,
                        // ApplicationUpArrow => 57419,
                        // ApplicationDownArrow => 57420,
                        // KeyPadHome => 57423,
                        // KeyPadEnd => 57424,
                        // KeyPadBegin => 57427,
                        // KeyPadPageUp => 57421,
                        // KeyPadPageDown => 57422,
                        Insert => 57425,
                        // KeypadDelete => 57426,
                        // MediaPlayPause => 57430,
                        // MediaStop => 57432,
                        // MediaNextTrack => 57435,
                        // MediaPrevTrack => 57436,
                        VolumeDown => 57436,
                        VolumeUp => 57439,
                        VolumeMute => 57440,
                        LeftShift => 57441,
                        LeftControl => 57442,
                        LeftAlt => 57443,
                        LeftWindows => 57444,
                        RightShift => 57447,
                        RightControl => 57448,
                        RightAlt => 57449,
                        RightWindows => 57450,
                        _ => return None,
                    }
                }
                _ => return None,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which key was pressed.
    /// This is the potentially processed/composed version
    /// of the input.
    pub key: KeyCode,
    /// Which modifiers are down
    pub modifiers: Modifiers,

    pub leds: KeyboardLedStatus,

    /// How many times this key repeats
    pub repeat_count: u16,

    /// If true, this is a key down rather than a key up event
    pub key_is_down: bool,

    /// If triggered from a raw key event, here it is.
    pub raw: Option<RawKeyEvent>,

    #[cfg(windows)]
    pub win32_uni_char: Option<char>,
}

fn normalize_shift(key: KeyCode, modifiers: Modifiers) -> (KeyCode, Modifiers) {
    if modifiers.contains(Modifiers::SHIFT) {
        match key {
            KeyCode::Char(c) if c.is_ascii_uppercase() => (key, modifiers - Modifiers::SHIFT),
            KeyCode::Char(c) if c.is_ascii_lowercase() => (
                KeyCode::Char(c.to_ascii_uppercase()),
                modifiers - Modifiers::SHIFT,
            ),
            _ => (key, modifiers),
        }
    } else {
        (key, modifiers)
    }
}

pub fn is_ascii_control(c: char) -> Option<char> {
    let c = c as u32;
    if c < 0x20 {
        let de_ctrl = ((c as u8) | 0x40) as char;
        Some(de_ctrl.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_ctrl(key: KeyCode, modifiers: Modifiers) -> (KeyCode, Modifiers) {
    if modifiers.contains(Modifiers::CTRL) {
        if let KeyCode::Char(c) = key {
            if (c as u32) < 0x20 {
                let de_ctrl = ((c as u8) | 0x40) as char;
                return (KeyCode::Char(de_ctrl.to_ascii_lowercase()), modifiers);
            }
        }
    }
    (key, modifiers)
}

impl KeyEvent {
    /// if SHIFT is held and we have KeyCode::Char('c') we want to normalize
    /// that keycode to KeyCode::Char('C'); that is what this function does.
    pub fn normalize_shift(mut self) -> Self {
        let (key, modifiers) = normalize_shift(self.key, self.modifiers);
        self.key = key;
        self.modifiers = modifiers;

        self
    }

    /// If the key code is a modifier key (Control, Alt, Shift), check
    /// the underlying raw event to see if we had a positional version
    /// of that key.
    /// If so, switch to the positional version.
    pub fn resurface_positional_modifier_key(mut self) -> Self {
        match self.key {
            KeyCode::Control
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::LeftControl | KeyCode::Physical(PhysKeyCode::LeftControl),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::LeftControl;
            }
            KeyCode::Control
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::RightControl | KeyCode::Physical(PhysKeyCode::RightControl),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::RightControl;
            }
            KeyCode::Alt
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::LeftAlt | KeyCode::Physical(PhysKeyCode::LeftAlt),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::LeftAlt;
            }
            KeyCode::Alt
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::RightAlt | KeyCode::Physical(PhysKeyCode::RightAlt),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::RightAlt;
            }
            KeyCode::Shift
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::LeftShift | KeyCode::Physical(PhysKeyCode::LeftShift),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::LeftShift;
            }
            KeyCode::Shift
                if matches!(
                    self.raw,
                    Some(RawKeyEvent {
                        key: KeyCode::RightShift | KeyCode::Physical(PhysKeyCode::RightShift),
                        ..
                    })
                ) =>
            {
                self.key = KeyCode::RightShift;
            }
            _ => {}
        }

        self
    }

    /// If CTRL is held down and we have KeyCode::Char(_) with the
    /// ASCII control value encoded, decode it back to the ASCII
    /// alpha keycode instead.
    pub fn normalize_ctrl(mut self) -> Self {
        let (key, modifiers) = normalize_ctrl(self.key, self.modifiers);
        self.key = key;
        self.modifiers = modifiers;

        self
    }

    #[cfg(not(windows))]
    pub fn encode_win32_input_mode(&self) -> Option<String> {
        None
    }

    /// <https://github.com/microsoft/terminal/blob/main/doc/specs/%234999%20-%20Improved%20keyboard%20handling%20in%20Conpty.md>
    #[cfg(windows)]
    pub fn encode_win32_input_mode(&self) -> Option<String> {
        let phys = self.raw.as_ref()?;

        let vkey = phys.raw_code;
        let scan_code = phys.scan_code;
        // <https://docs.microsoft.com/en-us/windows/console/key-event-record-str>
        // defines the dwControlKeyState values
        let mut control_key_state = 0;
        const SHIFT_PRESSED: usize = 0x10;
        const ENHANCED_KEY: usize = 0x100;
        const RIGHT_ALT_PRESSED: usize = 0x01;
        const LEFT_ALT_PRESSED: usize = 0x02;
        const LEFT_CTRL_PRESSED: usize = 0x08;
        const RIGHT_CTRL_PRESSED: usize = 0x04;

        if self
            .modifiers
            .intersects(Modifiers::SHIFT | Modifiers::LEFT_SHIFT | Modifiers::RIGHT_SHIFT)
        {
            control_key_state |= SHIFT_PRESSED;
        }

        if self.modifiers.contains(Modifiers::RIGHT_ALT) {
            control_key_state |= RIGHT_ALT_PRESSED;
        } else if self.modifiers.contains(Modifiers::ALT) {
            control_key_state |= LEFT_ALT_PRESSED;
        }
        if self.modifiers.contains(Modifiers::LEFT_ALT) {
            control_key_state |= LEFT_ALT_PRESSED;
        }
        if self.modifiers.contains(Modifiers::RIGHT_CTRL) {
            control_key_state |= RIGHT_CTRL_PRESSED;
        } else if self.modifiers.contains(Modifiers::CTRL) {
            control_key_state |= LEFT_CTRL_PRESSED;
        }
        if self.modifiers.contains(Modifiers::LEFT_CTRL) {
            control_key_state |= LEFT_CTRL_PRESSED;
        }
        if self.modifiers.contains(Modifiers::ENHANCED_KEY) {
            control_key_state |= ENHANCED_KEY;
        }

        let key_down = if self.key_is_down { 1 } else { 0 };

        match &self.key {
            KeyCode::Composed(_) => None,
            KeyCode::Char(c) => {
                let uni = self.win32_uni_char.unwrap_or(*c) as u32;
                Some(format!(
                    "\u{1b}[{};{};{};{};{};{}_",
                    vkey, scan_code, uni, key_down, control_key_state, self.repeat_count
                ))
            }
            _ => {
                let uni = 0;
                Some(format!(
                    "\u{1b}[{};{};{};{};{};{}_",
                    vkey, scan_code, uni, key_down, control_key_state, self.repeat_count
                ))
            }
        }
    }

    pub fn encode_kitty(&self, flags: KittyKeyboardFlags) -> String {
        use KeyCode::*;

        if !flags.contains(KittyKeyboardFlags::REPORT_EVENT_TYPES) && !self.key_is_down {
            return String::new();
        }

        if self.modifiers.is_empty()
            && !flags.contains(KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
            && self.key_is_down
        {
            // Check for simple text generating keys
            match &self.key {
                Char('\x08') => return '\x7f'.to_string(),
                Char('\x7f') => return '\x08'.to_string(),
                Char(c) => return c.to_string(),
                _ => {}
            }
        }

        let raw_modifiers = self
            .raw
            .as_ref()
            .map(|raw| raw.modifiers)
            .unwrap_or(self.modifiers);

        let mut modifiers = 0;
        if raw_modifiers.contains(Modifiers::SHIFT) {
            modifiers |= 1;
        }
        if raw_modifiers.contains(Modifiers::ALT) {
            modifiers |= 2;
        }
        if raw_modifiers.contains(Modifiers::CTRL) {
            modifiers |= 4;
        }
        if raw_modifiers.contains(Modifiers::SUPER) {
            modifiers |= 8;
        }
        // TODO: Hyper and Meta are not handled yet.
        // We should somehow detect this?
        // See: https://github.com/wezterm/wezterm/pull/4605#issuecomment-1823604708
        if self.leds.contains(KeyboardLedStatus::CAPS_LOCK) {
            modifiers |= 64;
        }
        if self.leds.contains(KeyboardLedStatus::NUM_LOCK) {
            modifiers |= 128;
        }
        modifiers += 1;

        let event_type =
            if flags.contains(KittyKeyboardFlags::REPORT_EVENT_TYPES) && !self.key_is_down {
                ":3"
            } else {
                ""
            };

        let is_legacy_key = match &self.key {
            Char(c) => c.is_ascii_alphanumeric() || c.is_ascii_punctuation(),
            _ => false,
        };

        let generated_text =
            if self.key_is_down && flags.contains(KittyKeyboardFlags::REPORT_ASSOCIATED_TEXT) {
                match &self.key {
                    Char(c) => format!(";{}", *c as u32),
                    KeyCode::Numpad(n) => format!(";{}", '0' as u32 + *n as u32),
                    Composed(s) => {
                        let mut codepoints = ";".to_string();
                        for c in s.chars() {
                            if codepoints.len() > 1 {
                                codepoints.push(':');
                            }
                            write!(&mut codepoints, "{}", c as u32).ok();
                        }
                        codepoints
                    }
                    _ => String::new(),
                }
            } else {
                String::new()
            };

        let guess_phys = self
            .raw
            .as_ref()
            .and_then(|raw| raw.phys_code)
            .or_else(|| self.key.to_phys());

        let is_numpad = guess_phys.and_then(|phys| match phys {
                PhysKeyCode::Keypad0
                | PhysKeyCode::Keypad1
                | PhysKeyCode::Keypad2
                | PhysKeyCode::Keypad3
                | PhysKeyCode::Keypad4
                | PhysKeyCode::Keypad5
                | PhysKeyCode::Keypad6
                | PhysKeyCode::Keypad7
                | PhysKeyCode::Keypad8
                | PhysKeyCode::Keypad9
                // | PhysKeyCode::KeypadClear not a physical numpad key?
                | PhysKeyCode::KeypadDecimal
                | PhysKeyCode::KeypadDelete
                | PhysKeyCode::KeypadDivide
                | PhysKeyCode::KeypadEnter
                | PhysKeyCode::KeypadEquals
                | PhysKeyCode::KeypadSubtract
                | PhysKeyCode::KeypadMultiply
                | PhysKeyCode::KeypadAdd
             => Some(phys),
            _ => None,
        });

        if let Some(numpad) = is_numpad {
            let code = match (numpad, self.leds.contains(KeyboardLedStatus::NUM_LOCK)) {
                (PhysKeyCode::Keypad0, true) => 57399,
                (PhysKeyCode::Keypad0, false) => 57425,
                (PhysKeyCode::Keypad1, true) => 57400,
                (PhysKeyCode::Keypad1, false) => 57424,
                (PhysKeyCode::Keypad2, true) => 57401,
                (PhysKeyCode::Keypad2, false) => 57420,
                (PhysKeyCode::Keypad3, true) => 57402,
                (PhysKeyCode::Keypad3, false) => 57422,
                (PhysKeyCode::Keypad4, true) => 57403,
                (PhysKeyCode::Keypad4, false) => 57417,
                (PhysKeyCode::Keypad5, true) => 57404,
                (PhysKeyCode::Keypad5, false) => {
                    let xt_mods = self.modifiers.encode_xterm();
                    return if xt_mods == 0 && self.key_is_down {
                        "\x1b[E".to_string()
                    } else {
                        format!("\x1b[1;{}{event_type}E", 1 + xt_mods)
                    };
                }
                (PhysKeyCode::Keypad6, true) => 57405,
                (PhysKeyCode::Keypad6, false) => 57418,
                (PhysKeyCode::Keypad7, true) => 57406,
                (PhysKeyCode::Keypad7, false) => 57423,
                (PhysKeyCode::Keypad8, true) => 57407,
                (PhysKeyCode::Keypad8, false) => 57419,
                (PhysKeyCode::Keypad9, true) => 57408,
                (PhysKeyCode::Keypad9, false) => 57421,
                (PhysKeyCode::KeypadDecimal, _) => 57409,
                (PhysKeyCode::KeypadDelete, _) => 57426,
                (PhysKeyCode::KeypadDivide, _) => 57410,
                (PhysKeyCode::KeypadEnter, _) => 57414,
                (PhysKeyCode::KeypadEquals, _) => 57415,
                (PhysKeyCode::KeypadSubtract, _) => 57412,
                (PhysKeyCode::KeypadMultiply, _) => 57411,
                (PhysKeyCode::KeypadAdd, _) => 57413,
                _ => unreachable!(),
            };
            return format!("\x1b[{code};{modifiers}{event_type}{generated_text}u");
        }

        match &self.key {
            PageUp | PageDown | Insert | Char('\x7f') => {
                let c = match &self.key {
                    Insert => 2,
                    Char('\x7f') => 3, // Delete
                    PageUp => 5,
                    PageDown => 6,
                    _ => unreachable!(),
                };

                format!("\x1b[{c};{modifiers}{event_type}~")
            }
            Char(shifted_key) => {
                let shifted_key = if *shifted_key == '\x08' {
                    // Backspace is really VERASE -> ASCII DEL
                    '\x7f'
                } else {
                    *shifted_key
                };

                let use_legacy = !flags.contains(KittyKeyboardFlags::REPORT_ALTERNATE_KEYS)
                    && event_type.is_empty()
                    && is_legacy_key
                    && !(flags.contains(KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES)
                        && (self.modifiers.contains(Modifiers::CTRL)
                            || self.modifiers.contains(Modifiers::ALT)))
                    && !self.modifiers.intersects(
                        Modifiers::SUPER, /* TODO: Hyper and Meta should be added here. */
                    );

                if use_legacy {
                    // Legacy text key
                    // https://sw.kovidgoyal.net/kitty/keyboard-protocol/#legacy-text-keys
                    let mut output = String::new();
                    if self.modifiers.contains(Modifiers::ALT) {
                        output.push('\x1b');
                    }

                    if self.modifiers.contains(Modifiers::CTRL) {
                        csi_u_encode(
                            &mut output,
                            shifted_key.to_ascii_uppercase(),
                            self.modifiers,
                        );
                    } else {
                        output.push(shifted_key);
                    }

                    return output;
                }

                // FIXME: ideally we'd get the correct unshifted key from
                // the OS based on the current keyboard layout. That needs
                // more plumbing, so for now, we're assuming the US layout.
                let c = us_layout_unshift(shifted_key);

                let base_layout = self
                    .raw
                    .as_ref()
                    .and_then(|raw| raw.phys_code.as_ref())
                    .and_then(|phys| match phys.to_key_code() {
                        KeyCode::Char(base) if base != c => Some(base),
                        _ => None,
                    });

                let mut key_code = format!("{}", (c as u32));

                if flags.contains(KittyKeyboardFlags::REPORT_ALTERNATE_KEYS)
                    && (c != shifted_key || base_layout.is_some())
                {
                    key_code.push(':');
                    if c != shifted_key {
                        key_code.push_str(&format!("{}", (shifted_key as u32)));
                    }
                    if let Some(base) = base_layout {
                        key_code.push_str(&format!(":{}", (base as u32)));
                    }
                }

                format!("\x1b[{key_code};{modifiers}{event_type}{generated_text}u")
            }
            LeftArrow | RightArrow | UpArrow | DownArrow | Home | End => {
                let c = match &self.key {
                    UpArrow => 'A',
                    DownArrow => 'B',
                    RightArrow => 'C',
                    LeftArrow => 'D',
                    Home => 'H',
                    End => 'F',
                    _ => unreachable!(),
                };
                format!("\x1b[1;{modifiers}{event_type}{c}")
            }
            Function(n) if *n < 25 => {
                // The spec says that kitty prefers an SS3 form for F1-F4,
                // but then has some variance in the encoding and cites a
                // compatibility issue with a cursor position report.
                // Since it allows reporting these all unambiguously with
                // the same general scheme, that is what we're using here.
                let intro = match *n {
                    1 => "\x1b[11",
                    2 => "\x1b[12",
                    3 => "\x1b[13",
                    4 => "\x1b[14",
                    5 => "\x1b[15",
                    6 => "\x1b[17",
                    7 => "\x1b[18",
                    8 => "\x1b[19",
                    9 => "\x1b[20",
                    10 => "\x1b[21",
                    11 => "\x1b[23",
                    12 => "\x1b[24",
                    13 => "\x1b[57376",
                    14 => "\x1b[57377",
                    15 => "\x1b[57378",
                    16 => "\x1b[57379",
                    17 => "\x1b[57380",
                    18 => "\x1b[57381",
                    19 => "\x1b[57382",
                    20 => "\x1b[57383",
                    21 => "\x1b[57384",
                    22 => "\x1b[57385",
                    23 => "\x1b[57386",
                    24 => "\x1b[57387",
                    _ => unreachable!(),
                };
                // for F1-F12 the spec says we should terminate with ~
                // for F13 and up the spec says we should terminate with u
                let end_char = if *n < 13 { '~' } else { 'u' };

                format!("{intro};{modifiers}{event_type}{end_char}")
            }

            _ => {
                let code = self.raw.as_ref().and_then(|raw| raw.kitty_function_code());

                match (
                    code,
                    flags.contains(KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES),
                ) {
                    (Some(code), true) => {
                        format!("\x1b[{code};{modifiers}{event_type}{generated_text}u")
                    }
                    _ => String::new(),
                }
            }
        }
    }
}

fn csi_u_encode(buf: &mut String, c: char, mods: Modifiers) {
    let c = if mods.contains(Modifiers::CTRL) && ctrl_mapping(c).is_some() {
        ctrl_mapping(c).unwrap()
    } else {
        c
    };
    if mods.contains(Modifiers::ALT) {
        buf.push(0x1b as char);
    }
    write!(buf, "{}", c).ok();
}

bitflags::bitflags! {
pub struct KittyKeyboardFlags: u16 {
    const NONE = 0;
    const DISAMBIGUATE_ESCAPE_CODES = 1;
    const REPORT_EVENT_TYPES = 2;
    const REPORT_ALTERNATE_KEYS = 4;
    const REPORT_ALL_KEYS_AS_ESCAPE_CODES = 8;
    const REPORT_ASSOCIATED_TEXT = 16;
}
}

bitflags! {
    #[derive(FromDynamic, ToDynamic)]
    #[cfg_attr(feature="serde", derive(Serialize, Deserialize), serde(try_from = "String"))]
    #[dynamic(try_from = "String", into = "String")]
    pub struct WindowDecorations: u8 {
        const TITLE = 1;
        const RESIZE = 2;
        const NONE = 0;
        // Reserve two bits for this enable/disable shadow,
        // so that we effective have Option<bool>
        const MACOS_FORCE_DISABLE_SHADOW = 4;
        const MACOS_FORCE_ENABLE_SHADOW = 4|8;
        const INTEGRATED_BUTTONS = 16;
        const MACOS_FORCE_SQUARE_CORNERS = 32;
    }
}

impl Into<String> for &WindowDecorations {
    fn into(self) -> String {
        let mut s = vec![];
        if self.contains(WindowDecorations::TITLE) {
            s.push("TITLE");
        }
        if self.contains(WindowDecorations::RESIZE) {
            s.push("RESIZE");
        }
        if self.contains(WindowDecorations::INTEGRATED_BUTTONS) {
            s.push("INTEGRATED_BUTTONS");
        }
        if self.contains(WindowDecorations::MACOS_FORCE_ENABLE_SHADOW) {
            s.push("MACOS_FORCE_ENABLE_SHADOW");
        } else if self.contains(WindowDecorations::MACOS_FORCE_DISABLE_SHADOW) {
            s.push("MACOS_FORCE_DISABLE_SHADOW");
        } else if self.contains(WindowDecorations::MACOS_FORCE_SQUARE_CORNERS) {
            s.push("MACOS_FORCE_SQUARE_CORNERS");
        }
        if s.is_empty() {
            "NONE".to_string()
        } else {
            s.join("|")
        }
    }
}

impl TryFrom<String> for WindowDecorations {
    type Error = String;
    fn try_from(s: String) -> std::result::Result<WindowDecorations, String> {
        let mut flags = Self::NONE;
        for ele in s.split('|') {
            let ele = ele.trim();
            if ele == "TITLE" {
                flags |= Self::TITLE;
            } else if ele == "NONE" || ele == "None" {
                flags = Self::NONE;
            } else if ele == "RESIZE" {
                flags |= Self::RESIZE;
            } else if ele == "MACOS_FORCE_DISABLE_SHADOW" {
                flags |= Self::MACOS_FORCE_DISABLE_SHADOW;
            } else if ele == "MACOS_FORCE_ENABLE_SHADOW" {
                flags |= Self::MACOS_FORCE_ENABLE_SHADOW;
            } else if ele == "MACOS_FORCE_SQUARE_CORNERS" {
                flags |= Self::MACOS_FORCE_SQUARE_CORNERS;
            } else if ele == "INTEGRATED_BUTTONS" {
                flags |= Self::INTEGRATED_BUTTONS;
            } else {
                return Err(format!("invalid WindowDecoration name {} in {}", ele, s));
            }
        }
        Ok(flags)
    }
}

impl Default for WindowDecorations {
    fn default() -> Self {
        WindowDecorations::TITLE | WindowDecorations::RESIZE
    }
}

#[derive(Debug, FromDynamic, ToDynamic, PartialEq, Eq, Clone, Copy)]
pub enum IntegratedTitleButton {
    Hide,
    Maximize,
    Close,
}

#[derive(Debug, Default, FromDynamic, ToDynamic, PartialEq, Eq, Clone, Copy)]
pub enum IntegratedTitleButtonAlignment {
    #[default]
    Right,
    Left,
}

#[derive(Debug, ToDynamic, PartialEq, Eq, Clone, Copy)]
pub enum IntegratedTitleButtonStyle {
    Windows,
    Gnome,
    MacOsNative,
}

impl Default for IntegratedTitleButtonStyle {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOsNative
        } else {
            Self::Windows
        }
    }
}

impl FromDynamic for IntegratedTitleButtonStyle {
    fn from_dynamic(
        value: &wezterm_dynamic::Value,
        _options: wezterm_dynamic::FromDynamicOptions,
    ) -> Result<Self, wezterm_dynamic::Error>
    where
        Self: Sized,
    {
        let type_name = "integrated_title_button_style";

        if let wezterm_dynamic::Value::String(string) = value {
            let style = match string.as_str() {
                "Windows" => Self::Windows,
                "Gnome" => Self::Gnome,
                "MacOsNative" if cfg!(target_os = "macos") => Self::MacOsNative,
                _ => {
                    return Err(wezterm_dynamic::Error::InvalidVariantForType {
                        variant_name: string.to_string(),
                        type_name,
                        possible: &["Windows", "Gnome", "MacOsNative"],
                    });
                }
            };
            Ok(style)
        } else {
            Err(wezterm_dynamic::Error::InvalidVariantForType {
                variant_name: value.variant_name().to_string(),
                type_name,
                possible: &["String"],
            })
        }
    }
}

/// Kitty wants us to report the un-shifted version of a key.
/// It's a PITA to obtain that from the OS-dependent keyboard
/// layout stuff. For the moment, we'll do the slightly gross
/// thing and make an assumption that a US ANSI layout is in
/// use; this function encodes that mapping.
fn us_layout_unshift(c: char) -> char {
    match c {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        '+' => '=',
        '~' => '`',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        c => {
            let s: Vec<char> = c.to_lowercase().collect();
            if s.len() == 1 {
                s[0]
            } else {
                c
            }
        }
    }
}

/// Map c to its Ctrl equivalent.
/// In theory, this mapping is simply translating alpha characters
/// to upper case and then masking them by 0x1f, but xterm inherits
/// some built-in translation from legacy X11 so that are some
/// aliased mappings and a couple that might be technically tied
/// to US keyboard layout (particularly the punctuation characters
/// produced in combination with SHIFT) that may not be 100%
/// the right thing to do here for users with non-US layouts.
pub fn ctrl_mapping(c: char) -> Option<char> {
    Some(match c {
        '@' | '`' | ' ' | '2' => '\x00',
        'A' | 'a' => '\x01',
        'B' | 'b' => '\x02',
        'C' | 'c' => '\x03',
        'D' | 'd' => '\x04',
        'E' | 'e' => '\x05',
        'F' | 'f' => '\x06',
        'G' | 'g' => '\x07',
        'H' | 'h' => '\x08',
        'I' | 'i' => '\x09',
        'J' | 'j' => '\x0a',
        'K' | 'k' => '\x0b',
        'L' | 'l' => '\x0c',
        'M' | 'm' => '\x0d',
        'N' | 'n' => '\x0e',
        'O' | 'o' => '\x0f',
        'P' | 'p' => '\x10',
        'Q' | 'q' => '\x11',
        'R' | 'r' => '\x12',
        'S' | 's' => '\x13',
        'T' | 't' => '\x14',
        'U' | 'u' => '\x15',
        'V' | 'v' => '\x16',
        'W' | 'w' => '\x17',
        'X' | 'x' => '\x18',
        'Y' | 'y' => '\x19',
        'Z' | 'z' => '\x1a',
        '[' | '3' | '{' => '\x1b',
        '\\' | '4' | '|' => '\x1c',
        ']' | '5' | '}' => '\x1d',
        '^' | '6' | '~' => '\x1e',
        '_' | '7' | '/' => '\x1f',
        '8' | '?' => '\x7f', // `Delete`
        _ => return None,
    })
}

#[derive(Debug, FromDynamic, ToDynamic, Clone, Copy, PartialEq, Eq)]
pub enum UIKeyCapRendering {
    /// Super, Meta, Ctrl, Shift
    UnixLong,
    /// Super, M, C, S
    Emacs,
    /// Apple macOS style symbols
    AppleSymbols,
    /// Win, Alt, Ctrl, Shift
    WindowsLong,
    /// Like WindowsLong, but using a logo for the Win key
    WindowsSymbols,
}

impl Default for UIKeyCapRendering {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::AppleSymbols
        } else if cfg!(windows) {
            Self::WindowsSymbols
        } else {
            Self::UnixLong
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encode_issue_3220() {
        let flags =
            KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES | KittyKeyboardFlags::REPORT_EVENT_TYPES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('o'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "o".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('o'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: false,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[111;1:3u".to_string()
        );
    }

    #[test]
    fn encode_issue_3473() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Function(1),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[11;1~".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Function(1),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: false,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[11;1:3~".to_string()
        );
    }

    #[test]
    fn encode_issue_2546() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('i'),
                modifiers: Modifiers::ALT | Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;4u".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('I'),
                modifiers: Modifiers::ALT | Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;4u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('1'),
                modifiers: Modifiers::ALT | Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[49;4u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Char('!'),
                    modifiers: Modifiers::ALT | Modifiers::SHIFT,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::K1)
            )
            .encode_kitty(flags),
            "\x1b[49;4u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('i'),
                modifiers: Modifiers::SHIFT | Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;6u".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('I'),
                modifiers: Modifiers::SHIFT | Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;6u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('I'),
                modifiers: Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: Some(RawKeyEvent {
                    key: KeyCode::Char('I'),
                    modifiers: Modifiers::SHIFT | Modifiers::CTRL,
                    handled: Handled::new(),
                    key_is_down: true,
                    raw_code: 0,
                    leds: KeyboardLedStatus::empty(),
                    phys_code: Some(PhysKeyCode::I),
                    #[cfg(windows)]
                    scan_code: 0,
                    repeat_count: 1,
                }),
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;6u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('i'),
                modifiers: Modifiers::ALT | Modifiers::SHIFT | Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;8u".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('I'),
                modifiers: Modifiers::ALT | Modifiers::SHIFT | Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[105;8u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('\x08'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x7f".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('\x08'),
                modifiers: Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\x1b[127;5u".to_string()
        );
    }

    #[test]
    fn encode_issue_3474() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('A'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\u{1b}[97:65;1u".to_string()
        );
        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('A'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: false,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\u{1b}[97:65;1:3u".to_string()
        );
    }

    fn make_event_with_raw(mut event: KeyEvent, phys: Option<PhysKeyCode>) -> KeyEvent {
        let phys = match phys {
            Some(phys) => Some(phys),
            None => event.key.to_phys(),
        };

        event.raw = Some(RawKeyEvent {
            key: event.key.clone(),
            modifiers: event.modifiers,
            leds: KeyboardLedStatus::empty(),
            phys_code: phys,
            raw_code: 0,
            #[cfg(windows)]
            scan_code: 0,
            repeat_count: 1,
            key_is_down: event.key_is_down,
            handled: Handled::new(),
        });

        event
    }

    #[test]
    fn encode_issue_3476() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::LeftShift,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57441;1u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::LeftShift,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: false,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57441;1:3u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::LeftControl,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57442;1u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::LeftControl,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: false,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57442;1:3u".to_string()
        );
    }

    #[test]
    fn encode_issue_3478() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(0),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57425;1u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(0),
                    modifiers: Modifiers::SHIFT,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57425;2u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(1),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57424;1u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(1),
                    modifiers: Modifiers::SHIFT,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                None
            )
            .encode_kitty(flags),
            "\u{1b}[57424;2u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(0),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::NUM_LOCK,
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad0)
            )
            .encode_kitty(flags),
            "\u{1b}[57399;129u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(0),
                    modifiers: Modifiers::SHIFT,
                    leds: KeyboardLedStatus::NUM_LOCK,
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad0)
            )
            .encode_kitty(flags),
            "\u{1b}[57399;130u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::NUM_LOCK,
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[57404;129u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[E".to_string()
        );
    }

    #[test]
    fn encode_issue_3478_extra() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_ASSOCIATED_TEXT;

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::NUM_LOCK,
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[57404;129;53u".to_string()
        );
        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::NUM_LOCK,
                    repeat_count: 1,
                    key_is_down: false,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[57404;129:3u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[E".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Numpad(5),
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: false,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::Keypad5)
            )
            .encode_kitty(flags),
            "\u{1b}[1;1:3E".to_string()
        );
    }

    #[test]
    fn encode_issue_3315() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('"'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\"".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('"'),
                modifiers: Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\"".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('!'),
                modifiers: Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "!".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::LeftShift,
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "".to_string()
        );
    }

    #[test]
    fn encode_issue_3479() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Char('ф'),
                    modifiers: Modifiers::CTRL,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::A)
            )
            .encode_kitty(flags),
            "\x1b[1092::97;5u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Char('Ф'),
                    modifiers: Modifiers::CTRL | Modifiers::SHIFT,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::A)
            )
            .encode_kitty(flags),
            "\x1b[1092:1060:97;6u".to_string()
        );
    }

    #[test]
    fn encode_issue_3484() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_EVENT_TYPES
            | KittyKeyboardFlags::REPORT_ALTERNATE_KEYS
            | KittyKeyboardFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            | KittyKeyboardFlags::REPORT_ASSOCIATED_TEXT;

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Char('ф'),
                    modifiers: Modifiers::CTRL,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::A)
            )
            .encode_kitty(flags),
            "\x1b[1092::97;5;1092u".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::Char('Ф'),
                    modifiers: Modifiers::CTRL | Modifiers::SHIFT,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::A)
            )
            .encode_kitty(flags),
            "\x1b[1092:1060:97;6;1060u".to_string()
        );
    }

    #[test]
    fn encode_issue_3526() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char(' '),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::NUM_LOCK,
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            " ".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char(' '),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::CAPS_LOCK,
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            " ".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::NumLock,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::NumLock)
            )
            .encode_kitty(flags),
            "".to_string()
        );

        assert_eq!(
            make_event_with_raw(
                KeyEvent {
                    key: KeyCode::CapsLock,
                    modifiers: Modifiers::NONE,
                    leds: KeyboardLedStatus::empty(),
                    repeat_count: 1,
                    key_is_down: true,
                    raw: None,
                    #[cfg(windows)]
                    win32_uni_char: None,
                },
                Some(PhysKeyCode::CapsLock)
            )
            .encode_kitty(flags),
            "".to_string()
        );
    }

    #[test]
    fn encode_issue_4436() {
        let flags = KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES;

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('q'),
                modifiers: Modifiers::NONE,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "q".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('f'),
                modifiers: Modifiers::SUPER,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\u{1b}[102;9u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('f'),
                modifiers: Modifiers::SUPER | Modifiers::SHIFT,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\u{1b}[102;10u".to_string()
        );

        assert_eq!(
            KeyEvent {
                key: KeyCode::Char('f'),
                modifiers: Modifiers::SUPER | Modifiers::SHIFT | Modifiers::CTRL,
                leds: KeyboardLedStatus::empty(),
                repeat_count: 1,
                key_is_down: true,
                raw: None,
                #[cfg(windows)]
                win32_uni_char: None,
            }
            .encode_kitty(flags),
            "\u{1b}[102;14u".to_string()
        );
    }
}
