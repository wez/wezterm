use crate::configuration;
use crate::LeaderKey;
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use wezterm_input_types::{KeyCode, Modifiers};
use wezterm_term::input::MouseButton;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
pub enum SelectionMode {
    Cell,
    Word,
    Line,
    SemanticZone,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Pattern {
    CaseSensitiveString(String),
    CaseInSensitiveString(String),
    Regex(String),
}

impl std::ops::Deref for Pattern {
    type Target = String;
    fn deref(&self) -> &String {
        match self {
            Pattern::CaseSensitiveString(s) => s,
            Pattern::CaseInSensitiveString(s) => s,
            Pattern::Regex(s) => s,
        }
    }
}

impl std::ops::DerefMut for Pattern {
    fn deref_mut(&mut self) -> &mut String {
        match self {
            Pattern::CaseSensitiveString(s) => s,
            Pattern::CaseInSensitiveString(s) => s,
            Pattern::Regex(s) => s,
        }
    }
}

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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SpawnTabDomain {
    /// Use the default domain
    DefaultDomain,
    /// Use the domain from the current tab in the associated window
    CurrentPaneDomain,
    /// Use a specific domain by name
    DomainName(String),
}

impl Default for SpawnTabDomain {
    fn default() -> Self {
        Self::CurrentPaneDomain
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum PaneDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum ScrollbackEraseMode {
    ScrollbackOnly,
    ScrollbackAndViewport,
}

impl Default for ScrollbackEraseMode {
    fn default() -> Self {
        Self::ScrollbackOnly
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum ClipboardCopyDestination {
    Clipboard,
    PrimarySelection,
    ClipboardAndPrimarySelection,
}

impl Default for ClipboardCopyDestination {
    fn default() -> Self {
        Self::ClipboardAndPrimarySelection
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum ClipboardPasteSource {
    Clipboard,
    PrimarySelection,
}

impl Default for ClipboardPasteSource {
    fn default() -> Self {
        Self::Clipboard
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum KeyAssignment {
    SpawnTab(SpawnTabDomain),
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    CopyTo(ClipboardCopyDestination),
    Paste,
    PastePrimarySelection,
    PasteFrom(ClipboardPasteSource),
    ActivateTabRelative(isize),
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ResetFontAndWindowSize,
    ActivateTab(isize),
    SendString(String),
    Nop,
    DisableDefaultAssignment,
    Hide,
    Show,
    CloseCurrentTab { confirm: bool },
    ReloadConfiguration,
    MoveTabRelative(isize),
    MoveTab(usize),
    ScrollByPage(isize),
    ScrollByLine(isize),
    ScrollToPrompt(isize),
    ShowTabNavigator,
    HideApplication,
    QuitApplication,
    SpawnCommandInNewTab(SpawnCommand),
    SpawnCommandInNewWindow(SpawnCommand),
    SplitHorizontal(SpawnCommand),
    SplitVertical(SpawnCommand),
    ShowLauncher,
    ClearScrollback(ScrollbackEraseMode),
    Search(Pattern),
    ActivateCopyMode,

    SelectTextAtMouseCursor(SelectionMode),
    ExtendSelectionToMouseCursor(Option<SelectionMode>),
    OpenLinkAtMouseCursor,
    CompleteSelection(ClipboardCopyDestination),
    CompleteSelectionOrOpenLinkAtMouseCursor(ClipboardCopyDestination),
    StartWindowDrag,

    AdjustPaneSize(PaneDirection, usize),
    ActivatePaneDirection(PaneDirection),
    TogglePaneZoomState,
    CloseCurrentPane { confirm: bool },
    EmitEvent(String),
}
impl_lua_conversion!(KeyAssignment);

pub struct InputMap {
    keys: HashMap<(KeyCode, Modifiers), KeyAssignment>,
    mouse: HashMap<(MouseEventTrigger, Modifiers), KeyAssignment>,
    leader: Option<LeaderKey>,
}

impl InputMap {
    pub fn new() -> Self {
        let config = configuration();
        let mut mouse = config.mouse_bindings();

        let mut keys = config.key_bindings();

        let leader = config.leader.clone();

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

        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;

        if !config.disable_default_key_bindings {
            // Apply the default bindings; if the user has already mapped
            // a given entry then that will take precedence.
            k!(
                // Clipboard
                [
                    Modifiers::SHIFT,
                    KeyCode::Insert,
                    PasteFrom(ClipboardPasteSource::PrimarySelection)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Insert,
                    CopyTo(ClipboardCopyDestination::PrimarySelection)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('c'),
                    CopyTo(ClipboardCopyDestination::Clipboard)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('v'),
                    PasteFrom(ClipboardPasteSource::Clipboard)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('C'),
                    CopyTo(ClipboardCopyDestination::Clipboard)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('V'),
                    PasteFrom(ClipboardPasteSource::Clipboard)
                ],
                // Window management
                [Modifiers::ALT, KeyCode::Char('\n'), ToggleFullScreen],
                [Modifiers::ALT, KeyCode::Char('\r'), ToggleFullScreen],
                [Modifiers::SUPER, KeyCode::Char('m'), Hide],
                [Modifiers::SUPER, KeyCode::Char('n'), SpawnWindow],
                [Modifiers::CTRL, KeyCode::Char('M'), Hide],
                [Modifiers::CTRL, KeyCode::Char('N'), SpawnWindow],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('k'),
                    ClearScrollback(ScrollbackEraseMode::ScrollbackOnly)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('K'),
                    ClearScrollback(ScrollbackEraseMode::ScrollbackOnly)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('f'),
                    Search(Pattern::CaseSensitiveString("".into()))
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('F'),
                    Search(Pattern::CaseSensitiveString("".into()))
                ],
                // Font size manipulation
                [Modifiers::CTRL, KeyCode::Char('-'), DecreaseFontSize],
                [Modifiers::CTRL, KeyCode::Char('0'), ResetFontSize],
                [Modifiers::CTRL, KeyCode::Char('='), IncreaseFontSize],
                [Modifiers::SUPER, KeyCode::Char('-'), DecreaseFontSize],
                [Modifiers::SUPER, KeyCode::Char('0'), ResetFontSize],
                [Modifiers::SUPER, KeyCode::Char('='), IncreaseFontSize],
                // Tab navigation and management
                [
                    Modifiers::SUPER,
                    KeyCode::Char('t'),
                    SpawnTab(SpawnTabDomain::CurrentPaneDomain)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('T'),
                    SpawnTab(SpawnTabDomain::CurrentPaneDomain)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('T'),
                    SpawnTab(SpawnTabDomain::CurrentPaneDomain)
                ],
                [Modifiers::SUPER, KeyCode::Char('1'), ActivateTab(0)],
                [Modifiers::SUPER, KeyCode::Char('2'), ActivateTab(1)],
                [Modifiers::SUPER, KeyCode::Char('3'), ActivateTab(2)],
                [Modifiers::SUPER, KeyCode::Char('4'), ActivateTab(3)],
                [Modifiers::SUPER, KeyCode::Char('5'), ActivateTab(4)],
                [Modifiers::SUPER, KeyCode::Char('6'), ActivateTab(5)],
                [Modifiers::SUPER, KeyCode::Char('7'), ActivateTab(6)],
                [Modifiers::SUPER, KeyCode::Char('8'), ActivateTab(7)],
                [Modifiers::SUPER, KeyCode::Char('9'), ActivateTab(-1)],
                [
                    Modifiers::SUPER,
                    KeyCode::Char('w'),
                    CloseCurrentTab { confirm: true }
                ],
                [ctrl_shift, KeyCode::Char('1'), ActivateTab(0)],
                [ctrl_shift, KeyCode::Char('2'), ActivateTab(1)],
                [ctrl_shift, KeyCode::Char('3'), ActivateTab(2)],
                [ctrl_shift, KeyCode::Char('4'), ActivateTab(3)],
                [ctrl_shift, KeyCode::Char('5'), ActivateTab(4)],
                [ctrl_shift, KeyCode::Char('6'), ActivateTab(5)],
                [ctrl_shift, KeyCode::Char('7'), ActivateTab(6)],
                [ctrl_shift, KeyCode::Char('8'), ActivateTab(7)],
                [ctrl_shift, KeyCode::Char('9'), ActivateTab(-1)],
                [
                    Modifiers::CTRL,
                    KeyCode::Char('W'),
                    CloseCurrentTab { confirm: true }
                ],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Char('['),
                    ActivateTabRelative(-1)
                ],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Char('{'),
                    ActivateTabRelative(-1)
                ],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Char(']'),
                    ActivateTabRelative(1)
                ],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Char('}'),
                    ActivateTabRelative(1)
                ],
                [Modifiers::SUPER, KeyCode::Char('r'), ReloadConfiguration],
                [Modifiers::CTRL, KeyCode::Char('R'), ReloadConfiguration],
                [ctrl_shift, KeyCode::PageUp, MoveTabRelative(-1)],
                [ctrl_shift, KeyCode::PageDown, MoveTabRelative(1)],
                [Modifiers::SHIFT, KeyCode::PageUp, ScrollByPage(-1)],
                [Modifiers::SHIFT, KeyCode::PageDown, ScrollByPage(1)],
                [Modifiers::ALT, KeyCode::Char('9'), ShowTabNavigator],
                [Modifiers::CTRL, KeyCode::Char('X'), ActivateCopyMode],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Char('"'),
                    SplitVertical(SpawnCommand {
                        domain: SpawnTabDomain::CurrentPaneDomain,
                        ..Default::default()
                    })
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Char('%'),
                    SplitHorizontal(SpawnCommand {
                        domain: SpawnTabDomain::CurrentPaneDomain,
                        ..Default::default()
                    })
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::LeftArrow,
                    AdjustPaneSize(PaneDirection::Left, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::RightArrow,
                    AdjustPaneSize(PaneDirection::Right, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::UpArrow,
                    AdjustPaneSize(PaneDirection::Up, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::DownArrow,
                    AdjustPaneSize(PaneDirection::Down, 1)
                ],
                [
                    ctrl_shift,
                    KeyCode::LeftArrow,
                    ActivatePaneDirection(PaneDirection::Left)
                ],
                [
                    ctrl_shift,
                    KeyCode::RightArrow,
                    ActivatePaneDirection(PaneDirection::Right)
                ],
                [
                    ctrl_shift,
                    KeyCode::UpArrow,
                    ActivatePaneDirection(PaneDirection::Up)
                ],
                [
                    ctrl_shift,
                    KeyCode::DownArrow,
                    ActivatePaneDirection(PaneDirection::Down)
                ],
                [Modifiers::CTRL, KeyCode::Char('Z'), TogglePaneZoomState],
            );

            #[cfg(target_os = "macos")]
            k!([Modifiers::SUPER, KeyCode::Char('h'), HideApplication],);
        }

        if !config.disable_default_mouse_bindings {
            m!(
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Down {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Line)
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Down {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Word)
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Cell)
                ],
                [
                    Modifiers::SHIFT,
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(None)
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Up {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    CompleteSelectionOrOpenLinkAtMouseCursor(
                        ClipboardCopyDestination::PrimarySelection
                    )
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Up {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    CompleteSelection(ClipboardCopyDestination::PrimarySelection)
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Up {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    CompleteSelection(ClipboardCopyDestination::PrimarySelection)
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(Some(SelectionMode::Cell))
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Drag {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(Some(SelectionMode::Word))
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Drag {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(Some(SelectionMode::Line))
                ],
                [
                    Modifiers::NONE,
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Middle
                    },
                    PasteFrom(ClipboardPasteSource::PrimarySelection)
                ],
                [
                    Modifiers::SUPER,
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left,
                    },
                    StartWindowDrag
                ],
                [
                    ctrl_shift,
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left,
                    },
                    StartWindowDrag
                ],
            );
        }

        keys.retain(|_, v| *v != KeyAssignment::DisableDefaultAssignment);
        mouse.retain(|_, v| *v != KeyAssignment::DisableDefaultAssignment);

        Self {
            keys,
            leader,
            mouse,
        }
    }

    pub fn is_leader(&self, key: &KeyCode, mods: Modifiers) -> Option<std::time::Duration> {
        if let Some(leader) = self.leader.as_ref() {
            if leader.key == *key && leader.mods == mods {
                return Some(std::time::Duration::from_millis(
                    leader.timeout_milliseconds,
                ));
            }
        }
        None
    }

    fn remove_positional_alt(mods: Modifiers) -> Modifiers {
        mods - (Modifiers::LEFT_ALT | Modifiers::RIGHT_ALT)
    }

    pub fn lookup_key(&self, key: &KeyCode, mods: Modifiers) -> Option<KeyAssignment> {
        self.keys
            .get(&key.normalize_shift(Self::remove_positional_alt(mods)))
            .cloned()
    }

    pub fn lookup_mouse(&self, event: MouseEventTrigger, mods: Modifiers) -> Option<KeyAssignment> {
        self.mouse
            .get(&(event, Self::remove_positional_alt(mods)))
            .cloned()
    }
}
