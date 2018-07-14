#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystemCommand {
    Unspecified(Vec<Vec<u8>>),
    #[doc(hidden)]
    __Nonexhaustive,
}

#[derive(Debug, Clone, PartialEq, Eq, FromPrimitive)]
pub enum OperatingSystemCommandCode {
    SetIconNameAndWindowTitle = 0,
    SetIconName = 1,
    SetWindowTitle = 2,
    SetXWindowProperty = 3,
    ChangeColorNumber = 4,
    /// iTerm2
    ChangeTitleTabColor = 6,
    SetCurrentWorkingDirectory = 7,
    /// See https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
    Hyperlink = 8,
    /// iTerm2
    SystemNotification = 9,
    SetTextForegroundColor = 10,
    SetTextBackgroundColor = 11,
    SetTextCursorColor = 12,
    SetMouseForegroundColor = 13,
    SetMouseBackgroundColor = 14,
    SetTektronixForegroundColor = 15,
    SetTektronixBackgroundColor = 16,
    SetHighlightColor = 17,
    SetTektronixCursorColor = 18,
    SetLogFileName = 46,
    SetFont = 50,
    EmacsShell = 51,
    ManipulateSelectionData = 52,
    RxvtProprietary = 777,
    ITermProprietary = 1337,
}
