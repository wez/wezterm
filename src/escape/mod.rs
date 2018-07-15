//! This module provides the ability to parse escape sequences and attach
//! semantic meaning to them.  It can also encode the semantic values as
//! escape sequences.  It provides encoding and decoding functionality
//! only; it does not provide terminal emulation facilities itself.
use num;
use std;

pub mod csi;
pub mod esc;
pub mod osc;
pub mod parser;

use self::csi::CSI;
use self::esc::Esc;
use self::osc::OperatingSystemCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a single printable character to the display
    Print(char),
    /// A C0 or C1 control code
    Control(Control),
    /// Device control.  This is uncommon wrt. terminal emulation.
    DeviceControl(DeviceControlMode),
    /// A command that typically doesn't change the contents of the
    /// terminal, but rather influences how it displays or otherwise
    /// interacts with the rest of the system
    OperatingSystemCommand(OperatingSystemCommand),
    CSI(CSI),
    Esc(Esc),
}

/// Encode self as an escape sequence.  The escape sequence may potentially
/// be clear text with no actual escape sequences.
pub trait EncodeEscape {
    fn encode_escape<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error>;
}

impl EncodeEscape for Action {
    fn encode_escape<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        match self {
            Action::Print(c) => write!(w, "{}", c),
            Action::Control(Control::Code(c)) => w.write_all(&[c.clone() as u8]),
            Action::Control(Control::Unspecified(c)) => w.write_all(&[*c]),
            Action::DeviceControl(_) => unimplemented!(),
            Action::OperatingSystemCommand(osc) => osc.encode_escape(w),
            Action::CSI(csi) => csi.encode_escape(w),
            Action::Esc(esc) => esc.encode_escape(w),
        }
    }
}

impl<T: EncodeEscape> EncodeEscape for [T] {
    fn encode_escape<W: std::io::Write>(&self, w: &mut W) -> Result<(), std::io::Error> {
        for item in self {
            item.encode_escape(w)?;
        }
        Ok(())
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
#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Code(ControlCode),
    Unspecified(u8),
}

impl From<u8> for Control {
    fn from(b: u8) -> Self {
        match num::FromPrimitive::from_u8(b) {
            Some(result) => Control::Code(result),
            None => Control::Unspecified(b),
        }
    }
}
