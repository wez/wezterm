#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
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
}

impl Action {
    #[inline(always)]
    #[allow(dead_code)]
    pub fn from_u8(v: u8) -> Self {
        unsafe { std::mem::transmute(v) }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
#[allow(dead_code)]
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
    SosPmApcString = 13,
    Anywhere = 14,
}

impl State {
    #[inline(always)]
    #[allow(dead_code)]
    pub fn from_u8(v: u8) -> Self {
        unsafe { std::mem::transmute(v) }
    }
}
