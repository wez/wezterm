// suppress inscrutable useless_attribute clippy that shows up when
// using derive(FromPrimitive)
#![cfg_attr(feature = "cargo-clippy", allow(clippy::useless_attribute))]
//! This module provides the ability to parse escape sequences and attach
//! semantic meaning to them.  It can also encode the semantic values as
//! escape sequences.  It provides encoding and decoding functionality
//! only; it does not provide terminal emulation facilities itself.
use num_derive::*;
use std::fmt::{Display, Error as FmtError, Formatter, Write as FmtWrite};

pub mod csi;
pub mod esc;
pub mod osc;
pub mod parser;

pub use self::csi::CSI;
pub use self::esc::Esc;
pub use self::esc::EscCode;
pub use self::osc::OperatingSystemCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a single printable character to the display
    Print(char),
    /// A C0 or C1 control code
    Control(ControlCode),
    /// Device control.  This is uncommon wrt. terminal emulation.
    DeviceControl(Box<DeviceControlMode>),
    /// A command that typically doesn't change the contents of the
    /// terminal, but rather influences how it displays or otherwise
    /// interacts with the rest of the system
    OperatingSystemCommand(Box<OperatingSystemCommand>),
    CSI(CSI),
    Esc(Esc),
}

/// Encode self as an escape sequence.  The escape sequence may potentially
/// be clear text with no actual escape sequences.
impl Display for Action {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Action::Print(c) => write!(f, "{}", c),
            Action::Control(c) => f.write_char(*c as u8 as char),
            Action::DeviceControl(_) => unimplemented!(),
            Action::OperatingSystemCommand(osc) => osc.fmt(f),
            Action::CSI(csi) => csi.fmt(f),
            Action::Esc(esc) => esc.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceControlMode {
    /// Identify device control mode from the encoded parameters.
    /// This mode is activated and must remain active until
    /// `Exit` is observed.  While the mode is
    /// active, data is made available to the device mode via
    /// the `Data` variant.
    Enter {
        params: Vec<i64>,
        // TODO: can we just make intermediates a single u8?
        intermediates: Vec<u8>,
        /// if true, more than two intermediates arrived and the
        /// remaining data was ignored
        ignored_extra_intermediates: bool,
    },
    /// Exit the current device control mode
    Exit,
    /// Data for the device mode to consume
    Data(u8),
}

/// C0 or C1 control codes
#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive)]
#[repr(u8)]
pub enum ControlCode {
    Null = 0,
    StartOfHeading = 1,
    StartOfText = 2,
    EndOfText = 3,
    EndOfTransmission = 4,
    Enquiry = 5,
    Acknowledge = 6,
    Bell = 7,
    Backspace = 8,
    HorizontalTab = b'\t',
    LineFeed = b'\n',
    VerticalTab = 0xb,
    FormFeed = 0xc,
    CarriageReturn = b'\r',
    ShiftOut = 0xe,
    ShiftIn = 0xf,
    DataLinkEscape = 0x10,
    DeviceControlOne = 0x11,
    DeviceControlTwo = 0x12,
    DeviceControlThree = 0x13,
    DeviceControlFour = 0x14,
    NegativeAcknowledge = 0x15,
    SynchronousIdle = 0x16,
    EndOfTransmissionBlock = 0x17,
    Cancel = 0x18,
    EndOfMedium = 0x19,
    Substitute = 0x1a,
    Escape = 0x1b,
    FileSeparator = 0x1c,
    GroupSeparator = 0x1d,
    RecordSeparator = 0x1e,
    UnitSeparator = 0x1f,

    // C1 8-bit values
    BPH = 0x82,
    NBH = 0x83,
    NEL = 0x85,
    SSA = 0x86,
    ESA = 0x87,
    HTS = 0x88,
    HTJ = 0x89,
    VTS = 0x8a,
    PLD = 0x8b,
    PLU = 0x8c,
    RI = 0x8d,
    SS2 = 0x8e,
    SS3 = 0x8f,
    DCS = 0x90,
    PU1 = 0x91,
    PU2 = 0x92,
    STS = 0x93,
    CCH = 0x94,
    MW = 0x95,
    SPA = 0x96,
    EPA = 0x97,
    SOS = 0x98,
    SCI = 0x9a,
    CSI = 0x9b,
    ST = 0x9c,
    OSC = 0x9d,
    PM = 0x9e,
    APC = 0x9f,
}

/// A helper type to avoid accidentally tripping over problems with
/// 1-based values in escape sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneBased {
    value: u32,
}

impl OneBased {
    pub fn new(value: u32) -> Self {
        debug_assert!(
            value != 0,
            "programmer error: deliberately assigning zero to a OneBased"
        );
        Self { value }
    }

    pub fn from_zero_based(value: u32) -> Self {
        Self { value: value + 1 }
    }

    /// Map a value from an escape sequence parameter
    pub fn from_esc_param(v: i64) -> Result<Self, ()> {
        if v == 0 {
            Ok(Self { value: num::one() })
        } else if v > 0 && v <= i64::from(u32::max_value()) {
            Ok(Self { value: v as u32 })
        } else {
            Err(())
        }
    }

    /// Map a value from an optional escape sequence parameter
    pub fn from_optional_esc_param(o: Option<&i64>) -> Result<Self, ()> {
        Self::from_esc_param(o.map(|x| *x).unwrap_or(1))
    }

    /// Return the underlying value as a 0-based value
    pub fn as_zero_based(self) -> u32 {
        self.value.saturating_sub(1)
    }

    pub fn as_one_based(self) -> u32 {
        self.value
    }
}

impl Display for OneBased {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        self.value.fmt(f)
    }
}
