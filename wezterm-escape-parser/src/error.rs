use core::fmt::Display;

use crate::allocate::*;

/// The termwiz Error type encapsulates a range of internal
/// errors in an opaque manner.  You can use the `source`
/// method to reach the underlying errors if
/// necessary, but it is not expected that most code will
/// need to do so.  Please file an issue if you've got a
/// usecase for this!
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct Error(pub(crate) InternalError);

/// A Result whose error type is a termwiz Error
pub type Result<T> = core::result::Result<T, Error>;

impl<E> From<E> for Error
where
    E: Into<InternalError>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

/// This enum encapsulates the various errors that can be
/// mapped into the termwiz Error type.
/// The intent is that this is effectively private to termwiz
/// itself, but since Rust doesn't allow enums with private
/// variants, we're dancing around with a newtype of an enum
/// and hiding it from the docs.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
#[doc(hidden)]
pub enum InternalError {
    #[error(transparent)]
    Fmt(#[from] core::fmt::Error),

    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "std")]
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[cfg(not(feature = "std"))]
    #[error(transparent)]
    FromUtf8(#[from] alloc::string::FromUtf8Error),

    #[error(transparent)]
    Utf8(#[from] core::str::Utf8Error),

    #[error(transparent)]
    ParseFloat(#[from] core::num::ParseFloatError),

    #[error(transparent)]
    ParseInt(#[from] core::num::ParseIntError),

    #[error("{0}")]
    StringErr(#[from] StringWrap),

    #[cfg(feature = "image")]
    #[error(transparent)]
    BlobLease(#[from] wezterm_blob_leases::Error),

    #[cfg(feature = "use_image")]
    #[error(transparent)]
    ImageError(#[from] image::ImageError),

    #[cfg(feature = "tmux_cc")]
    #[error(transparent)]
    Pest(#[from] pest::error::Error<crate::tmux_cc::parser::Rule>),

    #[error("{}", .context)]
    Context {
        context: String,
        source: Box<dyn core::error::Error + Send + Sync + 'static>,
    },
}

impl From<String> for InternalError {
    fn from(s: String) -> Self {
        InternalError::StringErr(StringWrap(s))
    }
}

#[derive(thiserror::Error, Debug)]
#[doc(hidden)]
#[error("{0}")]
pub struct StringWrap(pub String);

#[macro_export]
macro_rules! format_err {
    ($msg:literal $(,)?) => {
        return $crate::error::Error::from($crate::error::StringWrap($msg.to_string()))
    };
    ($err:expr $(,)?) => {
        return $crate::error::Error::from($crate::error::StringWrap(crate::format!($err)))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return $crate::error::Error::from($crate::error::StringWrap(crate::format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! bail {
    ($msg:literal $(,)?) => {
        return Err($crate::error::StringWrap($msg.to_string()).into())
    };
    ($err:expr $(,)?) => {
        return Err($crate::error::StringWrap(crate::format!($err)).into())
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::StringWrap(crate::format!($fmt, $($arg)*)).into())
    };
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $msg:literal $(,)?) => {
        if !$cond {
            return Err($crate::error::StringWrap(crate::format!($msg)).into());
        }
    };
    ($cond:expr, $err:expr $(,)?) => {
        if !$cond {
            return Err($crate::error::StringWrap(crate::format!($err)).into());
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !$cond {
            return Err($crate::error::StringWrap(crate::format!($fmt, $($arg)*)).into());
        }
    };
}

/// This trait allows extending the Result type so that it can create a
/// `termwiz::Error` that wraps an underlying other error and provide
/// additional context on that error.
pub trait Context<T, E> {
    /// Wrap the error value with additional context.
    fn context<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static;

    /// Wrap the error value with additional context that is evaluated lazily
    /// only once an error does occur.
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> Context<T, E> for core::result::Result<T, E>
where
    E: core::error::Error + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        self.map_err(|error| {
            Error(InternalError::Context {
                context: context.to_string(),
                source: Box::new(error),
            })
        })
    }

    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|error| {
            Error(InternalError::Context {
                context: context().to_string(),
                source: Box::new(error),
            })
        })
    }
}
