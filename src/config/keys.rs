use crate::keyassignment::{KeyAssignment, SpawnTabDomain};
use anyhow::{anyhow, Error};
use serde::{Deserialize, Deserializer};
use termwiz::input::{KeyCode, Modifiers};

#[derive(Debug, Deserialize, Clone)]
pub struct Key {
    #[serde(deserialize_with = "de_keycode")]
    pub key: KeyCode,
    #[serde(deserialize_with = "de_modifiers")]
    pub mods: Modifiers,
    pub action: KeyAction,
    pub arg: Option<String>,
}

impl std::convert::TryInto<KeyAssignment> for &Key {
    type Error = Error;
    fn try_into(self) -> Result<KeyAssignment, Error> {
        Ok(match self.action {
            KeyAction::SpawnTab => KeyAssignment::SpawnTab(SpawnTabDomain::DefaultDomain),
            KeyAction::SpawnTabInCurrentTabDomain => {
                KeyAssignment::SpawnTab(SpawnTabDomain::CurrentTabDomain)
            }
            KeyAction::SpawnTabInDomain => {
                let arg = self
                    .arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?;

                if let Ok(id) = arg.parse() {
                    KeyAssignment::SpawnTab(SpawnTabDomain::Domain(id))
                } else {
                    KeyAssignment::SpawnTab(SpawnTabDomain::DomainName(arg.to_string()))
                }
            }
            KeyAction::SpawnWindow => KeyAssignment::SpawnWindow,
            KeyAction::ToggleFullScreen => KeyAssignment::ToggleFullScreen,
            KeyAction::Copy => KeyAssignment::Copy,
            KeyAction::Paste => KeyAssignment::Paste,
            KeyAction::Hide => KeyAssignment::Hide,
            KeyAction::Show => KeyAssignment::Show,
            KeyAction::IncreaseFontSize => KeyAssignment::IncreaseFontSize,
            KeyAction::DecreaseFontSize => KeyAssignment::DecreaseFontSize,
            KeyAction::ResetFontSize => KeyAssignment::ResetFontSize,
            KeyAction::Nop => KeyAssignment::Nop,
            KeyAction::CloseCurrentTab => KeyAssignment::CloseCurrentTab,
            KeyAction::ActivateTab => KeyAssignment::ActivateTab(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::ActivateTabRelative => KeyAssignment::ActivateTabRelative(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::SendString => KeyAssignment::SendString(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .to_owned(),
            ),
            KeyAction::ReloadConfiguration => KeyAssignment::ReloadConfiguration,
            KeyAction::MoveTab => KeyAssignment::MoveTab(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::MoveTabRelative => KeyAssignment::MoveTabRelative(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::ScrollByPage => KeyAssignment::ScrollByPage(
                self.arg
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing arg for {:?}", self))?
                    .parse()?,
            ),
            KeyAction::ShowTabNavigator => KeyAssignment::ShowTabNavigator,
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum KeyAction {
    SpawnTab,
    SpawnTabInCurrentTabDomain,
    SpawnTabInDomain,
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    Paste,
    ActivateTabRelative,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ActivateTab,
    SendString,
    Nop,
    Hide,
    Show,
    CloseCurrentTab,
    ReloadConfiguration,
    MoveTab,
    MoveTabRelative,
    ScrollByPage,
    ShowTabNavigator,
}

fn de_keycode<'de, D>(deserializer: D) -> Result<KeyCode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    macro_rules! m {
        ($($val:ident),* $(,)?) => {
            $(
            if s == stringify!($val) {
                return Ok(KeyCode::$val);
            }
            )*
        }
    }

    m!(
        Hyper,
        Super,
        Meta,
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

    if s.len() > 1 && s.starts_with('F') {
        let num: u8 = s[1..].parse().map_err(|_| {
            serde::de::Error::custom(format!(
                "expected F<NUMBER> function key string, got: {}",
                s
            ))
        })?;
        return Ok(KeyCode::Function(num));
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

fn de_modifiers<'de, D>(deserializer: D) -> Result<Modifiers, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let mut mods = Modifiers::NONE;
    for ele in s.split('|') {
        if ele == "SHIFT" {
            mods |= Modifiers::SHIFT;
        } else if ele == "ALT" || ele == "OPT" || ele == "META" {
            mods |= Modifiers::ALT;
        } else if ele == "CTRL" {
            mods |= Modifiers::CTRL;
        } else if ele == "SUPER" || ele == "CMD" || ele == "WIN" {
            mods |= Modifiers::SUPER;
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
