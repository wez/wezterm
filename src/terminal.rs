//! An abstraction over a terminal device
//! `Terminal` implements `Read` and `Write` and offers methods
//! for changing the input mode.  The interface considers the differences
//! between POSIX and Windows systems, but is implemented only for POSIX
//! at this time.

use failure::Error;
use istty::IsTty;
use libc::{self, winsize};
use num::{self, NumCast};
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::{stdin, stdout, Stdin, Stdout};
use std::io::{Error as IOError, Read, Result as IOResult, Write};
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use termios::{cfmakeraw, tcsetattr, Termios, TCSANOW};

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

impl Handle {
    fn read<T, F: FnOnce(&mut Read) -> T>(&mut self, func: F) -> T {
        match self {
            Handle::File(f) => func(f),
            Handle::Stdio { stdin, .. } => func(stdin),
        }
    }

    fn write<T, F: FnOnce(&mut Write) -> T>(&mut self, func: F) -> T {
        match self {
            Handle::File(f) => func(f),
            Handle::Stdio { stdout, .. } => func(stdout),
        }
    }

    fn writable_fd(&self) -> RawFd {
        match self {
            Handle::File(f) => f.as_raw_fd(),
            Handle::Stdio { stdout, .. } => stdout.as_raw_fd(),
        }
    }
}

/// A unix style terminal
pub struct UnixTerminal {
    handle: Handle,
    saved_termios: Termios,
}

impl UnixTerminal {
    /// Attempt to create an instance from the stdin and stdout of the
    /// process.  This will fail unless both are associated with a tty.
    pub fn new_from_stdio() -> Result<UnixTerminal, Error> {
        let read = stdin();
        let write = stdout();

        if !read.is_tty() || !write.is_tty() {
            bail!("stdin and stdout must both be tty handles");
        }
        let saved_termios = Termios::from_fd(write.as_raw_fd())?;

        Ok(UnixTerminal {
            handle: Handle::Stdio {
                stdin: read,
                stdout: write,
            },
            saved_termios,
        })
    }

    /// Attempt to explicitly open a handle to the terminal device
    /// (/dev/tty) and build a `UnixTerminal` from there.  This will
    /// yield a terminal even if the stdio streams have been redirected,
    /// provided that the process has an associated controlling terminal.
    pub fn new() -> Result<UnixTerminal, Error> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let saved_termios = Termios::from_fd(file.as_raw_fd())?;
        Ok(UnixTerminal {
            handle: Handle::File(file),
            saved_termios,
        })
    }
}

impl Read for UnixTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.handle.read(|r| r.read(buf))
    }
}

impl Write for UnixTerminal {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        self.handle.write(|w| w.write(buf))
    }

    fn flush(&mut self) -> IOResult<()> {
        self.handle.write(|w| w.flush())
    }
}

fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U, Error> {
    num::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}

impl Terminal for UnixTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let fd = self.handle.writable_fd();
        let mut raw = Termios::from_fd(fd)?;
        cfmakeraw(&mut raw);
        tcsetattr(fd, TCSANOW, &raw).map_err(|e| format_err!("failed to set raw mode: {}", e))
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let fd = self.handle.writable_fd();
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size as *mut _) } != 0 {
            bail!("failed to ioctl(TIOCGWINSZ): {}", IOError::last_os_error());
        }
        Ok(ScreenSize {
            rows: cast(size.ws_row)?,
            cols: cast(size.ws_col)?,
            xpixel: cast(size.ws_xpixel)?,
            ypixel: cast(size.ws_ypixel)?,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
        let fd = self.handle.writable_fd();

        let size = winsize {
            ws_row: cast(size.rows)?,
            ws_col: cast(size.cols)?,
            ws_xpixel: cast(size.xpixel)?,
            ws_ypixel: cast(size.ypixel)?,
        };

        if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &size as *const _) } != 0 {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                IOError::last_os_error()
            );
        }

        Ok(())
    }
}

impl Drop for UnixTerminal {
    fn drop(&mut self) {
        let fd = self.handle.writable_fd();
        tcsetattr(fd, TCSANOW, &self.saved_termios)
            .expect("failed to restore original termios state");
    }
}
