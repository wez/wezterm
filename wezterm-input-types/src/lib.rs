use bitflags::*;
use serde::*;
use std::collections::HashMap;
use std::convert::TryFrom;

pub struct PixelUnit;
pub struct ScreenPixelUnit;
pub type Point = euclid::Point2D<isize, PixelUnit>;
pub type PointF = euclid::Point2D<f32, PixelUnit>;
pub type ScreenPoint = euclid::Point2D<isize, ScreenPixelUnit>;

/// Which key is pressed.  Not all of these are probable to appear
/// on most systems.  A lot of this list is @wez trawling docs and
/// making an entry for things that might be possible in this first pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
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
}

impl ToString for KeyCode {
    fn to_string(&self) -> String {
        match self {
            Self::RawCode(n) => format!("raw:{}", n),
            Self::Char(c) => c.to_string(),
            Self::Composed(s) => s.to_string(),
            Self::Numpad(n) => format!("Numpad{}", n),
            Self::Function(n) => format!("F{}", n),
            other => format!("{:?}", other),
        }
    }
}

bitflags! {
    #[derive(Default, Deserialize, Serialize)]
    pub struct Modifiers: u8 {
        const NONE = 0;
        const SHIFT = 1<<1;
        const ALT = 1<<2;
        const CTRL = 1<<3;
        const SUPER = 1<<4;
        const LEFT_ALT = 1<<5;
        const RIGHT_ALT = 1<<6;
        const LEADER = 1<<7;
    }
}

impl ToString for Modifiers {
    fn to_string(&self) -> String {
        let mut s = String::new();
        if *self == Self::NONE {
            s.push_str("NONE");
        }

        for (value, label) in [
            (Self::SHIFT, "SHIFT"),
            (Self::ALT, "ALT"),
            (Self::CTRL, "CTRL"),
            (Self::SUPER, "SUPER"),
            (Self::LEFT_ALT, "LEFT_ALT"),
            (Self::RIGHT_ALT, "RIGHT_ALT"),
            (Self::LEADER, "LEADER"),
        ] {
            if !self.contains(value) {
                continue;
            }
            if !s.is_empty() {
                s.push('|');
            }
            s.push_str(label);
        }

        s
    }
}

/// These keycodes identify keys based on their physical
/// position on an ANSI-standard US keyboard.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash, Copy)]
pub enum PhysKeyCode {
    A,
    B,
    Backslash,
    C,
    CapsLock,
    Comma,
    D,
    Delete,
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
    ForwardDelete,
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
    KeypadMinus,
    KeypadMultiply,
    KeypadPlus,
    L,
    LeftAlt,
    LeftArrow,
    LeftBracket,
    LeftControl,
    LeftShift,
    LeftWindows,
    M,
    Minus,
    Mute,
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
    fn make_map() -> HashMap<String, Self> {
        let mut map = HashMap::new();

        macro_rules! m {
            ($($val:ident),* $(,)?) => {
                $(
                    map.insert(stringify!($val).to_string(), PhysKeyCode::$val);
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
            Delete,
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
            ForwardDelete,
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
            KeypadMinus,
            KeypadMultiply,
            KeypadPlus,
            L,
            LeftAlt,
            LeftArrow,
            LeftBracket,
            LeftControl,
            LeftShift,
            LeftWindows,
            M,
            Minus,
            Mute,
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
            map.insert(v.clone(), k.clone());
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which key was pressed.
    /// This is the potentially processed/composed version
    /// of the input.
    pub key: KeyCode,
    /// Which modifiers are down
    pub modifiers: Modifiers,

    /// The raw unprocessed key press if it was different from
    /// the processed/composed version
    pub raw_key: Option<KeyCode>,
    pub raw_modifiers: Modifiers,
    pub raw_code: Option<u32>,

    /// The physical location of the key on an ANSI-Standard US layout
    pub phys_code: Option<PhysKeyCode>,

    /// How many times this key repeats
    pub repeat_count: u16,

    /// If true, this is a key down rather than a key up event
    pub key_is_down: bool,
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

        if let Some(raw) = self.raw_key.take() {
            let (key, modifiers) = normalize_shift(raw, self.raw_modifiers);
            self.raw_key.replace(key);
            self.raw_modifiers = modifiers;
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

        if let Some(raw) = self.raw_key.take() {
            let (key, modifiers) = normalize_ctrl(raw, self.raw_modifiers);
            self.raw_key.replace(key);
            self.raw_modifiers = modifiers;
        }

        self
    }
}

bitflags! {
    #[derive(Deserialize, Serialize)]
    #[serde(try_from = "String")]
    pub struct WindowDecorations: u8 {
        const TITLE = 1;
        const RESIZE = 2;
        const NONE = 0;
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
