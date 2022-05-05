use crate::commands::CommandDef;
use config::keyassignment::{
    ClipboardCopyDestination, ClipboardPasteSource, KeyAssignment, KeyTableEntry, KeyTables,
    MouseEventTrigger, SelectionMode,
};
use config::ConfigHandle;
use std::collections::HashMap;
use std::time::Duration;
use wezterm_term::input::MouseButton;
use window::{KeyCode, Modifiers};

pub struct InputMap {
    pub keys: KeyTables,
    pub mouse: HashMap<(MouseEventTrigger, Modifiers), KeyAssignment>,
    leader: Option<(KeyCode, Modifiers, Duration)>,
}

impl InputMap {
    pub fn new(config: &ConfigHandle) -> Self {
        let mut mouse = config.mouse_bindings();

        let mut keys = config.key_bindings();

        let leader = config.leader.as_ref().map(|leader| {
            (
                leader.key.key.resolve(config.key_map_preference).clone(),
                leader.key.mods,
                Duration::from_millis(leader.timeout_milliseconds),
            )
        });

        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;

        macro_rules! m {
            ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
                $(
                mouse.entry(($code, $mod)).or_insert($action);
                )*
            };
        }

        use KeyAssignment::*;

        if !config.disable_default_key_bindings {
            for (mods, code, action) in CommandDef::default_key_assignments(config) {
                keys.default
                    .entry((code, mods))
                    .or_insert(KeyTableEntry { action });
            }
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

        keys.default
            .retain(|_, v| v.action != KeyAssignment::DisableDefaultAssignment);
        mouse.retain(|_, v| *v != KeyAssignment::DisableDefaultAssignment);

        keys.by_name
            .entry("copy_mode".to_string())
            .or_insert_with(crate::overlay::copy::key_table);

        Self {
            keys,
            leader,
            mouse,
        }
    }

    pub fn is_leader(&self, key: &KeyCode, mods: Modifiers) -> Option<std::time::Duration> {
        if let Some((leader_key, leader_mods, timeout)) = self.leader.as_ref() {
            if *leader_key == *key && *leader_mods == mods {
                return Some(timeout.clone());
            }
        }
        None
    }

    fn remove_positional_alt(mods: Modifiers) -> Modifiers {
        mods - (Modifiers::LEFT_ALT | Modifiers::RIGHT_ALT)
    }

    pub fn has_table(&self, name: &str) -> bool {
        self.keys.by_name.contains_key(name)
    }

    pub fn lookup_key(
        &self,
        key: &KeyCode,
        mods: Modifiers,
        table_name: Option<&str>,
    ) -> Option<KeyTableEntry> {
        let table = match table_name {
            Some(name) => self.keys.by_name.get(name)?,
            None => &self.keys.default,
        };

        table
            .get(&key.normalize_shift(Self::remove_positional_alt(mods)))
            .cloned()
    }

    pub fn lookup_mouse(&self, event: MouseEventTrigger, mods: Modifiers) -> Option<KeyAssignment> {
        self.mouse
            .get(&(event, Self::remove_positional_alt(mods)))
            .cloned()
    }
}
