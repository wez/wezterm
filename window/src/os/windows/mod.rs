pub mod connection;
pub mod event;
pub mod gdi;
mod wgl;
pub mod window;

pub use self::window::*;
pub use connection::*;
pub use event::*;
pub use gdi::*;

/// Convert a rust string to a windows wide string
fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
