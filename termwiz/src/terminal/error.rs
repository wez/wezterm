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

    /// Wrapped render error.
    #[error("render: {0}")]
    Render(#[from] crate::render::RenderError),

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
        err.with_context("termios")
    }

    pub(crate) fn syscall(syscall: impl Into<Cow<'static, str>>) -> Self {
        Self::from(IoError::last_os_error()).with_context(syscall)
    }

    pub(crate) fn with_context(self, context: impl Into<Cow<'static, str>>) -> Self {
        Self::WithContext {
            context: context.into(),
            error: Box::new(self),
        }
    }
}

#[macro_export]
macro_rules! terminal_bail {
    ($msg:literal $(,)?) => {
        return Err(Error::from(::anyhow::anyhow!($msg)));
    };
    ($err:expr $(,)?) => {
        return Err(Error::from(::anyhow::anyhow!($err)));
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(Error::from(::anyhow::anyhow!($fmt, $($arg)*)));
    };
}
