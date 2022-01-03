use crate::{KeyAssignment, MouseEventTrigger};
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::convert::TryFrom;
use wezterm_input_types::{KeyCode, Modifiers, PhysKeyCode};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KeyNoAction {
    #[serde(deserialize_with = "de_keycode", serialize_with = "ser_keycode")]
    pub key: KeyCode,
    #[serde(
        deserialize_with = "de_modifiers",
        serialize_with = "ser_modifiers",
        default
    )]
    pub mods: Modifiers,
}
impl_lua_conversion!(KeyNoAction);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Key {
    #[serde(deserialize_with = "de_keycode", serialize_with = "ser_keycode")]
    pub key: KeyCode,
    #[serde(
        deserialize_with = "de_modifiers",
        serialize_with = "ser_modifiers",
        default
    )]
    pub mods: Modifiers,
    pub action: KeyAssignment,
}
impl_lua_conversion!(Key);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LeaderKey {
    #[serde(deserialize_with = "de_keycode", serialize_with = "ser_keycode")]
    pub key: KeyCode,
    #[serde(
        deserialize_with = "de_modifiers",
        serialize_with = "ser_modifiers",
        default
    )]
    pub mods: Modifiers,
    #[serde(default = "default_leader_timeout")]
    pub timeout_milliseconds: u64,
}
impl_lua_conversion!(LeaderKey);

fn default_leader_timeout() -> u64 {
    1000
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Mouse {
    pub event: MouseEventTrigger,
    #[serde(
        deserialize_with = "de_modifiers",
        serialize_with = "ser_modifiers",
        default
    )]
    pub mods: Modifiers,
    pub action: KeyAssignment,
}
impl_lua_conversion!(Mouse);

fn make_map() -> HashMap<String, KeyCode> {
    let mut map = HashMap::new();

    macro_rules! m {
        ($($val:ident),* $(,)?) => {
            $(
                let v = KeyCode::$val;
                if let Some(phys) = v.to_phys() {
                    map.insert(stringify!($val).to_string(), KeyCode::Physical(phys));
                    map.insert(format!("mapped:{}", stringify!($val)), v);
                } else {
                    map.insert(stringify!($val).to_string(), v);
                }
            )*
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

    for (label, phys) in &[
        ("Backspace", PhysKeyCode::Backspace),
        ("Delete", PhysKeyCode::Delete),
        ("Enter", PhysKeyCode::Return),
        ("Escape", PhysKeyCode::Escape),
        ("Tab", PhysKeyCode::Tab),
    ] {
        map.insert(label.to_string(), KeyCode::Physical(*phys));
        map.insert(format!("mapped:{}", label), phys.to_key_code());
    }

    for i in 0..=9 {
        let k = KeyCode::Numpad(i);
        map.insert(
            format!("Numpad{}", i),
            KeyCode::Physical(k.to_phys().unwrap()),
        );
        // Not sure how likely someone is to remap the numpad, but...
        map.insert(format!("mapped:Numpad{}", i), k);
    }

    for i in 1..=24 {
        let k = KeyCode::Function(i);
        if let Some(phys) = k.to_phys() {
            map.insert(format!("F{}", i), KeyCode::Physical(phys));
            map.insert(format!("mapped:F{}", i), k);
        } else {
            // 21 and up don't have phys equivalents
            map.insert(format!("F{}", i), k);
        }
    }

    map
}

fn make_inv_map() -> HashMap<KeyCode, String> {
    let mut map = HashMap::new();
    for (k, v) in KEYCODE_MAP.iter() {
        map.insert(v.clone(), k.clone());
    }
    map
}

lazy_static::lazy_static! {
    static ref KEYCODE_MAP: HashMap<String, KeyCode> = make_map();
    static ref INV_KEYCODE_MAP: HashMap<KeyCode, String> = make_inv_map();
}

pub(crate) fn ser_keycode<S>(key: &KeyCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(s) = INV_KEYCODE_MAP.get(key) {
        serializer.serialize_str(s)
    } else {
        let s = key.to_string();
        serializer.serialize_str(&s)
    }
}

fn de_keycode<'de, D>(deserializer: D) -> Result<KeyCode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if let Some(c) = KEYCODE_MAP.get(&s) {
        return Ok(c.clone());
    }

    if s.len() > 5 && s.starts_with("phys:") {
        let phys = PhysKeyCode::try_from(&s[5..]).map_err(|_| {
            serde::de::Error::custom(format!(
                "expected phys:CODE physical keycode string, got: {}",
                s
            ))
        })?;
        return Ok(KeyCode::Physical(phys));
    }

    if s.len() > 4 && s.starts_with("raw:") {
        let num: u32 = s[4..].parse().map_err(|_| {
            serde::de::Error::custom(format!(
                "expected raw:<NUMBER> raw keycode string, got: {}",
                s
            ))
        })?;
        return Ok(KeyCode::RawCode(num));
    }

    if let Some(mapped) = s.strip_prefix("mapped:") {
        let chars: Vec<char> = mapped.chars().collect();
        return if chars.len() == 1 {
            Ok(KeyCode::Char(chars[0]))
        } else {
            Err(serde::de::Error::custom(format!(
                "invalid KeyCode string {}",
                s
            )))
        };
    }

    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        let k = KeyCode::Char(chars[0]);
        if let Some(phys) = k.to_phys() {
            Ok(KeyCode::Physical(phys))
        } else {
            Ok(k)
        }
    } else {
        Err(serde::de::Error::custom(format!(
            "invalid KeyCode string {}",
            s
        )))
    }
}

pub(crate) fn ser_modifiers<S>(mods: &Modifiers, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = mods.to_string();
    serializer.serialize_str(&s)
}

pub(crate) fn de_modifiers<'de, D>(deserializer: D) -> Result<Modifiers, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
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
            return Err(serde::de::Error::custom(format!(
                "invalid modifier name {} in {}",
                ele, s
            )));
        }
    }
    Ok(mods)
}
