use crate::render::RenderTty;
use crate::terminal::ProbeCapabilities;
use crate::{bail, Context, Result};
use filedescriptor::{poll, pollfd, FileDescriptor, POLLIN};
use libc::{self, winsize};
use signal_hook::{self, SigId};
use std::borrow::BorrowMut;
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::error::Error as _;
use std::fs::OpenOptions;
use std::io::{stdin, stdout, Error as IoError, ErrorKind, Read, Write};
use std::iter::Once;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use termios::{
    cfmakeraw, tcdrain, tcflush, tcsetattr, Termios, TCIFLUSH, TCIOFLUSH, TCOFLUSH, TCSADRAIN,
    TCSAFLUSH, TCSANOW,
};
use wezterm_input_types::KittyKeyboardFlags;

use crate::caps::Capabilities;
use crate::escape::csi::{
    DecPrivateMode, DecPrivateModeCode, Keyboard, Mode, XtermKeyModifierResource,
    XtermModifyOtherKeys, CSI,
};
use crate::input::{InputEvent, InputParser, KeyboardEncoding};
use crate::render::terminfo::TerminfoRenderer;
use crate::surface::Change;
use crate::terminal::{cast, Blocking, ScreenSize, Terminal};

const BUF_SIZE: usize = 4096;

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
    fn get_size(&mut self) -> Result<winsize>;
    fn set_size(&mut self, size: winsize) -> Result<()>;
    fn get_termios(&mut self) -> Result<Termios>;
    fn set_termios(&mut self, termios: &Termios, when: SetAttributeWhen) -> Result<()>;
    fn modify_termios(
        &mut self,
        modifier: impl FnOnce(&mut Termios),
        when: SetAttributeWhen,
    ) -> Result<()> {
        self.get_termios().and_then(|mut termios| {
            modifier(&mut termios);
            self.set_termios(&termios, when)
        })
    }
    /// Waits until all written data has been transmitted.
    fn drain(&mut self) -> Result<()>;
    fn purge(&mut self, purge: Purge) -> Result<()>;
}

pub struct TtyReadHandle {
    fd: FileDescriptor,
}

impl TtyReadHandle {
    fn new(fd: FileDescriptor) -> Self {
        Self { fd }
    }

    fn set_blocking(&mut self, blocking: Blocking) -> Result<()> {
        self.fd.set_non_blocking(blocking == Blocking::DoNotWait)?;
        Ok(())
    }
}

impl Read for TtyReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, IoError> {
        let size =
            unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(IoError::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

pub struct TtyWriteHandle {
    fd: FileDescriptor,
    write_buffer: Vec<u8>,
}

impl TtyWriteHandle {
    fn new(fd: FileDescriptor) -> Self {
        Self {
            fd,
            write_buffer: Vec::with_capacity(BUF_SIZE),
        }
    }

    fn flush_local_buffer(&mut self) -> std::result::Result<(), IoError> {
        if !self.write_buffer.is_empty() {
            self.fd.write_all(&self.write_buffer)?;
            self.write_buffer.clear();
        }
        Ok(())
    }

    fn modify_other_keys(&mut self, level: XtermModifyOtherKeys) -> std::io::Result<()> {
        write!(
            self,
            "{}",
            CSI::Mode(Mode::XtermKeyMode {
                resource: XtermKeyModifierResource::OtherKeys,
                value: Some(level as i64),
            })
        )
    }
}

impl Write for TtyWriteHandle {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, IoError> {
        if self.write_buffer.len() + buf.len() > self.write_buffer.capacity() {
            self.flush()?;
        }
        if buf.len() >= self.write_buffer.capacity() {
            self.fd.write(buf)
        } else {
            self.write_buffer.write(buf)
        }
    }

    fn flush(&mut self) -> std::result::Result<(), IoError> {
        self.flush_local_buffer()?;
        self.drain()
            .map_err(|e| IoError::new(ErrorKind::Other, format!("{}", e)))?;
        Ok(())
    }
}

impl RenderTty for TtyWriteHandle {
    fn get_size_in_cells(&mut self) -> Result<(usize, usize)> {
        let size = self.get_size()?;
        Ok((size.ws_col as usize, size.ws_row as usize))
    }
}

impl UnixTty for TtyWriteHandle {
    fn get_size(&mut self) -> Result<winsize> {
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(self.fd.as_raw_fd(), libc::TIOCGWINSZ as _, &mut size) } != 0 {
            bail!("failed to ioctl(TIOCGWINSZ): {}", IoError::last_os_error());
        }
        Ok(size)
    }

    fn set_size(&mut self, size: winsize) -> Result<()> {
        if unsafe {
            libc::ioctl(
                self.fd.as_raw_fd(),
                libc::TIOCSWINSZ as _,
                &size as *const _,
            )
        } != 0
        {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                IoError::last_os_error()
            );
        }

        Ok(())
    }

    fn get_termios(&mut self) -> Result<Termios> {
        Termios::from_fd(self.fd.as_raw_fd()).context("get_termios failed")
    }

    fn set_termios(&mut self, termios: &Termios, when: SetAttributeWhen) -> Result<()> {
        let when = match when {
            SetAttributeWhen::Now => TCSANOW,
            SetAttributeWhen::AfterDrainOutputQueue => TCSADRAIN,
            SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue => TCSAFLUSH,
        };
        tcsetattr(self.fd.as_raw_fd(), when, termios).context("set_termios failed")
    }

    fn drain(&mut self) -> Result<()> {
        tcdrain(self.fd.as_raw_fd()).context("tcdrain failed")
    }

    fn purge(&mut self, purge: Purge) -> Result<()> {
        let param = match purge {
            Purge::InputQueue => TCIFLUSH,
            Purge::OutputQueue => TCOFLUSH,
            Purge::InputAndOutputQueue => TCIOFLUSH,
        };
        tcflush(self.fd.as_raw_fd(), param).context("tcflush failed")
    }
}

/// A unix style terminal
pub struct UnixTerminal {
    read: TtyReadHandle,
    write: TermiosGuard<TtyWriteHandle>,
    renderer: TerminfoRenderer,
    input_parser: InputParser,
    input_queue: VecDeque<InputEvent>,
    sigwinch_id: SigId,
    sigwinch_pipe: UnixStream,
    wake_pipe: UnixStream,
    wake_pipe_write: Arc<Mutex<UnixStream>>,
    caps: Capabilities,
    in_alternate_screen: bool,
    keyboard_enhancement_supported: Option<bool>,
    saved_keyboard_encoding: Option<KeyboardEncoding>,
}

impl UnixTerminal {
    /// Attempt to create an instance from the stdin and stdout of the
    /// process.  This will fail unless both are associated with a tty.
    /// Note that this will duplicate the underlying file descriptors
    /// and will no longer participate in the stdin/stdout locking
    /// provided by the rust standard library.
    pub fn new_from_stdio(caps: Capabilities) -> Result<UnixTerminal> {
        Self::new_with(caps, &stdin(), &stdout())
    }

    pub fn new_with<A: AsRawFd, B: AsRawFd>(
        caps: Capabilities,
        read: &A,
        write: &B,
    ) -> Result<UnixTerminal> {
        let read = TtyReadHandle::new(FileDescriptor::dup(read)?);
        let write = TermiosGuard::new(TtyWriteHandle::new(FileDescriptor::dup(write)?))?;
        let renderer = TerminfoRenderer::new(caps.clone());
        let input_parser = InputParser::new();
        let input_queue = VecDeque::new();

        let (sigwinch_pipe, sigwinch_pipe_write) = UnixStream::pair()?;
        let sigwinch_id =
            signal_hook::low_level::pipe::register(libc::SIGWINCH, sigwinch_pipe_write)?;
        sigwinch_pipe.set_nonblocking(true)?;
        let (wake_pipe, wake_pipe_write) = UnixStream::pair()?;
        wake_pipe.set_nonblocking(true)?;
        wake_pipe_write.set_nonblocking(true)?;

        Ok(UnixTerminal {
            caps,
            read,
            write,
            renderer,
            input_parser,
            input_queue,
            sigwinch_pipe,
            sigwinch_id,
            wake_pipe,
            wake_pipe_write: Arc::new(Mutex::new(wake_pipe_write)),
            in_alternate_screen: false,
            keyboard_enhancement_supported: None,
            saved_keyboard_encoding: None,
        })
    }

    /// Attempt to explicitly open a handle to the terminal device
    /// (/dev/tty) and build a `UnixTerminal` from there.  This will
    /// yield a terminal even if the stdio streams have been redirected,
    /// provided that the process has an associated controlling terminal.
    pub fn new(caps: Capabilities) -> Result<UnixTerminal> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        Self::new_with(caps, &file, &file)
    }

    /// Test whether we caught delivery of SIGWINCH.
    /// If so, yield an `InputEvent` with the current size of the tty.
    fn caught_sigwinch(&mut self) -> Result<Option<InputEvent>> {
        let mut buf = [0u8; 64];

        match self.sigwinch_pipe.read(&mut buf) {
            Ok(0) => Ok(None),
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
            Err(e) => bail!("failed to read sigwinch pipe {}", e),
        }
    }

    pub fn get_keyboard_encoding(&mut self) -> Result<KeyboardEncoding> {
        Ok(self
            .probe_capabilities()
            .map(|mut probe| probe.enhanced_keyboard_state())
            .transpose()?
            .flatten()
            .map(|kitty| KeyboardEncoding::Kitty(kitty))
            .unwrap_or(KeyboardEncoding::Xterm))
    }

    /// Check if the terminal supports Kitty keyboard enhancement protocol
    pub fn keyboard_enhancement_supported(&mut self) -> Result<bool> {
        match self.keyboard_enhancement_supported {
            Some(x) => Ok(x),
            None => {
                let result = self
                    .probe_capabilities()
                    .map(|mut probe| probe.enhanced_keyboard_state())
                    .transpose()?
                    .flatten()
                    .is_some();
                self.keyboard_enhancement_supported = Some(result);
                Ok(result)
            }
        }
    }

    /// Restores the keyboard encoding to its original state, if it had been changed
    pub fn restore_keyboard_encoding(&mut self) -> Result<()> {
        self.saved_keyboard_encoding
            .map_or(Ok(()), |enc| self.set_keyboard_encoding(enc))
    }
}

#[derive(Clone)]
pub struct UnixTerminalWaker {
    pipe: Arc<Mutex<UnixStream>>,
}

impl UnixTerminalWaker {
    pub fn wake(&self) -> std::result::Result<(), IoError> {
        let mut pipe = self.pipe.lock().unwrap();
        match pipe.write(b"W") {
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => Ok(()),
                _ => Err(e),
            },
            Ok(_) => Ok(()),
        }
    }
}

impl Terminal for UnixTerminal {
    fn set_raw_mode(&mut self) -> Result<()> {
        self.write.modify_termios(
            cfmakeraw,
            SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue,
        )?;

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

        if self.caps.probe_for_enhanced_keyboard() && self.keyboard_enhancement_supported()? {
            if self.saved_keyboard_encoding.is_none() {
                // don't override the original state if this is called multiple times
                self.saved_keyboard_encoding = Some(self.get_keyboard_encoding()?);
            }
            self.set_keyboard_encoding(KeyboardEncoding::Kitty(KittyKeyboardFlags::all()))?;
        } else {
            self.write
                .modify_other_keys(XtermModifyOtherKeys::Enabled)?;
        }
        self.flush()
    }

    fn set_cooked_mode(&mut self) -> Result<()> {
        self.write
            .modify_other_keys(XtermModifyOtherKeys::Partial)?;
        self.restore_keyboard_encoding()?;
        // FIXME: this only works if the original mode was cooked.
        // There's no `termios::cfmakesane()`, so we'd have to set the flags manually
        self.write.restore_termios()
    }

    fn enter_alternate_screen(&mut self) -> Result<()> {
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

    fn exit_alternate_screen(&mut self) -> Result<()> {
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

    fn get_screen_size(&mut self) -> Result<ScreenSize> {
        let size = self.write.get_size()?;
        Ok(ScreenSize {
            rows: cast(size.ws_row)?,
            cols: cast(size.ws_col)?,
            xpixel: cast(size.ws_xpixel)?,
            ypixel: cast(size.ws_ypixel)?,
        })
    }

    fn probe_capabilities(&mut self) -> Option<ProbeCapabilities> {
        // enter raw mode
        let raw_writer = self.write.with_modified_termios(cfmakeraw).ok()?;
        Some(ProbeCapabilities::new(&mut self.read, raw_writer))
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> Result<()> {
        let size = winsize {
            ws_row: cast(size.rows)?,
            ws_col: cast(size.cols)?,
            ws_xpixel: cast(size.xpixel)?,
            ws_ypixel: cast(size.ypixel)?,
        };

        self.write.set_size(size)
    }
    fn render(&mut self, changes: &[Change]) -> Result<()> {
        self.renderer.render_to(changes, &mut *self.write)
    }
    fn flush(&mut self) -> Result<()> {
        self.write.flush().context("flush failed")
    }

    fn poll_input(&mut self, wait: Option<Duration>) -> Result<Option<InputEvent>> {
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
                fd: self.read.fd.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
            pollfd {
                fd: self.wake_pipe.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        if let Err(err) = poll(&mut pfd, wait) {
            return match err
                .source()
                .ok_or_else(|| anyhow::anyhow!("error has no source! {:#}", err))?
                .downcast_ref::<std::io::Error>()
            {
                Some(err) => {
                    if err.kind() == ErrorKind::Interrupted {
                        // SIGWINCH may have been the source of the interrupt.
                        // Check for that now so that we reduce the latency of
                        // processing the resize
                        if let Some(resize) = self.caught_sigwinch()? {
                            Ok(Some(resize))
                        } else {
                            Ok(None)
                        }
                    } else {
                        bail!("poll(2) error: {}", err)
                    }
                }
                None => bail!("poll(2) error: {}", err),
            };
        };

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
                Err(e) => bail!("failed to read input {}", e),
            }
        }

        if pfd[2].revents != 0 {
            let mut buf = [0u8; 64];
            if let Ok(n) = self.wake_pipe.read(&mut buf) {
                if n > 0 {
                    return Ok(Some(InputEvent::Wake));
                }
            }
        }

        Ok(None)
    }

    fn waker(&self) -> UnixTerminalWaker {
        UnixTerminalWaker {
            pipe: self.wake_pipe_write.clone(),
        }
    }

    fn set_keyboard_encoding(&mut self, encoding: KeyboardEncoding) -> Result<()> {
        let is_kitty_supported = self.keyboard_enhancement_supported()?;
        match encoding {
            KeyboardEncoding::Xterm => {
                if is_kitty_supported {
                    write!(
                        self.write,
                        "{}",
                        CSI::Keyboard(Keyboard::PushKittyState {
                            flags: KittyKeyboardFlags::empty(),
                            mode: crate::escape::csi::KittyKeyboardMode::AssignAll
                        })
                    )?;
                }
            }
            KeyboardEncoding::Kitty(flags) if is_kitty_supported => {
                write!(
                    self.write,
                    "{}",
                    // Ideally, we'd use SetKittyState, but it doesn't work on some terminals (iTerm2)
                    CSI::Keyboard(Keyboard::PushKittyState {
                        flags,
                        mode: crate::escape::csi::KittyKeyboardMode::AssignAll
                    })
                )?;
            }
            _ => bail!("Unsupported keyboard encoding {encoding:?} for Unix terminal"),
        }
        Ok(())
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

        self.restore_keyboard_encoding().unwrap();
        self.render(&[Change::CursorVisibility(
            crate::surface::CursorVisibility::Visible,
        )])
        .ok();
        if self.caps.bracketed_paste() {
            decreset!(BracketedPaste);
        }
        if self.caps.mouse_reporting() {
            decreset!(SGRMouse);
            decreset!(AnyEventMouse);
        }
        self.write
            .modify_other_keys(XtermModifyOtherKeys::Disabled)
            .unwrap();
        self.exit_alternate_screen().unwrap();
        self.write.flush().unwrap();

        signal_hook::low_level::unregister(self.sigwinch_id);
    }
}

pub(crate) struct TermiosGuard<W: BorrowMut<TtyWriteHandle>> {
    writer: W,
    saved_state: Termios,
}

/// Wrapper for `TtyWriteHandle` with a modified Termios state, which restores the original state when dropped.
/// The generic BorrowMut parameter is needed because we want allow both owned and borrowed handles.
impl<W: BorrowMut<TtyWriteHandle>> TermiosGuard<W> {
    /// Restore the termios state to the original saved state now.
    #[inline]
    fn restore_termios(&mut self) -> Result<()> {
        self.writer
            .borrow_mut()
            .set_termios(&self.saved_state, SetAttributeWhen::Now)
            .context("failed to restore original termios state")
    }

    /// Modifies the termios state using the given closure and returns a new `TermiosGuard`,
    /// which restores to the original termios state when dropped.
    #[inline]
    fn with_modified_termios(
        &mut self,
        modifier: impl FnOnce(&mut Termios),
    ) -> Result<TermiosGuard<impl BorrowMut<TtyWriteHandle> + '_>> {
        let mut new = TermiosGuard::new(self.writer.borrow_mut())?;
        new.modify_termios(
            modifier,
            SetAttributeWhen::AfterDrainOutputQueuePurgeInputQueue,
        )
        .context("failed to modify termios state")?;
        Ok(new)
    }

    /// Creates a new `TermiosGuard`, saving the current termios state.
    /// The state is restored when the guard is dropped.
    #[inline]
    fn new(mut writer: W) -> Result<Self> {
        let saved_state = writer.borrow_mut().get_termios()?;
        Ok(Self {
            writer,
            saved_state,
        })
    }
}

impl<W: BorrowMut<TtyWriteHandle>> Write for TermiosGuard<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.borrow_mut().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.borrow_mut().flush()
    }
}

impl<W: BorrowMut<TtyWriteHandle>> Deref for TermiosGuard<W> {
    type Target = TtyWriteHandle;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.writer.borrow()
    }
}

impl<W: BorrowMut<TtyWriteHandle>> DerefMut for TermiosGuard<W> {
    #[inline]
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        self.writer.borrow_mut()
    }
}

impl<W: BorrowMut<TtyWriteHandle>> Drop for TermiosGuard<W> {
    fn drop(&mut self) {
        self.restore_termios().unwrap()
    }
}
