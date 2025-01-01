use std::convert::TryFrom;
use thiserror::Error;

/// Represents a result whose error is [`SftpError`]
pub type SftpResult<T> = Result<T, SftpError>;

/// Represents errors associated with sftp operations
#[derive(Copy, Clone, Debug, Error, Hash, PartialEq, Eq)]
pub enum SftpError {
    // Following are available on libssh and libssh2
    #[error("End-of-file encountered")]
    Eof = 1,
    #[error("File doesn't exist")]
    NoSuchFile = 2,
    #[error("Permission denied")]
    PermissionDenied = 3,
    #[error("Generic failure")]
    Failure = 4,
    #[error("Garbage received from server")]
    BadMessage = 5,
    #[error("No connection has been set up")]
    NoConnection = 6,
    #[error("There was a connection, but we lost it")]
    ConnectionLost = 7,
    #[error("Operation not supported by the server")]
    OpUnsupported = 8,
    #[error("Invalid file handle")]
    InvalidHandle = 9,
    #[error("No such file or directory path exists")]
    NoSuchPath = 10,
    #[error("An attempt to create an already existing file or directory has been made")]
    FileAlreadyExists = 11,
    #[error("We are trying to write on a write-protected filesystem")]
    WriteProtect = 12,
    #[error("No media in remote drive")]
    NoMedia = 13,

    // Below are libssh2-specific errors
    #[cfg(feature = "ssh2")]
    #[error("No space available on filesystem")]
    NoSpaceOnFilesystem = 14,
    #[cfg(feature = "ssh2")]
    #[error("Quota exceeded")]
    QuotaExceeded = 15,
    #[cfg(feature = "ssh2")]
    #[error("Unknown principal")]
    UnknownPrincipal = 16,
    #[cfg(feature = "ssh2")]
    #[error("Filesystem lock conflict")]
    LockConflict = 17,
    #[cfg(feature = "ssh2")]
    #[error("Directory is not empty")]
    DirNotEmpty = 18,
    #[cfg(feature = "ssh2")]
    #[error("Operation attempted against a path that is not a directory")]
    NotADirectory = 19,
    #[cfg(feature = "ssh2")]
    #[error("Filename invalid")]
    InvalidFilename = 20,
    #[cfg(feature = "ssh2")]
    #[error("Symlink loop encountered")]
    LinkLoop = 21,
}

impl SftpError {
    /// Produces an SFTP error from the given code if it matches a known error type
    pub fn from_error_code(code: i32) -> Option<SftpError> {
        Self::try_from(code).ok()
    }

    /// Converts into an error code
    pub fn to_error_code(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for SftpError {
    type Error = Result<(), i32>;

    /// Attempt to convert an arbitrary code to an sftp error, returning
    /// `Ok` if matching an sftp error or `Err` if the code represented a
    /// success or was unknown
    fn try_from(code: i32) -> Result<Self, Self::Error> {
        match code {
            // 0 means okay in libssh and libssh2, which isn't an error
            0 => Err(Ok(())),

            1 => Ok(Self::Eof),
            2 => Ok(Self::NoSuchFile),
            3 => Ok(Self::PermissionDenied),
            4 => Ok(Self::Failure),
            5 => Ok(Self::BadMessage),
            6 => Ok(Self::NoConnection),
            7 => Ok(Self::ConnectionLost),
            8 => Ok(Self::OpUnsupported),
            9 => Ok(Self::InvalidHandle),
            10 => Ok(Self::NoSuchPath),
            11 => Ok(Self::FileAlreadyExists),
            12 => Ok(Self::WriteProtect),
            13 => Ok(Self::NoMedia),

            // Errors only available with ssh2
            #[cfg(feature = "ssh2")]
            14 => Ok(Self::NoSpaceOnFilesystem),
            #[cfg(feature = "ssh2")]
            15 => Ok(Self::QuotaExceeded),
            #[cfg(feature = "ssh2")]
            16 => Ok(Self::UnknownPrincipal),
            #[cfg(feature = "ssh2")]
            17 => Ok(Self::LockConflict),
            #[cfg(feature = "ssh2")]
            18 => Ok(Self::DirNotEmpty),
            #[cfg(feature = "ssh2")]
            19 => Ok(Self::NotADirectory),
            #[cfg(feature = "ssh2")]
            20 => Ok(Self::InvalidFilename),
            #[cfg(feature = "ssh2")]
            21 => Ok(Self::LinkLoop),

            // Unsupported codes get reflected back
            x => Err(Err(x)),
        }
    }
}

#[cfg(feature = "ssh2")]
impl TryFrom<ssh2::Error> for SftpError {
    type Error = ssh2::Error;

    fn try_from(err: ssh2::Error) -> Result<Self, Self::Error> {
        match err.code() {
            ssh2::ErrorCode::SFTP(x) => match Self::from_error_code(x) {
                Some(err) => Ok(err),
                None => Err(err),
            },
            _ => Err(err),
        }
    }
}

#[cfg(feature = "ssh2")]
impl TryFrom<ssh2::ErrorCode> for SftpError {
    type Error = ssh2::ErrorCode;

    fn try_from(code: ssh2::ErrorCode) -> Result<Self, Self::Error> {
        match code {
            ssh2::ErrorCode::SFTP(x) => match Self::from_error_code(x) {
                Some(err) => Ok(err),
                None => Err(code),
            },
            x => Err(x),
        }
    }
}
