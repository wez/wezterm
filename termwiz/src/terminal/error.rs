//! Error types.
use std::{
    result::Result as StdResult,
    io::Error as IoError,
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

    #[error("{0} is out of bounds for this system")]
    NumCastOutOfBounds(String),

    /// Wrapped terminfo error.
    #[error("terminfo: {0}")]
    Terminfo(#[from] crate::render::terminfo::TerminfoError),

    /// Wrapped FileDescriptor (anyhow) error.
    #[error("filedescriptor: {0}")]
    FileDescriptor(anyhow::Error),

    /// Wrapped error about termios.
    #[error("termios: {0}")]
    Termios(#[source] Box<Self>)
}

impl Error {
    pub(crate) fn termios(err: Self) -> Self {
        Self::Termios(Box::new(err))
    }
}