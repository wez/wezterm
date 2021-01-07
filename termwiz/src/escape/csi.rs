use super::OneBased;
use crate::cell::{Blink, Intensity, Underline};
use crate::color::{AnsiColor, ColorSpec, RgbColor};
use crate::input::{Modifiers, MouseButtons};
use num_derive::*;
use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt::{Display, Error as FmtError, Formatter};

pub use vtparse::CsiParam;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CSI {
    /// SGR: Set Graphics Rendition.
    /// These values affect how the character is rendered.
    Sgr(Sgr),

    /// CSI codes that relate to the cursor
    Cursor(Cursor),

    Edit(Edit),

    Mode(Mode),

    Device(Box<Device>),

    Mouse(MouseReport),

    Window(Window),

    /// Unknown or unspecified; should be rare and is rather
    /// large, so it is boxed and kept outside of the enum
    /// body to help reduce space usage in the common cases.
    Unspecified(Box<Unspecified>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unspecified {
    pub params: Vec<CsiParam>,
    // TODO: can we just make intermediates a single u8?
    pub intermediates: Vec<u8>,
    /// if true, more than two intermediates arrived and the
    /// remaining data was ignored
    pub ignored_extra_intermediates: bool,
    /// The final character in the CSI sequence; this typically
    /// defines how to interpret the other parameters.
    pub control: char,
}

impl Display for Unspecified {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        for i in &self.intermediates {
            write!(f, "{}", *i as char)?;
        }
        for (idx, p) in self.params.iter().enumerate() {
            if idx > 0 {
                write!(f, ";{}", p)?;
            } else {
                write!(f, "{}", p)?;
            }
        }
        write!(f, "{}", self.control)
    }
}

impl Display for CSI {
    // TODO: data size optimization opportunity: if we could somehow know that we
    // had a run of CSI instances being encoded in sequence, we could
    // potentially collapse them together.  This is a few bytes difference in
    // practice so it may not be worthwhile with modern networks.
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "\x1b[")?;
        match self {
            CSI::Sgr(sgr) => sgr.fmt(f)?,
            CSI::Cursor(c) => c.fmt(f)?,
            CSI::Edit(e) => e.fmt(f)?,
            CSI::Mode(mode) => mode.fmt(f)?,
            CSI::Unspecified(unspec) => unspec.fmt(f)?,
            CSI::Mouse(mouse) => mouse.fmt(f)?,
            CSI::Device(dev) => dev.fmt(f)?,
            CSI::Window(window) => window.fmt(f)?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum CursorStyle {
    Default = 0,
    BlinkingBlock = 1,
    SteadyBlock = 2,
    BlinkingUnderline = 3,
    SteadyUnderline = 4,
    BlinkingBar = 5,
    SteadyBar = 6,
}

impl Default for CursorStyle {
    fn default() -> CursorStyle {
        CursorStyle::Default
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum DeviceAttributeCodes {
    Columns132 = 1,
    Printer = 2,
    RegisGraphics = 3,
    SixelGraphics = 4,
    SelectiveErase = 6,
    UserDefinedKeys = 8,
    NationalReplacementCharsets = 9,
    TechnicalCharacters = 15,
    UserWindows = 18,
    HorizontalScrolling = 21,
    AnsiColor = 22,
    AnsiTextLocator = 29,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceAttribute {
    Code(DeviceAttributeCodes),
    Unspecified(CsiParam),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAttributeFlags {
    pub attributes: Vec<DeviceAttribute>,
}

impl DeviceAttributeFlags {
    fn emit(&self, f: &mut Formatter, leader: &str) -> Result<(), FmtError> {
        write!(f, "{}", leader)?;
        for item in &self.attributes {
            match item {
                DeviceAttribute::Code(c) => write!(f, ";{}", c.to_u16().ok_or_else(|| FmtError)?)?,
                DeviceAttribute::Unspecified(param) => write!(f, ";{}", param)?,
            }
        }
        write!(f, "c")?;
        Ok(())
    }

    pub fn new(attributes: Vec<DeviceAttribute>) -> Self {
        Self { attributes }
    }

    fn from_params(params: &[CsiParam]) -> Self {
        let mut attributes = Vec::new();
        for i in params {
            match i {
                CsiParam::Integer(p) => match FromPrimitive::from_i64(*p) {
                    Some(c) => attributes.push(DeviceAttribute::Code(c)),
                    None => attributes.push(DeviceAttribute::Unspecified(i.clone())),
                },
                _ => attributes.push(DeviceAttribute::Unspecified(i.clone())),
            }
        }
        Self { attributes }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceAttributes {
    Vt100WithAdvancedVideoOption,
    Vt101WithNoOptions,
    Vt102,
    Vt220(DeviceAttributeFlags),
    Vt320(DeviceAttributeFlags),
    Vt420(DeviceAttributeFlags),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Device {
    DeviceAttributes(DeviceAttributes),
    /// DECSTR - https://vt100.net/docs/vt510-rm/DECSTR.html
    SoftReset,
    RequestPrimaryDeviceAttributes,
    RequestSecondaryDeviceAttributes,
    StatusReport,
    /// https://github.com/mintty/mintty/issues/881
    /// https://gitlab.gnome.org/GNOME/vte/-/issues/235
    RequestTerminalNameAndVersion,
}

impl Display for Device {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Device::DeviceAttributes(DeviceAttributes::Vt100WithAdvancedVideoOption) => {
                write!(f, "?1;2c")?
            }
            Device::DeviceAttributes(DeviceAttributes::Vt101WithNoOptions) => write!(f, "?1;0c")?,
            Device::DeviceAttributes(DeviceAttributes::Vt102) => write!(f, "?6c")?,
            Device::DeviceAttributes(DeviceAttributes::Vt220(attr)) => attr.emit(f, "?62")?,
            Device::DeviceAttributes(DeviceAttributes::Vt320(attr)) => attr.emit(f, "?63")?,
            Device::DeviceAttributes(DeviceAttributes::Vt420(attr)) => attr.emit(f, "?64")?,
            Device::SoftReset => write!(f, "!p")?,
            Device::RequestPrimaryDeviceAttributes => write!(f, "c")?,
            Device::RequestSecondaryDeviceAttributes => write!(f, ">c")?,
            Device::RequestTerminalNameAndVersion => write!(f, ">q")?,
            Device::StatusReport => write!(f, "5n")?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Button1Press,
    Button2Press,
    Button3Press,
    Button4Press,
    Button5Press,
    Button1Release,
    Button2Release,
    Button3Release,
    Button4Release,
    Button5Release,
    Button1Drag,
    Button2Drag,
    Button3Drag,
    None,
}

impl From<MouseButton> for MouseButtons {
    fn from(button: MouseButton) -> MouseButtons {
        match button {
            MouseButton::Button1Press | MouseButton::Button1Drag => MouseButtons::LEFT,
            MouseButton::Button2Press | MouseButton::Button2Drag => MouseButtons::MIDDLE,
            MouseButton::Button3Press | MouseButton::Button3Drag => MouseButtons::RIGHT,
            MouseButton::Button4Press => MouseButtons::VERT_WHEEL | MouseButtons::WHEEL_POSITIVE,
            MouseButton::Button5Press => MouseButtons::VERT_WHEEL,
            _ => MouseButtons::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Window {
    DeIconify,
    Iconify,
    MoveWindow {
        x: i64,
        y: i64,
    },
    ResizeWindowPixels {
        width: Option<i64>,
        height: Option<i64>,
    },
    RaiseWindow,
    LowerWindow,
    RefreshWindow,
    ResizeWindowCells {
        width: Option<i64>,
        height: Option<i64>,
    },
    RestoreMaximizedWindow,
    MaximizeWindow,
    MaximizeWindowVertically,
    MaximizeWindowHorizontally,
    UndoFullScreenMode,
    ChangeToFullScreenMode,
    ToggleFullScreen,
    ReportWindowState,
    ReportWindowPosition,
    ReportTextAreaPosition,
    ReportTextAreaSizePixels,
    ReportWindowSizePixels,
    ReportScreenSizePixels,
    ReportCellSizePixels,
    ReportCellSizePixelsResponse {
        width: Option<i64>,
        height: Option<i64>,
    },
    ReportTextAreaSizeCells,
    ReportScreenSizeCells,
    ReportIconLabel,
    ReportWindowTitle,
    PushIconAndWindowTitle,
    PushIconTitle,
    PushWindowTitle,
    PopIconAndWindowTitle,
    PopIconTitle,
    PopWindowTitle,
    /// DECRQCRA; used by esctest
    ChecksumRectangularArea {
        request_id: i64,
        page_number: i64,
        top: OneBased,
        left: OneBased,
        bottom: OneBased,
        right: OneBased,
    },
}

fn numstr_or_empty(x: &Option<i64>) -> String {
    match x {
        Some(x) => format!("{}", x),
        None => "".to_owned(),
    }
}

impl Display for Window {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Window::DeIconify => write!(f, "1t"),
            Window::Iconify => write!(f, "2t"),
            Window::MoveWindow { x, y } => write!(f, "3;{};{}t", x, y),
            Window::ResizeWindowPixels { width, height } => write!(
                f,
                "4;{};{}t",
                numstr_or_empty(height),
                numstr_or_empty(width),
            ),
            Window::RaiseWindow => write!(f, "5t"),
            Window::LowerWindow => write!(f, "6t"),
            Window::RefreshWindow => write!(f, "7t"),
            Window::ResizeWindowCells { width, height } => write!(
                f,
                "8;{};{}t",
                numstr_or_empty(height),
                numstr_or_empty(width),
            ),
            Window::RestoreMaximizedWindow => write!(f, "9;0t"),
            Window::MaximizeWindow => write!(f, "9;1t"),
            Window::MaximizeWindowVertically => write!(f, "9;2t"),
            Window::MaximizeWindowHorizontally => write!(f, "9;3t"),
            Window::UndoFullScreenMode => write!(f, "10;0t"),
            Window::ChangeToFullScreenMode => write!(f, "10;1t"),
            Window::ToggleFullScreen => write!(f, "10;2t"),
            Window::ReportWindowState => write!(f, "11t"),
            Window::ReportWindowPosition => write!(f, "13t"),
            Window::ReportTextAreaPosition => write!(f, "13;2t"),
            Window::ReportTextAreaSizePixels => write!(f, "14t"),
            Window::ReportWindowSizePixels => write!(f, "14;2t"),
            Window::ReportScreenSizePixels => write!(f, "15t"),
            Window::ReportCellSizePixels => write!(f, "16t"),
            Window::ReportCellSizePixelsResponse { width, height } => write!(
                f,
                "6;{};{}t",
                numstr_or_empty(height),
                numstr_or_empty(width),
            ),
            Window::ReportTextAreaSizeCells => write!(f, "18t"),
            Window::ReportScreenSizeCells => write!(f, "19t"),
            Window::ReportIconLabel => write!(f, "20t"),
            Window::ReportWindowTitle => write!(f, "21t"),
            Window::PushIconAndWindowTitle => write!(f, "22;0t"),
            Window::PushIconTitle => write!(f, "22;1t"),
            Window::PushWindowTitle => write!(f, "22;2t"),
            Window::PopIconAndWindowTitle => write!(f, "23;0t"),
            Window::PopIconTitle => write!(f, "23;1t"),
            Window::PopWindowTitle => write!(f, "23;2t"),
            Window::ChecksumRectangularArea {
                request_id,
                page_number,
                top,
                left,
                bottom,
                right,
            } => write!(
                f,
                "{};{};{};{};{};{}*y",
                request_id, page_number, top, left, bottom, right,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseReport {
    SGR1006 {
        x: u16,
        y: u16,
        button: MouseButton,
        modifiers: Modifiers,
    },
}

impl Display for MouseReport {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            MouseReport::SGR1006 {
                x,
                y,
                button,
                modifiers,
            } => {
                let mut b = 0;
                if (*modifiers & Modifiers::SHIFT) != Modifiers::NONE {
                    b |= 4;
                }
                if (*modifiers & Modifiers::ALT) != Modifiers::NONE {
                    b |= 8;
                }
                if (*modifiers & Modifiers::CTRL) != Modifiers::NONE {
                    b |= 16;
                }
                b |= match button {
                    MouseButton::Button1Press | MouseButton::Button1Release => 0,
                    MouseButton::Button2Press | MouseButton::Button2Release => 1,
                    MouseButton::Button3Press | MouseButton::Button3Release => 2,
                    MouseButton::Button4Press | MouseButton::Button4Release => 64,
                    MouseButton::Button5Press | MouseButton::Button5Release => 65,
                    MouseButton::Button1Drag => 32,
                    MouseButton::Button2Drag => 33,
                    MouseButton::Button3Drag => 34,
                    MouseButton::None => 35,
                };
                let trailer = match button {
                    MouseButton::Button1Press
                    | MouseButton::Button2Press
                    | MouseButton::Button3Press
                    | MouseButton::Button4Press
                    | MouseButton::Button5Press
                    | MouseButton::Button1Drag
                    | MouseButton::Button2Drag
                    | MouseButton::Button3Drag
                    | MouseButton::None => 'M',
                    _ => 'm',
                };
                write!(f, "<{};{};{}{}", b, x, y, trailer)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XtermKeyModifierResource {
    Keyboard,
    CursorKeys,
    FunctionKeys,
    OtherKeys,
}

impl XtermKeyModifierResource {
    pub fn parse(value: i64) -> Option<Self> {
        Some(match value {
            0 => XtermKeyModifierResource::Keyboard,
            1 => XtermKeyModifierResource::CursorKeys,
            2 => XtermKeyModifierResource::FunctionKeys,
            4 => XtermKeyModifierResource::OtherKeys,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    SetDecPrivateMode(DecPrivateMode),
    ResetDecPrivateMode(DecPrivateMode),
    SaveDecPrivateMode(DecPrivateMode),
    RestoreDecPrivateMode(DecPrivateMode),
    SetMode(TerminalMode),
    ResetMode(TerminalMode),
    XtermKeyMode {
        resource: XtermKeyModifierResource,
        value: Option<i64>,
    },
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        macro_rules! emit {
            ($flag:expr, $mode:expr) => {{
                let value = match $mode {
                    DecPrivateMode::Code(mode) => mode.to_u16().ok_or_else(|| FmtError)?,
                    DecPrivateMode::Unspecified(mode) => *mode,
                };
                write!(f, "?{}{}", value, $flag)
            }};
        }
        macro_rules! emit_mode {
            ($flag:expr, $mode:expr) => {{
                let value = match $mode {
                    TerminalMode::Code(mode) => mode.to_u16().ok_or_else(|| FmtError)?,
                    TerminalMode::Unspecified(mode) => *mode,
                };
                write!(f, "?{}{}", value, $flag)
            }};
        }
        match self {
            Mode::SetDecPrivateMode(mode) => emit!("h", mode),
            Mode::ResetDecPrivateMode(mode) => emit!("l", mode),
            Mode::SaveDecPrivateMode(mode) => emit!("s", mode),
            Mode::RestoreDecPrivateMode(mode) => emit!("r", mode),
            Mode::SetMode(mode) => emit_mode!("h", mode),
            Mode::ResetMode(mode) => emit_mode!("l", mode),
            Mode::XtermKeyMode { resource, value } => {
                write!(
                    f,
                    ">{}",
                    match resource {
                        XtermKeyModifierResource::Keyboard => 0,
                        XtermKeyModifierResource::CursorKeys => 1,
                        XtermKeyModifierResource::FunctionKeys => 2,
                        XtermKeyModifierResource::OtherKeys => 4,
                    }
                )?;
                if let Some(value) = value {
                    write!(f, ";{}", value)?;
                }
                write!(f, "m")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecPrivateMode {
    Code(DecPrivateModeCode),
    Unspecified(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum DecPrivateModeCode {
    /// https://vt100.net/docs/vt510-rm/DECCKM.html
    /// This mode is only effective when the terminal is in keypad application mode (see DECKPAM)
    /// and the ANSI/VT52 mode (DECANM) is set (see DECANM). Under these conditions, if the cursor
    /// key mode is reset, the four cursor function keys will send ANSI cursor control commands. If
    /// cursor key mode is set, the four cursor function keys will send application functions.
    ApplicationCursorKeys = 1,

    /// https://vt100.net/docs/vt510-rm/DECANM.html
    /// Behave like a vt52
    DecAnsiMode = 2,

    /// https://vt100.net/docs/vt510-rm/DECCOLM.html
    Select132Columns = 3,
    /// https://vt100.net/docs/vt510-rm/DECSCLM.html
    SmoothScroll = 4,
    /// https://vt100.net/docs/vt510-rm/DECSCNM.html
    ReverseVideo = 5,
    /// https://vt100.net/docs/vt510-rm/DECOM.html
    /// When OriginMode is enabled, cursor is constrained to the
    /// scroll region and its position is relative to the scroll
    /// region.
    OriginMode = 6,
    /// https://vt100.net/docs/vt510-rm/DECAWM.html
    /// When enabled, wrap to next line, Otherwise replace the last
    /// character
    AutoWrap = 7,
    /// https://vt100.net/docs/vt510-rm/DECARM.html
    AutoRepeat = 8,
    StartBlinkingCursor = 12,
    ShowCursor = 25,

    ReverseWraparound = 45,

    /// https://vt100.net/docs/vt510-rm/DECLRMM.html
    LeftRightMarginMode = 69,

    /// DECSDM - https://vt100.net/docs/vt3xx-gp/chapter14.html
    SixelScrolling = 80,
    /// Enable mouse button press/release reporting
    MouseTracking = 1000,
    /// Warning: this requires a cooperative and timely response from
    /// the application otherwise the terminal can hang
    HighlightMouseTracking = 1001,
    /// Enable mouse button press/release and drag reporting
    ButtonEventMouse = 1002,
    /// Enable mouse motion, button press/release and drag reporting
    AnyEventMouse = 1003,
    /// Enable FocusIn/FocusOut events
    FocusTracking = 1004,
    /// Use extended coordinate system in mouse reporting.  Does not
    /// enable mouse reporting itself, it just controls how reports
    /// will be encoded.
    SGRMouse = 1006,
    /// Save cursor as in DECSC
    SaveCursor = 1048,
    ClearAndEnableAlternateScreen = 1049,
    EnableAlternateScreen = 47,
    OptEnableAlternateScreen = 1047,
    BracketedPaste = 2004,
    /// Applies to sixel and regis modes
    UsePrivateColorRegistersForEachGraphic = 1070,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalMode {
    Code(TerminalModeCode),
    Unspecified(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum TerminalModeCode {
    /// https://vt100.net/docs/vt510-rm/KAM.html
    KeyboardAction = 2,
    /// https://vt100.net/docs/vt510-rm/IRM.html
    Insert = 4,
    /// https://vt100.net/docs/vt510-rm/SRM.html
    /// But in the MS terminal this is cursor blinking.
    SendReceive = 12,
    /// https://vt100.net/docs/vt510-rm/LNM.html
    AutomaticNewline = 20,
    /// MS terminal cursor visibility
    ShowCursor = 25,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    /// CBT Moves cursor to the Ps tabs backward. The default value of Ps is 1.
    BackwardTabulation(u32),

    /// TBC - TABULATION CLEAR
    TabulationClear(TabulationClear),

    /// CHA: Moves cursor to the Ps-th column of the active line. The default
    /// value of Ps is 1.
    CharacterAbsolute(OneBased),

    /// HPA CHARACTER POSITION ABSOLUTE
    /// HPA Moves cursor to the Ps-th column of the active line. The default
    /// value of Ps is 1.
    CharacterPositionAbsolute(OneBased),

    /// HPB - CHARACTER POSITION BACKWARD
    /// HPB Moves cursor to the left Ps columns. The default value of Ps is 1.
    CharacterPositionBackward(u32),

    /// HPR - CHARACTER POSITION FORWARD
    /// HPR Moves cursor to the right Ps columns. The default value of Ps is 1.
    CharacterPositionForward(u32),

    /// HVP - CHARACTER AND LINE POSITION
    /// HVP Moves cursor to the Ps1-th line and to the Ps2-th column. The
    /// default value of Ps1 and Ps2 is 1.
    CharacterAndLinePosition {
        line: OneBased,
        col: OneBased,
    },

    /// VPA - LINE POSITION ABSOLUTE
    /// Move to the corresponding vertical position (line Ps) of the current
    /// column. The default value of Ps is 1.
    LinePositionAbsolute(u32),

    /// VPB - LINE POSITION BACKWARD
    /// Moves cursor up Ps lines in the same column. The default value of Ps is
    /// 1.
    LinePositionBackward(u32),

    /// VPR - LINE POSITION FORWARD
    /// Moves cursor down Ps lines in the same column. The default value of Ps
    /// is 1.
    LinePositionForward(u32),

    /// CHT
    /// Moves cursor to the Ps tabs forward. The default value of Ps is 1.
    ForwardTabulation(u32),

    /// CNL Moves cursor to the first column of Ps-th following line. The
    /// default value of Ps is 1.
    NextLine(u32),

    /// CPL Moves cursor to the first column of Ps-th preceding line. The
    /// default value of Ps is 1.
    PrecedingLine(u32),

    /// CPR - ACTIVE POSITION REPORT
    /// If the DEVICE COMPONENT SELECT MODE (DCSM)
    /// is set to PRESENTATION, CPR is used to report the active presentation
    /// position of the sending device as residing in the presentation
    /// component at the n-th line position according to the line progression
    /// and at the m-th character position according to the character path,
    /// where n equals the value of Pn1 and m equal s the value of Pn2.
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to DATA, CPR is used
    /// to report the active data position of the sending device as
    /// residing in the data component at the n-th line position according
    /// to the line progression and at the m-th character position
    /// according to the character progression, where n equals the value of
    /// Pn1 and m equals the value of Pn2. CPR may be solicited by a DEVICE
    /// STATUS REPORT (DSR) or be sent unsolicited .
    ActivePositionReport {
        line: OneBased,
        col: OneBased,
    },

    /// CPR: this is the request from the client.
    /// The terminal will respond with ActivePositionReport.
    RequestActivePositionReport,

    /// SCP - Save Cursor Position.
    /// Only works when DECLRMM is disabled
    SaveCursor,
    RestoreCursor,

    /// CTC - CURSOR TABULATION CONTROL
    /// CTC causes one or more tabulation stops to be set or cleared in the
    /// presentation component, depending on the parameter values.
    /// In the case of parameter values 0, 2 or 4 the number of lines affected
    /// depends on the setting of the TABULATION STOP MODE (TSM).
    TabulationControl(CursorTabulationControl),

    /// CUB - Cursor Left
    /// Moves cursor to the left Ps columns. The default value of Ps is 1.
    Left(u32),

    /// CUD - Cursor Down
    Down(u32),

    /// CUF - Cursor Right
    Right(u32),

    /// CUP - Cursor Position
    /// Moves cursor to the Ps1-th line and to the Ps2-th column. The default
    /// value of Ps1 and Ps2 is 1.
    Position {
        line: OneBased,
        col: OneBased,
    },

    /// CUU - Cursor Up
    Up(u32),

    /// CVT - Cursor Line Tabulation
    /// CVT causes the active presentation position to be moved to the
    /// corresponding character position of the line corresponding to the n-th
    /// following line tabulation stop in the presentation component, where n
    /// equals the value of Pn.
    LineTabulation(u32),

    /// DECSTBM - Set top and bottom margins.
    SetTopAndBottomMargins {
        top: OneBased,
        bottom: OneBased,
    },

    /// https://vt100.net/docs/vt510-rm/DECSLRM.html
    SetLeftAndRightMargins {
        left: OneBased,
        right: OneBased,
    },

    CursorStyle(CursorStyle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// DCH - DELETE CHARACTER
    /// Deletes Ps characters from the cursor position to the right. The
    /// default value of Ps is 1. If the DEVICE COMPONENT SELECT MODE
    /// (DCSM) is set to PRESENTATION, DCH causes the contents of the
    /// active presentation position and, depending on the setting of the
    /// CHARACTER EDITING MODE (HEM), the contents of the n-1 preceding or
    /// following character positions to be removed from the presentation
    /// component, where n equals the value of Pn. The resulting gap is
    /// closed by shifting the contents of the adjacent character positions
    /// towards the active presentation position. At the other end of the
    /// shifted part, n character positions are put into the erased state.
    DeleteCharacter(u32),

    /// DL - DELETE LINE
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, DL
    /// causes the contents of the active line (the line that contains the
    /// active presentation position) and, depending on the setting of the
    /// LINE EDITING MODE (VEM), the contents of the n-1 preceding or
    /// following lines to be removed from the presentation component, where n
    /// equals the value of Pn. The resulting gap is closed by shifting the
    /// contents of a number of adjacent lines towards the active line. At
    /// the other end of the shifted part, n lines are put into the
    /// erased state.  The active presentation position is moved to the line
    /// home position in the active line. The line home position is
    /// established by the parameter value of SET LINE HOME (SLH). If the
    /// TABULATION STOP MODE (TSM) is set to SINGLE, character tabulation stops
    /// are cleared in the lines that are put into the erased state.  The
    /// extent of the shifted part is established by SELECT EDITING EXTENT
    /// (SEE).  Any occurrences of the start or end of a selected area, the
    /// start or end of a qualified area, or a tabulation stop in the shifted
    /// part, are also shifted.
    DeleteLine(u32),

    /// ECH - ERASE CHARACTER
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, ECH
    /// causes the active presentation position and the n-1 following
    /// character positions in the presentation component to be put into
    /// the erased state, where n equals the value of Pn.
    EraseCharacter(u32),

    /// EL - ERASE IN LINE
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, EL
    /// causes some or all character positions of the active line (the line
    /// which contains the active presentation position in the presentation
    /// component) to be put into the erased state, depending on the
    /// parameter values
    EraseInLine(EraseInLine),

    /// ICH - INSERT CHARACTER
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, ICH
    /// is used to prepare the insertion of n characters, by putting into the
    /// erased state the active presentation position and, depending on the
    /// setting of the CHARACTER EDITING MODE (HEM), the n-1 preceding or
    /// following character positions in the presentation component, where n
    /// equals the value of Pn. The previous contents of the active
    /// presentation position and an adjacent string of character positions are
    /// shifted away from the active presentation position. The contents of n
    /// character positions at the other end of the shifted part are removed.
    /// The active presentation position is moved to the line home position in
    /// the active line. The line home position is established by the parameter
    /// value of SET LINE HOME (SLH).
    InsertCharacter(u32),

    /// IL - INSERT LINE
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, IL
    /// is used to prepare the insertion of n lines, by putting into the
    /// erased state in the presentation component the active line (the
    /// line that contains the active presentation position) and, depending on
    /// the setting of the LINE EDITING MODE (VEM), the n-1 preceding or
    /// following lines, where n equals the value of Pn. The previous
    /// contents of the active line and of adjacent lines are shifted away
    /// from the active line. The contents of n lines at the other end of the
    /// shifted part are removed. The active presentation position is moved
    /// to the line home position in the active line. The line home
    /// position is established by the parameter value of SET LINE
    /// HOME (SLH).
    InsertLine(u32),

    /// SD - SCROLL DOWN
    /// SD causes the data in the presentation component to be moved by n line
    /// positions if the line orientation is horizontal, or by n character
    /// positions if the line orientation is vertical, such that the data
    /// appear to move down; where n equals the value of Pn. The active
    /// presentation position is not affected by this control function.
    ///
    /// Also known as Pan Up in DEC:
    /// https://vt100.net/docs/vt510-rm/SD.html
    ScrollDown(u32),

    /// SU - SCROLL UP
    /// SU causes the data in the presentation component to be moved by n line
    /// positions if the line orientation is horizontal, or by n character
    /// positions if the line orientation is vertical, such that the data
    /// appear to move up; where n equals the value of Pn. The active
    /// presentation position is not affected by this control function.
    ScrollUp(u32),

    /// ED - ERASE IN PAGE (XTerm calls this Erase in Display)
    EraseInDisplay(EraseInDisplay),

    /// REP - Repeat the preceding character n times
    Repeat(u32),
}

trait EncodeCSIParam {
    fn write_csi(&self, f: &mut Formatter, control: &str) -> Result<(), FmtError>;
}

impl<T: ParamEnum + PartialEq + ToPrimitive> EncodeCSIParam for T {
    fn write_csi(&self, f: &mut Formatter, control: &str) -> Result<(), FmtError> {
        if *self == ParamEnum::default() {
            write!(f, "{}", control)
        } else {
            let value = self.to_i64().ok_or_else(|| FmtError)?;
            write!(f, "{}{}", value, control)
        }
    }
}

impl EncodeCSIParam for u32 {
    fn write_csi(&self, f: &mut Formatter, control: &str) -> Result<(), FmtError> {
        if *self == 1 {
            write!(f, "{}", control)
        } else {
            write!(f, "{}{}", *self, control)
        }
    }
}

impl EncodeCSIParam for OneBased {
    fn write_csi(&self, f: &mut Formatter, control: &str) -> Result<(), FmtError> {
        if self.as_one_based() == 1 {
            write!(f, "{}", control)
        } else {
            write!(f, "{}{}", *self, control)
        }
    }
}

impl Display for Edit {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Edit::DeleteCharacter(n) => n.write_csi(f, "P")?,
            Edit::DeleteLine(n) => n.write_csi(f, "M")?,
            Edit::EraseCharacter(n) => n.write_csi(f, "X")?,
            Edit::EraseInLine(n) => n.write_csi(f, "K")?,
            Edit::InsertCharacter(n) => n.write_csi(f, "@")?,
            Edit::InsertLine(n) => n.write_csi(f, "L")?,
            Edit::ScrollDown(n) => n.write_csi(f, "T")?,
            Edit::ScrollUp(n) => n.write_csi(f, "S")?,
            Edit::EraseInDisplay(n) => n.write_csi(f, "J")?,
            Edit::Repeat(n) => n.write_csi(f, "b")?,
        }
        Ok(())
    }
}

impl Display for Cursor {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Cursor::BackwardTabulation(n) => n.write_csi(f, "Z")?,
            Cursor::CharacterAbsolute(col) => col.write_csi(f, "G")?,
            Cursor::ForwardTabulation(n) => n.write_csi(f, "I")?,
            Cursor::NextLine(n) => n.write_csi(f, "E")?,
            Cursor::PrecedingLine(n) => n.write_csi(f, "F")?,
            Cursor::ActivePositionReport { line, col } => write!(f, "{};{}R", line, col)?,
            Cursor::Left(n) => n.write_csi(f, "D")?,
            Cursor::Down(n) => n.write_csi(f, "B")?,
            Cursor::Right(n) => n.write_csi(f, "C")?,
            Cursor::Up(n) => n.write_csi(f, "A")?,
            Cursor::Position { line, col } => write!(f, "{};{}H", line, col)?,
            Cursor::LineTabulation(n) => n.write_csi(f, "Y")?,
            Cursor::TabulationControl(n) => n.write_csi(f, "W")?,
            Cursor::TabulationClear(n) => n.write_csi(f, "g")?,
            Cursor::CharacterPositionAbsolute(n) => n.write_csi(f, "`")?,
            Cursor::CharacterPositionBackward(n) => n.write_csi(f, "j")?,
            Cursor::CharacterPositionForward(n) => n.write_csi(f, "a")?,
            Cursor::CharacterAndLinePosition { line, col } => write!(f, "{};{}f", line, col)?,
            Cursor::LinePositionAbsolute(n) => n.write_csi(f, "d")?,
            Cursor::LinePositionBackward(n) => n.write_csi(f, "k")?,
            Cursor::LinePositionForward(n) => n.write_csi(f, "e")?,
            Cursor::SetTopAndBottomMargins { top, bottom } => {
                if top.as_one_based() == 1 && bottom.as_one_based() == u32::max_value() {
                    write!(f, "r")?;
                } else {
                    write!(f, "{};{}r", top, bottom)?;
                }
            }
            Cursor::SetLeftAndRightMargins { left, right } => {
                if left.as_one_based() == 1 && right.as_one_based() == u32::max_value() {
                    write!(f, "s")?;
                } else {
                    write!(f, "{};{}s", left, right)?;
                }
            }
            Cursor::RequestActivePositionReport => write!(f, "6n")?,
            Cursor::SaveCursor => write!(f, "s")?,
            Cursor::RestoreCursor => write!(f, "u")?,
            Cursor::CursorStyle(style) => write!(f, "{} q", *style as u8)?,
        }
        Ok(())
    }
}

/// This trait aids in parsing escape sequences.
/// In many cases we simply want to collect integral values >= 1,
/// but in some we build out an enum.  The trait helps to generalize
/// the parser code while keeping it relatively terse.
trait ParseParams: Sized {
    fn parse_params(params: &[CsiParam]) -> Result<Self, ()>;
}

/// Parse an input parameter into a 1-based unsigned value
impl ParseParams for u32 {
    fn parse_params(params: &[CsiParam]) -> Result<u32, ()> {
        if params.is_empty() {
            Ok(1)
        } else if params.len() == 1 {
            to_1b_u32(&params[0])
        } else {
            Err(())
        }
    }
}

/// Parse an input parameter into a 1-based unsigned value
impl ParseParams for OneBased {
    fn parse_params(params: &[CsiParam]) -> Result<OneBased, ()> {
        if params.is_empty() {
            Ok(OneBased::new(1))
        } else if params.len() == 1 {
            OneBased::from_esc_param(&params[0])
        } else {
            Err(())
        }
    }
}

/// Parse a pair of 1-based unsigned values into a tuple.
/// This is typically used to build a struct comprised of
/// the pair of values.
impl ParseParams for (OneBased, OneBased) {
    fn parse_params(params: &[CsiParam]) -> Result<(OneBased, OneBased), ()> {
        if params.is_empty() {
            Ok((OneBased::new(1), OneBased::new(1)))
        } else if params.len() == 1 {
            Ok((OneBased::from_esc_param(&params[0])?, OneBased::new(1)))
        } else if params.len() == 2 {
            Ok((
                OneBased::from_esc_param(&params[0])?,
                OneBased::from_esc_param(&params[1])?,
            ))
        } else {
            Err(())
        }
    }
}

/// This is ostensibly a marker trait that is used within this module
/// to denote an enum.  It does double duty as a stand-in for Default.
/// We need separate traits for this to disambiguate from a regular
/// primitive integer.
trait ParamEnum: FromPrimitive {
    fn default() -> Self;
}

/// implement ParseParams for the enums that also implement ParamEnum.
impl<T: ParamEnum> ParseParams for T {
    fn parse_params(params: &[CsiParam]) -> Result<Self, ()> {
        if params.is_empty() {
            Ok(ParamEnum::default())
        } else if params.len() == 1 {
            match params[0] {
                CsiParam::Integer(i) => FromPrimitive::from_i64(i).ok_or(()),
                CsiParam::ColonList(_) => Err(()),
            }
        } else {
            Err(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, Copy, ToPrimitive)]
pub enum CursorTabulationControl {
    SetCharacterTabStopAtActivePosition = 0,
    SetLineTabStopAtActiveLine = 1,
    ClearCharacterTabStopAtActivePosition = 2,
    ClearLineTabstopAtActiveLine = 3,
    ClearAllCharacterTabStopsAtActiveLine = 4,
    ClearAllCharacterTabStops = 5,
    ClearAllLineTabStops = 6,
}

impl ParamEnum for CursorTabulationControl {
    fn default() -> Self {
        CursorTabulationControl::SetCharacterTabStopAtActivePosition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, Copy, ToPrimitive)]
pub enum TabulationClear {
    ClearCharacterTabStopAtActivePosition = 0,
    ClearLineTabStopAtActiveLine = 1,
    ClearCharacterTabStopsAtActiveLine = 2,
    ClearAllCharacterTabStops = 3,
    ClearAllLineTabStops = 4,
    ClearAllTabStops = 5,
}

impl ParamEnum for TabulationClear {
    fn default() -> Self {
        TabulationClear::ClearCharacterTabStopAtActivePosition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, Copy, ToPrimitive)]
pub enum EraseInLine {
    EraseToEndOfLine = 0,
    EraseToStartOfLine = 1,
    EraseLine = 2,
}

impl ParamEnum for EraseInLine {
    fn default() -> Self {
        EraseInLine::EraseToEndOfLine
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive, Copy, ToPrimitive)]
pub enum EraseInDisplay {
    /// the active presentation position and the character positions up to the
    /// end of the page are put into the erased state
    EraseToEndOfDisplay = 0,
    /// the character positions from the beginning of the page up to and
    /// including the active presentation position are put into the erased
    /// state
    EraseToStartOfDisplay = 1,
    /// all character positions of the page are put into the erased state
    EraseDisplay = 2,
    /// Clears the scrollback.  This is an Xterm extension to ECMA-48.
    EraseScrollback = 3,
}

impl ParamEnum for EraseInDisplay {
    fn default() -> Self {
        EraseInDisplay::EraseToEndOfDisplay
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sgr {
    /// Resets rendition to defaults.  Typically switches off
    /// all other Sgr options, but may have greater or lesser impact.
    Reset,
    /// Set the intensity/bold level
    Intensity(Intensity),
    Underline(Underline),
    UnderlineColor(ColorSpec),
    Blink(Blink),
    Italic(bool),
    Inverse(bool),
    Invisible(bool),
    StrikeThrough(bool),
    Font(Font),
    Foreground(ColorSpec),
    Background(ColorSpec),
    Overline(bool),
}

impl Display for Sgr {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        macro_rules! code {
            ($t:ident) => {
                write!(f, "{}m", SgrCode::$t as i64)?
            };
        }

        macro_rules! ansi_color {
            ($idx:expr, $eightbit:ident, $( ($Ansi:ident, $code:ident) ),*) => {
                if let Some(ansi) = FromPrimitive::from_u8($idx) {
                    match ansi {
                        $(AnsiColor::$Ansi => code!($code) ,)*
                    }
                } else {
                    write!(f, "{}:5:{}m", SgrCode::$eightbit as i64, $idx)?
                }
            }
        }

        match self {
            Sgr::Reset => code!(Reset),
            Sgr::Intensity(Intensity::Bold) => code!(IntensityBold),
            Sgr::Intensity(Intensity::Half) => code!(IntensityDim),
            Sgr::Intensity(Intensity::Normal) => code!(NormalIntensity),
            Sgr::Underline(Underline::Single) => code!(UnderlineOn),
            Sgr::Underline(Underline::Double) => code!(UnderlineDouble),
            Sgr::Underline(Underline::Curly) => write!(f, "4:3m")?,
            Sgr::Underline(Underline::Dotted) => write!(f, "4:4m")?,
            Sgr::Underline(Underline::Dashed) => write!(f, "4:5m")?,
            Sgr::Underline(Underline::None) => code!(UnderlineOff),
            Sgr::Blink(Blink::Slow) => code!(BlinkOn),
            Sgr::Blink(Blink::Rapid) => code!(RapidBlinkOn),
            Sgr::Blink(Blink::None) => code!(BlinkOff),
            Sgr::Italic(true) => code!(ItalicOn),
            Sgr::Italic(false) => code!(ItalicOff),
            Sgr::Inverse(true) => code!(InverseOn),
            Sgr::Inverse(false) => code!(InverseOff),
            Sgr::Invisible(true) => code!(InvisibleOn),
            Sgr::Invisible(false) => code!(InvisibleOff),
            Sgr::StrikeThrough(true) => code!(StrikeThroughOn),
            Sgr::StrikeThrough(false) => code!(StrikeThroughOff),
            Sgr::Overline(true) => code!(OverlineOn),
            Sgr::Overline(false) => code!(OverlineOff),
            Sgr::Font(Font::Default) => code!(DefaultFont),
            Sgr::Font(Font::Alternate(1)) => code!(AltFont1),
            Sgr::Font(Font::Alternate(2)) => code!(AltFont2),
            Sgr::Font(Font::Alternate(3)) => code!(AltFont3),
            Sgr::Font(Font::Alternate(4)) => code!(AltFont4),
            Sgr::Font(Font::Alternate(5)) => code!(AltFont5),
            Sgr::Font(Font::Alternate(6)) => code!(AltFont6),
            Sgr::Font(Font::Alternate(7)) => code!(AltFont7),
            Sgr::Font(Font::Alternate(8)) => code!(AltFont8),
            Sgr::Font(Font::Alternate(9)) => code!(AltFont9),
            Sgr::Font(_) => { /* there are no other possible font values */ }
            Sgr::Foreground(ColorSpec::Default) => code!(ForegroundDefault),
            Sgr::Background(ColorSpec::Default) => code!(BackgroundDefault),
            Sgr::Foreground(ColorSpec::PaletteIndex(idx)) => ansi_color!(
                *idx,
                ForegroundColor,
                (Black, ForegroundBlack),
                (Maroon, ForegroundRed),
                (Green, ForegroundGreen),
                (Olive, ForegroundYellow),
                (Navy, ForegroundBlue),
                (Purple, ForegroundMagenta),
                (Teal, ForegroundCyan),
                (Silver, ForegroundWhite),
                // Note: these brights are emitted using codes in the 100 range.
                // I don't know how portable this is vs. the 256 color sequences,
                // so we may need to make an adjustment here later.
                (Grey, ForegroundBrightBlack),
                (Red, ForegroundBrightRed),
                (Lime, ForegroundBrightGreen),
                (Yellow, ForegroundBrightYellow),
                (Blue, ForegroundBrightBlue),
                (Fuschia, ForegroundBrightMagenta),
                (Aqua, ForegroundBrightCyan),
                (White, ForegroundBrightWhite)
            ),
            Sgr::Foreground(ColorSpec::TrueColor(c)) => write!(
                f,
                "{}:2::{}:{}:{}m",
                SgrCode::ForegroundColor as i64,
                c.red,
                c.green,
                c.blue
            )?,
            Sgr::Background(ColorSpec::PaletteIndex(idx)) => ansi_color!(
                *idx,
                BackgroundColor,
                (Black, BackgroundBlack),
                (Maroon, BackgroundRed),
                (Green, BackgroundGreen),
                (Olive, BackgroundYellow),
                (Navy, BackgroundBlue),
                (Purple, BackgroundMagenta),
                (Teal, BackgroundCyan),
                (Silver, BackgroundWhite),
                // Note: these brights are emitted using codes in the 100 range.
                // I don't know how portable this is vs. the 256 color sequences,
                // so we may need to make an adjustment here later.
                (Grey, BackgroundBrightBlack),
                (Red, BackgroundBrightRed),
                (Lime, BackgroundBrightGreen),
                (Yellow, BackgroundBrightYellow),
                (Blue, BackgroundBrightBlue),
                (Fuschia, BackgroundBrightMagenta),
                (Aqua, BackgroundBrightCyan),
                (White, BackgroundBrightWhite)
            ),
            Sgr::Background(ColorSpec::TrueColor(c)) => write!(
                f,
                "{}:2::{}:{}:{}m",
                SgrCode::BackgroundColor as i64,
                c.red,
                c.green,
                c.blue
            )?,
            Sgr::UnderlineColor(ColorSpec::Default) => code!(ResetUnderlineColor),
            Sgr::UnderlineColor(ColorSpec::TrueColor(c)) => write!(
                f,
                "{}:2::{}:{}:{}m",
                SgrCode::UnderlineColor as i64,
                c.red,
                c.green,
                c.blue
            )?,
            Sgr::UnderlineColor(ColorSpec::PaletteIndex(idx)) => {
                write!(f, "{}:5:{}m", SgrCode::UnderlineColor as i64, *idx)?
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Font {
    Default,
    Alternate(u8),
}

/// Constrol Sequence Initiator (CSI) Parser.
/// Since many sequences allow for composition of actions by separating
/// `;` character, we need to be able to iterate over
/// the set of parsed actions from a given CSI sequence.
/// `CSIParser` implements an Iterator that yields `CSI` instances as
/// it parses them out from the input sequence.
struct CSIParser<'a> {
    intermediates: &'a [u8],
    /// this flag is set when more than two intermediates
    /// arrived and subsequent characters were ignored.
    ignored_extra_intermediates: bool,
    control: char,
    /// While params is_some we have more data to consume.  The advance_by
    /// method updates the slice as we consume data.
    /// In a number of cases an empty params list is used to indicate
    /// default values, especially for SGR, so we need to be careful not
    /// to update params to an empty slice.
    params: Option<&'a [CsiParam]>,
}

impl CSI {
    /// Parse a CSI sequence.
    /// Returns an iterator that yields individual CSI actions.
    /// Why not a single?  Because sequences like `CSI [ 1 ; 3 m`
    /// embed two separate actions but are sent as a single unit.
    /// If no semantic meaning is known for a subsequence, the remainder
    /// of the sequence is returned wrapped in a `CSI::Unspecified` container.
    pub fn parse<'a>(
        params: &'a [CsiParam],
        intermediates: &'a [u8],
        ignored_extra_intermediates: bool,
        control: char,
    ) -> impl Iterator<Item = CSI> + 'a {
        CSIParser {
            intermediates,
            ignored_extra_intermediates,
            control,
            params: Some(params),
        }
    }
}

/// A little helper to convert i64 -> u8 if safe
fn to_u8(v: &CsiParam) -> Result<u8, ()> {
    match v {
        CsiParam::ColonList(_) => Err(()),
        CsiParam::Integer(v) => {
            if *v <= i64::from(u8::max_value()) {
                Ok(*v as u8)
            } else {
                Err(())
            }
        }
    }
}

/// Convert the input value to 1-based u32.
/// The intent is to protect consumers from out of range values
/// when operating on the data, while balancing strictness with
/// practical implementation bugs.  For example, it is common
/// to see 0 values being emitted from existing libraries, and
/// we desire to see the intended output.
/// Ensures that the value is in the range 1..=max_value.
/// If the input is 0 it is treated as 1.  If the value is
/// otherwise outside that range, an error is propagated and
/// that will typically case the sequence to be reported via
/// the Unspecified placeholder.
fn to_1b_u32(v: &CsiParam) -> Result<u32, ()> {
    match v {
        CsiParam::Integer(v) if *v == 0 => Ok(1),
        CsiParam::Integer(v) if *v > 0 && *v <= i64::from(u32::max_value()) => Ok(*v as u32),
        _ => Err(()),
    }
}

macro_rules! noparams {
    ($ns:ident, $variant:ident, $params:expr) => {{
        if $params.len() != 0 {
            Err(())
        } else {
            Ok(CSI::$ns($ns::$variant))
        }
    }};
}

macro_rules! parse {
    ($ns:ident, $variant:ident, $params:expr) => {{
        let value = ParseParams::parse_params($params)?;
        Ok(CSI::$ns($ns::$variant(value)))
    }};

    ($ns:ident, $variant:ident, $first:ident, $second:ident, $params:expr) => {{
        let (p1, p2): (OneBased, OneBased) = ParseParams::parse_params($params)?;
        Ok(CSI::$ns($ns::$variant {
            $first: p1,
            $second: p2,
        }))
    }};
}

impl<'a> CSIParser<'a> {
    fn parse_next(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        match (self.control, self.intermediates) {
            ('@', &[]) => parse!(Edit, InsertCharacter, params),
            ('`', &[]) => parse!(Cursor, CharacterPositionAbsolute, params),
            ('A', &[]) => parse!(Cursor, Up, params),
            ('B', &[]) => parse!(Cursor, Down, params),
            ('C', &[]) => parse!(Cursor, Right, params),
            ('D', &[]) => parse!(Cursor, Left, params),
            ('E', &[]) => parse!(Cursor, NextLine, params),
            ('F', &[]) => parse!(Cursor, PrecedingLine, params),
            ('G', &[]) => parse!(Cursor, CharacterAbsolute, params),
            ('H', &[]) => parse!(Cursor, Position, line, col, params),
            ('I', &[]) => parse!(Cursor, ForwardTabulation, params),
            ('J', &[]) => parse!(Edit, EraseInDisplay, params),
            ('K', &[]) => parse!(Edit, EraseInLine, params),
            ('L', &[]) => parse!(Edit, InsertLine, params),
            ('M', &[]) => parse!(Edit, DeleteLine, params),
            ('P', &[]) => parse!(Edit, DeleteCharacter, params),
            ('R', &[]) => parse!(Cursor, ActivePositionReport, line, col, params),
            ('S', &[]) => parse!(Edit, ScrollUp, params),
            ('T', &[]) => parse!(Edit, ScrollDown, params),
            ('W', &[]) => parse!(Cursor, TabulationControl, params),
            ('X', &[]) => parse!(Edit, EraseCharacter, params),
            ('Y', &[]) => parse!(Cursor, LineTabulation, params),
            ('Z', &[]) => parse!(Cursor, BackwardTabulation, params),

            ('a', &[]) => parse!(Cursor, CharacterPositionForward, params),
            ('b', &[]) => parse!(Edit, Repeat, params),
            ('d', &[]) => parse!(Cursor, LinePositionAbsolute, params),
            ('e', &[]) => parse!(Cursor, LinePositionForward, params),
            ('f', &[]) => parse!(Cursor, CharacterAndLinePosition, line, col, params),
            ('g', &[]) => parse!(Cursor, TabulationClear, params),
            ('h', &[]) => self
                .terminal_mode(params)
                .map(|mode| CSI::Mode(Mode::SetMode(mode))),
            ('j', &[]) => parse!(Cursor, CharacterPositionBackward, params),
            ('k', &[]) => parse!(Cursor, LinePositionBackward, params),
            ('l', &[]) => self
                .terminal_mode(params)
                .map(|mode| CSI::Mode(Mode::ResetMode(mode))),

            ('m', &[]) => self.sgr(params).map(CSI::Sgr),
            ('n', &[]) => self.dsr(params),
            ('q', &[b' ']) => self.cursor_style(params),
            ('r', &[]) => self.decstbm(params),
            ('s', &[]) => self.decslrm(params),
            ('t', &[]) => self.window(params).map(CSI::Window),
            ('u', &[]) => noparams!(Cursor, RestoreCursor, params),
            ('y', &[b'*']) => {
                fn p(params: &[CsiParam], idx: usize) -> Result<i64, ()> {
                    params.get(idx).and_then(CsiParam::as_integer).ok_or(())
                }
                let request_id = p(params, 0)?;
                let page_number = p(params, 1)?;
                let top = OneBased::from_optional_esc_param(params.get(2))?;
                let left = OneBased::from_optional_esc_param(params.get(3))?;
                let bottom = OneBased::from_optional_esc_param(params.get(4))?;
                let right = OneBased::from_optional_esc_param(params.get(5))?;
                Ok(CSI::Window(Window::ChecksumRectangularArea {
                    request_id,
                    page_number,
                    top,
                    left,
                    bottom,
                    right,
                }))
            }

            ('p', &[b'!']) => Ok(CSI::Device(Box::new(Device::SoftReset))),

            ('h', &[b'?']) => self
                .dec(params)
                .map(|mode| CSI::Mode(Mode::SetDecPrivateMode(mode))),
            ('l', &[b'?']) => self
                .dec(params)
                .map(|mode| CSI::Mode(Mode::ResetDecPrivateMode(mode))),
            ('r', &[b'?']) => self
                .dec(params)
                .map(|mode| CSI::Mode(Mode::RestoreDecPrivateMode(mode))),
            ('q', &[b'>']) => self
                .req_terminal_name_and_version(params)
                .map(|dev| CSI::Device(Box::new(dev))),
            ('s', &[b'?']) => self
                .dec(params)
                .map(|mode| CSI::Mode(Mode::SaveDecPrivateMode(mode))),

            ('m', &[b'<']) | ('M', &[b'<']) => self.mouse_sgr1006(params).map(CSI::Mouse),
            ('m', &[b'>']) => self.xterm_key_modifier(params),

            ('c', &[]) => self
                .req_primary_device_attributes(params)
                .map(|dev| CSI::Device(Box::new(dev))),
            ('c', &[b'>']) => self
                .req_secondary_device_attributes(params)
                .map(|dev| CSI::Device(Box::new(dev))),
            ('c', &[b'?']) => self
                .secondary_device_attributes(params)
                .map(|dev| CSI::Device(Box::new(dev))),

            _ => Err(()),
        }
    }

    /// Consume some number of elements from params and update it.
    /// Take care to avoid setting params back to an empty slice
    /// as this would trigger returning a default value and/or
    /// an unterminated parse loop.
    fn advance_by<T>(&mut self, n: usize, params: &'a [CsiParam], result: T) -> T {
        let (_, next) = params.split_at(n);
        if !next.is_empty() {
            self.params = Some(next);
        }
        result
    }

    fn cursor_style(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        if params.len() != 1 {
            Err(())
        } else {
            match FromPrimitive::from_i64(params[0].as_integer().unwrap()) {
                None => Err(()),
                Some(style) => {
                    Ok(self.advance_by(1, params, CSI::Cursor(Cursor::CursorStyle(style))))
                }
            }
        }
    }

    fn dsr(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        if params == [CsiParam::Integer(5)] {
            Ok(self.advance_by(1, params, CSI::Device(Box::new(Device::StatusReport))))
        } else if params == [CsiParam::Integer(6)] {
            Ok(self.advance_by(1, params, CSI::Cursor(Cursor::RequestActivePositionReport)))
        } else {
            Err(())
        }
    }

    fn decstbm(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        if params.is_empty() {
            Ok(CSI::Cursor(Cursor::SetTopAndBottomMargins {
                top: OneBased::new(1),
                bottom: OneBased::new(u32::max_value()),
            }))
        } else if params.len() == 1 {
            Ok(self.advance_by(
                1,
                params,
                CSI::Cursor(Cursor::SetTopAndBottomMargins {
                    top: OneBased::from_esc_param(&params[0])?,
                    bottom: OneBased::new(u32::max_value()),
                }),
            ))
        } else if params.len() == 2 {
            Ok(self.advance_by(
                2,
                params,
                CSI::Cursor(Cursor::SetTopAndBottomMargins {
                    top: OneBased::from_esc_param(&params[0])?,
                    bottom: OneBased::from_esc_param_with_big_default(&params[1])?,
                }),
            ))
        } else {
            Err(())
        }
    }

    fn xterm_key_modifier(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        if params.len() == 2 {
            let resource = XtermKeyModifierResource::parse(params[0].as_integer().unwrap())
                .ok_or_else(|| ())?;
            Ok(self.advance_by(
                2,
                params,
                CSI::Mode(Mode::XtermKeyMode {
                    resource,
                    value: Some(params[1].as_integer().ok_or_else(|| ())?),
                }),
            ))
        } else if params.len() == 1 {
            let resource = XtermKeyModifierResource::parse(params[0].as_integer().unwrap())
                .ok_or_else(|| ())?;
            Ok(self.advance_by(
                1,
                params,
                CSI::Mode(Mode::XtermKeyMode {
                    resource,
                    value: None,
                }),
            ))
        } else {
            Err(())
        }
    }

    fn decslrm(&mut self, params: &'a [CsiParam]) -> Result<CSI, ()> {
        if params.is_empty() {
            // with no params this is a request to save the cursor
            // and is technically in conflict with SetLeftAndRightMargins.
            // The emulator needs to decide based on DECSLRM mode
            // whether this saves the cursor or is SetLeftAndRightMargins
            // with default parameters!
            Ok(CSI::Cursor(Cursor::SaveCursor))
        } else if params.len() == 1 {
            Ok(self.advance_by(
                1,
                params,
                CSI::Cursor(Cursor::SetLeftAndRightMargins {
                    left: OneBased::from_esc_param(&params[0])?,
                    right: OneBased::new(u32::max_value()),
                }),
            ))
        } else if params.len() == 2 {
            Ok(self.advance_by(
                2,
                params,
                CSI::Cursor(Cursor::SetLeftAndRightMargins {
                    left: OneBased::from_esc_param(&params[0])?,
                    right: OneBased::from_esc_param(&params[1])?,
                }),
            ))
        } else {
            Err(())
        }
    }

    fn req_primary_device_attributes(&mut self, params: &'a [CsiParam]) -> Result<Device, ()> {
        if params == [] {
            Ok(Device::RequestPrimaryDeviceAttributes)
        } else if params == [CsiParam::Integer(0)] {
            Ok(self.advance_by(1, params, Device::RequestPrimaryDeviceAttributes))
        } else {
            Err(())
        }
    }

    fn req_terminal_name_and_version(&mut self, params: &'a [CsiParam]) -> Result<Device, ()> {
        if params == [] {
            Ok(Device::RequestTerminalNameAndVersion)
        } else if params == [CsiParam::Integer(0)] {
            Ok(self.advance_by(1, params, Device::RequestTerminalNameAndVersion))
        } else {
            Err(())
        }
    }

    fn req_secondary_device_attributes(&mut self, params: &'a [CsiParam]) -> Result<Device, ()> {
        if params == [] {
            Ok(Device::RequestSecondaryDeviceAttributes)
        } else if params == [CsiParam::Integer(0)] {
            Ok(self.advance_by(1, params, Device::RequestSecondaryDeviceAttributes))
        } else {
            Err(())
        }
    }

    fn secondary_device_attributes(&mut self, params: &'a [CsiParam]) -> Result<Device, ()> {
        if params == [CsiParam::Integer(1), CsiParam::Integer(0)] {
            Ok(self.advance_by(
                2,
                params,
                Device::DeviceAttributes(DeviceAttributes::Vt101WithNoOptions),
            ))
        } else if params == [CsiParam::Integer(6)] {
            Ok(self.advance_by(1, params, Device::DeviceAttributes(DeviceAttributes::Vt102)))
        } else if params == [CsiParam::Integer(1), CsiParam::Integer(2)] {
            Ok(self.advance_by(
                2,
                params,
                Device::DeviceAttributes(DeviceAttributes::Vt100WithAdvancedVideoOption),
            ))
        } else if !params.is_empty() && params[0] == CsiParam::Integer(62) {
            Ok(self.advance_by(
                params.len(),
                params,
                Device::DeviceAttributes(DeviceAttributes::Vt220(
                    DeviceAttributeFlags::from_params(&params[1..]),
                )),
            ))
        } else if !params.is_empty() && params[0] == CsiParam::Integer(63) {
            Ok(self.advance_by(
                params.len(),
                params,
                Device::DeviceAttributes(DeviceAttributes::Vt320(
                    DeviceAttributeFlags::from_params(&params[1..]),
                )),
            ))
        } else if !params.is_empty() && params[0] == CsiParam::Integer(64) {
            Ok(self.advance_by(
                params.len(),
                params,
                Device::DeviceAttributes(DeviceAttributes::Vt420(
                    DeviceAttributeFlags::from_params(&params[1..]),
                )),
            ))
        } else {
            Err(())
        }
    }

    /// Parse extended mouse reports known as SGR 1006 mode
    fn mouse_sgr1006(&mut self, params: &'a [CsiParam]) -> Result<MouseReport, ()> {
        if params.len() != 3 {
            return Err(());
        }

        let p0 = params[0].as_integer().unwrap();

        // 'M' encodes a press, 'm' a release.
        let button = match (self.control, p0 & 0b110_0011) {
            ('M', 0) => MouseButton::Button1Press,
            ('m', 0) => MouseButton::Button1Release,
            ('M', 1) => MouseButton::Button2Press,
            ('m', 1) => MouseButton::Button2Release,
            ('M', 2) => MouseButton::Button3Press,
            ('m', 2) => MouseButton::Button3Release,
            ('M', 64) => MouseButton::Button4Press,
            ('m', 64) => MouseButton::Button4Release,
            ('M', 65) => MouseButton::Button5Press,
            ('m', 65) => MouseButton::Button5Release,
            ('M', 32) => MouseButton::Button1Drag,
            ('M', 33) => MouseButton::Button2Drag,
            ('M', 34) => MouseButton::Button3Drag,
            // Note that there is some theoretical ambiguity with these None values.
            // The ambiguity stems from alternative encodings of the mouse protocol;
            // when set to SGR1006 mode the variants with the `3` parameter do not
            // occur.  They included here as a reminder for when support for those
            // other encodings is added and this block is likely copied and pasted
            // or refactored for re-use with them.
            ('M', 35) => MouseButton::None, // mouse motion with no buttons
            ('M', 3) => MouseButton::None,  // legacy notification about button release
            ('m', 3) => MouseButton::None,  // release+press doesn't make sense
            _ => {
                return Err(());
            }
        };

        let mut modifiers = Modifiers::NONE;
        if p0 & 4 != 0 {
            modifiers |= Modifiers::SHIFT;
        }
        if p0 & 8 != 0 {
            modifiers |= Modifiers::ALT;
        }
        if p0 & 16 != 0 {
            modifiers |= Modifiers::CTRL;
        }

        let p1 = params[1].as_integer().unwrap();
        let p2 = params[2].as_integer().unwrap();

        Ok(self.advance_by(
            3,
            params,
            MouseReport::SGR1006 {
                x: p1 as u16,
                y: p2 as u16,
                button,
                modifiers,
            },
        ))
    }

    fn dec(&mut self, params: &'a [CsiParam]) -> Result<DecPrivateMode, ()> {
        let p0 = params
            .get(0)
            .and_then(CsiParam::as_integer)
            .ok_or_else(|| ())?;
        match FromPrimitive::from_i64(p0) {
            None => Ok(self.advance_by(
                1,
                params,
                DecPrivateMode::Unspecified(p0.to_u16().ok_or(())?),
            )),
            Some(mode) => Ok(self.advance_by(1, params, DecPrivateMode::Code(mode))),
        }
    }

    fn terminal_mode(&mut self, params: &'a [CsiParam]) -> Result<TerminalMode, ()> {
        let p0 = params
            .get(0)
            .and_then(CsiParam::as_integer)
            .ok_or_else(|| ())?;
        match FromPrimitive::from_i64(p0) {
            None => {
                Ok(self.advance_by(1, params, TerminalMode::Unspecified(p0.to_u16().ok_or(())?)))
            }
            Some(mode) => Ok(self.advance_by(1, params, TerminalMode::Code(mode))),
        }
    }

    fn parse_sgr_color(&mut self, params: &'a [CsiParam]) -> Result<ColorSpec, ()> {
        if params.len() >= 5 && params[1].as_integer() == Some(2) {
            let red = to_u8(&params[2])?;
            let green = to_u8(&params[3])?;
            let blue = to_u8(&params[4])?;
            let res = RgbColor::new(red, green, blue).into();
            Ok(self.advance_by(5, params, res))
        } else if params.len() >= 3 && params[1].as_integer() == Some(5) {
            let idx = to_u8(&params[2])?;
            Ok(self.advance_by(3, params, ColorSpec::PaletteIndex(idx)))
        } else {
            Err(())
        }
    }

    fn window(&mut self, params: &'a [CsiParam]) -> Result<Window, ()> {
        if params.is_empty() {
            Err(())
        } else {
            let arg1 = params.get(1).and_then(CsiParam::as_integer);
            let arg2 = params.get(2).and_then(CsiParam::as_integer);
            match params[0].as_integer() {
                None => Err(()),
                Some(p) => match p {
                    1 => Ok(Window::DeIconify),
                    2 => Ok(Window::Iconify),
                    3 => Ok(Window::MoveWindow {
                        x: arg1.unwrap_or(0),
                        y: arg2.unwrap_or(0),
                    }),
                    4 => Ok(Window::ResizeWindowPixels {
                        height: arg1,
                        width: arg2,
                    }),
                    5 => Ok(Window::RaiseWindow),
                    6 => match params.len() {
                        1 => Ok(Window::LowerWindow),
                        3 => Ok(Window::ReportCellSizePixelsResponse {
                            height: arg1,
                            width: arg2,
                        }),
                        _ => Err(()),
                    },
                    7 => Ok(Window::RefreshWindow),
                    8 => Ok(Window::ResizeWindowCells {
                        height: arg1,
                        width: arg2,
                    }),
                    9 => match arg1 {
                        Some(0) => Ok(Window::RestoreMaximizedWindow),
                        Some(1) => Ok(Window::MaximizeWindow),
                        Some(2) => Ok(Window::MaximizeWindowVertically),
                        Some(3) => Ok(Window::MaximizeWindowHorizontally),
                        _ => Err(()),
                    },
                    10 => match arg1 {
                        Some(0) => Ok(Window::UndoFullScreenMode),
                        Some(1) => Ok(Window::ChangeToFullScreenMode),
                        Some(2) => Ok(Window::ToggleFullScreen),
                        _ => Err(()),
                    },
                    11 => Ok(Window::ReportWindowState),
                    13 => match arg1 {
                        None => Ok(Window::ReportWindowPosition),
                        Some(2) => Ok(Window::ReportTextAreaPosition),
                        _ => Err(()),
                    },
                    14 => match arg1 {
                        None => Ok(Window::ReportTextAreaSizePixels),
                        Some(2) => Ok(Window::ReportWindowSizePixels),
                        _ => Err(()),
                    },
                    15 => Ok(Window::ReportScreenSizePixels),
                    16 => Ok(Window::ReportCellSizePixels),
                    18 => Ok(Window::ReportTextAreaSizeCells),
                    19 => Ok(Window::ReportScreenSizeCells),
                    20 => Ok(Window::ReportIconLabel),
                    21 => Ok(Window::ReportWindowTitle),
                    22 => match arg1 {
                        Some(0) => Ok(Window::PushIconAndWindowTitle),
                        Some(1) => Ok(Window::PushIconTitle),
                        Some(2) => Ok(Window::PushWindowTitle),
                        _ => Err(()),
                    },
                    23 => match arg1 {
                        Some(0) => Ok(Window::PopIconAndWindowTitle),
                        Some(1) => Ok(Window::PopIconTitle),
                        Some(2) => Ok(Window::PopWindowTitle),
                        _ => Err(()),
                    },
                    _ => Err(()),
                },
            }
        }
    }

    fn sgr(&mut self, params: &'a [CsiParam]) -> Result<Sgr, ()> {
        if params.is_empty() {
            // With no parameters, treat as equivalent to Reset.
            Ok(Sgr::Reset)
        } else {
            // Consume a single parameter and return the parsed result
            macro_rules! one {
                ($t:expr) => {
                    Ok(self.advance_by(1, params, $t))
                };
            };

            match &params[0] {
                CsiParam::Integer(i) => match FromPrimitive::from_i64(*i) {
                    None => Err(()),
                    Some(sgr) => match sgr {
                        SgrCode::Reset => one!(Sgr::Reset),
                        SgrCode::IntensityBold => one!(Sgr::Intensity(Intensity::Bold)),
                        SgrCode::IntensityDim => one!(Sgr::Intensity(Intensity::Half)),
                        SgrCode::NormalIntensity => one!(Sgr::Intensity(Intensity::Normal)),
                        SgrCode::UnderlineOn => one!(Sgr::Underline(Underline::Single)),
                        SgrCode::UnderlineDouble => one!(Sgr::Underline(Underline::Double)),
                        SgrCode::UnderlineOff => one!(Sgr::Underline(Underline::None)),
                        SgrCode::UnderlineColor => {
                            self.parse_sgr_color(params).map(Sgr::UnderlineColor)
                        }
                        SgrCode::ResetUnderlineColor => {
                            one!(Sgr::UnderlineColor(ColorSpec::default()))
                        }
                        SgrCode::BlinkOn => one!(Sgr::Blink(Blink::Slow)),
                        SgrCode::RapidBlinkOn => one!(Sgr::Blink(Blink::Rapid)),
                        SgrCode::BlinkOff => one!(Sgr::Blink(Blink::None)),
                        SgrCode::ItalicOn => one!(Sgr::Italic(true)),
                        SgrCode::ItalicOff => one!(Sgr::Italic(false)),
                        SgrCode::ForegroundColor => {
                            self.parse_sgr_color(params).map(Sgr::Foreground)
                        }
                        SgrCode::ForegroundBlack => one!(Sgr::Foreground(AnsiColor::Black.into())),
                        SgrCode::ForegroundRed => one!(Sgr::Foreground(AnsiColor::Maroon.into())),
                        SgrCode::ForegroundGreen => one!(Sgr::Foreground(AnsiColor::Green.into())),
                        SgrCode::ForegroundYellow => one!(Sgr::Foreground(AnsiColor::Olive.into())),
                        SgrCode::ForegroundBlue => one!(Sgr::Foreground(AnsiColor::Navy.into())),
                        SgrCode::ForegroundMagenta => {
                            one!(Sgr::Foreground(AnsiColor::Purple.into()))
                        }
                        SgrCode::ForegroundCyan => one!(Sgr::Foreground(AnsiColor::Teal.into())),
                        SgrCode::ForegroundWhite => one!(Sgr::Foreground(AnsiColor::Silver.into())),
                        SgrCode::ForegroundDefault => one!(Sgr::Foreground(ColorSpec::Default)),
                        SgrCode::ForegroundBrightBlack => {
                            one!(Sgr::Foreground(AnsiColor::Grey.into()))
                        }
                        SgrCode::ForegroundBrightRed => {
                            one!(Sgr::Foreground(AnsiColor::Red.into()))
                        }
                        SgrCode::ForegroundBrightGreen => {
                            one!(Sgr::Foreground(AnsiColor::Lime.into()))
                        }
                        SgrCode::ForegroundBrightYellow => {
                            one!(Sgr::Foreground(AnsiColor::Yellow.into()))
                        }
                        SgrCode::ForegroundBrightBlue => {
                            one!(Sgr::Foreground(AnsiColor::Blue.into()))
                        }
                        SgrCode::ForegroundBrightMagenta => {
                            one!(Sgr::Foreground(AnsiColor::Fuschia.into()))
                        }
                        SgrCode::ForegroundBrightCyan => {
                            one!(Sgr::Foreground(AnsiColor::Aqua.into()))
                        }
                        SgrCode::ForegroundBrightWhite => {
                            one!(Sgr::Foreground(AnsiColor::White.into()))
                        }

                        SgrCode::BackgroundColor => {
                            self.parse_sgr_color(params).map(Sgr::Background)
                        }
                        SgrCode::BackgroundBlack => one!(Sgr::Background(AnsiColor::Black.into())),
                        SgrCode::BackgroundRed => one!(Sgr::Background(AnsiColor::Maroon.into())),
                        SgrCode::BackgroundGreen => one!(Sgr::Background(AnsiColor::Green.into())),
                        SgrCode::BackgroundYellow => one!(Sgr::Background(AnsiColor::Olive.into())),
                        SgrCode::BackgroundBlue => one!(Sgr::Background(AnsiColor::Navy.into())),
                        SgrCode::BackgroundMagenta => {
                            one!(Sgr::Background(AnsiColor::Purple.into()))
                        }
                        SgrCode::BackgroundCyan => one!(Sgr::Background(AnsiColor::Teal.into())),
                        SgrCode::BackgroundWhite => one!(Sgr::Background(AnsiColor::Silver.into())),
                        SgrCode::BackgroundDefault => one!(Sgr::Background(ColorSpec::Default)),
                        SgrCode::BackgroundBrightBlack => {
                            one!(Sgr::Background(AnsiColor::Grey.into()))
                        }
                        SgrCode::BackgroundBrightRed => {
                            one!(Sgr::Background(AnsiColor::Red.into()))
                        }
                        SgrCode::BackgroundBrightGreen => {
                            one!(Sgr::Background(AnsiColor::Lime.into()))
                        }
                        SgrCode::BackgroundBrightYellow => {
                            one!(Sgr::Background(AnsiColor::Yellow.into()))
                        }
                        SgrCode::BackgroundBrightBlue => {
                            one!(Sgr::Background(AnsiColor::Blue.into()))
                        }
                        SgrCode::BackgroundBrightMagenta => {
                            one!(Sgr::Background(AnsiColor::Fuschia.into()))
                        }
                        SgrCode::BackgroundBrightCyan => {
                            one!(Sgr::Background(AnsiColor::Aqua.into()))
                        }
                        SgrCode::BackgroundBrightWhite => {
                            one!(Sgr::Background(AnsiColor::White.into()))
                        }

                        SgrCode::InverseOn => one!(Sgr::Inverse(true)),
                        SgrCode::InverseOff => one!(Sgr::Inverse(false)),
                        SgrCode::InvisibleOn => one!(Sgr::Invisible(true)),
                        SgrCode::InvisibleOff => one!(Sgr::Invisible(false)),
                        SgrCode::StrikeThroughOn => one!(Sgr::StrikeThrough(true)),
                        SgrCode::StrikeThroughOff => one!(Sgr::StrikeThrough(false)),
                        SgrCode::OverlineOn => one!(Sgr::Overline(true)),
                        SgrCode::OverlineOff => one!(Sgr::Overline(false)),
                        SgrCode::DefaultFont => one!(Sgr::Font(Font::Default)),
                        SgrCode::AltFont1 => one!(Sgr::Font(Font::Alternate(1))),
                        SgrCode::AltFont2 => one!(Sgr::Font(Font::Alternate(2))),
                        SgrCode::AltFont3 => one!(Sgr::Font(Font::Alternate(3))),
                        SgrCode::AltFont4 => one!(Sgr::Font(Font::Alternate(4))),
                        SgrCode::AltFont5 => one!(Sgr::Font(Font::Alternate(5))),
                        SgrCode::AltFont6 => one!(Sgr::Font(Font::Alternate(6))),
                        SgrCode::AltFont7 => one!(Sgr::Font(Font::Alternate(7))),
                        SgrCode::AltFont8 => one!(Sgr::Font(Font::Alternate(8))),
                        SgrCode::AltFont9 => one!(Sgr::Font(Font::Alternate(9))),
                    },
                },
                CsiParam::ColonList(list) => {
                    match list.as_slice() {
                        // Kitty styled underlines
                        &[Some(4), Some(0)] => one!(Sgr::Underline(Underline::None)),
                        &[Some(4), Some(1)] => one!(Sgr::Underline(Underline::Single)),
                        &[Some(4), Some(2)] => one!(Sgr::Underline(Underline::Double)),
                        &[Some(4), Some(3)] => one!(Sgr::Underline(Underline::Curly)),
                        &[Some(4), Some(4)] => one!(Sgr::Underline(Underline::Dotted)),
                        &[Some(4), Some(5)] => one!(Sgr::Underline(Underline::Dashed)),

                        &[Some(38), Some(2), _colorspace, Some(r), Some(g), Some(b)] => one!(
                            Sgr::Foreground(RgbColor::new(r as u8, g as u8, b as u8).into())
                        ),
                        &[Some(38), Some(5), Some(idx)] => {
                            one!(Sgr::Foreground(ColorSpec::PaletteIndex(idx as u8)))
                        }

                        &[Some(48), Some(2), _colorspace, Some(r), Some(g), Some(b)] => one!(
                            Sgr::Background(RgbColor::new(r as u8, g as u8, b as u8).into())
                        ),
                        &[Some(48), Some(5), Some(idx)] => {
                            one!(Sgr::Background(ColorSpec::PaletteIndex(idx as u8)))
                        }

                        &[Some(58), Some(2), _colorspace, Some(r), Some(g), Some(b)] => one!(
                            Sgr::UnderlineColor(RgbColor::new(r as u8, g as u8, b as u8).into())
                        ),
                        &[Some(58), Some(5), Some(idx)] => {
                            one!(Sgr::UnderlineColor(ColorSpec::PaletteIndex(idx as u8)))
                        }

                        _ => Err(()),
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive)]
pub enum SgrCode {
    Reset = 0,
    IntensityBold = 1,
    IntensityDim = 2,
    ItalicOn = 3,
    UnderlineOn = 4,
    /// Blinks < 150 times per minute
    BlinkOn = 5,
    /// Blinks > 150 times per minute
    RapidBlinkOn = 6,
    InverseOn = 7,
    InvisibleOn = 8,
    StrikeThroughOn = 9,
    DefaultFont = 10,
    AltFont1 = 11,
    AltFont2 = 12,
    AltFont3 = 13,
    AltFont4 = 14,
    AltFont5 = 15,
    AltFont6 = 16,
    AltFont7 = 17,
    AltFont8 = 18,
    AltFont9 = 19,
    // Fraktur = 20,
    UnderlineDouble = 21,
    NormalIntensity = 22,
    ItalicOff = 23,
    UnderlineOff = 24,
    BlinkOff = 25,
    InverseOff = 27,
    InvisibleOff = 28,
    StrikeThroughOff = 29,
    ForegroundBlack = 30,
    ForegroundRed = 31,
    ForegroundGreen = 32,
    ForegroundYellow = 33,
    ForegroundBlue = 34,
    ForegroundMagenta = 35,
    ForegroundCyan = 36,
    ForegroundWhite = 37,
    ForegroundDefault = 39,
    BackgroundBlack = 40,
    BackgroundRed = 41,
    BackgroundGreen = 42,
    BackgroundYellow = 43,
    BackgroundBlue = 44,
    BackgroundMagenta = 45,
    BackgroundCyan = 46,
    BackgroundWhite = 47,
    BackgroundDefault = 49,
    OverlineOn = 53,
    OverlineOff = 55,

    UnderlineColor = 58,
    ResetUnderlineColor = 59,

    ForegroundBrightBlack = 90,
    ForegroundBrightRed = 91,
    ForegroundBrightGreen = 92,
    ForegroundBrightYellow = 93,
    ForegroundBrightBlue = 94,
    ForegroundBrightMagenta = 95,
    ForegroundBrightCyan = 96,
    ForegroundBrightWhite = 97,

    BackgroundBrightBlack = 100,
    BackgroundBrightRed = 101,
    BackgroundBrightGreen = 102,
    BackgroundBrightYellow = 103,
    BackgroundBrightBlue = 104,
    BackgroundBrightMagenta = 105,
    BackgroundBrightCyan = 106,
    BackgroundBrightWhite = 107,

    /// Maybe followed either either a 256 color palette index or
    /// a sequence describing a true color rgb value
    ForegroundColor = 38,
    BackgroundColor = 48,
}

impl<'a> Iterator for CSIParser<'a> {
    type Item = CSI;

    fn next(&mut self) -> Option<CSI> {
        let params = match self.params.take() {
            None => return None,
            Some(params) => params,
        };

        match self.parse_next(&params) {
            Ok(csi) => Some(csi),
            Err(()) => Some(CSI::Unspecified(Box::new(Unspecified {
                params: params.to_vec(),
                intermediates: self.intermediates.to_vec(),
                ignored_extra_intermediates: self.ignored_extra_intermediates,
                control: self.control,
            }))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;

    fn parse(control: char, params: &[i64], expected: &str) -> Vec<CSI> {
        let params = params
            .iter()
            .map(|&i| CsiParam::Integer(i))
            .collect::<Vec<_>>();
        let res = CSI::parse(&params, &[], false, control).collect();
        assert_eq!(encode(&res), expected);
        res
    }

    fn parse_int(control: char, params: &[i64], intermediate: u8, expected: &str) -> Vec<CSI> {
        let params = params
            .iter()
            .map(|&i| CsiParam::Integer(i))
            .collect::<Vec<_>>();
        let intermediates = [intermediate];
        let res = CSI::parse(&params, &intermediates, false, control).collect();
        assert_eq!(encode(&res), expected);
        res
    }

    fn encode(seq: &Vec<CSI>) -> String {
        let mut res = Vec::new();
        for s in seq {
            write!(res, "{}", s).unwrap();
        }
        String::from_utf8(res).unwrap()
    }

    #[test]
    fn basic() {
        assert_eq!(parse('m', &[], "\x1b[0m"), vec![CSI::Sgr(Sgr::Reset)]);
        assert_eq!(parse('m', &[0], "\x1b[0m"), vec![CSI::Sgr(Sgr::Reset)]);
        assert_eq!(
            parse('m', &[1], "\x1b[1m"),
            vec![CSI::Sgr(Sgr::Intensity(Intensity::Bold))]
        );
        assert_eq!(
            parse('m', &[1, 3], "\x1b[1m\x1b[3m"),
            vec![
                CSI::Sgr(Sgr::Intensity(Intensity::Bold)),
                CSI::Sgr(Sgr::Italic(true)),
            ]
        );

        // Verify that we propagate Unspecified for codes
        // that we don't recognize.
        assert_eq!(
            parse('m', &[1, 3, 1231231], "\x1b[1m\x1b[3m\x1b[1231231m"),
            vec![
                CSI::Sgr(Sgr::Intensity(Intensity::Bold)),
                CSI::Sgr(Sgr::Italic(true)),
                CSI::Unspecified(Box::new(Unspecified {
                    params: [CsiParam::Integer(1231231)].to_vec(),
                    intermediates: vec![],
                    ignored_extra_intermediates: false,
                    control: 'm',
                })),
            ]
        );
        assert_eq!(
            parse('m', &[1, 1231231, 3], "\x1b[1m\x1b[1231231;3m"),
            vec![
                CSI::Sgr(Sgr::Intensity(Intensity::Bold)),
                CSI::Unspecified(Box::new(Unspecified {
                    params: [CsiParam::Integer(1231231), CsiParam::Integer(3)].to_vec(),
                    intermediates: vec![],
                    ignored_extra_intermediates: false,
                    control: 'm',
                })),
            ]
        );
        assert_eq!(
            parse('m', &[1231231, 3], "\x1b[1231231;3m"),
            vec![CSI::Unspecified(Box::new(Unspecified {
                params: [CsiParam::Integer(1231231), CsiParam::Integer(3)].to_vec(),
                intermediates: vec![],
                ignored_extra_intermediates: false,
                control: 'm',
            }))]
        );
    }

    #[test]
    fn underlines() {
        assert_eq!(
            parse('m', &[21], "\x1b[21m"),
            vec![CSI::Sgr(Sgr::Underline(Underline::Double))]
        );
        assert_eq!(
            parse('m', &[4], "\x1b[4m"),
            vec![CSI::Sgr(Sgr::Underline(Underline::Single))]
        );
    }

    #[test]
    fn underline_color() {
        assert_eq!(
            parse('m', &[58, 2], "\x1b[58;2m"),
            vec![CSI::Unspecified(Box::new(Unspecified {
                params: [CsiParam::Integer(58), CsiParam::Integer(2)].to_vec(),
                intermediates: vec![],
                ignored_extra_intermediates: false,
                control: 'm',
            }))]
        );

        assert_eq!(
            parse('m', &[58, 2, 255, 255, 255], "\x1b[58:2::255:255:255m"),
            vec![CSI::Sgr(Sgr::UnderlineColor(ColorSpec::TrueColor(
                RgbColor::new(255, 255, 255),
            )))]
        );
        assert_eq!(
            parse('m', &[58, 5, 220, 255, 255], "\x1b[58:5:220m\x1b[255;255m"),
            vec![
                CSI::Sgr(Sgr::UnderlineColor(ColorSpec::PaletteIndex(220))),
                CSI::Unspecified(Box::new(Unspecified {
                    params: [CsiParam::Integer(255), CsiParam::Integer(255)].to_vec(),
                    intermediates: vec![],
                    ignored_extra_intermediates: false,
                    control: 'm',
                })),
            ]
        );
    }

    #[test]
    fn color() {
        assert_eq!(
            parse('m', &[38, 2], "\x1b[38;2m"),
            vec![CSI::Unspecified(Box::new(Unspecified {
                params: [CsiParam::Integer(38), CsiParam::Integer(2)].to_vec(),
                intermediates: vec![],
                ignored_extra_intermediates: false,
                control: 'm',
            }))]
        );

        assert_eq!(
            parse('m', &[38, 2, 255, 255, 255], "\x1b[38:2::255:255:255m"),
            vec![CSI::Sgr(Sgr::Foreground(ColorSpec::TrueColor(
                RgbColor::new(255, 255, 255),
            )))]
        );
        assert_eq!(
            parse('m', &[38, 5, 220, 255, 255], "\x1b[38:5:220m\x1b[255;255m"),
            vec![
                CSI::Sgr(Sgr::Foreground(ColorSpec::PaletteIndex(220))),
                CSI::Unspecified(Box::new(Unspecified {
                    params: [CsiParam::Integer(255), CsiParam::Integer(255)].to_vec(),
                    intermediates: vec![],
                    ignored_extra_intermediates: false,
                    control: 'm',
                })),
            ]
        );
    }

    #[test]
    fn edit() {
        assert_eq!(
            parse('J', &[], "\x1b[J"),
            vec![CSI::Edit(Edit::EraseInDisplay(
                EraseInDisplay::EraseToEndOfDisplay,
            ))]
        );
        assert_eq!(
            parse('J', &[0], "\x1b[J"),
            vec![CSI::Edit(Edit::EraseInDisplay(
                EraseInDisplay::EraseToEndOfDisplay,
            ))]
        );
        assert_eq!(
            parse('J', &[1], "\x1b[1J"),
            vec![CSI::Edit(Edit::EraseInDisplay(
                EraseInDisplay::EraseToStartOfDisplay,
            ))]
        );
    }

    #[test]
    fn window() {
        assert_eq!(
            parse('t', &[6], "\x1b[6t"),
            vec![CSI::Window(Window::LowerWindow)]
        );
        assert_eq!(
            parse('t', &[6, 15, 7], "\x1b[6;15;7t"),
            vec![CSI::Window(Window::ReportCellSizePixelsResponse {
                width: Some(7),
                height: Some(15)
            })]
        );
    }

    #[test]
    fn cursor() {
        assert_eq!(
            parse('C', &[], "\x1b[C"),
            vec![CSI::Cursor(Cursor::Right(1))]
        );
        // check that 0 is treated as 1
        assert_eq!(
            parse('C', &[0], "\x1b[C"),
            vec![CSI::Cursor(Cursor::Right(1))]
        );
        assert_eq!(
            parse('C', &[1], "\x1b[C"),
            vec![CSI::Cursor(Cursor::Right(1))]
        );
        assert_eq!(
            parse('C', &[4], "\x1b[4C"),
            vec![CSI::Cursor(Cursor::Right(4))]
        );

        // Check that we default the second parameter of two
        // when only one is provided
        assert_eq!(
            parse('H', &[2], "\x1b[2;1H"),
            vec![CSI::Cursor(Cursor::Position {
                line: OneBased::new(2),
                col: OneBased::new(1)
            })]
        );
    }

    #[test]
    fn decset() {
        assert_eq!(
            parse_int('h', &[23434], b'?', "\x1b[?23434h"),
            vec![CSI::Mode(Mode::SetDecPrivateMode(
                DecPrivateMode::Unspecified(23434),
            ))]
        );
        assert_eq!(
            parse_int('l', &[1], b'?', "\x1b[?1l"),
            vec![CSI::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ApplicationCursorKeys,
            )))]
        );

        assert_eq!(
            parse_int('s', &[25], b'?', "\x1b[?25s"),
            vec![CSI::Mode(Mode::SaveDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ShowCursor,
            )))]
        );
        assert_eq!(
            parse_int('r', &[2004], b'?', "\x1b[?2004r"),
            vec![CSI::Mode(Mode::RestoreDecPrivateMode(
                DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste),
            ))]
        );
        assert_eq!(
            parse_int('h', &[12, 25], b'?', "\x1b[?12h\x1b[?25h"),
            vec![
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::StartBlinkingCursor,
                ))),
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::ShowCursor,
                ))),
            ]
        );

        assert_eq!(
            parse_int(
                'h',
                &[1002, 1003, 1005, 1006],
                b'?',
                "\x1b[?1002h\x1b[?1003h\x1b[?1005h\x1b[?1006h"
            ),
            vec![
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::ButtonEventMouse,
                ))),
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::AnyEventMouse,
                ))),
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Unspecified(1005))),
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::SGRMouse,
                ))),
            ]
        );
    }

    #[test]
    fn mouse() {
        assert_eq!(
            parse_int('M', &[0, 12, 300], b'<', "\x1b[<0;12;300M"),
            vec![CSI::Mouse(MouseReport::SGR1006 {
                x: 12,
                y: 300,
                button: MouseButton::Button1Press,
                modifiers: Modifiers::NONE,
            })]
        );
    }

    #[test]
    fn device_attr() {
        assert_eq!(
            parse_int(
                'c',
                &[63, 1, 2, 4, 6, 9, 15, 22],
                b'?',
                "\x1b[?63;1;2;4;6;9;15;22c"
            ),
            vec![CSI::Device(Box::new(Device::DeviceAttributes(
                DeviceAttributes::Vt320(DeviceAttributeFlags::new(vec![
                    DeviceAttribute::Code(DeviceAttributeCodes::Columns132),
                    DeviceAttribute::Code(DeviceAttributeCodes::Printer),
                    DeviceAttribute::Code(DeviceAttributeCodes::SixelGraphics),
                    DeviceAttribute::Code(DeviceAttributeCodes::SelectiveErase),
                    DeviceAttribute::Code(DeviceAttributeCodes::NationalReplacementCharsets),
                    DeviceAttribute::Code(DeviceAttributeCodes::TechnicalCharacters),
                    DeviceAttribute::Code(DeviceAttributeCodes::AnsiColor),
                ])),
            )))]
        );
    }
}
