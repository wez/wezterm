use super::{SessionRequest, SessionSender};
use crate::sftp::dir::{Dir, DirRequest};
use crate::sftp::file::{File, FileRequest};
use crate::sftp::types::{Metadata, OpenFileType, OpenOptions, RenameOptions, WriteMode};
use camino::Utf8PathBuf;
use error::SftpError;
use smol::channel::{bounded, RecvError, Sender};
use std::convert::TryInto;
use std::io;
use thiserror::Error;

pub(crate) mod dir;
pub(crate) mod error;
pub(crate) mod file;
pub(crate) mod types;

fn into_invalid_data<E>(err: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    io::Error::new(io::ErrorKind::InvalidData, err)
}

/// Represents the result of some SFTP channel operation
pub type SftpChannelResult<T> = Result<T, SftpChannelError>;

/// Represents an error that can occur when working with the SFTP channel
#[derive(Debug, Error)]
pub enum SftpChannelError {
    #[error(transparent)]
    Sftp(#[from] SftpError),

    #[error("File IO failed: {}", .0)]
    FileIo(#[from] std::io::Error),

    #[error("Failed to send request: {}", .0)]
    SendFailed(#[from] anyhow::Error),

    #[error("Failed to receive response: {}", .0)]
    RecvFailed(#[from] RecvError),

    #[cfg(feature = "ssh2")]
    #[error("Library-specific error: {}", .0)]
    Ssh2(#[source] ssh2::Error),

    #[cfg(feature = "libssh-rs")]
    #[error("Library-specific error: {}", .0)]
    LibSsh(#[source] libssh_rs::Error),

    #[error("Not Implemented")]
    NotImplemented,
}

/// Represents an open sftp channel for performing filesystem operations
#[derive(Clone, Debug)]
pub struct Sftp {
    pub(crate) tx: SessionSender,
}

impl Sftp {
    /// Open a handle to a file.
    pub async fn open_with_mode<T, E>(
        &self,
        filename: T,
        opts: OpenOptions,
    ) -> SftpChannelResult<File>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);

        self.tx
            .send(SessionRequest::Sftp(SftpRequest::OpenWithMode(
                OpenWithMode {
                    filename: filename.try_into().map_err(into_invalid_data)?,
                    opts,
                },
                reply,
            )))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to open a file in the `Read` mode.
    pub async fn open<T, E>(&self, filename: T) -> SftpChannelResult<File>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.open_with_mode(
            filename,
            OpenOptions {
                read: true,
                write: None,
                mode: 0,
                ty: OpenFileType::File,
            },
        )
        .await
    }

    /// Helper to create a file in write-only mode with truncation.
    pub async fn create<T, E>(&self, filename: T) -> SftpChannelResult<File>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.open_with_mode(
            filename,
            OpenOptions {
                read: false,
                write: Some(WriteMode::Write),
                mode: 0o666,
                ty: OpenFileType::File,
            },
        )
        .await
    }

    /// Helper to open a directory for reading its contents.
    pub async fn open_dir<T, E>(&self, filename: T) -> SftpChannelResult<Dir>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::OpenDir(
                filename.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Convenience function to read the files in a directory.
    ///
    /// The returned paths are all joined with dirname when returned, and the paths . and .. are
    /// filtered out of the returned list.
    pub async fn read_dir<T, E>(
        &self,
        filename: T,
    ) -> SftpChannelResult<Vec<(Utf8PathBuf, Metadata)>>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::ReadDir(
                filename.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Create a directory on the remote filesystem.
    pub async fn create_dir<T, E>(&self, filename: T, mode: i32) -> SftpChannelResult<()>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::CreateDir(
                CreateDir {
                    filename: filename.try_into().map_err(into_invalid_data)?,
                    mode,
                },
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Remove a directory from the remote filesystem.
    pub async fn remove_dir<T, E>(&self, filename: T) -> SftpChannelResult<()>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::RemoveDir(
                filename.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Get the metadata for a file, performed by stat(2).
    pub async fn metadata<T, E>(&self, filename: T) -> SftpChannelResult<Metadata>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Metadata(
                filename.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Get the metadata for a file, performed by lstat(2).
    pub async fn symlink_metadata<T, E>(&self, filename: T) -> SftpChannelResult<Metadata>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::SymlinkMetadata(
                filename.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Set the metadata for a file.
    pub async fn set_metadata<T, E>(&self, filename: T, metadata: Metadata) -> SftpChannelResult<()>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::SetMetadata(
                SetMetadata {
                    filename: filename.try_into().map_err(into_invalid_data)?,
                    metadata,
                },
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Create symlink at `target` pointing at `path`.
    pub async fn symlink<T1, T2, E1, E2>(&self, path: T1, target: T2) -> SftpChannelResult<()>
    where
        T1: TryInto<Utf8PathBuf, Error = E1>,
        T2: TryInto<Utf8PathBuf, Error = E2>,
        E1: Into<Box<dyn std::error::Error + Send + Sync>>,
        E2: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Symlink(
                Symlink {
                    path: path.try_into().map_err(into_invalid_data)?,
                    target: target.try_into().map_err(into_invalid_data)?,
                },
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Read a symlink at `path`.
    pub async fn read_link<T, E>(&self, path: T) -> SftpChannelResult<Utf8PathBuf>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::ReadLink(
                path.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Resolve the real path for `path`.
    pub async fn canonicalize<T, E>(&self, path: T) -> SftpChannelResult<Utf8PathBuf>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Canonicalize(
                path.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Rename the filesystem object on the remote filesystem.
    pub async fn rename<T1, T2, E1, E2>(
        &self,
        src: T1,
        dst: T2,
        opts: RenameOptions,
    ) -> SftpChannelResult<()>
    where
        T1: TryInto<Utf8PathBuf, Error = E1>,
        T2: TryInto<Utf8PathBuf, Error = E2>,
        E1: Into<Box<dyn std::error::Error + Send + Sync>>,
        E2: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Rename(
                Rename {
                    src: src.try_into().map_err(into_invalid_data)?,
                    dst: dst.try_into().map_err(into_invalid_data)?,
                    opts,
                },
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Remove a file on the remote filesystem.
    pub async fn remove_file<T, E>(&self, file: T) -> SftpChannelResult<()>
    where
        T: TryInto<Utf8PathBuf, Error = E>,
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::RemoveFile(
                file.try_into().map_err(into_invalid_data)?,
                reply,
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) enum SftpRequest {
    OpenWithMode(OpenWithMode, Sender<SftpChannelResult<File>>),
    OpenDir(Utf8PathBuf, Sender<SftpChannelResult<Dir>>),
    ReadDir(
        Utf8PathBuf,
        Sender<SftpChannelResult<Vec<(Utf8PathBuf, Metadata)>>>,
    ),
    CreateDir(CreateDir, Sender<SftpChannelResult<()>>),
    RemoveDir(Utf8PathBuf, Sender<SftpChannelResult<()>>),
    Metadata(Utf8PathBuf, Sender<SftpChannelResult<Metadata>>),
    SymlinkMetadata(Utf8PathBuf, Sender<SftpChannelResult<Metadata>>),
    SetMetadata(SetMetadata, Sender<SftpChannelResult<()>>),
    Symlink(Symlink, Sender<SftpChannelResult<()>>),
    ReadLink(Utf8PathBuf, Sender<SftpChannelResult<Utf8PathBuf>>),
    Canonicalize(Utf8PathBuf, Sender<SftpChannelResult<Utf8PathBuf>>),
    Rename(Rename, Sender<SftpChannelResult<()>>),
    RemoveFile(Utf8PathBuf, Sender<SftpChannelResult<()>>),

    /// Specialized type for file-based operations
    File(FileRequest),
    Dir(DirRequest),
}

#[derive(Debug)]
pub(crate) struct OpenWithMode {
    pub filename: Utf8PathBuf,
    pub opts: OpenOptions,
}

#[derive(Debug)]
pub(crate) struct CreateDir {
    pub filename: Utf8PathBuf,
    pub mode: i32,
}

#[derive(Debug)]
pub(crate) struct SetMetadata {
    pub filename: Utf8PathBuf,
    pub metadata: Metadata,
}

#[derive(Debug)]
pub(crate) struct Symlink {
    pub path: Utf8PathBuf,
    pub target: Utf8PathBuf,
}

#[derive(Debug)]
pub(crate) struct Rename {
    pub src: Utf8PathBuf,
    pub dst: Utf8PathBuf,
    pub opts: RenameOptions,
}

#[cfg(feature = "ssh2")]
impl From<ssh2::Error> for SftpChannelError {
    fn from(err: ssh2::Error) -> Self {
        use std::convert::TryFrom;
        match SftpError::try_from(err) {
            Ok(x) => Self::Sftp(x),
            Err(x) => Self::Ssh2(x),
        }
    }
}

#[cfg(feature = "libssh-rs")]
impl From<libssh_rs::Error> for SftpChannelError {
    fn from(err: libssh_rs::Error) -> Self {
        Self::LibSsh(err)
    }
}
