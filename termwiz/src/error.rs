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

    /// Generic formatting error.
    #[error("formatting: {0}")]
    Fmt(#[from] std::fmt::Error),

    /// Regex error.
    #[error("regex: {0}")]
    Regex(#[from] regex::Error),

    /// UTF-8 decoding error.
    #[error("utf-8 decode: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

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