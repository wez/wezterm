#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(feature = "wayland")]
pub mod wayland;
pub mod x11;
pub mod x_and_wayland;
pub mod xdg_desktop_portal;
pub mod xkeysyms;

#[cfg(all(unix, not(target_os = "macos")))]
pub use x_and_wayland::*;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;

pub mod parameters;
