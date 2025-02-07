use crate::commands::CommandDef;
use config::keyassignment::{
    ClipboardCopyDestination, ClipboardPasteSource, KeyAssignment, KeyTableEntry, KeyTables,
    MouseEventTrigger, SelectionMode,
};
use config::{ConfigHandle, MouseEventAltScreen, MouseEventTriggerMods};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use wezterm_dynamic::{ToDynamic, Value};
use wezterm_term::input::MouseButton;
use window::{KeyCode, Modifiers, PhysKeyCode, UIKeyCapRendering};

pub struct InputMap {
    pub keys: KeyTables,
    pub mouse: HashMap<(MouseEventTrigger, MouseEventTriggerMods), KeyAssignment>,
    leader: Option<(KeyCode, Modifiers, Duration)>,
}

impl InputMap {
    pub fn default_input_map() -> Self {
        let config = ConfigHandle::default_config();
        Self::new(&config)
    }

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
                // If the user configures {key='p', mods='CTRL|SHIFT'} that gets
                // normalized into {key='P', mods='CTRL'} in Config::key_bindings(),
                // and that value exists in `keys.default` when we reach this point.
                //
                // When we get here with the default assignments for ActivateCommandPalette
                // we are going to register un-normalized entries that don't match
                // the existing normalized entry.
                //
                // Ideally we'd unconditionally normalize_shift
                // here and register the result if it isn't already in the map.
                //
                // Our default set of assignments deliberately and explicitly emits
                // variations on SHIFT as a workaround for an issue with
                // normalization under X11: <https://github.com/wezterm/wezterm/issues/1906>.
                // Until that is resolved, we need to keep emitting both variants.
                //
                // In order for the DisableDefaultAssignment behavior to work with the
                // least surprises, and for these normalization related workarounds
                // to continue? to work, the approach we take here is to lookup the
                // normalized version of what we're about to register, and if we get
                // a match, skip this key.  Otherwise register the non-normalized
                // version from default_key_assignments().
                //
                // See: <https://github.com/wezterm/wezterm/issues/3262>
                let (disable_code, disable_mods) = code.normalize_shift(mods);
                if keys
                    .default
                    .contains_key(&(disable_code.clone(), disable_mods))
                {
                    continue;
                }
                keys.default
                    .entry((code, mods))
                    .or_insert(KeyTableEntry { action });
            }
        }

        if !config.disable_default_mouse_bindings {
            m!(
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::False,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::WheelUp(1),
                    },
                    ScrollByCurrentEventWheelDelta
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::False,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::WheelDown(1),
                    },
                    ScrollByCurrentEventWheelDelta
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Line)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Word)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Cell)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::ALT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    SelectTextAtMouseCursor(SelectionMode::Block)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::SHIFT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Cell)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::SHIFT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    CompleteSelectionOrOpenLinkAtMouseCursor(
                        ClipboardCopyDestination::ClipboardAndPrimarySelection
                    )
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    CompleteSelectionOrOpenLinkAtMouseCursor(
                        ClipboardCopyDestination::ClipboardAndPrimarySelection
                    )
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::ALT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    CompleteSelection(ClipboardCopyDestination::ClipboardAndPrimarySelection)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::ALT | Modifiers::SHIFT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Block)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::ALT | Modifiers::SHIFT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    CompleteSelectionOrOpenLinkAtMouseCursor(
                        ClipboardCopyDestination::PrimarySelection
                    )
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    CompleteSelection(ClipboardCopyDestination::ClipboardAndPrimarySelection)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Up {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    CompleteSelection(ClipboardCopyDestination::ClipboardAndPrimarySelection)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Cell)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::ALT,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Block)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Drag {
                        streak: 2,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Word)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Drag {
                        streak: 3,
                        button: MouseButton::Left
                    },
                    ExtendSelectionToMouseCursor(SelectionMode::Line)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::NONE,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Down {
                        streak: 1,
                        button: MouseButton::Middle
                    },
                    PasteFrom(ClipboardPasteSource::PrimarySelection)
                ],
                [
                    MouseEventTriggerMods {
                        mods: Modifiers::SUPER,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
                    MouseEventTrigger::Drag {
                        streak: 1,
                        button: MouseButton::Left,
                    },
                    StartWindowDrag
                ],
                [
                    MouseEventTriggerMods {
                        mods: ctrl_shift,
                        mouse_reporting: false,
                        alt_screen: MouseEventAltScreen::Any,
                    },
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
        // Expand MouseEventAltScreen::Any to individual True/False entries
        let mut expanded_mouse = vec![];
        for ((code, mods), v) in &mouse {
            if mods.alt_screen == MouseEventAltScreen::Any {
                let mods_true = MouseEventTriggerMods {
                    alt_screen: MouseEventAltScreen::True,
                    ..*mods
                };
                let mods_false = MouseEventTriggerMods {
                    alt_screen: MouseEventAltScreen::False,
                    ..*mods
                };
                expanded_mouse.push((code.clone(), mods_true, v.clone()));
                expanded_mouse.push((code.clone(), mods_false, v.clone()));
            }
        }
        // Eliminate ::Any
        mouse.retain(|(_, mods), _| mods.alt_screen != MouseEventAltScreen::Any);
        for (code, mods, v) in expanded_mouse {
            mouse.insert((code, mods), v);
        }

        keys.by_name
            .entry("copy_mode".to_string())
            .or_insert_with(crate::overlay::copy::copy_key_table);
        keys.by_name
            .entry("search_mode".to_string())
            .or_insert_with(crate::overlay::copy::search_key_table);

        Self {
            keys,
            leader,
            mouse,
        }
    }

    /// Given an action, return the corresponding set of application-wide key assignments that are
    /// mapped to it.
    /// If any key_tables reference a given combination, then that combination
    /// is removed from the list.
    /// This is used to figure out whether an application-wide keyboard shortcut
    /// can be safely configured for this action, without interfering with any
    /// transient key_table mappings.
    #[allow(dead_code)]
    pub fn locate_app_wide_key_assignment(
        &self,
        action: &KeyAssignment,
    ) -> Vec<(KeyCode, Modifiers)> {
        let mut candidates = vec![];

        for ((key, mods), entry) in &self.keys.default {
            if mods.contains(Modifiers::LEADER) {
                continue;
            }
            if entry.action == *action {
                candidates.push((key.clone(), mods.clone()));
            }
        }

        // Now ensure that this combination is not part of a key table
        candidates.retain(|tuple| {
            for table in self.keys.by_name.values() {
                if table.contains_key(tuple) {
                    return false;
                }
            }
            true
        });

        candidates
    }

    pub fn is_leader(&self, key: &KeyCode, mods: Modifiers) -> Option<std::time::Duration> {
        if let Some((leader_key, leader_mods, timeout)) = self.leader.as_ref() {
            if *leader_key == *key && *leader_mods == mods.remove_positional_mods() {
                return Some(timeout.clone());
            }
        }
        None
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
            .get(&key.normalize_shift(mods.remove_positional_mods()))
            .cloned()
    }

    pub fn lookup_mouse(
        &self,
        event: MouseEventTrigger,
        mut mods: MouseEventTriggerMods,
    ) -> Option<KeyAssignment> {
        mods.mods = mods.mods.remove_positional_mods();
        self.mouse.get(&(event, mods)).cloned()
    }

    pub fn dump_config(&self, key_table: Option<&str>) {
        println!("local wezterm = require 'wezterm'");
        println!("local act = wezterm.action");
        println!();
        println!("return {{");

        if key_table.is_none() {
            println!("  keys = {{");
            show_key_table_as_lua(&self.keys.default, 4);
            println!("  }},");
            println!();
        }

        let mut table_names = self.keys.by_name.keys().collect::<Vec<_>>();
        table_names.sort();
        println!("  key_tables = {{");
        for name in table_names {
            if let Some(wanted_table) = key_table {
                if name != wanted_table {
                    continue;
                }
            }
            if let Some(table) = self.keys.by_name.get(name) {
                println!("    {name} = {{");
                show_key_table_as_lua(table, 6);
                println!("    }},");
                println!();
            }
        }
        println!("  }}");

        println!("}}");
    }

    pub fn show_keys(&self) {
        if let Some((key, mods, duration)) = &self.leader {
            println!("Leader: {key:?} {mods:?} {duration:?}");
        }

        section_header("Default key table");
        show_key_table(&self.keys.default);
        println!();

        let mut table_names = self.keys.by_name.keys().collect::<Vec<_>>();
        table_names.sort();
        for name in table_names {
            if let Some(table) = self.keys.by_name.get(name) {
                section_header(&format!("Key Table: {name}"));
                show_key_table(table);
                println!();
            }
        }

        self.show_mouse();
    }

    fn show_mouse(&self) {
        for (label, alt_screen, mouse_reporting) in [
            ("Mouse", MouseEventAltScreen::False, false),
            ("Mouse: alt_screen", MouseEventAltScreen::True, false),
            ("Mouse: mouse_reporting", MouseEventAltScreen::False, true),
            (
                "Mouse: mouse_reporting + alt_screen",
                MouseEventAltScreen::True,
                true,
            ),
        ] {
            let ordered = self
                .mouse
                .iter()
                .filter(|((_, m), _)| {
                    m.alt_screen == alt_screen && m.mouse_reporting == mouse_reporting
                })
                .collect::<BTreeMap<_, _>>();

            if ordered.is_empty() {
                continue;
            }

            section_header(label);

            let mut trigger_width = 0;
            let mut mod_width = 0;
            for (trigger, mods) in ordered.keys() {
                mod_width = mod_width.max(format!("{:?}", mods.mods).len());
                trigger_width = trigger_width.max(format!("{trigger:?}").len());
            }

            for ((trigger, mods), action) in ordered {
                let mods = if mods.mods == Modifiers::NONE {
                    String::new()
                } else {
                    format!("{:?}", mods.mods)
                };
                let trigger = format!("{trigger:?}");
                println!("\t{mods:mod_width$}   {trigger:trigger_width$}   ->   {action:?}");
            }

            println!();
        }
    }
}

fn section_header(title: &str) {
    let dash = "-".repeat(title.len());
    println!("{title}");
    println!("{dash}");
    println!();
}

pub fn ui_key(key: &KeyCode, ui_key_cap_rendering: UIKeyCapRendering) -> String {
    match key {
        KeyCode::Char('\x1b') | KeyCode::Char('\x7f')
            if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols =>
        {
            "\u{238b}".to_string()
        }
        KeyCode::Char('\x1b') | KeyCode::Char('\x7f') => "Esc".to_string(),
        KeyCode::Char('\x08') if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols => {
            "\u{232b}".to_string()
        }
        KeyCode::Char('\x08') => "Del".to_string(),
        KeyCode::Char('\r') if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols => {
            "\u{21b5}".to_string()
        }
        KeyCode::Char('\r') => "Enter".to_string(),
        KeyCode::Physical(PhysKeyCode::Space) | KeyCode::Char(' ')
            if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols =>
        {
            "\u{2423}".to_string()
        }
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char('\t') if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols => {
            "\u{21e5}".to_string()
        }
        KeyCode::Char('\t') => "Tab".to_string(),
        KeyCode::Char(c) if c.is_ascii_control() => c.escape_debug().to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),

        KeyCode::Physical(PhysKeyCode::PageUp) | KeyCode::PageUp
            if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols =>
        {
            "\u{21de}".to_string()
        }
        KeyCode::Physical(PhysKeyCode::PageDown) | KeyCode::PageDown
            if ui_key_cap_rendering == UIKeyCapRendering::AppleSymbols =>
        {
            "\u{21df}".to_string()
        }
        KeyCode::Physical(PhysKeyCode::LeftArrow) | KeyCode::LeftArrow => "\u{2190}".to_string(),
        KeyCode::Physical(PhysKeyCode::UpArrow) | KeyCode::UpArrow => "\u{2191}".to_string(),
        KeyCode::Physical(PhysKeyCode::RightArrow) | KeyCode::RightArrow => "\u{2192}".to_string(),
        KeyCode::Physical(PhysKeyCode::DownArrow) | KeyCode::DownArrow => "\u{2193}".to_string(),
        KeyCode::Function(n) => format!("F{n}"),
        KeyCode::Numpad(n) => format!("Numpad{n}"),
        KeyCode::Physical(phys) => phys.to_string(),
        _ => format!("{key:?}"),
    }
}

pub fn human_key(key: &KeyCode) -> String {
    match key {
        KeyCode::Char('\x1b') => "Escape".to_string(),
        KeyCode::Char('\x7f') => "Escape".to_string(),
        KeyCode::Char('\x08') => "Backspace".to_string(),
        KeyCode::Char('\r') => "Enter".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char('\t') => "Tab".to_string(),
        KeyCode::Char(c) if c.is_ascii_control() => c.escape_debug().to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Function(n) => format!("F{n}"),
        KeyCode::Numpad(n) => format!("Numpad{n}"),
        KeyCode::Physical(phys) => format!("{} (Physical)", phys.to_string()),
        _ => format!("{key:?}"),
    }
}

fn lua_key_code(key: &KeyCode) -> String {
    match key {
        KeyCode::Char('\x1b') => "Escape".to_string(),
        KeyCode::Char('\x7f') => "Escape".to_string(),
        KeyCode::Char('\x08') => "Backspace".to_string(),
        KeyCode::Char('\r') => "Enter".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char('\t') => "Tab".to_string(),
        KeyCode::Char(c) if c.is_ascii_control() => c.escape_debug().to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Function(n) => format!("F{n}"),
        KeyCode::Numpad(n) => format!("Numpad{n}"),
        KeyCode::Physical(phys) => format!("phys:{}", phys.to_string()),
        _ => format!("{key:?}"),
    }
}

fn luaify(value: Value, is_top: bool) -> String {
    match value {
        Value::String(s) if is_top => format!("act.{s}"),
        Value::String(s) => quote_lua_string(&s),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Null => "nil".to_string(),
        Value::U64(u) => u.to_string(),
        Value::F64(u) => u.to_string(),
        Value::I64(u) => u.to_string(),
        Value::Array(a) => {
            format!("wat {a:?}")
        }
        Value::Object(o) if is_top => {
            for (k, v) in o {
                let k = match k {
                    Value::String(s) => s,
                    _ => unreachable!(),
                };
                let arg = match v {
                    Value::String(_) => format!(" {}", luaify(v, false)),
                    Value::Array(a) => {
                        let b: Vec<String> = a.into_iter().map(|v| luaify(v, false)).collect();
                        format!("{{ {} }}", b.join(", "))
                    }
                    Value::I64(i) => format!("({i})"),
                    Value::U64(i) => format!("({i})"),
                    Value::F64(i) => format!("({i})"),
                    _ => luaify(v, false),
                };
                return format!("act.{k}{arg}");
            }
            unreachable!()
        }
        Value::Object(o) => {
            let mut fields = vec![];
            for (k, v) in o {
                let k = match k {
                    Value::String(s) => s,
                    _ => unreachable!(),
                };
                let arg = match v {
                    Value::Null => continue,
                    Value::String(_) => format!(" {}", luaify(v, false)),
                    Value::Array(a) => {
                        let b: Vec<String> = a.into_iter().map(|v| luaify(v, false)).collect();
                        format!("{{ {} }}", b.join(", "))
                    }
                    Value::I64(i) => format!("({i})"),
                    Value::U64(i) => format!("({i})"),
                    Value::F64(i) => format!("({i})"),
                    Value::Object(o) if o.is_empty() => continue,
                    _ => luaify(v, false),
                };
                fields.push(format!("{k} = {arg}"));
            }
            format!("{{ {} }}", fields.join(", "))
        }
    }
}

fn quote_lua_string(s: &str) -> String {
    let mut result = String::new();
    result.push('\'');
    for c in s.chars() {
        match c {
            '\u{07}' => {
                result.push_str("\\a");
            }
            '\u{08}' => {
                result.push_str("\\b");
            }
            '\u{0c}' => {
                result.push_str("\\f");
            }
            '\n' => {
                result.push_str("\\n");
            }
            '\r' => {
                result.push_str("\\r");
            }
            '\t' => {
                result.push_str("\\t");
            }
            '\u{0b}' => {
                result.push_str("\\v");
            }
            '\\' => {
                result.push_str("\\\\");
            }
            '"' => {
                result.push_str("\\\"");
            }
            '\'' => {
                result.push_str("\\'");
            }
            c if c.is_alphanumeric() || c.is_ascii_punctuation() => {
                result.push(c);
            }
            _ => {
                let b = c as u32;
                result.push_str(&format!("\\u{{{b:x}}}"));
            }
        }
    }
    result.push('\'');
    result
}

fn lua_key(key: &KeyCode, mods: Modifiers, action: &KeyAssignment) -> String {
    let dyn_action = action.to_dynamic();
    // println!(" -- {dyn_action:?}");
    let action = luaify(dyn_action, true);
    let key = lua_key_code(key);
    let key = quote_lua_string(&key);

    let mods = format!("{mods:?}").replace(" ", "");

    format!("{{ key = {key}, mods = '{mods}', action = {action} }}")
}

fn show_key_table(table: &config::keyassignment::KeyTable) {
    let ordered = table.iter().collect::<BTreeMap<_, _>>();

    let mut key_width = 0;
    let mut mod_width = 0;
    for (key, mods) in ordered.keys() {
        mod_width = mod_width.max(format!("{mods:?}").len());
        key_width = key_width.max(human_key(key).len());
    }

    for ((key, mods), entry) in ordered {
        let action = &entry.action;
        let mods = if *mods == Modifiers::NONE {
            String::new()
        } else {
            format!("{mods:?}")
        };
        let key = human_key(key);
        println!("\t{mods:mod_width$}   {key:key_width$}   ->   {action:?}");
    }
}

fn show_key_table_as_lua(table: &config::keyassignment::KeyTable, indent: usize) {
    let ordered = table.iter().collect::<BTreeMap<_, _>>();

    let pad = " ".repeat(indent);
    for ((key, mods), entry) in ordered {
        let action = &entry.action;
        println!("{pad}{},", lua_key(key, *mods, action));
    }
}
