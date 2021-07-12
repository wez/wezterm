use crate::{KeyAssignment, MouseEventTrigger};
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use wezterm_input_types::{KeyCode, Modifiers};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Key {
    #[serde(deserialize_with = "de_keycode")]
    pub key: KeyCode,
    #[serde(deserialize_with = "de_modifiers", default)]
    pub mods: Modifiers,
    pub action: KeyAssignment,
}
impl_lua_conversion!(Key);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LeaderKey {
    #[serde(deserialize_with = "de_keycode")]
    pub key: KeyCode,
    #[serde(deserialize_with = "de_modifiers", default)]
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
    #[serde(deserialize_with = "de_modifiers", default)]
    pub mods: Modifiers,
    pub action: KeyAssignment,
}
impl_lua_conversion!(Mouse);

fn make_map() -> HashMap<String, KeyCode> {
    let mut map = HashMap::new();

    macro_rules! m {
        ($($val:ident),* $(,)?) => {
            $(
                map.insert(stringify!($val).to_string(), KeyCode::$val);
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

    map.insert("Backspace".to_string(), KeyCode::Char('\u{8}'));
    map.insert("Delete".to_string(), KeyCode::Char('\u{7f}'));
    map.insert("Enter".to_string(), KeyCode::Char('\r'));
    map.insert("Escape".to_string(), KeyCode::Char('\u{1b}'));
    map.insert("Tab".to_string(), KeyCode::Char('\t'));

    for i in 0..=9 {
        map.insert(format!("Numpad{}", i), KeyCode::Numpad(i));
    }

    for i in 1..=24 {
        map.insert(format!("F{}", i), KeyCode::Function(i));
    }

    map
}

lazy_static::lazy_static! {
    static ref KEYCODE_MAP: HashMap<String, KeyCode> = make_map();
}

fn de_keycode<'de, D>(deserializer: D) -> Result<KeyCode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if let Some(c) = KEYCODE_MAP.get(&s) {
        return Ok(c.clone());
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

    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        Ok(KeyCode::Char(chars[0]))
    } else {
        Err(serde::de::Error::custom(format!(
            "invalid KeyCode string {}",
            s
        )))
    }
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
