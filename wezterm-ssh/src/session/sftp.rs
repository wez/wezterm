use super::{SessionRequest, SessionSender};
use smol::channel::{bounded, RecvError, Sender};
use std::path::PathBuf;
use thiserror::Error;

mod error;
pub use error::{SftpError, SftpResult};

mod file;
pub use file::File;
pub(crate) use file::{
    CloseFile, FileId, FileRequest, FlushFile, FsyncFile, MetadataFile, ReadDirFile, ReadFile,
    SetMetadataFile, WriteFile,
};

mod types;
pub use types::{
    FilePermissions, FileType, Metadata, OpenFileType, OpenOptions, RenameOptions, WriteMode,
};

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

    #[error("Library-specific error: {}", .0)]
    Other(#[source] ssh2::Error),
}

/// Represents an open sftp channel for performing filesystem operations
#[derive(Clone, Debug)]
pub struct Sftp {
    pub(crate) tx: SessionSender,
}

impl Sftp {
    /// Open a handle to a file.
    pub async fn open_with_mode(
        &self,
        filename: impl Into<PathBuf>,
        opts: OpenOptions,
    ) -> SftpChannelResult<File> {
        let (reply, rx) = bounded(1);

        self.tx
            .send(SessionRequest::Sftp(SftpRequest::OpenWithMode(
                OpenWithMode {
                    filename: filename.into(),
                    opts,
                    reply,
                },
            )))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to open a file in the `Read` mode.
    pub async fn open(&self, filename: impl Into<PathBuf>) -> SftpChannelResult<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Open(Open {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to create a file in write-only mode with truncation.
    pub async fn create(&self, filename: impl Into<PathBuf>) -> SftpChannelResult<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Create(Create {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to open a directory for reading its contents.
    pub async fn open_dir(&self, filename: impl Into<PathBuf>) -> SftpChannelResult<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::OpenDir(OpenDir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await??;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Convenience function to read the files in a directory.
    ///
    /// The returned paths are all joined with dirname when returned, and the paths . and .. are
    /// filtered out of the returned list.
    pub async fn read_dir(
        &self,
        filename: impl Into<PathBuf>,
    ) -> SftpChannelResult<Vec<(PathBuf, Metadata)>> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::ReadDir(ReadDir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Create a directory on the remote filesystem.
    pub async fn create_dir(
        &self,
        filename: impl Into<PathBuf>,
        mode: i32,
    ) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::CreateDir(CreateDir {
                filename: filename.into(),
                mode,
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Remove a directory from the remote filesystem.
    pub async fn remove_dir(&self, filename: impl Into<PathBuf>) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::RemoveDir(RemoveDir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Get the metadata for a file, performed by stat(2).
    pub async fn metadata(&self, filename: impl Into<PathBuf>) -> SftpChannelResult<Metadata> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Metadata(GetMetadata {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Get the metadata for a file, performed by lstat(2).
    pub async fn symlink_metadata(
        &self,
        filename: impl Into<PathBuf>,
    ) -> SftpChannelResult<Metadata> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::SymlinkMetadata(
                SymlinkMetadata {
                    filename: filename.into(),
                    reply,
                },
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Set the metadata for a file.
    pub async fn set_metadata(
        &self,
        filename: impl Into<PathBuf>,
        metadata: Metadata,
    ) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::SetMetadata(
                SetMetadata {
                    filename: filename.into(),
                    metadata,
                    reply,
                },
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Create symlink at `target` pointing at `path`.
    pub async fn symlink(
        &self,
        path: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
    ) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Symlink(Symlink {
                path: path.into(),
                target: target.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Read a symlink at `path`.
    pub async fn read_link(&self, path: impl Into<PathBuf>) -> SftpChannelResult<PathBuf> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::ReadLink(ReadLink {
                path: path.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Resolve the real path for `path`.
    pub async fn canonicalize(&self, path: impl Into<PathBuf>) -> SftpChannelResult<PathBuf> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Canonicalize(
                Canonicalize {
                    path: path.into(),
                    reply,
                },
            )))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Rename the filesystem object on the remote filesystem.
    pub async fn rename(
        &self,
        src: impl Into<PathBuf>,
        dst: impl Into<PathBuf>,
        opts: RenameOptions,
    ) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Rename(Rename {
                src: src.into(),
                dst: dst.into(),
                opts,
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }

    /// Remove a file on the remote filesystem.
    pub async fn remove_file(&self, file: impl Into<PathBuf>) -> SftpChannelResult<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::RemoveFile(RemoveFile {
                file: file.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await??;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) enum SftpRequest {
    OpenWithMode(OpenWithMode),
    Open(Open),
    Create(Create),
    OpenDir(OpenDir),
    ReadDir(ReadDir),
    CreateDir(CreateDir),
    RemoveDir(RemoveDir),
    Metadata(GetMetadata),
    SymlinkMetadata(SymlinkMetadata),
    SetMetadata(SetMetadata),
    Symlink(Symlink),
    ReadLink(ReadLink),
    Canonicalize(Canonicalize),
    Rename(Rename),
    RemoveFile(RemoveFile),

    /// Specialized type for file-based operations
    File(FileRequest),
}

#[derive(Debug)]
pub(crate) struct OpenWithMode {
    pub filename: PathBuf,
    pub opts: OpenOptions,
    pub reply: Sender<SftpChannelResult<File>>,
}

#[derive(Debug)]
pub(crate) struct Open {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<File>>,
}

#[derive(Debug)]
pub(crate) struct Create {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<File>>,
}

#[derive(Debug)]
pub(crate) struct OpenDir {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<File>>,
}

#[derive(Debug)]
pub(crate) struct ReadDir {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<Vec<(PathBuf, Metadata)>>>,
}

#[derive(Debug)]
pub(crate) struct CreateDir {
    pub filename: PathBuf,
    pub mode: i32,
    pub reply: Sender<SftpChannelResult<()>>,
}

#[derive(Debug)]
pub(crate) struct RemoveDir {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<()>>,
}

#[derive(Debug)]
pub(crate) struct GetMetadata {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<Metadata>>,
}

#[derive(Debug)]
pub(crate) struct SymlinkMetadata {
    pub filename: PathBuf,
    pub reply: Sender<SftpChannelResult<Metadata>>,
}

#[derive(Debug)]
pub(crate) struct SetMetadata {
    pub filename: PathBuf,
    pub metadata: Metadata,
    pub reply: Sender<SftpChannelResult<()>>,
}

#[derive(Debug)]
pub(crate) struct Symlink {
    pub path: PathBuf,
    pub target: PathBuf,
    pub reply: Sender<SftpChannelResult<()>>,
}

#[derive(Debug)]
pub(crate) struct ReadLink {
    pub path: PathBuf,
    pub reply: Sender<SftpChannelResult<PathBuf>>,
}

#[derive(Debug)]
pub(crate) struct Canonicalize {
    pub path: PathBuf,
    pub reply: Sender<SftpChannelResult<PathBuf>>,
}

#[derive(Debug)]
pub(crate) struct Rename {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub opts: RenameOptions,
    pub reply: Sender<SftpChannelResult<()>>,
}

#[derive(Debug)]
pub(crate) struct RemoveFile {
    pub file: PathBuf,
    pub reply: Sender<SftpChannelResult<()>>,
}

mod ssh2_impl {
    use super::*;
    use std::convert::TryFrom;

    impl From<ssh2::Error> for SftpChannelError {
        fn from(err: ssh2::Error) -> Self {
            match SftpError::try_from(err) {
                Ok(x) => Self::Sftp(x),
                Err(x) => Self::Other(x),
            }
        }
    }
}
