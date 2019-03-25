pub mod cmdline;
pub mod conpty;
pub mod ownedhandle;
pub mod winpty;

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}
