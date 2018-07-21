//! An abstraction over a terminal device
//! `Terminal` implements `Read` and `Write` and offers methods
//! for changing the input mode.  The interface considers the differences
//! between POSIX and Windows systems, but is implemented only for POSIX
//! at this time.

use failure::Error;
use istty::IsTty;
#[cfg(unix)]
use libc::{self, winsize};
use num::{self, NumCast};
use std::fmt::Display;
use std::fs::File;
use std::io::{stdin, stdout, Stdin, Stdout};
use std::io::{Error as IOError, Read, Result as IOResult, Write};
use std::mem;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, RawHandle};
#[cfg(unix)]
use termios::{cfmakeraw, tcsetattr, Termios, TCSANOW};
#[cfg(windows)]
use winapi::um::consoleapi;
#[cfg(windows)]
use winapi::um::wincon::{
    GetConsoleScreenBufferInfo, SetConsoleScreenBufferSize, CONSOLE_SCREEN_BUFFER_INFO, COORD,
    DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
};

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

#[cfg(unix)]
impl Handle {
    fn writable_fd(&self) -> RawFd {
        match self {
            Handle::File(f) => f.as_raw_fd(),
            Handle::Stdio { stdout, .. } => stdout.as_raw_fd(),
        }
    }
}

#[cfg(windows)]
impl Handle {
    fn writable_handle(&self) -> RawHandle {
        match self {
            Handle::File(f) => f.as_raw_handle(),
            Handle::Stdio { stdout, .. } => stdout.as_raw_handle(),
        }
    }

    fn readable_handle(&self) -> RawHandle {
        match self {
            Handle::File(f) => f.as_raw_handle(),
            Handle::Stdio { stdin, .. } => stdin.as_raw_handle(),
        }
    }

    fn enable_virtual_terminal_processing(&self) -> Result<(), Error> {
        let mode = self.get_console_output_mode()?;
        self.set_console_output_mode(
            mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN,
        )?;

        let mode = self.get_console_input_mode()?;
        self.set_console_output_mode(mode | ENABLE_VIRTUAL_TERMINAL_INPUT)?;
        Ok(())
    }

    fn get_console_input_mode(&self) -> Result<u32, Error> {
        let mut mode = 0;
        let handle = self.readable_handle();
        if unsafe { consoleapi::GetConsoleMode(handle, &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }

    fn set_console_input_mode(&self, mode: u32) -> Result<(), Error> {
        let handle = self.readable_handle();
        if unsafe { consoleapi::SetConsoleMode(handle, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }

    fn get_console_output_mode(&self) -> Result<u32, Error> {
        let mut mode = 0;
        let handle = self.writable_handle();
        if unsafe { consoleapi::GetConsoleMode(handle, &mut mode) } == 0 {
            bail!("GetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(mode)
    }

    fn set_console_output_mode(&self, mode: u32) -> Result<(), Error> {
        let handle = self.writable_handle();
        if unsafe { consoleapi::SetConsoleMode(handle, mode) } == 0 {
            bail!("SetConsoleMode failed: {}", IOError::last_os_error());
        }
        Ok(())
    }
}

const BUF_SIZE: usize = 128;

#[cfg(windows)]
pub struct WindowsTerminal {
    handle: Handle,
    write_buffer: Vec<u8>,
    saved_input_mode: u32,
    saved_output_mode: u32,
}

#[cfg(windows)]
impl Drop for WindowsTerminal {
    fn drop(&mut self) {
        self.handle
            .set_console_input_mode(self.saved_input_mode)
            .expect("failed to restore console input mode");
        self.handle
            .set_console_output_mode(self.saved_output_mode)
            .expect("failed to restore console output mode");
    }
}

#[cfg(windows)]
impl WindowsTerminal {
    pub fn new() -> Result<Self, Error> {
        let read = stdin();
        let write = stdout();

        if !read.is_tty() || !write.is_tty() {
            bail!("stdin and stdout must both be tty handles");
        }

        let handle = Handle::Stdio {
            stdin: read,
            stdout: write,
        };

        let saved_input_mode = handle.get_console_input_mode()?;
        let saved_output_mode = handle.get_console_output_mode()?;

        handle.enable_virtual_terminal_processing()?;

        Ok(Self {
            handle,
            saved_input_mode,
            saved_output_mode,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        })
    }
}

#[cfg(windows)]
impl Read for WindowsTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.handle.read(buf)
    }
}

#[cfg(windows)]
impl Write for WindowsTerminal {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.handle.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        if self.write_buffer.len() > 0 {
            self.handle.write(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        self.handle.flush()
    }
}

#[cfg(windows)]
impl Terminal for WindowsTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mode = self.handle.get_console_input_mode()?;

        self.handle.set_console_input_mode(
            mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT),
        )
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { mem::zeroed() };
        let handle = self.handle.writable_handle();
        let ok = unsafe { GetConsoleScreenBufferInfo(handle, &mut info as *mut _) };
        if ok != 1 {
            bail!(
                "failed to GetConsoleScreenBufferInfo: {}",
                IOError::last_os_error()
            );
        }

        Ok(ScreenSize {
            rows: cast(info.dwSize.Y)?,
            cols: cast(info.dwSize.X)?,
            xpixel: 0,
            ypixel: 0,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
        let size = COORD {
            X: cast(size.cols)?,
            Y: cast(size.rows)?,
        };
        let handle = self.handle.writable_handle();
        if unsafe { SetConsoleScreenBufferSize(handle, size) } != 1 {
            bail!(
                "failed to SetConsoleScreenBufferSize: {}",
                IOError::last_os_error()
            );
        }
        Ok(())
    }
}

/// A unix style terminal
#[cfg(unix)]
pub struct UnixTerminal {
    handle: Handle,
    saved_termios: Termios,
    write_buffer: Vec<u8>,
}

#[cfg(unix)]
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
            write_buffer: Vec::with_capacity(BUF_SIZE),
        })
    }

    /// Attempt to explicitly open a handle to the terminal device
    /// (/dev/tty) and build a `UnixTerminal` from there.  This will
    /// yield a terminal even if the stdio streams have been redirected,
    /// provided that the process has an associated controlling terminal.
    pub fn new() -> Result<UnixTerminal, Error> {
        use std::fs::OpenOptions;
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let saved_termios = Termios::from_fd(file.as_raw_fd())?;
        Ok(UnixTerminal {
            handle: Handle::File(file),
            saved_termios,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        })
    }
}

#[cfg(unix)]
impl Read for UnixTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.handle.read(buf)
    }
}

#[cfg(unix)]
impl Write for UnixTerminal {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.handle.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> IOResult<()> {
        if self.write_buffer.len() > 0 {
            self.handle.write(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        self.handle.flush()
    }
}

fn cast<T: NumCast + Display + Copy, U: NumCast>(n: T) -> Result<U, Error> {
    num::cast(n).ok_or_else(|| format_err!("{} is out of bounds for this system", n))
}

#[cfg(unix)]
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

#[cfg(unix)]
impl Drop for UnixTerminal {
    fn drop(&mut self) {
        let fd = self.handle.writable_fd();
        tcsetattr(fd, TCSANOW, &self.saved_termios)
            .expect("failed to restore original termios state");
    }
}
