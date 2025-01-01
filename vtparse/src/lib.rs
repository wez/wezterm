//! An implementation of the state machine described by
//! [DEC ANSI Parser](https://vt100.net/emu/dec_ansi_parser), modified to support UTF-8.
//!
//! This is sufficient to broadly categorize ANSI/ECMA-48 escape sequences that are
//! commonly used in terminal emulators.  It does not ascribe semantic meaning to
//! those escape sequences; for example, if you wish to parse the SGR sequence
//! that makes text bold, you will need to know which codes correspond to bold
//! in your implementation of `VTActor`.
//!
//! You may wish to use `termwiz::escape::parser::Parser` in the
//! [termwiz](https://docs.rs/termwiz/) crate if you don't want to have to research
//! all those possible escape sequences for yourself.
#![allow(clippy::upper_case_acronyms)]
use utf8parse::Parser as Utf8Parser;
mod enums;
use crate::enums::*;
mod transitions;

use transitions::{ENTRY, EXIT, TRANSITIONS};

#[inline(always)]
fn lookup(state: State, b: u8) -> (Action, State) {
    let v = unsafe {
        TRANSITIONS
            .get_unchecked(state as usize)
            .get_unchecked(b as usize)
    };
    (Action::from_u16(v >> 8), State::from_u16(v & 0xff))
}

#[inline(always)]
#[cfg(not(test))]
fn lookup_entry(state: State) -> Action {
    unsafe { *ENTRY.get_unchecked(state as usize) }
}

#[inline(always)]
#[cfg(test)]
fn lookup_entry(state: State) -> Action {
    *ENTRY
        .get(state as usize)
        .unwrap_or_else(|| panic!("State {:?} has no entry in ENTRY", state))
}

#[inline(always)]
#[cfg(test)]
fn lookup_exit(state: State) -> Action {
    *EXIT
        .get(state as usize)
        .unwrap_or_else(|| panic!("State {:?} has no entry in EXIT", state))
}

#[inline(always)]
#[cfg(not(test))]
fn lookup_exit(state: State) -> Action {
    unsafe { *EXIT.get_unchecked(state as usize) }
}

/// `VTActor` is a trait that allows the host application to process
/// the different kinds of sequence as they are parsed from the input
/// stream.
///
/// The functions defined by this trait correspond to the actions defined
/// in the [state machine](https://vt100.net/emu/dec_ansi_parser).
///
/// ## Terminology:
/// An intermediate is a character in the range 0x20-0x2f that
/// occurs before the final character in an escape sequence.
///
/// `ignored_excess_intermediates` is a boolean that is set in the case
/// where there were more than two intermediate characters; no standard
/// defines any codes with more than two.  Intermediates after
/// the second will set this flag and are discarded.
///
/// `params` in most of the functions of this trait are decimal integer parameters in escape
/// sequences.  They are separated by semicolon characters.  An omitted parameter is returned in
/// this interface as a zero, which represents the default value for that parameter.
///
/// Other jargon used here is defined in
/// [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf).
pub trait VTActor {
    /// The current code should be mapped to a glyph according to the character set mappings and
    /// shift states in effect, and that glyph should be displayed.
    ///
    /// If the input was UTF-8 then it will have been mapped to a unicode code point.  Invalid
    /// sequences are represented here using the unicode REPLACEMENT_CHARACTER.
    ///
    /// Otherwise the parameter will be a 7-bit printable value and may be subject to mapping
    /// depending on other state maintained by the embedding application.
    ///
    /// ## Some commentary from the state machine documentation:
    /// GL characters (20 to 7F) are
    /// printed. 20 (SP) and 7F (DEL) are included in this area, although both codes have special
    /// behaviour. If a 94-character set is mapped into GL, 20 will cause a space to be displayed,
    /// and 7F will be ignored. When a 96-character set is mapped into GL, both 20 and 7F may cause
    /// a character to be displayed. Later models of the VT220 included the DEC Multinational
    /// Character Set (MCS), which has 94 characters in its supplemental set (i.e. the characters
    /// supplied in addition to ASCII), so terminals only claiming VT220 compatibility can always
    /// ignore 7F. The VT320 introduced ISO Latin-1, which has 96 characters in its supplemental
    /// set, so emulators with a VT320 compatibility mode need to treat 7F as a printable
    /// character.
    fn print(&mut self, b: char);

    /// The C0 or C1 control function should be executed, which may have any one of a variety of
    /// effects, including changing the cursor position, suspending or resuming communications or
    /// changing the shift states in effect.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on C0 and C1 control functions.
    fn execute_c0_or_c1(&mut self, control: u8);

    /// invoked when a final character arrives in the first part of a device control string. It
    /// determines the control function from the private marker, intermediate character(s) and
    /// final character, and executes it, passing in the parameter list. It also selects a handler
    /// function for the rest of the characters in the control string.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on device control strings.
    fn dcs_hook(
        &mut self,
        mode: u8,
        params: &[i64],
        intermediates: &[u8],
        ignored_excess_intermediates: bool,
    );

    /// This action passes characters from the data string part of a device control string to a
    /// handler that has previously been selected by the dcs_hook action. C0 controls are also
    /// passed to the handler.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on device control strings.
    fn dcs_put(&mut self, byte: u8);

    /// When a device control string is terminated by ST, CAN, SUB or ESC, this action calls the
    /// previously selected handler function with an “end of data” parameter. This allows the
    /// handler to finish neatly.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on device control strings.
    fn dcs_unhook(&mut self);

    /// The final character of an escape sequence has arrived, so determine the control function
    /// to be executed from the intermediate character(s) and final character, and execute it.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on escape sequences.
    fn esc_dispatch(
        &mut self,
        params: &[i64],
        intermediates: &[u8],
        ignored_excess_intermediates: bool,
        byte: u8,
    );

    /// A final character of a Control Sequence Initiator has arrived, so determine the control function to be executed from
    /// private marker, intermediate character(s) and final character, and execute it, passing in
    /// the parameter list.
    ///
    /// See [ECMA-48](http://www.ecma-international.org/publications/files/ECMA-ST/ECMA-48,%202nd%20Edition,%20August%201979.pdf)
    /// for more information on control functions.
    fn csi_dispatch(&mut self, params: &[CsiParam], parameters_truncated: bool, byte: u8);

    /// Called when an OSC string is terminated by ST, CAN, SUB or ESC.
    ///
    /// `params` is an array of byte strings (which may also be valid utf-8)
    /// that were passed as semicolon separated parameters to the operating
    /// system command.
    fn osc_dispatch(&mut self, params: &[&[u8]]);

    /// Called when an APC string is terminated by ST
    /// `data` is the data contained within the APC sequence.
    fn apc_dispatch(&mut self, data: Vec<u8>);
}

/// `VTAction` is an alternative way to work with the parser; rather
/// than implementing the VTActor trait you can use `CollectingVTActor`
/// to capture the sequence of events into a `Vec<VTAction>`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum VTAction {
    Print(char),
    ExecuteC0orC1(u8),
    DcsHook {
        params: Vec<i64>,
        intermediates: Vec<u8>,
        ignored_excess_intermediates: bool,
        byte: u8,
    },
    DcsPut(u8),
    DcsUnhook,
    EscDispatch {
        params: Vec<i64>,
        intermediates: Vec<u8>,
        ignored_excess_intermediates: bool,
        byte: u8,
    },
    CsiDispatch {
        params: Vec<CsiParam>,
        parameters_truncated: bool,
        byte: u8,
    },
    OscDispatch(Vec<Vec<u8>>),
    ApcDispatch(Vec<u8>),
}

/// This is an implementation of `VTActor` that captures the events
/// into an internal vector.
/// It can be iterated via `into_iter` or have the internal
/// vector extracted via `into_vec`.
#[derive(Default)]
pub struct CollectingVTActor {
    actions: Vec<VTAction>,
}

impl IntoIterator for CollectingVTActor {
    type Item = VTAction;
    type IntoIter = std::vec::IntoIter<VTAction>;

    fn into_iter(self) -> Self::IntoIter {
        self.actions.into_iter()
    }
}

impl CollectingVTActor {
    pub fn into_vec(self) -> Vec<VTAction> {
        self.actions
    }
}

impl VTActor for CollectingVTActor {
    fn print(&mut self, b: char) {
        self.actions.push(VTAction::Print(b));
    }

    fn execute_c0_or_c1(&mut self, control: u8) {
        self.actions.push(VTAction::ExecuteC0orC1(control));
    }

    fn dcs_hook(
        &mut self,
        byte: u8,
        params: &[i64],
        intermediates: &[u8],
        ignored_excess_intermediates: bool,
    ) {
        self.actions.push(VTAction::DcsHook {
            byte,
            params: params.to_vec(),
            intermediates: intermediates.to_vec(),
            ignored_excess_intermediates,
        });
    }

    fn dcs_put(&mut self, byte: u8) {
        self.actions.push(VTAction::DcsPut(byte));
    }

    fn dcs_unhook(&mut self) {
        self.actions.push(VTAction::DcsUnhook);
    }

    fn esc_dispatch(
        &mut self,
        params: &[i64],
        intermediates: &[u8],
        ignored_excess_intermediates: bool,
        byte: u8,
    ) {
        self.actions.push(VTAction::EscDispatch {
            params: params.to_vec(),
            intermediates: intermediates.to_vec(),
            ignored_excess_intermediates,
            byte,
        });
    }

    fn csi_dispatch(&mut self, params: &[CsiParam], parameters_truncated: bool, byte: u8) {
        self.actions.push(VTAction::CsiDispatch {
            params: params.to_vec(),
            parameters_truncated,
            byte,
        });
    }

    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        self.actions.push(VTAction::OscDispatch(
            params.iter().map(|i| i.to_vec()).collect(),
        ));
    }

    fn apc_dispatch(&mut self, data: Vec<u8>) {
        self.actions.push(VTAction::ApcDispatch(data));
    }
}

const MAX_INTERMEDIATES: usize = 2;
const MAX_OSC: usize = 64;
const MAX_PARAMS: usize = 32;

struct OscState {
    buffer: Vec<u8>,
    param_indices: [usize; MAX_OSC],
    num_params: usize,
    full: bool,
}

impl OscState {
    fn put(&mut self, param: char) {
        if param == ';' {
            match self.num_params {
                MAX_OSC => {
                    self.full = true;
                }
                num => {
                    self.param_indices[num.saturating_sub(1)] = self.buffer.len();
                    self.num_params += 1;
                }
            }
        } else if !self.full {
            if self.num_params == 0 {
                self.num_params = 1;
            }

            let mut buf = [0u8; 8];
            self.buffer
                .extend_from_slice(param.encode_utf8(&mut buf).as_bytes());
        }
    }
}

/// The virtual terminal parser.  It works together with an implementation of `VTActor`.
pub struct VTParser {
    state: State,

    intermediates: [u8; MAX_INTERMEDIATES],
    num_intermediates: usize,
    ignored_excess_intermediates: bool,

    osc: OscState,

    params: [CsiParam; MAX_PARAMS],
    num_params: usize,
    current_param: Option<CsiParam>,
    params_full: bool,
    apc_data: Vec<u8>,

    utf8_parser: Utf8Parser,
    utf8_return_state: State,
}

/// Represents a parameter to a CSI-based escaped sequence.
///
/// CSI escapes typically have the form: `CSI 3 m`, but can also
/// bundle multiple values together: `CSI 3 ; 4 m`.  In both
/// of those examples the parameters are simple integer values
/// and latter of which would be expressed as a slice containing
/// `[CsiParam::Integer(3), CsiParam::Integer(4)]`.
///
/// There are some escape sequences that use colons to subdivide and
/// extend the meaning.  For example: `CSI 4:3 m` is a sequence used
/// to denote a curly underline.  That would be represented as:
/// `[CsiParam::ColonList(vec![Some(4), Some(3)])]`.
///
/// Later: reading ECMA 48, CSI is defined as:
/// CSI P ... P  I ... I  F
/// Where P are parameter bytes in the range 0x30-0x3F [0-9:;<=>?]
/// and I are intermediate bytes in the range 0x20-0x2F
/// and F is the final byte in the range 0x40-0x7E
///
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum CsiParam {
    Integer(i64),
    P(u8),
}

impl Default for CsiParam {
    fn default() -> Self {
        Self::Integer(0)
    }
}

impl CsiParam {
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }
}

impl std::fmt::Display for CsiParam {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CsiParam::Integer(v) => {
                write!(f, "{}", v)?;
            }
            CsiParam::P(p) => {
                write!(f, "{}", *p as char)?;
            }
        }
        Ok(())
    }
}

impl VTParser {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let param_indices = [0usize; MAX_OSC];

        Self {
            state: State::Ground,
            utf8_return_state: State::Ground,

            intermediates: [0, 0],
            num_intermediates: 0,
            ignored_excess_intermediates: false,

            osc: OscState {
                buffer: Vec::new(),
                param_indices,
                num_params: 0,
                full: false,
            },

            params: Default::default(),
            num_params: 0,
            params_full: false,
            current_param: None,

            utf8_parser: Utf8Parser::new(),
            apc_data: vec![],
        }
    }

    /// Returns if the state machine is in the ground state,
    /// i.e. there is no pending state held by the state machine.
    pub fn is_ground(&self) -> bool {
        self.state == State::Ground
    }

    fn as_integer_params(&self) -> [i64; MAX_PARAMS] {
        let mut res = [0i64; MAX_PARAMS];
        let mut i = 0;
        for src in &self.params[0..self.num_params] {
            if let CsiParam::Integer(value) = src {
                res[i] = *value;
            } else if let CsiParam::P(b';') = src {
                i += 1;
            }
        }
        res
    }

    fn finish_param(&mut self) {
        if let Some(val) = self.current_param.take() {
            if self.num_params < MAX_PARAMS {
                self.params[self.num_params] = val;
                self.num_params += 1;
            }
        }
    }

    /// Promote early intermediates to parameters.
    /// This is handle sequences such as DECSET that use `?`
    /// prior to other numeric parameters.
    /// `?` is technically in the intermediate range and shouldn't
    /// appear in the parameter position according to ECMA 48
    fn promote_intermediates_to_params(&mut self) {
        if self.num_intermediates > 0 {
            for &p in &self.intermediates[..self.num_intermediates] {
                if self.num_params >= MAX_PARAMS {
                    self.ignored_excess_intermediates = true;
                    break;
                }
                self.params[self.num_params] = CsiParam::P(p);
                self.num_params += 1;
            }
            self.num_intermediates = 0;
        }
    }

    fn action(&mut self, action: Action, param: u8, actor: &mut dyn VTActor) {
        match action {
            Action::None | Action::Ignore => {}
            Action::Print => actor.print(param as char),
            Action::Execute => actor.execute_c0_or_c1(param),
            Action::Clear => {
                self.num_intermediates = 0;
                self.ignored_excess_intermediates = false;
                self.osc.num_params = 0;
                self.osc.full = false;
                self.num_params = 0;
                self.params_full = false;
                self.current_param.take();
                self.apc_data.clear();
            }
            Action::Collect => {
                if self.num_intermediates < MAX_INTERMEDIATES {
                    self.intermediates[self.num_intermediates] = param;
                    self.num_intermediates += 1;
                } else {
                    self.ignored_excess_intermediates = true;
                }
            }
            Action::Param => {
                if self.params_full {
                    return;
                }

                self.promote_intermediates_to_params();

                match param {
                    b'0'..=b'9' => match self.current_param.take() {
                        Some(CsiParam::Integer(i)) => {
                            self.current_param.replace(CsiParam::Integer(
                                i.saturating_mul(10).saturating_add((param - b'0') as i64),
                            ));
                        }
                        Some(_) => unreachable!(),
                        None => {
                            self.current_param
                                .replace(CsiParam::Integer((param - b'0') as i64));
                        }
                    },
                    p => {
                        self.finish_param();

                        if self.num_params + 1 > MAX_PARAMS {
                            self.params_full = true;
                        } else {
                            self.params[self.num_params] = CsiParam::P(p);
                            self.num_params += 1;
                        }
                    }
                }
            }
            Action::Hook => {
                self.finish_param();
                actor.dcs_hook(
                    param,
                    &self.as_integer_params()[0..self.num_params],
                    &self.intermediates[0..self.num_intermediates],
                    self.ignored_excess_intermediates,
                );
            }
            Action::Put => actor.dcs_put(param),
            Action::EscDispatch => {
                self.finish_param();
                actor.esc_dispatch(
                    &self.as_integer_params()[0..self.num_params],
                    &self.intermediates[0..self.num_intermediates],
                    self.ignored_excess_intermediates,
                    param,
                );
            }
            Action::CsiDispatch => {
                self.finish_param();
                self.promote_intermediates_to_params();
                actor.csi_dispatch(
                    &self.params[0..self.num_params],
                    self.ignored_excess_intermediates,
                    param,
                );
            }
            Action::Unhook => actor.dcs_unhook(),
            Action::OscStart => {
                self.osc.buffer.clear();
                self.osc.num_params = 0;
                self.osc.full = false;
            }
            Action::OscPut => self.osc.put(param as char),

            Action::OscEnd => {
                if self.osc.num_params == 0 {
                    actor.osc_dispatch(&[]);
                } else {
                    let mut params: [&[u8]; MAX_OSC] = [b""; MAX_OSC];
                    let mut offset = 0usize;
                    let mut slice = self.osc.buffer.as_slice();
                    let limit = self.osc.num_params.min(MAX_OSC);
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..limit - 1 {
                        let (a, b) = slice.split_at(self.osc.param_indices[i] - offset);
                        params[i] = a;
                        slice = b;
                        offset = self.osc.param_indices[i];
                    }
                    params[limit - 1] = slice;
                    actor.osc_dispatch(&params[0..limit]);
                }
            }

            Action::ApcStart => {
                self.apc_data.clear();
            }
            Action::ApcPut => {
                self.apc_data.push(param);
            }
            Action::ApcEnd => {
                actor.apc_dispatch(std::mem::take(&mut self.apc_data));
            }

            Action::Utf8 => self.next_utf8(actor, param),
        }
    }

    // Process a utf-8 multi-byte sequence.
    // The state tables emit Action::Utf8 to initiate a multi-byte
    // sequence, and once we're in the utf-8 state we'll defer to
    // this method for each byte until the Decode struct is signalled
    // that we're done.
    // We use the REPLACEMENT_CHARACTER for invalid sequences.
    // We return to the ground state after each codepoint, successful
    // or otherwise.
    fn next_utf8(&mut self, actor: &mut dyn VTActor, byte: u8) {
        struct Decoder {
            codepoint: Option<char>,
        }

        impl utf8parse::Receiver for Decoder {
            fn codepoint(&mut self, c: char) {
                self.codepoint.replace(c);
            }

            fn invalid_sequence(&mut self) {
                self.codepoint(std::char::REPLACEMENT_CHARACTER);
            }
        }

        let mut decoder = Decoder { codepoint: None };

        self.utf8_parser.advance(&mut decoder, byte);
        if let Some(c) = decoder.codepoint {
            // Slightly gross special cases C1 controls that were
            // encoded as UTF-8 rather than emitted as raw 8-bit.
            // If the decoded value is in the byte range, and that
            // value would cause a state transition, then we process
            // that state transition rather than performing the default
            // string accumulation.
            if c as u32 <= 0xff {
                let byte = ((c as u32) & 0xff) as u8;

                let (action, state) = lookup(self.utf8_return_state, byte);
                if action == Action::Execute
                    || (state != self.utf8_return_state && state != State::Utf8Sequence)
                {
                    self.action(lookup_exit(self.utf8_return_state), 0, actor);
                    self.action(action, byte, actor);
                    self.action(lookup_entry(state), 0, actor);
                    self.utf8_return_state = self.state;
                    self.state = state;
                    return;
                }
            }

            match self.utf8_return_state {
                State::Ground => actor.print(c),
                State::OscString => self.osc.put(c),
                state => panic!("unreachable state {:?}", state),
            };
            self.state = self.utf8_return_state;
        }
    }

    /// Parse a single byte.  This may result in a call to one of the
    /// methods on the provided `actor`.
    #[inline(always)]
    pub fn parse_byte(&mut self, byte: u8, actor: &mut dyn VTActor) {
        // While in utf-8 parsing mode, co-opt the vt state
        // table and instead use the utf-8 state table from the
        // parser.  It will drop us back into the Ground state
        // after each recognized (or invalid) codepoint.
        if self.state == State::Utf8Sequence {
            self.next_utf8(actor, byte);
            return;
        }

        let (action, state) = lookup(self.state, byte);

        if state != self.state {
            if state != State::Utf8Sequence {
                self.action(lookup_exit(self.state), 0, actor);
            }
            self.action(action, byte, actor);
            self.action(lookup_entry(state), byte, actor);
            self.utf8_return_state = self.state;
            self.state = state;
        } else {
            self.action(action, byte, actor);
        }
    }

    /// Parse a sequence of bytes.  The sequence need not be complete.
    /// This may result in some number of calls to the methods on the
    /// provided `actor`.
    pub fn parse(&mut self, bytes: &[u8], actor: &mut dyn VTActor) {
        for b in bytes {
            self.parse_byte(*b, actor);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use k9::assert_equal as assert_eq;

    fn parse_as_vec(bytes: &[u8]) -> Vec<VTAction> {
        let mut parser = VTParser::new();
        let mut actor = CollectingVTActor::default();
        parser.parse(bytes, &mut actor);
        actor.into_vec()
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            parse_as_vec(b"yo\x07\x1b[32mwoot\x1b[0mdone"),
            vec![
                VTAction::Print('y'),
                VTAction::Print('o'),
                VTAction::ExecuteC0orC1(0x07,),
                VTAction::CsiDispatch {
                    params: vec![CsiParam::Integer(32)],
                    parameters_truncated: false,
                    byte: b'm',
                },
                VTAction::Print('w',),
                VTAction::Print('o',),
                VTAction::Print('o',),
                VTAction::Print('t',),
                VTAction::CsiDispatch {
                    params: vec![CsiParam::Integer(0)],
                    parameters_truncated: false,
                    byte: b'm',
                },
                VTAction::Print('d',),
                VTAction::Print('o',),
                VTAction::Print('n',),
                VTAction::Print('e',),
            ]
        );
    }

    #[test]
    fn test_print() {
        assert_eq!(
            parse_as_vec(b"yo"),
            vec![VTAction::Print('y'), VTAction::Print('o')]
        );
    }

    #[test]
    fn test_osc_with_c1_st() {
        assert_eq!(
            parse_as_vec(b"\x1b]0;there\x9c"),
            vec![VTAction::OscDispatch(vec![
                b"0".to_vec(),
                b"there".to_vec()
            ])]
        );
    }

    #[test]
    fn test_osc_with_bel_st() {
        assert_eq!(
            parse_as_vec(b"\x1b]0;hello\x07"),
            vec![VTAction::OscDispatch(vec![
                b"0".to_vec(),
                b"hello".to_vec()
            ])]
        );
    }

    #[test]
    fn test_decset() {
        assert_eq!(
            parse_as_vec(b"\x1b[?1l"),
            vec![VTAction::CsiDispatch {
                params: vec![CsiParam::P(b'?'), CsiParam::Integer(1)],
                parameters_truncated: false,
                byte: b'l',
            },]
        );
    }

    #[test]
    fn test_osc_too_many_params() {
        let fields = (0..MAX_OSC + 2)
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>();
        let input = format!("\x1b]{}\x07", fields.join(";"));
        let actions = parse_as_vec(input.as_bytes());
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            VTAction::OscDispatch(parsed_fields) => {
                let fields: Vec<_> = fields.into_iter().map(|s| s.as_bytes().to_vec()).collect();
                assert_eq!(parsed_fields.as_slice(), &fields[0..MAX_OSC]);
            }
            other => panic!("Expected OscDispatch but got {:?}", other),
        }
    }

    #[test]
    fn test_osc_with_no_params() {
        assert_eq!(
            parse_as_vec(b"\x1b]\x07"),
            vec![VTAction::OscDispatch(vec![])]
        );
    }

    #[test]
    fn test_osc_with_esc_sequence_st() {
        // This case isn't the same as the other OSC cases; even though
        // `ESC \` is the long form escape sequence for ST, the ESC on its
        // own breaks out of the OSC state and jumps into the ESC state,
        // and that leaves the `\` character to be dispatched there in
        // the calling application.
        assert_eq!(
            parse_as_vec(b"\x1b]woot\x1b\\"),
            vec![
                VTAction::OscDispatch(vec![b"woot".to_vec()]),
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\'
                }
            ]
        );
    }

    #[test]
    fn test_fancy_underline() {
        assert_eq!(
            parse_as_vec(b"\x1b[4m"),
            vec![VTAction::CsiDispatch {
                params: vec![CsiParam::Integer(4)],
                parameters_truncated: false,
                byte: b'm'
            }]
        );

        assert_eq!(
            // This is the kitty curly underline sequence.
            parse_as_vec(b"\x1b[4:3m"),
            vec![VTAction::CsiDispatch {
                params: vec![
                    CsiParam::Integer(4),
                    CsiParam::P(b':'),
                    CsiParam::Integer(3)
                ],
                parameters_truncated: false,
                byte: b'm'
            }]
        );
    }

    #[test]
    fn test_colon_rgb() {
        assert_eq!(
            parse_as_vec(b"\x1b[38:2::128:64:192m"),
            vec![VTAction::CsiDispatch {
                params: vec![
                    CsiParam::Integer(38),
                    CsiParam::P(b':'),
                    CsiParam::Integer(2),
                    CsiParam::P(b':'),
                    CsiParam::P(b':'),
                    CsiParam::Integer(128),
                    CsiParam::P(b':'),
                    CsiParam::Integer(64),
                    CsiParam::P(b':'),
                    CsiParam::Integer(192),
                ],
                parameters_truncated: false,
                byte: b'm'
            }]
        );
    }

    #[test]
    fn test_csi_omitted_param() {
        assert_eq!(
            parse_as_vec(b"\x1b[;1m"),
            vec![VTAction::CsiDispatch {
                params: vec![CsiParam::P(b';'), CsiParam::Integer(1)],
                parameters_truncated: false,
                byte: b'm'
            }]
        );
    }

    #[test]
    fn test_csi_too_many_params() {
        assert_eq!(
            parse_as_vec(b"\x1b[0;1;2;3;4;5;6;7;8;9;0;1;2;3;4;51;6p"),
            vec![VTAction::CsiDispatch {
                params: vec![
                    CsiParam::Integer(0),
                    CsiParam::P(b';'),
                    CsiParam::Integer(1),
                    CsiParam::P(b';'),
                    CsiParam::Integer(2),
                    CsiParam::P(b';'),
                    CsiParam::Integer(3),
                    CsiParam::P(b';'),
                    CsiParam::Integer(4),
                    CsiParam::P(b';'),
                    CsiParam::Integer(5),
                    CsiParam::P(b';'),
                    CsiParam::Integer(6),
                    CsiParam::P(b';'),
                    CsiParam::Integer(7),
                    CsiParam::P(b';'),
                    CsiParam::Integer(8),
                    CsiParam::P(b';'),
                    CsiParam::Integer(9),
                    CsiParam::P(b';'),
                    CsiParam::Integer(0),
                    CsiParam::P(b';'),
                    CsiParam::Integer(1),
                    CsiParam::P(b';'),
                    CsiParam::Integer(2),
                    CsiParam::P(b';'),
                    CsiParam::Integer(3),
                    CsiParam::P(b';'),
                    CsiParam::Integer(4),
                    CsiParam::P(b';'),
                    CsiParam::Integer(51),
                    CsiParam::P(b';'),
                ],
                parameters_truncated: false,
                byte: b'p'
            }]
        );
    }

    #[test]
    fn test_csi_intermediates() {
        assert_eq!(
            parse_as_vec(b"\x1b[1 p"),
            vec![VTAction::CsiDispatch {
                params: vec![CsiParam::Integer(1), CsiParam::P(b' ')],
                parameters_truncated: false,
                byte: b'p'
            }]
        );
        assert_eq!(
            parse_as_vec(b"\x1b[1 !p"),
            vec![VTAction::CsiDispatch {
                params: vec![CsiParam::Integer(1), CsiParam::P(b' '), CsiParam::P(b'!')],
                parameters_truncated: false,
                byte: b'p'
            }]
        );
        assert_eq!(
            parse_as_vec(b"\x1b[1 !#p"),
            vec![VTAction::CsiDispatch {
                // Note that the `#` was discarded
                params: vec![CsiParam::Integer(1), CsiParam::P(b' '), CsiParam::P(b'!')],
                parameters_truncated: true,
                byte: b'p'
            }]
        );
    }

    #[test]
    fn osc_utf8() {
        assert_eq!(
            parse_as_vec("\x1b]\u{af}\x07".as_bytes()),
            vec![VTAction::OscDispatch(vec!["\u{af}".as_bytes().to_vec()])]
        );
    }

    #[test]
    fn osc_fedora_vte() {
        assert_eq!(
            parse_as_vec("\u{9d}777;preexec\u{9c}".as_bytes()),
            vec![VTAction::OscDispatch(vec![
                b"777".to_vec(),
                b"preexec".to_vec(),
            ])]
        );
    }

    #[test]
    fn print_utf8() {
        assert_eq!(
            parse_as_vec("\u{af}".as_bytes()),
            vec![VTAction::Print('\u{af}')]
        );
    }

    #[test]
    fn utf8_control() {
        assert_eq!(
            parse_as_vec("\u{8d}".as_bytes()),
            vec![VTAction::ExecuteC0orC1(0x8d)]
        );
    }

    #[test]
    fn tmux_control() {
        assert_eq!(
            parse_as_vec("\x1bP1000phello\x1b\\".as_bytes()),
            vec![
                VTAction::DcsHook {
                    byte: b'p',
                    params: vec![1000],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                },
                VTAction::DcsPut(b'h'),
                VTAction::DcsPut(b'e'),
                VTAction::DcsPut(b'l'),
                VTAction::DcsPut(b'l'),
                VTAction::DcsPut(b'o'),
                VTAction::DcsUnhook,
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\',
                }
            ]
        );
    }

    #[test]
    fn tmux_passthru() {
        // I'm not convinced that we *should* represent this tmux sequence
        // in this way, but it is how it currently maps.
        // It's worth noting that we see this as final byte `t` here, which
        // collides with decVT105G in https://vt100.net/emu/dcsseq_dec.html
        assert_eq!(
            parse_as_vec("\x1bPtmux;data\x1b\\".as_bytes()),
            vec![
                VTAction::DcsHook {
                    byte: b't',
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                },
                VTAction::DcsPut(b'm'),
                VTAction::DcsPut(b'u'),
                VTAction::DcsPut(b'x'),
                VTAction::DcsPut(b';'),
                VTAction::DcsPut(b'd'),
                VTAction::DcsPut(b'a'),
                VTAction::DcsPut(b't'),
                VTAction::DcsPut(b'a'),
                VTAction::DcsUnhook,
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\',
                }
            ]
        );
    }

    #[test]
    fn kitty_img() {
        assert_eq!(
            parse_as_vec("\x1b_Gf=24,s=10,v=20;payload\x1b\\".as_bytes()),
            vec![
                VTAction::ApcDispatch(b"Gf=24,s=10,v=20;payload".to_vec()),
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\',
                }
            ]
        );
    }

    #[test]
    fn sixel() {
        assert_eq!(
            parse_as_vec("\x1bPqhello\x1b\\".as_bytes()),
            vec![
                VTAction::DcsHook {
                    byte: b'q',
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                },
                VTAction::DcsPut(b'h'),
                VTAction::DcsPut(b'e'),
                VTAction::DcsPut(b'l'),
                VTAction::DcsPut(b'l'),
                VTAction::DcsPut(b'o'),
                VTAction::DcsUnhook,
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\',
                }
            ]
        );
    }

    #[test]
    fn test_ommitted_dcs_param() {
        assert_eq!(
            parse_as_vec("\x1bP;1q\x1b\\".as_bytes()),
            vec![
                VTAction::DcsHook {
                    byte: b'q',
                    params: vec![0, 1],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                },
                VTAction::DcsUnhook,
                VTAction::EscDispatch {
                    params: vec![],
                    intermediates: vec![],
                    ignored_excess_intermediates: false,
                    byte: b'\\',
                }
            ]
        );
    }
}
