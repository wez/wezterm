//! Error types.
use crate::error::InternalError;
use std::{borrow::Cow, io::Error as IoError, result::Result as StdResult};
use thiserror::Error;

/// Convenient return type for functions.
pub type Result<T> = StdResult<T, Error>;

/// Terminal error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Generic I/O error.
    #[error("i/o: {0}")]
    Io(#[from] IoError),

    /// Opaque internal error.
    #[error("{0}")]
    Internal(InternalError),

    /// Error about a specific ioctl.
    #[error("ioctl({ctl}): {error}")]
    Ioctl {
        ctl: &'static str,
        #[source]
        error: IoError,
    },

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

    /// Wrapped error about termios.
    #[error("termios: {0}")]
    Termios(#[source] Box<Self>),

    /// Error about a winapi/syscall.
    #[error("syscall {syscall}: {error}")]
    Syscall {
        syscall: Cow<'static, str>,
        #[source]
        error: IoError,
    },

    /// Custom error for implementers.
    #[error("custom: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),

    #[error("unimplemented")]
    Unimplemented,

    #[error("{context}: {error}")]
    WithContext {
        context: Cow<'static, str>,
        #[source]
        error: Box<Self>,
    },
}

impl<E> From<E> for Error
where
    E: Into<InternalError>,
{
    fn from(err: E) -> Self {
        Self::Internal(err.into())
    }
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

    pub(crate) fn with_context(self, context: impl Into<Cow<'static, str>>) -> Self {
        Self::WithContext {
            context: context.into(),
            error: Box::new(self),
        }
    }
}
