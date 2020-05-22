use crate::config::configuration;
use crate::frontend::gui::SelectionMode;
use crate::mux::domain::DomainId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use term::input::MouseButton;
use term::{KeyCode, KeyModifiers};

/// A mouse event that can trigger an action
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum MouseEventTrigger {
    /// Mouse button is pressed. streak is how many times in a row
    /// it was pressed.
    Down { streak: usize, button: MouseButton },
    /// Mouse button is held down while the cursor is moving. streak is how many times in a row
    /// it was pressed, with the last of those being held to form the drag.
    Drag { streak: usize, button: MouseButton },
    /// Mouse button is being released. streak is how many times
    /// in a row it was pressed and released.
    Up { streak: usize, button: MouseButton },
}

/// When spawning a tab, specify which domain should be used to
/// host/spawn that tab.
#[derive(Debug, Clone, Deserialize, Serialize)]
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

impl Default for SpawnTabDomain {
    fn default() -> Self {
        Self::CurrentTabDomain
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct SpawnCommand {
    /// Optional descriptive label
    pub label: Option<String>,

    /// The command line to use.
    /// If omitted, the default command associated with the
    /// domain will be used instead, which is typically the
    /// shell for the user.
    pub args: Option<Vec<String>>,

    /// Specifies the current working directory for the command.
    /// If omitted, a default will be used; typically that will
    /// be the home directory of the user, but may also be the
    /// current working directory of the wezterm process when
    /// it was launched, or for some domains it may be some
    /// other location appropriate to the domain.
    pub cwd: Option<PathBuf>,

    /// Specifies a map of environment variables that should be set.
    /// Whether this is used depends on the domain.
    #[serde(default)]
    pub set_environment_variables: HashMap<String, String>,

    #[serde(default)]
    pub domain: SpawnTabDomain,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum KeyAssignment {
    SpawnTab(SpawnTabDomain),
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    Paste,
    PastePrimarySelection,
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
    ReloadConfiguration,
    MoveTabRelative(isize),
    MoveTab(usize),
    ScrollByPage(isize),
    ShowTabNavigator,
    HideApplication,
    QuitApplication,
    SpawnCommandInNewTab(SpawnCommand),
    SpawnCommandInNewWindow(SpawnCommand),
    ShowLauncher,

    SelectTextAtMouseCursor(SelectionMode),
    ExtendSelectionToMouseCursor(Option<SelectionMode>),
    OpenLinkAtMouseCursor,
    CompleteSelection,
    CompleteSelectionOrOpenLinkAtMouseCursor,
}
impl_lua_conversion!(KeyAssignment);

pub struct InputMap {
    keys: HashMap<(KeyCode, KeyModifiers), KeyAssignment>,
    mouse: HashMap<(MouseEventTrigger, KeyModifiers), KeyAssignment>,
}

impl InputMap {
    pub fn new() -> Self {
        let mut mouse = HashMap::new();

        let mut keys = configuration()
            .key_bindings()
            .expect("keys section of config to be valid");

        macro_rules! k {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                keys.entry(($code, $mod)).or_insert($action);
                )*
            };
        };
        macro_rules! m {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                mouse.entry(($code, $mod)).or_insert($action);
                )*
            };
        };

        use KeyAssignment::*;

        let ctrl_shift = KeyModifiers::CTRL | KeyModifiers::SHIFT;

        // Apply the default bindings; if the user has already mapped
        // a given entry then that will take precedence.
        k!(
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
                SpawnTab(SpawnTabDomain::CurrentTabDomain)
            ],
            [
                ctrl_shift,
                KeyCode::Char('T'),
                SpawnTab(SpawnTabDomain::CurrentTabDomain)
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
            [KeyModifiers::SUPER, KeyCode::Char('r'), ReloadConfiguration],
            [ctrl_shift, KeyCode::Char('R'), ReloadConfiguration],
            [ctrl_shift, KeyCode::PageUp, MoveTabRelative(-1)],
            [ctrl_shift, KeyCode::PageDown, MoveTabRelative(1)],
            [KeyModifiers::SHIFT, KeyCode::PageUp, ScrollByPage(-1)],
            [KeyModifiers::SHIFT, KeyCode::PageDown, ScrollByPage(1)],
            [KeyModifiers::ALT, KeyCode::Char('9'), ShowTabNavigator],
        );

        #[cfg(target_os = "macos")]
        k!([KeyModifiers::SUPER, KeyCode::Char('h'), HideApplication],);

        m!(
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Down {
                    streak: 3,
                    button: MouseButton::Left
                },
                SelectTextAtMouseCursor(SelectionMode::Line)
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Down {
                    streak: 2,
                    button: MouseButton::Left
                },
                SelectTextAtMouseCursor(SelectionMode::Word)
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Down {
                    streak: 1,
                    button: MouseButton::Left
                },
                SelectTextAtMouseCursor(SelectionMode::Cell)
            ],
            [
                KeyModifiers::SHIFT,
                MouseEventTrigger::Down {
                    streak: 1,
                    button: MouseButton::Left
                },
                ExtendSelectionToMouseCursor(None)
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Up {
                    streak: 1,
                    button: MouseButton::Left
                },
                CompleteSelectionOrOpenLinkAtMouseCursor
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Up {
                    streak: 2,
                    button: MouseButton::Left
                },
                CompleteSelection
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Up {
                    streak: 3,
                    button: MouseButton::Left
                },
                CompleteSelection
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Drag {
                    streak: 1,
                    button: MouseButton::Left
                },
                ExtendSelectionToMouseCursor(Some(SelectionMode::Cell))
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Drag {
                    streak: 2,
                    button: MouseButton::Left
                },
                ExtendSelectionToMouseCursor(Some(SelectionMode::Word))
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Drag {
                    streak: 3,
                    button: MouseButton::Left
                },
                ExtendSelectionToMouseCursor(Some(SelectionMode::Line))
            ],
            [
                KeyModifiers::NONE,
                MouseEventTrigger::Down {
                    streak: 1,
                    button: MouseButton::Middle
                },
                Paste
            ],
        );

        Self { keys, mouse }
    }

    pub fn lookup_key(&self, key: KeyCode, mods: KeyModifiers) -> Option<KeyAssignment> {
        self.keys
            .get(&(key.normalize_shift_to_upper_case(mods), mods))
            .cloned()
    }

    pub fn lookup_mouse(
        &self,
        event: MouseEventTrigger,
        mods: KeyModifiers,
    ) -> Option<KeyAssignment> {
        self.mouse.get(&(event, mods)).cloned()
    }
}
