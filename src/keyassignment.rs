use crate::mux::domain::DomainId;
use crate::mux::Mux;
use std::collections::HashMap;
use term::{KeyCode, KeyModifiers};

/// When spawning a tab, specify which domain should be used to
/// host/spawn that tab.
#[derive(Debug, Clone)]
pub enum SpawnTabDomain {
    /// Use the default domain
    DefaultDomain,
    /// Use the domain from the current tab in the associated window
    CurrentTabDomain,
    /// Use a specific domain by id
    Domain(DomainId),
    /// Use a specific domain by name
    DomainName(String),
}

#[derive(Debug, Clone)]
pub enum KeyAssignment {
    SpawnTab(SpawnTabDomain),
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    Paste,
    ActivateTabRelative(isize),
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ActivateTab(usize),
    SendString(String),
    Nop,
    Hide,
    Show,
    CloseCurrentTab,
}

pub struct KeyMap(HashMap<(KeyCode, KeyModifiers), KeyAssignment>);

impl KeyMap {
    pub fn new() -> Self {
        let mux = Mux::get().unwrap();
        let mut map = mux
            .config()
            .key_bindings()
            .expect("keys section of config to be valid");

        macro_rules! m {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                map.entry(($code, $mod)).or_insert($action);
                )*
            };
        };

        use KeyAssignment::*;

        // Apply the default bindings; if the user has already mapped
        // a given entry then that will take precedence.
        m!(
            // Clipboard
            [KeyModifiers::SUPER, KeyCode::Char('c'), Copy],
            [KeyModifiers::SUPER, KeyCode::Char('v'), Paste],
            [KeyModifiers::SHIFT, KeyCode::Insert, Paste],
            // Window management
            [KeyModifiers::SUPER, KeyCode::Char('m'), Hide],
            [KeyModifiers::SUPER, KeyCode::Char('n'), SpawnWindow],
            [KeyModifiers::ALT, KeyCode::Char('\n'), ToggleFullScreen],
            [KeyModifiers::ALT, KeyCode::Char('\r'), ToggleFullScreen],
            [KeyModifiers::ALT, KeyCode::Enter, ToggleFullScreen],
            // Font size manipulation
            [KeyModifiers::SUPER, KeyCode::Char('-'), DecreaseFontSize],
            [KeyModifiers::CTRL, KeyCode::Char('-'), DecreaseFontSize],
            [KeyModifiers::SUPER, KeyCode::Char('='), IncreaseFontSize],
            [KeyModifiers::CTRL, KeyCode::Char('='), IncreaseFontSize],
            [KeyModifiers::SUPER, KeyCode::Char('0'), ResetFontSize],
            [KeyModifiers::CTRL, KeyCode::Char('0'), ResetFontSize],
            // Tab navigation and management
            [
                KeyModifiers::SUPER,
                KeyCode::Char('t'),
                SpawnTab(SpawnTabDomain::DefaultDomain)
            ],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char('T'),
                SpawnTab(SpawnTabDomain::CurrentTabDomain)
            ],
            [KeyModifiers::SUPER, KeyCode::Char('w'), CloseCurrentTab],
            [KeyModifiers::SUPER, KeyCode::Char('1'), ActivateTab(0)],
            [KeyModifiers::SUPER, KeyCode::Char('2'), ActivateTab(1)],
            [KeyModifiers::SUPER, KeyCode::Char('3'), ActivateTab(2)],
            [KeyModifiers::SUPER, KeyCode::Char('4'), ActivateTab(3)],
            [KeyModifiers::SUPER, KeyCode::Char('5'), ActivateTab(4)],
            [KeyModifiers::SUPER, KeyCode::Char('6'), ActivateTab(5)],
            [KeyModifiers::SUPER, KeyCode::Char('7'), ActivateTab(6)],
            [KeyModifiers::SUPER, KeyCode::Char('8'), ActivateTab(7)],
            [KeyModifiers::SUPER, KeyCode::Char('9'), ActivateTab(8)],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char('['),
                ActivateTabRelative(-1)
            ],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char('{'),
                ActivateTabRelative(-1)
            ],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char(']'),
                ActivateTabRelative(1)
            ],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char('}'),
                ActivateTabRelative(1)
            ],
        );

        Self(map)
    }

    pub fn lookup(&self, key: KeyCode, mods: KeyModifiers) -> Option<KeyAssignment> {
        self.0.get(&(key, mods)).cloned()
    }
}
