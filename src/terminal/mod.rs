//! An abstraction over a terminal device
//! `Terminal` implements `Read` and `Write` and offers methods
//! for changing the input mode.  The interface considers the differences
//! between POSIX and Windows systems, but is implemented only for POSIX
//! at this time.

use failure::Error;
use num::{self, NumCast};
use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Result as IOResult, Stdin, Stdout, Write};

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

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

    /*
    /// Sets the terminal to cooked mode, which is essentially the opposite
    /// to raw mode: input and output processing are enabled.
    fn set_cooked_mode(&mut self) -> Result<(), Error>;
    */
}

enum Handle {
    File(File),
    Stdio { stdin: Stdin, stdout: Stdout },
}

impl Read for Handle {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        match self {
            Handle::File(f) => f.read(buf),
            Handle::Stdio { stdin, .. } => stdin.read(buf),
        }
    }
}

impl Write for Handle {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        match self {
            Handle::File(f) => f.write(buf),
            Handle::Stdio { stdout, .. } => stdout.write(buf),
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        match self {
            Handle::File(f) => f.flush(),
            Handle::Stdio { stdout, .. } => stdout.flush(),
        }
    }
}

const BUF_SIZE: usize = 128;

fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U, Error> {
    num::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}

#[cfg(unix)]
pub use self::unix::UnixTerminal;
#[cfg(windows)]
pub use self::windows::WindowsTerminal;
