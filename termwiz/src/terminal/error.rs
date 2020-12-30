//! Error types.
use std::{
    result::Result as StdResult,
    io::Error as IoError,
    borrow::Cow,
};
use thiserror::Error;

/// Convenient return type for functions.
pub type Result<T> = StdResult<T, Error>;

/// Terminal error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Generic I/O error.
    #[error("i/o: {0}")]
    Io(#[from] IoError),

    /// Error about a specific ioctl.
    #[error("ioctl({ctl}): {error}")]
    Ioctl {
        ctl: &'static str,
        #[source] error: IoError
    },

    /// Error when stdin or stdout are not TTYs.
    #[error("stdin or stdout is not a TTY")]
    NotATTY,

    /// Error reading tty.
    #[error("tty read: {0}")]
    TtyRead(#[source] IoError),

    /// Error writing tty.
    #[error("tty write: {0}")]
    TtyWrite(#[source] IoError),

    /// Error flushing tty.
    #[error("tty flush: {0}")]
    TtyFlush(#[source] IoError),

    /// Error setting tty attribute.
    #[error("tty setattr: {0}")]
    TtySetAttr(#[source] IoError),

    /// Error writing a UnixStream.
    #[error("unix stream write: {0}")]
    UnixStreamWrite(#[source] IoError),

    /// Error reading a SIGWINCH pipe.
    #[error("sigwinch pipe read: {0}")]
    SigWinchPipeRead(#[source] IoError),

    /// Polling error.
    #[error("poll(2): {0}")]
    Poll(#[source] Box<Self>),

    /// Error when casting a bit-sized int to a machine-sized int.
    #[error("{0} is out of bounds for this system")]
    NumCastOutOfBounds(String),

    /// Error when the buffer isn't sized correctly for the screen.
    #[error("buffer size doesn't match screen size: cols={cols} * rows={rows} != buffer={buffer}")]
    BufferScreenMismatch {
        rows: usize,
        cols: usize,
        buffer: usize,
    },

    /// Wrapped render error.
    #[error("render: {0}")]
    Render(#[from] crate::render::RenderError),

    /// Wrapped FileDescriptor (anyhow) error.
    #[error("filedescriptor: {0}")]
    FileDescriptor(anyhow::Error),

    /// Wrapped error about termios.
    #[error("termios: {0}")]
    Termios(#[source] Box<Self>),

    /// Error about a winapi/syscall.
    #[error("syscall {syscall}: {error}")]
    Syscall {
        syscall: Cow<'static, str>,
        #[source] error: IoError,
    },

    /// Custom error for implementers.
    #[error("custom: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    #[cfg(unix)]
    pub(crate) fn termios(err: Self) -> Self {
        Self::Termios(Box::new(err))
    }

    #[cfg(windows)]
    pub(crate) fn syscall(syscall: impl Into<Cow<'static, str>>) -> Self {
        Self::Syscall {
            syscall: syscall.into(),
            error: IoError::last_os_error(),
        }
    }
}