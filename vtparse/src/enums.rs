#![allow(dead_code)]

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum Action {
    None = 0,
    Ignore = 1,
    Print = 2,
    Execute = 3,
    Clear = 4,
    Collect = 5,
    Param = 6,
    EscDispatch = 7,
    CsiDispatch = 8,
    Hook = 9,
    Put = 10,
    Unhook = 11,
    OscStart = 12,
    OscPut = 13,
    OscEnd = 14,
    Utf8 = 15,
    ApcStart = 16,
    ApcPut = 17,
    ApcEnd = 18,
}

impl Action {
    #[inline(always)]
    pub fn from_u16(v: u16) -> Self {
        unsafe { std::mem::transmute(v) }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum State {
    Ground = 0,
    Escape = 1,
    EscapeIntermediate = 2,
    CsiEntry = 3,
    CsiParam = 4,
    CsiIntermediate = 5,
    CsiIgnore = 6,
    DcsEntry = 7,
    DcsParam = 8,
    DcsIntermediate = 9,
    DcsPassthrough = 10,
    DcsIgnore = 11,
    OscString = 12,
    SosPmString = 13,
    ApcString = 14,
    // Special states, always last (no tables for these)
    Anywhere = 15,
    Utf8Sequence = 16,
}

impl State {
    #[inline(always)]
    pub fn from_u16(v: u16) -> Self {
        unsafe { std::mem::transmute(v) }
    }
}
