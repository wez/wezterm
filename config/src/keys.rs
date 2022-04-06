use crate::keyassignment::{KeyAssignment, MouseEventTrigger};
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

impl Into<String> for DeferredKeyCode {
    fn into(self) -> String {
        match self {
            DeferredKeyCode::KeyCode(key) => key.to_string(),
            DeferredKeyCode::Either { original, .. } => original.to_string(),
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
