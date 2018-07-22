//! An abstraction over a terminal device
//! `Terminal` implements `Read` and `Write` and offers methods
//! for changing the input mode.  The interface considers the differences
//! between POSIX and Windows systems, but is implemented only for POSIX
//! at this time.

use caps::Capabilities;
use failure::Error;
use num::{self, NumCast};
use std::fmt::Display;
use std::io::{Read, Write};
use surface::Change;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub use self::unix::UnixTerminal;
#[cfg(windows)]
pub use self::windows::WindowsTerminal;

/// Represents the size of the terminal screen.
/// The number of rows and columns of character cells are expressed.
/// Some implementations populate the size of those cells in pixels.
// On Windows, GetConsoleFontSize() can return the size of a cell in
// logical units and we can probably use this to populate xpixel, ypixel.
// GetConsoleScreenBufferInfo() can return the rows and cols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    /// The number of rows of text
    pub rows: usize,
    /// The number of columns per row
    pub cols: usize,
    /// The width of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub xpixel: usize,
    /// The height of a cell in pixels.  Some implementations never
    /// set this to anything other than zero.
    pub ypixel: usize,
}

/// `Terminal` abstracts over some basic terminal capabilities.
/// If the `set_raw_mode` or `set_cooked_mode` functions are used in
/// any combination, the implementation is required to restore the
/// terminal mode that was in effect when it was created.
pub trait Terminal: Read + Write {
    /// Raw mode disables input line buffering, allowing data to be
    /// read as the user presses keys, disables local echo, so keys
    /// pressed by the user do not implicitly render to the terminal
    /// output, and disables canonicalization of unix newlines to CRLF.
    fn set_raw_mode(&mut self) -> Result<(), Error>;

    /// Queries the current screen size, returning width, height.
    fn get_screen_size(&mut self) -> Result<ScreenSize, Error>;

    /// Sets the current screen size
    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error>;

    /// Render a series of changes to the terminal output
    fn render(&mut self, changes: &[Change]) -> Result<(), Error>;

    /*
    /// Sets the terminal to cooked mode, which is essentially the opposite
    /// to raw mode: input and output processing are enabled.
    fn set_cooked_mode(&mut self) -> Result<(), Error>;
*/
}

/// Construct a new instance of Terminal.
/// The terminal will have a renderer that is influenced by the configuration
/// in the provided `Capabilities` instance.
/// The terminal will explicitly open `/dev/tty` on Unix systems and
/// `CONIN$` and `CONOUT$` on Windows systems, so that it should yield a
/// functioning console with minimal headaches.
/// If you have a more advanced use case you will want to look to the
/// constructors for `UnixTerminal` and `WindowsTerminal` and call whichever
/// one is most suitable for your needs.
pub fn new_terminal(caps: Capabilities) -> Result<impl Terminal, Error> {
    new_terminal_sys(caps)
}

#[cfg(unix)]
fn new_terminal_sys(caps: Capabilities) -> Result<impl Terminal, Error> {
    UnixTerminal::new(caps)
}

#[cfg(windows)]
fn new_terminal_sys(caps: Capabilities) -> Result<impl Terminal, Error> {
    WindowsTerminal::new(caps)
}

const BUF_SIZE: usize = 128;

pub(crate) fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U, Error> {
    num::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}
