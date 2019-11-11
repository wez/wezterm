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

        let ctrl_shift = KeyModifiers::CTRL | KeyModifiers::SHIFT;

        // Apply the default bindings; if the user has already mapped
        // a given entry then that will take precedence.
        m!(
            // Clipboard
            [KeyModifiers::SHIFT, KeyCode::Insert, Paste],
            [KeyModifiers::SUPER, KeyCode::Char('c'), Copy],
            [KeyModifiers::SUPER, KeyCode::Char('v'), Paste],
            [ctrl_shift, KeyCode::Char('C'), Copy],
            [ctrl_shift, KeyCode::Char('V'), Paste],
            // Window management
            [KeyModifiers::ALT, KeyCode::Char('\n'), ToggleFullScreen],
            [KeyModifiers::ALT, KeyCode::Char('\r'), ToggleFullScreen],
            [KeyModifiers::ALT, KeyCode::Enter, ToggleFullScreen],
            [KeyModifiers::SUPER, KeyCode::Char('m'), Hide],
            [KeyModifiers::SUPER, KeyCode::Char('n'), SpawnWindow],
            [ctrl_shift, KeyCode::Char('M'), Hide],
            [ctrl_shift, KeyCode::Char('N'), SpawnWindow],
            // Font size manipulation
            [KeyModifiers::CTRL, KeyCode::Char('-'), DecreaseFontSize],
            [KeyModifiers::CTRL, KeyCode::Char('0'), ResetFontSize],
            [KeyModifiers::CTRL, KeyCode::Char('='), IncreaseFontSize],
            [KeyModifiers::SUPER, KeyCode::Char('-'), DecreaseFontSize],
            [KeyModifiers::SUPER, KeyCode::Char('0'), ResetFontSize],
            [KeyModifiers::SUPER, KeyCode::Char('='), IncreaseFontSize],
            // Tab navigation and management
            [
                KeyModifiers::SUPER,
                KeyCode::Char('t'),
                SpawnTab(SpawnTabDomain::DefaultDomain)
            ],
            [
                ctrl_shift,
                KeyCode::Char('T'),
                SpawnTab(SpawnTabDomain::DefaultDomain)
            ],
            [
                KeyModifiers::SUPER | KeyModifiers::SHIFT,
                KeyCode::Char('T'),
                SpawnTab(SpawnTabDomain::CurrentTabDomain)
            ],
            [KeyModifiers::SUPER, KeyCode::Char('1'), ActivateTab(0)],
            [KeyModifiers::SUPER, KeyCode::Char('2'), ActivateTab(1)],
            [KeyModifiers::SUPER, KeyCode::Char('3'), ActivateTab(2)],
            [KeyModifiers::SUPER, KeyCode::Char('4'), ActivateTab(3)],
            [KeyModifiers::SUPER, KeyCode::Char('5'), ActivateTab(4)],
            [KeyModifiers::SUPER, KeyCode::Char('6'), ActivateTab(5)],
            [KeyModifiers::SUPER, KeyCode::Char('7'), ActivateTab(6)],
            [KeyModifiers::SUPER, KeyCode::Char('8'), ActivateTab(7)],
            [KeyModifiers::SUPER, KeyCode::Char('9'), ActivateTab(8)],
            [KeyModifiers::SUPER, KeyCode::Char('w'), CloseCurrentTab],
            [ctrl_shift, KeyCode::Char('1'), ActivateTab(0)],
            [ctrl_shift, KeyCode::Char('2'), ActivateTab(1)],
            [ctrl_shift, KeyCode::Char('3'), ActivateTab(2)],
            [ctrl_shift, KeyCode::Char('4'), ActivateTab(3)],
            [ctrl_shift, KeyCode::Char('5'), ActivateTab(4)],
            [ctrl_shift, KeyCode::Char('6'), ActivateTab(5)],
            [ctrl_shift, KeyCode::Char('7'), ActivateTab(6)],
            [ctrl_shift, KeyCode::Char('8'), ActivateTab(7)],
            [ctrl_shift, KeyCode::Char('9'), ActivateTab(8)],
            [ctrl_shift, KeyCode::Char('W'), CloseCurrentTab],
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
        self.0
            .get(&(key.normalize_shift_to_upper_case(mods), mods))
            .cloned()
    }
}
