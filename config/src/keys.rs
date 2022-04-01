use crate::keyassignment::{KeyAssignment, MouseEventTrigger};
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::convert::TryFrom;
use wezterm_input_types::{KeyCode, Modifiers, PhysKeyCode};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum KeyMapPreference {
    Physical,
    Mapped,
}
impl_lua_conversion!(KeyMapPreference);

impl Default for KeyMapPreference {
    fn default() -> Self {
        Self::Mapped
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum DeferredKeyCode {
    KeyCode(KeyCode),
    Either { physical: KeyCode, mapped: KeyCode },
}

impl DeferredKeyCode {
    pub fn resolve(&self, position: KeyMapPreference) -> &KeyCode {
        match (self, position) {
            (Self::KeyCode(key), _) => key,
            (Self::Either { mapped, .. }, KeyMapPreference::Mapped) => mapped,
            (Self::Either { physical, .. }, KeyMapPreference::Physical) => physical,
        }
    }

    fn as_string(key: &KeyCode) -> String {
        if let Some(s) = INV_KEYCODE_MAP.get(key) {
            s.to_string()
        } else {
            key.to_string()
        }
    }

    fn parse_str(s: &str) -> anyhow::Result<KeyCode> {
        if let Some(c) = KEYCODE_MAP.get(s) {
            return Ok(c.clone());
        }

        if let Some(phys) = s.strip_prefix("phys:") {
            let phys = PhysKeyCode::try_from(phys).map_err(|_| {
                anyhow::anyhow!("expected phys:CODE physical keycode string, got: {}", s)
            })?;
            return Ok(KeyCode::Physical(phys));
        }

        if let Some(raw) = s.strip_prefix("raw:") {
            let num: u32 = raw.parse().map_err(|_| {
                anyhow::anyhow!("expected raw:<NUMBER> raw keycode string, got: {}", s)
            })?;
            return Ok(KeyCode::RawCode(num));
        }

        if let Some(mapped) = s.strip_prefix("mapped:") {
            let chars: Vec<char> = mapped.chars().collect();
            return if chars.len() == 1 {
                Ok(KeyCode::Char(chars[0]))
            } else {
                anyhow::bail!("invalid KeyCode string {}", s);
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
            anyhow::bail!("invalid KeyCode string {}", s);
        }
    }
}

impl Into<String> for DeferredKeyCode {
    fn into(self) -> String {
        match self {
            DeferredKeyCode::KeyCode(key) => Self::as_string(&key),
            DeferredKeyCode::Either { mapped, .. } => {
                let mapped = Self::as_string(&mapped);
                mapped
                    .strip_prefix("mapped:")
                    .expect("to have mapped: prefix")
                    .to_string()
            }
        }
    }
}

impl TryFrom<String> for DeferredKeyCode {
    type Error = anyhow::Error;
    fn try_from(s: String) -> anyhow::Result<DeferredKeyCode> {
        DeferredKeyCode::try_from(s.as_str())
    }
}

impl TryFrom<&str> for DeferredKeyCode {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> anyhow::Result<DeferredKeyCode> {
        if s.starts_with("mapped:") || s.starts_with("phys:") {
            let key = Self::parse_str(&s)?;
            return Ok(DeferredKeyCode::KeyCode(key));
        }

        let mapped = Self::parse_str(&format!("mapped:{}", s));
        let phys = Self::parse_str(&format!("phys:{}", s));

        match (mapped, phys) {
            (Ok(mapped), Ok(physical)) => Ok(DeferredKeyCode::Either { mapped, physical }),
            (Ok(mapped), Err(_)) => Ok(DeferredKeyCode::KeyCode(mapped)),
            (Err(_), Ok(phys)) => Ok(DeferredKeyCode::KeyCode(phys)),
            (Err(a), Err(b)) => anyhow::bail!("invalid keycode {}: {:#}, {:#}", s, a, b),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KeyNoAction {
    pub key: DeferredKeyCode,
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
    #[serde(flatten)]
    pub key: KeyNoAction,
    pub action: KeyAssignment,
}
impl_lua_conversion!(Key);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LeaderKey {
    #[serde(flatten)]
    pub key: KeyNoAction,
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
                    map.insert(format!("phys:{}", stringify!($val)), KeyCode::Physical(phys));
                    map.insert(format!("mapped:{}", stringify!($val)), v);
                } else {
                    map.insert(format!("mapped:{}", stringify!($val)), v);
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

    for (label, phys) in &[
        ("Backspace", PhysKeyCode::Backspace),
        ("Delete", PhysKeyCode::Delete),
        ("Enter", PhysKeyCode::Return),
        ("Escape", PhysKeyCode::Escape),
        ("Tab", PhysKeyCode::Tab),
    ] {
        map.insert(format!("phys:{}", label), KeyCode::Physical(*phys));
        map.insert(format!("mapped:{}", label), phys.to_key_code());
    }

    for i in 0..=9 {
        let k = KeyCode::Numpad(i);
        map.insert(
            format!("phys:Numpad{}", i),
            KeyCode::Physical(k.to_phys().unwrap()),
        );
        // Not sure how likely someone is to remap the numpad, but...
        map.insert(format!("mapped:Numpad{}", i), k);
    }

    for i in 1..=24 {
        let k = KeyCode::Function(i);
        if let Some(phys) = k.to_phys() {
            map.insert(format!("phys:F{}", i), KeyCode::Physical(phys));
            map.insert(format!("mapped:F{}", i), k);
        } else {
            // 21 and up don't have phys equivalents
            map.insert(format!("mapped:F{}", i), k);
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
