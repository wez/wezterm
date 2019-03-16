use serde::{de, ser};
use std::fmt::{self, Display};

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Message(String),
    Io(String),
    SequenceMustHaveLength,
    LebOverflow,
    DeserializeAnyNotSupported,
    DeserializeIdentifierNotSupported,
    DeserializeIgnoredAnyNotSupported,
    InvalidBoolEncoding(u8),
    InvalidCharEncoding(u32),
    InvalidUtf8Encoding(std::str::Utf8Error),
    InvalidTagEncoding(usize),
    NumberOutOfRange,
}

pub type Result<T> = std::result::Result<T, Error>;

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(format!("{}", err))
    }
}

impl From<leb128::read::Error> for Error {
    fn from(err: leb128::read::Error) -> Error {
        match err {
            leb128::read::Error::IoError(err) => Error::Io(format!("{}", err)),
            leb128::read::Error::Overflow => Error::LebOverflow,
        }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(std::error::Error::description(self))
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Message(ref msg) => msg,
            Error::Io(ref msg) => msg,
            Error::SequenceMustHaveLength => "SequenceMustHaveLength",
            Error::DeserializeAnyNotSupported => "DeserializeAnyNotSupported",
            Error::LebOverflow => "LEB128 Overflow",
            Error::InvalidBoolEncoding(_) => "Invalid Bool Encoding",
            Error::InvalidCharEncoding(_) => "Invalid char encoding",
            Error::DeserializeIdentifierNotSupported => "DeserializeIdentifierNotSupported",
            Error::DeserializeIgnoredAnyNotSupported => "DeserializeIgnoredAnyNotSupported",
            Error::InvalidUtf8Encoding(_) => "InvalidUtf8Encoding",
            Error::InvalidTagEncoding(_) => "InvalidTagEncoding",
            Error::NumberOutOfRange => "NumberOutOfRange",
        }
    }
}
