// Builds up the transition table for a state machine based on
// https://vt100.net/emu/dec_ansi_parser
use std::collections::HashMap;
use std::io::prelude::*;
use std::ops::RangeInclusive;

#[path = "src/enums.rs"]
mod enums;
use crate::enums::*;

type StateMap = HashMap<u8, (Action, State)>;

fn r(key: u8) -> RangeInclusive<u8> {
    key..=key
}

fn insert(map: &mut StateMap, keys: RangeInclusive<u8>, value: (Action, State)) {
    for k in keys {
        map.insert(k, value);
    }
}

macro_rules! sparse_table {
    ($( $key:expr => ($action:ident, $state:ident) ),* $(,)?) => {
        {
            let mut map = StateMap::new();

            $(
                insert(&mut map, $key, (Action::$action, State::$state));
            )*

            map
        }
    }
}

fn apply_anywhere(anywhere: &StateMap, mut map: StateMap) -> StateMap {
    for (k, v) in anywhere {
        assert!(!map.contains_key(k));
        map.insert(*k, *v);
    }
    map
}

struct Tables {
    transitions: HashMap<State, StateMap>,
    entry: HashMap<State, Action>,
    exit: HashMap<State, Action>,
}

fn build_tables() -> Tables {
    let mut transitions = HashMap::new();
    let mut entry = HashMap::new();
    let mut exit = HashMap::new();

    let anywhere = sparse_table! {
        r(0x18)     => (Execute, Ground),
        r(0x1a)     => (Execute, Ground),
        0x80..=0x8f => (Execute, Ground),
        0x91..=0x97 => (Execute, Ground),
        r(0x99)     => (Execute, Ground),
        r(0x9a)     => (Execute, Ground),
        r(0x9c)     => (None, Ground),
        r(0x1b)     => (None, Escape),
        r(0x98)     => (None, SosPmApcString),
        r(0x9e)     => (None, SosPmApcString),
        r(0x9f)     => (None, SosPmApcString),
        r(0x90)     => (None, DcsEntry),
        r(0x9d)     => (None, OscString),
        r(0x9b)     => (None, CsiEntry),
    };

    transitions.insert(
        State::Ground,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, Ground),
                r(0x19)     => (Execute, Ground),
                0x1c..=0x1f => (Execute, Ground),
                0x20..=0x7f => (Print, Ground),
            },
        ),
    );

    entry.insert(State::Escape, Action::Clear);
    transitions.insert(
        State::Escape,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, Escape),
                r(0x19)     => (Execute, Escape),
                0x1c..=0x1f => (Execute, Escape),
                r(0x7f)     => (Ignore, Escape),
                0x20..=0x2f => (Collect, EscapeIntermediate),
                0x30..=0x4f => (EscDispatch, Ground),
                0x51..=0x57 => (EscDispatch, Ground),
                r(0x59)     => (EscDispatch, Ground),
                r(0x5a)     => (EscDispatch, Ground),
                r(0x5c)     => (EscDispatch, Ground),
                0x60..=0x7e => (EscDispatch, Ground),
                r(0x5b)     => (None, CsiEntry),
                r(0x5d)     => (None, OscString),
                r(0x50)     => (None, DcsEntry),
                r(0x58)     => (None, SosPmApcString),
                r(0x5e)     => (None, SosPmApcString),
                r(0x5f)     => (None, SosPmApcString),
            },
        ),
    );

    transitions.insert(
        State::EscapeIntermediate,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, EscapeIntermediate),
                r(0x19)     => (Execute, EscapeIntermediate),
                0x1c..=0x1f => (Execute, EscapeIntermediate),
                0x20..=0x2f => (Collect, EscapeIntermediate),
                r(0x7f)     => (Ignore, EscapeIntermediate),
                0x30..=0x7e => (EscDispatch, Ground),
            },
        ),
    );

    entry.insert(State::CsiEntry, Action::Clear);
    transitions.insert(
        State::CsiEntry,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, CsiEntry),
                r(0x19)     => (Execute, CsiEntry),
                0x1c..=0x1f => (Execute, CsiEntry),
                r(0x7f)     => (Ignore, CsiEntry),
                0x20..=0x2f => (Collect, CsiIntermediate),
                r(0x3a)     => (None, CsiIgnore),
                0x30..=0x39 => (Param, CsiParam),
                r(0x3b)     => (Param, CsiParam),
                0x3c..=0x3f => (Collect, CsiParam),
                0x40..=0x7e => (CsiDispatch, Ground),
            },
        ),
    );

    transitions.insert(
        State::CsiIgnore,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, CsiIgnore),
                r(0x19)     => (Execute, CsiIgnore),
                0x1c..=0x1f => (Execute, CsiIgnore),
                0x20..=0x3f => (Ignore, CsiIgnore),
                r(0x7f)     => (Ignore, CsiIgnore),
                0x40..=0x7e => (None, Ground),
            },
        ),
    );

    transitions.insert(
        State::CsiParam,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, CsiParam),
                r(0x19)     => (Execute, CsiParam),
                0x1c..=0x1f => (Execute, CsiParam),
                0x30..=0x39 => (Param, CsiParam),
                r(0x3b)     => (Param, CsiParam),
                r(0x7f)     => (Ignore, CsiParam),
                r(0x3a)     => (None, CsiIgnore),
                0x3c..=0x3f => (None, CsiIgnore),
                0x20..=0x2f => (Collect, CsiIntermediate),
                0x40..=0x7e => (CsiDispatch, Ground),
            },
        ),
    );

    transitions.insert(
        State::CsiIntermediate,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Execute, CsiIntermediate),
                r(0x19)     => (Execute, CsiIntermediate),
                0x1c..=0x1f => (Execute, CsiIntermediate),
                0x20..=0x2f => (Collect, CsiIntermediate),
                r(0x7f)     => (Ignore, CsiIntermediate),
                0x30..=0x3f => (None, CsiIgnore),
                0x40..=0x7e => (CsiDispatch, Ground),
            },
        ),
    );

    entry.insert(State::DcsEntry, Action::Clear);
    transitions.insert(
        State::DcsEntry,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Ignore, DcsEntry),
                r(0x19)     => (Ignore, DcsEntry),
                0x1c..=0x1f => (Ignore, DcsEntry),
                r(0x7f)     => (Ignore, DcsEntry),
                r(0x3a)     => (None, DcsIgnore),
                0x20..=0x2f => (Collect, DcsIntermediate),
                0x30..=0x39 => (Param, DcsParam),
                r(0x3b)     => (Param, DcsParam),
                0x3c..=0x3f => (Collect, DcsParam),
                0x40..=0x7e => (None, DcsPassthrough),
            },
        ),
    );

    transitions.insert(
        State::DcsIntermediate,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Ignore, DcsIntermediate),
                r(0x19)     => (Ignore, DcsIntermediate),
                0x1c..=0x1f => (Ignore, DcsIntermediate),
                0x20..=0x2f => (Collect, DcsIntermediate),
                r(0x7f)     => (Ignore, DcsIntermediate),
                0x30..=0x3f => (None, DcsIgnore),
                0x40..=0x7e => (None, DcsPassthrough),
            },
        ),
    );

    transitions.insert(
        State::DcsIgnore,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Ignore, DcsIgnore),
                r(0x19)     => (Ignore, DcsIgnore),
                0x1c..=0x1f => (Ignore, DcsIgnore),
                0x20..=0x7f => (Ignore, DcsIgnore),
            },
        ),
    );

    transitions.insert(
        State::DcsParam,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Ignore, DcsParam),
                r(0x19)     => (Ignore, DcsParam),
                0x1c..=0x1f => (Ignore, DcsParam),
                0x30..=0x39 => (Param, DcsParam),
                r(0x3b)     => (Param, DcsParam),
                r(0x7f)     => (Ignore, DcsParam),
                r(0x3a)     => (None, DcsIgnore),
                0x3c..=0x3f => (None, DcsIgnore),
                0x20..=0x2f => (Collect, DcsIntermediate),
                0x40..=0x7e => (None, DcsPassthrough),
            },
        ),
    );

    entry.insert(State::DcsPassthrough, Action::Hook);
    exit.insert(State::DcsPassthrough, Action::Unhook);
    transitions.insert(
        State::DcsPassthrough,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Put, DcsPassthrough),
                r(0x19)     => (Put, DcsPassthrough),
                0x1c..=0x1f => (Put, DcsPassthrough),
                0x20..=0x7e => (Put, DcsPassthrough),
                r(0x7f)     => (Ignore, DcsPassthrough),
            },
        ),
    );

    transitions.insert(
        State::SosPmApcString,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x17 => (Ignore, SosPmApcString),
                r(0x19)     => (Ignore, SosPmApcString),
                0x1c..=0x1f => (Ignore, SosPmApcString),
                0x20..=0x7f => (Ignore, SosPmApcString),
            },
        ),
    );

    entry.insert(State::OscString, Action::OscStart);
    exit.insert(State::OscString, Action::OscEnd);
    transitions.insert(
        State::OscString,
        apply_anywhere(
            &anywhere,
            sparse_table! {
                0x00..=0x06 => (Ignore, OscString),
                // Using BEL in place of ST is a deviation from
                // https://vt100.net/emu/dec_ansi_parser and was
                // introduced AFAICT by xterm
                r(0x07)     => (Ignore, Ground),
                0x08..=0x17 => (Ignore, OscString),
                r(0x19)     => (Ignore, OscString),
                0x1c..=0x1f => (Ignore, OscString),
                0x20..=0x7f => (OscPut, OscString),
            },
        ),
    );

    Tables {
        transitions,
        entry,
        exit,
    }
}

fn pack(action: Action, state: State) -> u8 {
    ((action as u8) << 4) | (state as u8)
}

fn write_tables(dest_path: std::path::PathBuf, tables: &Tables) -> std::io::Result<()> {
    let mut f = std::fs::File::create(&dest_path)?;
    writeln!(f, "pub static TRANSITIONS: [[u8; 256]; 14] = [")?;
    for state_num in State::Ground as u8..State::Anywhere as u8 {
        let this_state = State::from_u8(state_num);
        writeln!(f, "  // State: {:?}", this_state)?;
        write!(f, "  [")?;
        let state_transitions = tables.transitions.get(&this_state).unwrap();
        for byte in 0u8..=0xff {
            let (action, state) = state_transitions
                .get(&byte)
                .cloned()
                .unwrap_or((Action::None, this_state));
            if byte % 12 == 0 {
                write!(f, "\n   ")?;
            }
            write!(f, " 0x{:02x},", pack(action, state))?;
        }
        writeln!(f, "  ],")?;
    }
    writeln!(f, "];")?;

    event_table("ENTRY", &mut f, &tables.entry)?;
    event_table("EXIT", &mut f, &tables.exit)?;
    Ok(())
}

fn event_table(
    label: &str,
    f: &mut std::fs::File,
    table: &HashMap<State, Action>,
) -> std::io::Result<()> {
    writeln!(f, "pub static {}: [Action; 14] = [", label)?;
    for state_num in State::Ground as u8..State::Anywhere as u8 {
        let this_state = State::from_u8(state_num);
        let action = table.get(&this_state).cloned().unwrap_or(Action::None);
        writeln!(f, "  Action::{:?}, // {:?}", action, this_state)?;
    }
    writeln!(f, "];")?;
    Ok(())
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("transitions.rs");

    let tables = build_tables();
    write_tables(dest_path, &tables).unwrap();
}
