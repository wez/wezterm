use bitflags::*;
use serde::*;
use std::collections::HashMap;
use std::convert::TryFrom;
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
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    Serialize,
    Ord,
    PartialOrd,
    FromDynamic,
    ToDynamic,
)]
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
}

impl KeyCode {
    /// Return true if the key represents a modifier key.
    pub fn is_modifier(&self) -> bool {
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
    #[derive(Default, Deserialize, Serialize, FromDynamic, ToDynamic)]
    #[dynamic(into="String", try_from="String")]
    pub struct Modifiers: u16 {
        const NONE = 0;
        const SHIFT = 1<<1;
        const ALT = 1<<2;
        const CTRL = 1<<3;
        const SUPER = 1<<4;
        const LEFT_ALT = 1<<5;
        const RIGHT_ALT = 1<<6;
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

impl Into<String> for &Modifiers {
    fn into(self) -> String {
        self.to_string()
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
    pub fn to_string_with_separator(&self, args: ModifierToStringArgs) -> String {
        let mut s = String::new();
        if args.want_none && *self == Self::NONE {
            s.push_str("NONE");
        }

        // The unicode escapes here are nerdfont symbols; we use those because
        // we're guaranteed to have them available, and the symbols are
        // very legible

        for (value, label, unix, emacs, apple, windows, win_sym) in [
            (
                Self::SHIFT,
                "SHIFT",
                "Shift",
                "S",
                "\u{fb35}",
                "Shift",
                "Shift",
            ),
            (Self::ALT, "ALT", "Alt", "M", "\u{fb34}", "Alt", "Alt"),
            (Self::CTRL, "CTRL", "Ctrl", "C", "\u{fb33}", "Ctrl", "Ctrl"),
            (
                Self::SUPER,
                "SUPER",
                "Super",
                "Super",
                "\u{fb32}",
                "Win",
                "\u{fab2}",
            ),
            (
                Self::LEFT_ALT,
                "LEFT_ALT",
                "Alt",
                "M",
                "\u{fb34}",
                "Alt",
                "Alt",
            ),
            (
                Self::RIGHT_ALT,
                "RIGHT_ALT",
                "Alt",
                "M",
                "\u{fb34}",
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
                "\u{fb33}",
                "Ctrl",
                "Ctrl",
            ),
            (
                Self::RIGHT_CTRL,
                "RIGHT_CTRL",
                "Ctrl",
                "C",
                "\u{fb33}",
                "Ctrl",
                "Ctrl",
            ),
            (
                Self::LEFT_SHIFT,
                "LEFT_SHIFT",
                "Shift",
                "S",
                "\u{fb35}",
                "Shift",
                "Shift",
            ),
            (
                Self::RIGHT_SHIFT,
                "RIGHT_SHIFT",
                "Shift",
                "S",
                "\u{fb35}",
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
#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Copy,
    Ord,
    PartialOrd,
    FromDynamic,
    ToDynamic,
)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which key was pressed.
    /// This is the potentially processed/composed version
    /// of the input.
    pub key: KeyCode,
    /// Which modifiers are down
    pub modifiers: Modifiers,

    /// How many times this key repeats
    pub repeat_count: u16,

    /// If true, this is a key down rather than a key up event
    pub key_is_down: bool,

    /// If triggered from a raw key event, here it is.
    pub raw: Option<RawKeyEvent>,
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

        if self.modifiers.contains(Modifiers::SHIFT) {
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
                let c = match *c {
                    // Delete key is transmitted as 0x0
                    '\x7f' => '\x00',
                    // Backspace key is transmitted as 0x8, 0x7f or 0x0
                    '\x08' => {
                        if self.modifiers.contains(Modifiers::CTRL) {
                            if self.modifiers.contains(Modifiers::ALT)
                                || self.modifiers.contains(Modifiers::SHIFT)
                            {
                                '\x00'
                            } else {
                                '\x7f'
                            }
                        } else {
                            '\x08'
                        }
                    }
                    _ => *c,
                };

                let c = if self.modifiers.contains(Modifiers::CTRL) {
                    // Ensure that we rewrite the unicode value to the ASCII CTRL
                    // equivalent value.
                    // <https://github.com/microsoft/terminal/issues/13134>
                    ctrl_mapping(c).unwrap_or(c)
                } else {
                    c
                };
                let uni = c as u32;

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
}

bitflags! {
    #[derive(Deserialize, Serialize, FromDynamic, ToDynamic)]
    #[serde(try_from = "String")]
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

/// Map c to its Ctrl equivalent.
/// In theory, this mapping is simply translating alpha characters
/// to upper case and then masking them by 0x1f, but xterm inherits
/// some built-in translation from legacy X11 so that are some
/// aliased mappings and a couple that might be technically tied
/// to US keyboard layout (particularly the punctuation characters
/// produced in combination with SHIFT) that may not be 100%
/// the right thing to do here for users with non-US layouts.
#[cfg(windows)]
fn ctrl_mapping(c: char) -> Option<char> {
    // Please also sync with the copy of this function that
    // lives in termwiz :-/
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
