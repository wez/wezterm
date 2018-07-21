use failure::Error;
use istty::IsTty;
use libc::{self, winsize};
use std::io::{stdin, stdout, Error as IOError, Read, Result as IOResult, Write};
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use termios::{cfmakeraw, tcsetattr, Termios, TCSANOW};

use terminal::{cast, Handle, ScreenSize, Terminal, BUF_SIZE};

impl Handle {
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
    write_buffer: Vec<u8>,
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

impl Read for UnixTerminal {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.handle.read(buf)
    }
}

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
