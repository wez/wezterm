#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod x11;
#[cfg(all(unix, not(target_os = "macos")))]
pub use self::x11::*;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
