use failure::Error;

mod none;
#[cfg(target_os = "macos")]
pub use self::none::NoClipboard as Clipboard;

#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

#[cfg(all(unix, not(target_os = "macos")))]
pub use self::x11::Clipboard;

use wakeup::Wakeup;

/// A fragment of the clipboard data received from another
/// app during paste.
#[derive(Debug)]
pub enum Paste {
    /// The whole content of the paste is available
    All(String),
    /// Someone else now owns the selection.  You should
    /// clear the selection locally.
    Cleared,
    /// The clipboard window has initialized successfully
    Running,
}

/// Abstracts away system specific clipboard implementation details.
pub trait ClipboardImpl {
    fn new(wakeup: Wakeup) -> Result<Self, Error>
    where
        Self: Sized;
    fn set_clipboard(&self, text: Option<String>) -> Result<(), Error>;
    fn get_clipboard(&self) -> Result<String, Error>;
}
