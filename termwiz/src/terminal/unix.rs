use crate::istty::IsTty;
use failure::{bail, ensure, format_err, Error, Fallible};
use libc::{self, poll, pollfd, winsize, POLLIN};
use signal_hook::{self, SigId};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{stdin, stdout, Error as IoError, ErrorKind, Read, Write};
use std::mem;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use termios::{
    cfmakeraw, tcdrain, tcflush, tcsetattr, Termios, TCIFLUSH, TCIOFLUSH, TCOFLUSH, TCSADRAIN,
    TCSAFLUSH, TCSANOW,
};

use crate::caps::Capabilities;
use crate::escape::csi::{DecPrivateMode, DecPrivateModeCode, Mode, CSI};
use crate::input::{InputEvent, InputParser};
use crate::render::terminfo::TerminfoRenderer;
use crate::surface::Change;
use crate::terminal::{cast, Blocking, ScreenSize, Terminal};

const BUF_SIZE: usize = 4096;

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
            Blocking::Wait => 0,
            Blocking::DoNotWait => 1,
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
        if !self.write_buffer.is_empty() {
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
    input_queue: VecDeque<InputEvent>,
    sigwinch_id: SigId,
    sigwinch_pipe: UnixStream,
    wake_pipe: UnixStream,
    wake_pipe_write: Arc<Mutex<UnixStream>>,
    caps: Capabilities,
    in_alternate_screen: bool,
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
        let mut read = TtyReadHandle::new(Fd::new(read)?);
        let mut write = TtyWriteHandle::new(Fd::new(write)?);
        let saved_termios = write.get_termios()?;
        let renderer = TerminfoRenderer::new(caps.clone());
        let input_parser = InputParser::new();
        let input_queue = VecDeque::new();

        let (sigwinch_pipe, sigwinch_pipe_write) = UnixStream::pair()?;
        let sigwinch_id = signal_hook::pipe::register(libc::SIGWINCH, sigwinch_pipe_write)?;
        sigwinch_pipe.set_nonblocking(true)?;
        let (wake_pipe, wake_pipe_write) = UnixStream::pair()?;
        wake_pipe.set_nonblocking(true)?;

        read.set_blocking(Blocking::DoNotWait)?;

        Ok(UnixTerminal {
            caps,
            read,
            write,
            saved_termios,
            renderer,
            input_parser,
            input_queue,
            sigwinch_pipe,
            sigwinch_id,
            wake_pipe,
            wake_pipe_write: Arc::new(Mutex::new(wake_pipe_write)),
            in_alternate_screen: false,
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

    /// Test whether we caught delivery of SIGWINCH.
    /// If so, yield an `InputEvent` with the current size of the tty.
    fn caught_sigwinch(&mut self) -> Result<Option<InputEvent>, Error> {
        let mut buf = [0u8; 64];

        match self.sigwinch_pipe.read(&mut buf) {
            Ok(_) => {
                let size = self.write.get_size()?;
                Ok(Some(InputEvent::Resized {
                    rows: cast(size.ws_row)?,
                    cols: cast(size.ws_col)?,
                }))
            }
            Err(ref e)
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted =>
            {
                Ok(None)
            }
            Err(e) => Err(format_err!("failed to read sigwinch pipe {}", e)),
        }
    }
}

#[derive(Clone)]
pub struct UnixTerminalWaker {
    pipe: Arc<Mutex<UnixStream>>,
}

impl UnixTerminalWaker {
    pub fn wake(&self) -> Result<(), IoError> {
        let mut pipe = self.pipe.lock().unwrap();
        let _ = pipe.write(b"W")?;
        Ok(())
    }
}

impl Terminal for UnixTerminal {
    fn set_raw_mode(&mut self) -> Result<(), Error> {
        let mut raw = self.write.get_termios()?;
        cfmakeraw(&mut raw);
        self.write
            .set_termios(&raw, SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue)
            .map_err(|e| format_err!("failed to set raw mode: {}", e))?;

        macro_rules! decset {
            ($variant:ident) => {
                write!(
                    self.write,
                    "{}",
                    CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                        DecPrivateModeCode::$variant
                    )))
                )?;
            };
        }

        if self.caps.bracketed_paste() {
            decset!(BracketedPaste);
        }
        if self.caps.mouse_reporting() {
            decset!(AnyEventMouse);
            decset!(SGRMouse);
        }
        self.write.flush()?;

        Ok(())
    }

    fn set_cooked_mode(&mut self) -> Fallible<()> {
        self.write
            .set_termios(&self.saved_termios, SetAttributeWhen::Now)
    }

    fn enter_alternate_screen(&mut self) -> Result<(), Error> {
        if !self.in_alternate_screen {
            write!(
                self.write,
                "{}",
                CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::ClearAndEnableAlternateScreen
                )))
            )?;
            self.in_alternate_screen = true;
        }
        Ok(())
    }

    fn exit_alternate_screen(&mut self) -> Result<(), Error> {
        if self.in_alternate_screen {
            write!(
                self.write,
                "{}",
                CSI::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                    DecPrivateModeCode::ClearAndEnableAlternateScreen
                )))
            )?;
            self.in_alternate_screen = false;
        }
        Ok(())
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

    fn poll_input(&mut self, wait: Option<Duration>) -> Result<Option<InputEvent>, Error> {
        if let Some(event) = self.input_queue.pop_front() {
            return Ok(Some(event));
        }

        // Some unfortunately verbose code here.  In order to safely hook and process
        // SIGWINCH we need to use the self-pipe trick to deliver signals to a pipe
        // so that we can use poll(2) to wait for events on both the tty input and
        // the sigwinch pipe at the same time.  In theory we could do away with this
        // and use sigaction to register SIGWINCH without SA_RESTART set; that way
        // we could do a blocking read and have it get EINTR on a resize.
        // Doing such a thing may introduce more problems for other components in
        // the rust crate ecosystem if they're not ready to deal with EINTR, so
        // we opt to take on the complexity here to make things overall easier to
        // integrate.

        let mut pfd = [
            pollfd {
                fd: self.sigwinch_pipe.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
            pollfd {
                fd: self.read.fd.fd,
                events: POLLIN,
                revents: 0,
            },
            pollfd {
                fd: self.wake_pipe.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        self.read.set_blocking(if wait.is_none() {
            Blocking::Wait
        } else {
            Blocking::DoNotWait
        })?;

        let poll_result = unsafe {
            poll(
                pfd.as_mut_ptr(),
                pfd.len() as _,
                wait.map(|wait| wait.as_millis() as libc::c_int)
                    .unwrap_or(-1),
            )
        };
        if poll_result < 0 {
            let err = IoError::last_os_error();

            if err.kind() == ErrorKind::Interrupted {
                // SIGWINCH may have been the source of the interrupt.
                // Check for that now so that we reduce the latency of
                // processing the resize
                if let Some(resize) = self.caught_sigwinch()? {
                    return Ok(Some(resize));
                }

                return Ok(None);
            }
            return Err(format_err!("poll(2) error: {}", err));
        }

        if pfd[0].revents != 0 {
            // SIGWINCH received via our pipe?
            if let Some(resize) = self.caught_sigwinch()? {
                return Ok(Some(resize));
            }
        }

        if pfd[1].revents != 0 {
            let mut buf = [0u8; 64];
            match self.read.read(&mut buf) {
                Ok(n) => {
                    let input_queue = &mut self.input_queue;
                    self.input_parser.parse(
                        &buf[0..n],
                        |evt| input_queue.push_back(evt),
                        n == buf.len(),
                    );
                    return Ok(self.input_queue.pop_front());
                }
                Err(ref e)
                    if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(format_err!("failed to read input {}", e)),
            }
        }

        if pfd[2].revents != 0 {
            let mut buf = [0u8; 64];
            if self.wake_pipe.read(&mut buf).is_ok() {
                return Ok(Some(InputEvent::Wake));
            }
        }

        Ok(None)
    }

    fn waker(&self) -> UnixTerminalWaker {
        UnixTerminalWaker {
            pipe: self.wake_pipe_write.clone(),
        }
    }
}

impl Drop for UnixTerminal {
    fn drop(&mut self) {
        macro_rules! decreset {
            ($variant:ident) => {
                write!(
                    self.write,
                    "{}",
                    CSI::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                        DecPrivateModeCode::$variant
                    )))
                )
                .unwrap();
            };
        }
        if self.caps.bracketed_paste() {
            decreset!(BracketedPaste);
        }
        if self.caps.mouse_reporting() {
            decreset!(SGRMouse);
            decreset!(AnyEventMouse);
        }
        self.exit_alternate_screen().unwrap();
        self.write.flush().unwrap();

        signal_hook::unregister(self.sigwinch_id);
        self.write
            .set_termios(&self.saved_termios, SetAttributeWhen::Now)
            .expect("failed to restore original termios state");
    }
}
