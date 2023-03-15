use crate::keyassignment::{KeyAssignment, MouseEventTrigger};
use std::convert::TryFrom;
use wezterm_dynamic::{Error as DynError, FromDynamic, FromDynamicOptions, ToDynamic, Value};
use wezterm_input_types::{KeyCode, Modifiers, PhysKeyCode};

#[derive(Debug, Clone, Copy, Eq, PartialEq, FromDynamic, ToDynamic)]
pub enum KeyMapPreference {
    Physical,
    Mapped,
}

impl Default for KeyMapPreference {
    fn default() -> Self {
        Self::Mapped
    }
}

#[derive(Debug, Clone, Eq, PartialEq, FromDynamic, ToDynamic)]
#[dynamic(into = "String", try_from = "String")]
pub enum DeferredKeyCode {
    KeyCode(KeyCode),
    Either {
        physical: KeyCode,
        mapped: KeyCode,
        original: String,
    },
}

impl DeferredKeyCode {
    pub fn resolve(&self, position: KeyMapPreference) -> KeyCode {
        match (self, position) {
            (Self::KeyCode(key), KeyMapPreference::Physical) => match key.to_phys() {
                Some(p) => KeyCode::Physical(p),
                None => key.clone(),
            },
            (Self::KeyCode(key), _) => key.clone(),
            (Self::Either { mapped, .. }, KeyMapPreference::Mapped) => mapped.clone(),
            (Self::Either { physical, .. }, KeyMapPreference::Physical) => physical.clone(),
        }
    }

    fn parse_str(s: &str) -> anyhow::Result<KeyCode> {
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
            return KeyCode::try_from(mapped).map_err(|err| anyhow::anyhow!("{}", err));
        }

        KeyCode::try_from(s).map_err(|err| anyhow::anyhow!("{}", err))
    }
}

impl From<&DeferredKeyCode> for String {
    fn from(val: &DeferredKeyCode) -> Self {
        match val {
            DeferredKeyCode::KeyCode(key) => key.to_string(),
            DeferredKeyCode::Either { original, .. } => original.to_string(),
        }
    }
}

impl From<DeferredKeyCode> for String {
    fn from(val: DeferredKeyCode) -> Self {
        match val {
            DeferredKeyCode::KeyCode(key) => key.to_string(),
            DeferredKeyCode::Either { original, .. } => original,
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
        if s.starts_with("mapped:") || s.starts_with("phys:") || s.starts_with("raw:") {
            let key = Self::parse_str(&s)?;
            return Ok(DeferredKeyCode::KeyCode(key));
        }

        let mapped = Self::parse_str(&format!("mapped:{}", s));
        let phys = Self::parse_str(&format!("phys:{}", s));

        match (mapped, phys) {
            (Ok(mapped), Ok(physical)) => Ok(DeferredKeyCode::Either {
                mapped,
                physical,
                original: s.to_string(),
            }),
            (Ok(mapped), Err(_)) => Ok(DeferredKeyCode::KeyCode(mapped)),
            (Err(_), Ok(phys)) => Ok(DeferredKeyCode::KeyCode(phys)),
            (Err(a), Err(b)) => anyhow::bail!("invalid keycode {}: {:#}, {:#}", s, a, b),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromDynamic, ToDynamic)]
pub struct KeyNoAction {
    pub key: DeferredKeyCode,
    #[dynamic(default)]
    pub mods: Modifiers,
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct Key {
    #[dynamic(flatten)]
    pub key: KeyNoAction,
    pub action: KeyAssignment,
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct LeaderKey {
    #[dynamic(flatten)]
    pub key: KeyNoAction,
    #[dynamic(default = "default_leader_timeout")]
    pub timeout_milliseconds: u64,
}

fn default_leader_timeout() -> u64 {
    1000
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct Mouse {
    pub event: MouseEventTrigger,
    #[dynamic(flatten)]
    pub mods: MouseEventTriggerMods,
    pub action: KeyAssignment,
}

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum MouseEventAltScreen {
    True,
    False,
    Any,
}

impl FromDynamic for MouseEventAltScreen {
    fn from_dynamic(value: &Value, _options: FromDynamicOptions) -> Result<Self, DynError> {
        match value {
            Value::Bool(true) => Ok(Self::True),
            Value::Bool(false) => Ok(Self::False),
            Value::String(s) if s == "Any" => Ok(Self::Any),
            _ => Err(DynError::Message(
                "must be either true, false or 'Any'".to_string(),
            )),
        }
    }
}

impl ToDynamic for MouseEventAltScreen {
    fn to_dynamic(&self) -> Value {
        match self {
            Self::True => true.to_dynamic(),
            Self::False => false.to_dynamic(),
            Self::Any => "Any".to_dynamic(),
        }
    }
}

impl Default for MouseEventAltScreen {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, FromDynamic, ToDynamic,
)]
pub struct MouseEventTriggerMods {
    #[dynamic(default, into = "String", try_from = "String")]
    pub mods: Modifiers,
    #[dynamic(default)]
    pub mouse_reporting: bool,
    #[dynamic(default)]
    pub alt_screen: MouseEventAltScreen,
}
