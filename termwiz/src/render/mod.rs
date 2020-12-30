use thiserror::Error;
use crate::terminal::Error as TerminalError;

pub mod terminfo;
#[cfg(windows)]
pub mod windows;

pub trait RenderTty: std::io::Write {
    /// Returns the (cols, rows) for the terminal
    fn get_size_in_cells(&mut self) -> Result<(usize, usize)>;
}

#[derive(Debug, Error)]
pub enum RenderError {
    /// Generic I/O error.
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    /// Upstream terminfo error.
    #[error("terminfo: {0}")]
    Terminfo(#[from] ::terminfo::Error),

    /// Wrapped terminal error.
    #[error("terminal: {0}")]
    Terminal(#[source] Box<TerminalError>),

    /// Wrapped error about sizing.
    #[error("sizing: {0}")]
    Sizing(#[source] Box<crate::error::Error>),
}

pub(self) type Result<T> = std::result::Result<T, RenderError>;

impl From<TerminalError> for RenderError {
    fn from(err: TerminalError) -> Self {
        Self::Terminal(Box::new(err))
    }
}