#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;

pub mod wayland;
pub mod x11;
pub mod x_and_wayland;
pub mod xkeysyms;

pub use x_and_wayland::*;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
