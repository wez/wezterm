//! Error types.
use thiserror::Error;

/// Convenient return type for functions.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Generic I/O error.
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    /// Opaque internal error.
    #[error("{0}")]
    Internal(InternalError),

    /// State reached that should be impossible.
    #[error("impossible!?: {0}")]
    ImpossibleState(&'static str),

    /// Wrapped terminal error.
    #[error("terminal: {0}")]
    Terminal(#[from] crate::terminal::Error),

    /// Wrapped OSC error.
    #[error("osc: {0}")]
    Osc(#[from] crate::escape::osc::OscError),
}

/// Internal errors.
///
/// You should consider this type as opaque and not attempt to deconstruct it.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InternalError {
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),

    #[error(transparent)]
    Regex(#[from] regex::Error),

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("{0}")]
    Other(anyhow::Error),
}

impl<E> From<E> for Error
where
    E: Into<InternalError>,
{
    fn from(err: E) -> Self {
        Self::Internal(err.into())
    }
}

impl From<anyhow::Error> for InternalError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}
