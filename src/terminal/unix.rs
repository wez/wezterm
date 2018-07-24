use failure::Error;
use istty::IsTty;
use libc::{self, winsize};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{stdin, stdout, Error as IoError, ErrorKind, Read, Write};
use std::mem;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};
use termios::{
    cfmakeraw, tcdrain, tcflush, tcsetattr, Termios, TCIFLUSH, TCIOFLUSH, TCOFLUSH, TCSADRAIN,
    TCSAFLUSH, TCSANOW,
};

use caps::Capabilities;
use input::{InputEvent, InputParser};
use render::terminfo::TerminfoRenderer;
use surface::Change;
use terminal::{cast, Blocking, ScreenSize, Terminal};

const BUF_SIZE: usize = 128;

/// Helper function to duplicate a file descriptor.
/// The duplicated descriptor will have the close-on-exec flag set.
fn dup(fd: RawFd) -> Result<RawFd, Error> {
    let new_fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if new_fd == -1 {
        bail!("dup of pty fd failed: {:?}", IoError::last_os_error())
    }
    Ok(new_fd)
}

pub enum Purge {
    InputQueue,
    OutputQueue,
    InputAndOutputQueue,
}

pub enum SetAttributeWhen {
    /// changes are applied immediately
    Now,
    /// Apply once the current output queue has drained
    AfterDrainOutputQueue,
    /// Wait for the current output queue to drain, then
    /// discard any unread input
    AfterDrainOutputQueuePurgeInputQueue,
}

pub trait UnixTty {
    fn get_size(&mut self) -> Result<winsize, Error>;
    fn set_size(&mut self, size: winsize) -> Result<(), Error>;
    fn get_termios(&mut self) -> Result<Termios, Error>;
    fn set_termios(&mut self, termios: &Termios, when: SetAttributeWhen) -> Result<(), Error>;
    /// Waits until all written data has been transmitted.
    fn drain(&mut self) -> Result<(), Error>;
    fn purge(&mut self, purge: Purge) -> Result<(), Error>;
}

struct Fd {
    fd: RawFd,
}

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl Fd {
    pub fn new<S: AsRawFd>(s: &S) -> Result<Self, Error> {
        ensure!(s.is_tty(), "Can only construct a TtyHandle from a tty");
        let fd = dup(s.as_raw_fd())?;
        Ok(Self { fd })
    }
}

impl Deref for Fd {
    type Target = RawFd;
    fn deref(&self) -> &RawFd {
        &self.fd
    }
}

pub struct TtyReadHandle {
    fd: Fd,
}

impl TtyReadHandle {
    fn new(fd: Fd) -> Self {
        Self { fd }
    }

    fn set_blocking(&mut self, blocking: Blocking) -> Result<(), Error> {
        let value: libc::c_int = match blocking {
            Blocking::Yes => 0,
            Blocking::No => 1,
        };
        if unsafe { libc::ioctl(*self.fd, libc::FIONBIO, &value as *const _) } != 0 {
            bail!("failed to ioctl(FIONBIO): {:?}", IoError::last_os_error());
        }
        Ok(())
    }
}

impl Read for TtyReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        let size = unsafe { libc::read(*self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(IoError::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

pub struct TtyWriteHandle {
    fd: Fd,
    write_buffer: Vec<u8>,
}

impl TtyWriteHandle {
    fn new(fd: Fd) -> Self {
        Self {
            fd,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        }
    }

    fn flush_local_buffer(&mut self) -> Result<(), IoError> {
        if self.write_buffer.len() > 0 {
            do_write(*self.fd, &self.write_buffer)?;
            self.write_buffer.clear();
        }
        Ok(())
    }
}

fn do_write(fd: RawFd, buf: &[u8]) -> Result<usize, IoError> {
    let size = unsafe { libc::write(fd, buf.as_ptr() as *const _, buf.len()) };
    if size == -1 {
        Err(IoError::last_os_error())
    } else {
        Ok(size as usize)
    }
}

impl Write for TtyWriteHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            do_write(*self.fd, buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> Result<(), IoError> {
        self.flush_local_buffer()?;
        self.drain()
            .map_err(|e| IoError::new(ErrorKind::Other, format!("{}", e)))?;
        Ok(())
    }
}

impl UnixTty for TtyWriteHandle {
    fn get_size(&mut self) -> Result<winsize, Error> {
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(*self.fd, libc::TIOCGWINSZ, &mut size) } != 0 {
            bail!("failed to ioctl(TIOCGWINSZ): {}", IoError::last_os_error());
        }
        Ok(size)
    }

    fn set_size(&mut self, size: winsize) -> Result<(), Error> {
        if unsafe { libc::ioctl(*self.fd, libc::TIOCSWINSZ, &size as *const _) } != 0 {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                IoError::last_os_error()
            );
        }

        Ok(())
    }

    fn get_termios(&mut self) -> Result<Termios, Error> {
        Termios::from_fd(*self.fd).map_err(|e| format_err!("get_termios failed: {}", e))
    }

    fn set_termios(&mut self, termios: &Termios, when: SetAttributeWhen) -> Result<(), Error> {
        let when = match when {
            SetAttributeWhen::Now => TCSANOW,
            SetAttributeWhen::AfterDrainOutputQueue => TCSADRAIN,
            SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue => TCSAFLUSH,
        };
        tcsetattr(*self.fd, when, termios).map_err(|e| format_err!("set_termios failed: {}", e))
    }

    fn drain(&mut self) -> Result<(), Error> {
        tcdrain(*self.fd).map_err(|e| format_err!("tcdrain failed: {}", e))
    }

    fn purge(&mut self, purge: Purge) -> Result<(), Error> {
        let param = match purge {
            Purge::InputQueue => TCIFLUSH,
            Purge::OutputQueue => TCOFLUSH,
            Purge::InputAndOutputQueue => TCIOFLUSH,
        };
        tcflush(*self.fd, param).map_err(|e| format_err!("tcflush failed: {}", e))
    }
}

/// A unix style terminal
pub struct UnixTerminal {
    read: TtyReadHandle,
    write: TtyWriteHandle,
    saved_termios: Termios,
    renderer: TerminfoRenderer,
    input_parser: InputParser,
    input_queue: Option<VecDeque<InputEvent>>,
}

impl UnixTerminal {
    /// Attempt to create an instance from the stdin and stdout of the
    /// process.  This will fail unless both are associated with a tty.
    /// Note that this will duplicate the underlying file descriptors
    /// and will no longer participate in the stdin/stdout locking
    /// provided by the rust standard library.
    pub fn new_from_stdio(caps: Capabilities) -> Result<UnixTerminal, Error> {
        Self::new_with(caps, &stdin(), &stdout())
    }

    pub fn new_with<A: AsRawFd, B: AsRawFd>(
        caps: Capabilities,
        read: &A,
        write: &B,
    ) -> Result<UnixTerminal, Error> {
        let read = TtyReadHandle::new(Fd::new(read)?);
        let mut write = TtyWriteHandle::new(Fd::new(write)?);
        let saved_termios = write.get_termios()?;
        let renderer = TerminfoRenderer::new(caps);
        let input_parser = InputParser::new();
        let input_queue = None;

        Ok(UnixTerminal {
            read,
            write,
            saved_termios,
            renderer,
            input_parser,
            input_queue,
        })
    }

    /// Attempt to explicitly open a handle to the terminal device
    /// (/dev/tty) and build a `UnixTerminal` from there.  This will
    /// yield a terminal even if the stdio streams have been redirected,
    /// provided that the process has an associated controlling terminal.
    pub fn new(caps: Capabilities) -> Result<UnixTerminal, Error> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        Self::new_with(caps, &file, &file)
    }
}

impl Terminal for UnixTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mut raw = self.write.get_termios()?;
        cfmakeraw(&mut raw);
        self.write
            .set_termios(&raw, SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue)
            .map_err(|e| format_err!("failed to set raw mode: {}", e))
    }

    fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
        let size = self.write.get_size()?;
        Ok(ScreenSize {
            rows: cast(size.ws_row)?,
            cols: cast(size.ws_col)?,
            xpixel: cast(size.ws_xpixel)?,
            ypixel: cast(size.ws_ypixel)?,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
        let size = winsize {
            ws_row: cast(size.rows)?,
            ws_col: cast(size.cols)?,
            ws_xpixel: cast(size.xpixel)?,
            ws_ypixel: cast(size.ypixel)?,
        };

        self.write.set_size(size)
    }
    fn render(&mut self, changes: &[Change]) -> Result<(), Error> {
        self.renderer
            .render_to(changes, &mut self.read, &mut self.write)
    }
    fn flush(&mut self) -> Result<(), Error> {
        self.write
            .flush()
            .map_err(|e| format_err!("flush failed: {}", e))
    }

    fn poll_input(&mut self, blocking: Blocking) -> Result<Option<InputEvent>, Error> {
        if let Some(ref mut queue) = self.input_queue {
            if let Some(event) = queue.pop_front() {
                return Ok(Some(event));
            }
        }

        self.read.set_blocking(blocking)?;

        let mut buf = [0u8; 64];
        match self.read.read(&mut buf) {
            Ok(n) => {
                // A little bit of a dance with moving the queue out of self
                // to appease the borrow checker.  We'll need to be sure to
                // move it back before we return!
                let mut queue = match self.input_queue.take() {
                    Some(queue) => queue,
                    None => VecDeque::new(),
                };
                self.input_parser
                    .parse(&buf[0..n], |evt| queue.push_back(evt), n == buf.len());
                let result = queue.pop_front();
                // Move the queue back into self before we leave this scope
                self.input_queue = Some(queue);
                Ok(result)
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(format_err!("failed to read input {}", e)),
        }
    }
}

impl Drop for UnixTerminal {
    fn drop(&mut self) {
        self.write
            .set_termios(&self.saved_termios, SetAttributeWhen::Now)
            .expect("failed to restore original termios state");
    }
}
