#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;

pub mod wayland;
pub mod x11;
pub mod xkeysyms;

#[cfg(all(unix, feature = "wayland", not(target_os = "macos")))]
pub use self::wayland::*;
#[cfg(all(unix, not(feature = "wayland"), not(target_os = "macos")))]
pub use self::x11::*;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
