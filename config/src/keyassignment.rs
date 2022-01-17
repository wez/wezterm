use crate::keys::KeyNoAction;
use crate::{de_notnan, ConfigHandle, LeaderKey};
use luahelper::impl_lua_conversion;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use wezterm_input_types::{KeyCode, Modifiers, PhysKeyCode};
use wezterm_term::input::MouseButton;

#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LauncherActionArgs {
    pub flags: LauncherFlags,
    pub title: Option<String>,
}

bitflags::bitflags! {
    #[derive(Default, Deserialize, Serialize)]
    #[serde(try_from="String", into="String")]
    pub struct LauncherFlags :u32 {
        const ZERO = 0;
        const FUZZY = 1;
        const TABS = 2;
        const LAUNCH_MENU_ITEMS = 4;
        const DOMAINS = 8;
        const KEY_ASSIGNMENTS = 16;
        const WORKSPACES = 32;
    }
}

impl Into<String> for LauncherFlags {
    fn into(self) -> String {
        self.to_string()
    }
}

impl ToString for LauncherFlags {
    fn to_string(&self) -> String {
        let mut s = vec![];
        if self.contains(Self::FUZZY) {
            s.push("FUZZY");
        }
        if self.contains(Self::TABS) {
            s.push("TABS");
        }
        if self.contains(Self::LAUNCH_MENU_ITEMS) {
            s.push("LAUNCH_MENU_ITEMS");
        }
        if self.contains(Self::DOMAINS) {
            s.push("DOMAINS");
        }
        if self.contains(Self::KEY_ASSIGNMENTS) {
            s.push("KEY_ASSIGNMENTS");
        }
        if self.contains(Self::WORKSPACES) {
            s.push("WORKSPACES");
        }
        s.join("|")
    }
}

impl TryFrom<String> for LauncherFlags {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let mut flags = LauncherFlags::default();

        for ele in s.split('|') {
            let ele = ele.trim();
            match ele {
                "FUZZY" => flags |= Self::FUZZY,
                "TABS" => flags |= Self::TABS,
                "LAUNCH_MENU_ITEMS" => flags |= Self::LAUNCH_MENU_ITEMS,
                "DOMAINS" => flags |= Self::DOMAINS,
                "KEY_ASSIGNMENTS" => flags |= Self::KEY_ASSIGNMENTS,
                "WORKSPACES" => flags |= Self::WORKSPACES,
                _ => {
                    return Err(format!("invalid LauncherFlags `{}` in `{}`", ele, s));
                }
            }
        }

        Ok(flags)
    }
}

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
    /// Use a specific domain by id
    DomainId(usize),
}

impl Default for SpawnTabDomain {
    fn default() -> Self {
        Self::CurrentPaneDomain
    }
}

#[derive(Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

impl std::fmt::Debug for SpawnCommand {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self)
    }
}

impl std::fmt::Display for SpawnCommand {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "SpawnCommand")?;
        if let Some(label) = &self.label {
            write!(fmt, " label='{}'", label)?;
        }
        write!(fmt, " domain={:?}", self.domain)?;
        if let Some(args) = &self.args {
            write!(fmt, " args={:?}", args)?;
        }
        if let Some(cwd) = &self.cwd {
            write!(fmt, " cwd={}", cwd.display())?;
        }
        for (k, v) in &self.set_environment_variables {
            write!(fmt, " {}={}", k, v)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum PaneDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Prev,
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

#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct QuickSelectArguments {
    /// Overrides the main quick_select_alphabet config
    #[serde(default)]
    pub alphabet: String,
    /// Overrides the main quick_select_patterns config
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub action: Option<Box<KeyAssignment>>,
    /// Label to use in place of "copy" when `action` is set
    #[serde(default)]
    pub label: String,
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
    ActivateTabRelativeNoWrap(isize),
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ResetFontAndWindowSize,
    ActivateTab(isize),
    ActivateLastTab,
    SendString(String),
    SendKey(KeyNoAction),
    Nop,
    DisableDefaultAssignment,
    Hide,
    Show,
    CloseCurrentTab {
        confirm: bool,
    },
    ReloadConfiguration,
    MoveTabRelative(isize),
    MoveTab(usize),
    #[serde(deserialize_with = "de_notnan")]
    ScrollByPage(NotNan<f64>),
    ScrollByLine(isize),
    ScrollToPrompt(isize),
    ScrollToTop,
    ScrollToBottom,
    ShowTabNavigator,
    ShowDebugOverlay,
    HideApplication,
    QuitApplication,
    SpawnCommandInNewTab(SpawnCommand),
    SpawnCommandInNewWindow(SpawnCommand),
    SplitHorizontal(SpawnCommand),
    SplitVertical(SpawnCommand),
    ShowLauncher,
    ShowLauncherArgs(LauncherActionArgs),
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
    ActivatePaneByIndex(usize),
    TogglePaneZoomState,
    CloseCurrentPane {
        confirm: bool,
    },
    EmitEvent(String),
    QuickSelect,
    QuickSelectArgs(QuickSelectArguments),

    Multiple(Vec<KeyAssignment>),

    SwitchToWorkspace {
        name: Option<String>,
        spawn: Option<SpawnCommand>,
    },
    SwitchWorkspaceRelative(isize),
}
impl_lua_conversion!(KeyAssignment);

pub struct InputMap {
    pub keys: HashMap<(KeyCode, Modifiers), KeyAssignment>,
    pub mouse: HashMap<(MouseEventTrigger, Modifiers), KeyAssignment>,
    leader: Option<LeaderKey>,
}

impl InputMap {
    pub fn new(config: &ConfigHandle) -> Self {
        let mut mouse = config.mouse_bindings();

        let mut keys = config.key_bindings();

        let leader = config.leader.clone();

        macro_rules! k {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                keys.entry(($code, $mod)).or_insert($action);
                )*
            };
        }
        macro_rules! m {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                mouse.entry(($code, $mod)).or_insert($action);
                )*
            };
        }

        use KeyAssignment::*;

        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;

        if !config.disable_default_key_bindings {
            // Apply the default bindings; if the user has already mapped
            // a given entry then that will take precedence.
            k!(
                // Clipboard
                [
                    Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::Insert),
                    PasteFrom(ClipboardPasteSource::PrimarySelection)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::Insert),
                    CopyTo(ClipboardCopyDestination::PrimarySelection)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::C),
                    CopyTo(ClipboardCopyDestination::Clipboard)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::V),
                    PasteFrom(ClipboardPasteSource::Clipboard)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::C),
                    CopyTo(ClipboardCopyDestination::Clipboard)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::V),
                    PasteFrom(ClipboardPasteSource::Clipboard)
                ],
                // Window management
                [
                    Modifiers::ALT,
                    KeyCode::Physical(PhysKeyCode::Return),
                    ToggleFullScreen
                ],
                [Modifiers::SUPER, KeyCode::Physical(PhysKeyCode::M), Hide],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::N),
                    SpawnWindow
                ],
                [ctrl_shift, KeyCode::Physical(PhysKeyCode::M), Hide],
                [ctrl_shift, KeyCode::Physical(PhysKeyCode::N), SpawnWindow],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K),
                    ClearScrollback(ScrollbackEraseMode::ScrollbackOnly)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K),
                    ClearScrollback(ScrollbackEraseMode::ScrollbackOnly)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::F),
                    Search(Pattern::CaseSensitiveString("".into()))
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::F),
                    Search(Pattern::CaseSensitiveString("".into()))
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::L),
                    ShowDebugOverlay
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::Space),
                    QuickSelect
                ],
                // Font size manipulation
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::Minus),
                    DecreaseFontSize
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::K0),
                    ResetFontSize
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::Equal),
                    IncreaseFontSize
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::Minus),
                    DecreaseFontSize
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K0),
                    ResetFontSize
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::Equal),
                    IncreaseFontSize
                ],
                // Tab navigation and management
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::T),
                    SpawnTab(SpawnTabDomain::CurrentPaneDomain)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::T),
                    SpawnTab(SpawnTabDomain::CurrentPaneDomain)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K1),
                    ActivateTab(0)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K2),
                    ActivateTab(1)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K3),
                    ActivateTab(2)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K4),
                    ActivateTab(3)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K5),
                    ActivateTab(4)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K6),
                    ActivateTab(5)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K7),
                    ActivateTab(6)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K8),
                    ActivateTab(7)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::K9),
                    ActivateTab(-1)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::W),
                    CloseCurrentTab { confirm: true }
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K1),
                    ActivateTab(0)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K2),
                    ActivateTab(1)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K3),
                    ActivateTab(2)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K4),
                    ActivateTab(3)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K5),
                    ActivateTab(4)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K6),
                    ActivateTab(5)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K7),
                    ActivateTab(6)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K8),
                    ActivateTab(7)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::K9),
                    ActivateTab(-1)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::W),
                    CloseCurrentTab { confirm: true }
                ],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::LeftBracket),
                    ActivateTabRelative(-1)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::Tab),
                    ActivateTabRelative(-1)
                ],
                [Modifiers::CTRL, KeyCode::PageUp, ActivateTabRelative(-1)],
                [
                    Modifiers::SUPER | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::RightBracket),
                    ActivateTabRelative(1)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::Tab),
                    ActivateTabRelative(1)
                ],
                [
                    Modifiers::CTRL,
                    KeyCode::Physical(PhysKeyCode::PageDown),
                    ActivateTabRelative(1)
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::R),
                    ReloadConfiguration
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::R),
                    ReloadConfiguration
                ],
                [ctrl_shift, KeyCode::PageUp, MoveTabRelative(-1)],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::PageDown),
                    MoveTabRelative(1)
                ],
                [
                    Modifiers::SHIFT,
                    KeyCode::PageUp,
                    ScrollByPage(NotNan::new(-1.0).unwrap())
                ],
                [
                    Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::PageDown),
                    ScrollByPage(NotNan::new(1.0).unwrap())
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::X),
                    ActivateCopyMode
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::Quote),
                    SplitVertical(SpawnCommand {
                        domain: SpawnTabDomain::CurrentPaneDomain,
                        ..Default::default()
                    })
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::K5),
                    SplitHorizontal(SpawnCommand {
                        domain: SpawnTabDomain::CurrentPaneDomain,
                        ..Default::default()
                    })
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::LeftArrow),
                    AdjustPaneSize(PaneDirection::Left, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::RightArrow),
                    AdjustPaneSize(PaneDirection::Right, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::UpArrow),
                    AdjustPaneSize(PaneDirection::Up, 1)
                ],
                [
                    Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT,
                    KeyCode::Physical(PhysKeyCode::DownArrow),
                    AdjustPaneSize(PaneDirection::Down, 1)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::LeftArrow),
                    ActivatePaneDirection(PaneDirection::Left)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::RightArrow),
                    ActivatePaneDirection(PaneDirection::Right)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::UpArrow),
                    ActivatePaneDirection(PaneDirection::Up)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::DownArrow),
                    ActivatePaneDirection(PaneDirection::Down)
                ],
                [
                    ctrl_shift,
                    KeyCode::Physical(PhysKeyCode::Z),
                    TogglePaneZoomState
                ],
            );

            #[cfg(target_os = "macos")]
            k!(
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::H),
                    HideApplication
                ],
                [
                    Modifiers::SUPER,
                    KeyCode::Physical(PhysKeyCode::Q),
                    QuitApplication
                ],
            );
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
                    Modifiers::SHIFT,
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
