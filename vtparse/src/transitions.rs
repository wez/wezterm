// Builds up the transition table for a state machine based on
// https://vt100.net/emu/dec_ansi_parser

use crate::enums::{Action, State};

/// Returns 1 if a <= x and x <= b. Otherwise returns 0.
const fn in_range(x: u8, a: u8, b: u8) -> u8 {
    ((x <= b) as u8) & ((a <= x) as u8)
}

/// Returns `true_value` if `condition` is true. Otherwise returns `false_value`.
const fn cond(condition: bool, true_value: u8, false_value: u8) -> u8 {
    ((condition as u8) * true_value) | ((!condition as u8) * false_value)
}

/// Match `u8` using given patterns. Return `OptionPack`.
/// The patterns should not overlap.
///
/// This should really be just a normal `match { .. }`
/// statement. However `match` is not const_fn right now.
/// See https://github.com/rust-lang/rust/issues/49146.
macro_rules! match_action_state {
    ( $name:ident => {
        $( $a:tt $( ..= $b:tt )? => ($action:ident, $state:ident), )*
    }) => {
        OptionPack(
            $(
                ({
                    // B: $b if $b exists, or $a if $b does not exist.
                    const B: u8 = [$a $(,$b)?][[$a $(,$b)?].len() - 1];
                    in_range($name, $a, B) as u16 * OptionPack::pack(Action::$action, State::$state).0
                }) |
            )* 0
        )
    }
}

/// Define `fn(u8) -> u8`.
macro_rules! define_function {
    ( $( $state:tt $func_name:ident { $($body:tt)* } )* ) => {
        $(
            const fn $func_name(i: u8) -> u8 {
                let v = match_action_state! { i => { $($body)* } };
                v.or(anywhere(i).or(pack(Action::None, State::$state)))
            }
        )*
    };
}

/// Apply all u8 values to `fn(u8) -> u8`, return `[u8; 256]`.
macro_rules! define_table {
    ( $func:tt ) => {{
        const fn gen() -> [u8; 256] {
            let mut arr = [0; 256];

            let mut i = 0;
            while i < 256 {
                arr[i] = $func(i as u8);
                i += 1;
            }
            return arr;
        }
        gen()
    }};
}

/// An alternative form of `Option<u8>` that works with const_fn.
///
/// This should really be just an `Option<u8>`. However that is
/// hard to express in const_fn right now.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct OptionPack(u16);

impl OptionPack {
    const fn is_none(self) -> bool {
        (self.0 & 1) == 0
    }

    const fn to_u8(self) -> u8 {
        (self.0 >> 1) as u8
    }

    const fn pack(action: Action, state: State) -> Self {
        Self(1 | ((((action as u16) << 4) | (state as u16)) << 1))
    }

    const fn or(self, default: u8) -> u8 {
        cond(self.is_none(), default, self.to_u8())
    }
}

const fn pack(action: Action, state: State) -> u8 {
    ((action as u8) << 4) | (state as u8)
}

const fn anywhere(i: u8) -> OptionPack {
    match_action_state! { i => {
        0x18        => (Execute, Ground),
        0x1a        => (Execute, Ground),
        0x80..=0x8f => (Execute, Ground),
        0x91..=0x97 => (Execute, Ground),
        0x99        => (Execute, Ground),
        0x9a        => (Execute, Ground),
        0x9c        => (None, Ground),
        0x1b        => (None, Escape),
        0x98        => (None, SosPmApcString),
        0x9e        => (None, SosPmApcString),
        0x9f        => (None, SosPmApcString),
        0x90        => (None, DcsEntry),
        0x9d        => (None, OscString),
        0x9b        => (None, CsiEntry),
    } }
}

define_function! {
    Ground ground {
        0x00..=0x17 => (Execute, Ground),
        0x19        => (Execute, Ground),
        0x1c..=0x1f => (Execute, Ground),
        0x20..=0x7f => (Print, Ground),
        // The following three ranges allow for
        // UTF-8 multibyte sequences to be recognized
        // and emitted as byte sequences in the ground
        // state.
        0xc2..=0xdf => (Utf8, Utf8Sequence),
        0xe0..=0xef => (Utf8, Utf8Sequence),
        0xf0..=0xf4 => (Utf8, Utf8Sequence),
    }

    Escape escape {
        0x00..=0x17 => (Execute, Escape),
        0x19        => (Execute, Escape),
        0x1c..=0x1f => (Execute, Escape),
        0x7f        => (Ignore, Escape),
        0x20..=0x2f => (Collect, EscapeIntermediate),
        0x30..=0x4f => (EscDispatch, Ground),
        0x51..=0x57 => (EscDispatch, Ground),
        0x59        => (EscDispatch, Ground),
        0x5a        => (EscDispatch, Ground),
        0x5c        => (EscDispatch, Ground),
        0x60..=0x7e => (EscDispatch, Ground),
        0x5b        => (None, CsiEntry),
        0x5d        => (None, OscString),
        0x50        => (None, DcsEntry),
        0x58        => (None, SosPmApcString),
        0x5e        => (None, SosPmApcString),
        0x5f        => (None, SosPmApcString),
    }

    EscapeIntermediate escape_intermediate {
        0x00..=0x17 => (Execute, EscapeIntermediate),
        0x19        => (Execute, EscapeIntermediate),
        0x1c..=0x1f => (Execute, EscapeIntermediate),
        0x20..=0x2f => (Collect, EscapeIntermediate),
        0x7f        => (Ignore, EscapeIntermediate),
        0x30..=0x7e => (EscDispatch, Ground),
    }

    CsiEntry csi_entry {
        0x00..=0x17 => (Execute, CsiEntry),
        0x19        => (Execute, CsiEntry),
        0x1c..=0x1f => (Execute, CsiEntry),
        0x7f        => (Ignore, CsiEntry),
        0x20..=0x2f => (Collect, CsiIntermediate),
        0x3a        => (None, CsiIgnore),
        0x30..=0x39 => (Param, CsiParam),
        0x3b        => (Param, CsiParam),
        0x3c..=0x3f => (Collect, CsiParam),
        0x40..=0x7e => (CsiDispatch, Ground),
    }

    CsiParam csi_param {
        0x00..=0x17 => (Execute, CsiParam),
        0x19        => (Execute, CsiParam),
        0x1c..=0x1f => (Execute, CsiParam),
        0x30..=0x3b => (Param, CsiParam),
        0x7f        => (Ignore, CsiParam),
        0x3c..=0x3f => (None, CsiIgnore),
        0x20..=0x2f => (Collect, CsiIntermediate),
        0x40..=0x7e => (CsiDispatch, Ground),
    }

    CsiIntermediate csi_intermediate {
        0x00..=0x17 => (Execute, CsiIntermediate),
        0x19        => (Execute, CsiIntermediate),
        0x1c..=0x1f => (Execute, CsiIntermediate),
        0x20..=0x2f => (Collect, CsiIntermediate),
        0x7f        => (Ignore, CsiIntermediate),
        0x30..=0x3f => (None, CsiIgnore),
        0x40..=0x7e => (CsiDispatch, Ground),
    }

    CsiIgnore csi_ignore {
        0x00..=0x17 => (Execute, CsiIgnore),
        0x19        => (Execute, CsiIgnore),
        0x1c..=0x1f => (Execute, CsiIgnore),
        0x20..=0x3f => (Ignore, CsiIgnore),
        0x7f        => (Ignore, CsiIgnore),
        0x40..=0x7e => (None, Ground),
    }

    DcsEntry dcs_entry {
        0x00..=0x17 => (Ignore, DcsEntry),
        0x19        => (Ignore, DcsEntry),
        0x1c..=0x1f => (Ignore, DcsEntry),
        0x7f        => (Ignore, DcsEntry),
        0x3a        => (None, DcsIgnore),
        0x20..=0x2f => (Collect, DcsIntermediate),
        0x30..=0x39 => (Param, DcsParam),
        0x3b        => (Param, DcsParam),
        0x3c..=0x3f => (Collect, DcsParam),
        0x40..=0x7e => (None, DcsPassthrough),
    }

    DcsParam dcs_param {
        0x00..=0x17 => (Ignore, DcsParam),
        0x19        => (Ignore, DcsParam),
        0x1c..=0x1f => (Ignore, DcsParam),
        0x30..=0x39 => (Param, DcsParam),
        0x3b        => (Param, DcsParam),
        0x7f        => (Ignore, DcsParam),
        0x3a        => (None, DcsIgnore),
        0x3c..=0x3f => (None, DcsIgnore),
        0x20..=0x2f => (Collect, DcsIntermediate),
        0x40..=0x7e => (None, DcsPassthrough),
    }

    DcsIntermediate dcs_intermediate {
        0x00..=0x17 => (Ignore, DcsIntermediate),
        0x19        => (Ignore, DcsIntermediate),
        0x1c..=0x1f => (Ignore, DcsIntermediate),
        0x20..=0x2f => (Collect, DcsIntermediate),
        0x7f        => (Ignore, DcsIntermediate),
        0x30..=0x3f => (None, DcsIgnore),
        0x40..=0x7e => (None, DcsPassthrough),
    }

    DcsPassthrough dcs_passthrough {
        0x00..=0x17 => (Put, DcsPassthrough),
        0x19        => (Put, DcsPassthrough),
        0x1c..=0x1f => (Put, DcsPassthrough),
        0x20..=0x7e => (Put, DcsPassthrough),
        0x7f        => (Ignore, DcsPassthrough),
    }

    DcsIgnore dcs_ignore {
        0x00..=0x17 => (Ignore, DcsIgnore),
        0x19        => (Ignore, DcsIgnore),
        0x1c..=0x1f => (Ignore, DcsIgnore),
        0x20..=0x7f => (Ignore, DcsIgnore),
    }

    OscString osc_string {
        0x00..=0x06 => (Ignore, OscString),
        // Using BEL in place of ST is a deviation from
        // https://vt100.net/emu/dec_ansi_parser and was
        // introduced AFAICT by xterm
        0x07        => (Ignore, Ground),
        0x08..=0x17 => (Ignore, OscString),
        0x19        => (Ignore, OscString),
        0x1c..=0x1f => (Ignore, OscString),
        0x20..=0x7f => (OscPut, OscString),
        // This extended range allows for UTF-8 characters
        // to be embedded in OSC parameters.  It is not
        // part of the base state machine.
        0xc2..=0xdf => (Utf8, Utf8Sequence),
        0xe0..=0xef => (Utf8, Utf8Sequence),
        0xf0..=0xf4 => (Utf8, Utf8Sequence),
    }

    SosPmApcString sos_pm_apc_string {
        0x00..=0x17 => (Ignore, SosPmApcString),
        0x19        => (Ignore, SosPmApcString),
        0x1c..=0x1f => (Ignore, SosPmApcString),
        0x20..=0x7f => (Ignore, SosPmApcString),
    }
}

pub(crate) static TRANSITIONS: [[u8; 256]; 14] = [
    define_table!(ground),
    define_table!(escape),
    define_table!(escape_intermediate),
    define_table!(csi_entry),
    define_table!(csi_param),
    define_table!(csi_intermediate),
    define_table!(csi_ignore),
    define_table!(dcs_entry),
    define_table!(dcs_param),
    define_table!(dcs_intermediate),
    define_table!(dcs_passthrough),
    define_table!(dcs_ignore),
    define_table!(osc_string),
    define_table!(sos_pm_apc_string),
];

pub(crate) static ENTRY: [Action; 14] = [
    Action::None,     // Ground
    Action::Clear,    // Escape
    Action::None,     // EscapeIntermediate
    Action::Clear,    // CsiEntry
    Action::None,     // CsiParam
    Action::None,     // CsiIntermediate
    Action::None,     // CsiIgnore
    Action::Clear,    // DcsEntry
    Action::None,     // DcsParam
    Action::None,     // DcsIntermediate
    Action::Hook,     // DcsPassthrough
    Action::None,     // DcsIgnore
    Action::OscStart, // OscString
    Action::None,     // SosPmApcString
];

pub(crate) static EXIT: [Action; 14] = [
    Action::None,   // Ground
    Action::None,   // Escape
    Action::None,   // EscapeIntermediate
    Action::None,   // CsiEntry
    Action::None,   // CsiParam
    Action::None,   // CsiIntermediate
    Action::None,   // CsiIgnore
    Action::None,   // DcsEntry
    Action::None,   // DcsParam
    Action::None,   // DcsIntermediate
    Action::Unhook, // DcsPassthrough
    Action::None,   // DcsIgnore
    Action::OscEnd, // OscString
    Action::None,   // SosPmApcString
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_transitions() {
        let v = format!("{:?}", TRANSITIONS).as_bytes().to_vec();
        assert_eq!(
            (
                v.len(),
                hash(&v, 0, 1),
                hash(&v, 5381, 33), // djb2
                hash(&v, 0, 65599), // sdbm
            ),
            (14021, 626090, 11884276359605205711, 6929800990073628062)
        );
    }

    fn hash(v: &[u8], init: u64, mul: u64) -> u64 {
        v.iter()
            .fold(init, |a, &b| a.wrapping_mul(mul).wrapping_add(b as u64))
    }
}
