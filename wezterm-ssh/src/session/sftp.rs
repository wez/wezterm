use super::{SessionRequest, SessionSender};
use smol::channel::{bounded, Sender};
use std::path::PathBuf;

mod file;
pub use file::File;
pub(crate) use file::{
    CloseFile, FileId, FileRequest, FlushFile, FsyncFile, ReadFile, ReaddirFile, SetstatFile,
    StatFile, WriteFile,
};

mod types;
pub use types::{
    FilePermissions, FileType, Metadata, OpenFileType, OpenOptions, RenameOptions, WriteMode,
};

/// Represents an open sftp channel for performing filesystem operations
#[derive(Clone, Debug)]
pub struct Sftp {
    pub(crate) tx: SessionSender,
}

impl Sftp {
    /// Open a handle to a file.
    ///
    /// See [`Sftp::open_mode`] for more information.
    pub async fn open_mode(
        &self,
        filename: impl Into<PathBuf>,
        opts: OpenOptions,
    ) -> anyhow::Result<File> {
        let (reply, rx) = bounded(1);

        self.tx
            .send(SessionRequest::Sftp(SftpRequest::OpenMode(OpenMode {
                filename: filename.into(),
                opts,
                reply,
            })))
            .await?;
        let mut result = rx.recv().await?;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to open a file in the `Read` mode.
    ///
    /// See [`Sftp::open`] for more information.
    pub async fn open(&self, filename: impl Into<PathBuf>) -> anyhow::Result<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Open(Open {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await?;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to create a file in write-only mode with truncation.
    ///
    /// See [`Sftp::create`] for more information.
    pub async fn create(&self, filename: impl Into<PathBuf>) -> anyhow::Result<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Create(Create {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await?;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Helper to open a directory for reading its contents.
    ///
    /// See [`Sftp::opendir`] for more information.
    pub async fn opendir(&self, filename: impl Into<PathBuf>) -> anyhow::Result<File> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Opendir(Opendir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let mut result = rx.recv().await?;
        result.initialize_sender(self.tx.clone());
        Ok(result)
    }

    /// Convenience function to read the files in a directory.
    ///
    /// The returned paths are all joined with dirname when returned, and the paths . and .. are
    /// filtered out of the returned list.
    ///
    /// See [`Sftp::readdir`] for more information.
    pub async fn readdir(
        &self,
        filename: impl Into<PathBuf>,
    ) -> anyhow::Result<Vec<(PathBuf, Metadata)>> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Readdir(Readdir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Create a directory on the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub async fn mkdir(&self, filename: impl Into<PathBuf>, mode: i32) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Mkdir(Mkdir {
                filename: filename.into(),
                mode,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Remove a directory from the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub async fn rmdir(&self, filename: impl Into<PathBuf>) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Rmdir(Rmdir {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Get the metadata for a file, performed by stat(2).
    ///
    /// See [`Sftp::stat`] for more information.
    pub async fn stat(&self, filename: impl Into<PathBuf>) -> anyhow::Result<Metadata> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Stat(Stat {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Get the metadata for a file, performed by lstat(2).
    ///
    /// See [`Sftp::lstat`] for more information.
    pub async fn lstat(&self, filename: impl Into<PathBuf>) -> anyhow::Result<Metadata> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Lstat(Lstat {
                filename: filename.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Set the metadata for a file.
    ///
    /// See [`Sftp::setstat`] for more information.
    pub async fn setstat(
        &self,
        filename: impl Into<PathBuf>,
        metadata: Metadata,
    ) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Setstat(Setstat {
                filename: filename.into(),
                metadata,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Create symlink at `target` pointing at `path`.
    ///
    /// See [`Sftp::symlink`] for more information.
    pub async fn symlink(
        &self,
        path: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
    ) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Symlink(Symlink {
                path: path.into(),
                target: target.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Read a symlink at `path`.
    ///
    /// See [`Sftp::readlink`] for more information.
    pub async fn readlink(&self, path: impl Into<PathBuf>) -> anyhow::Result<PathBuf> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Readlink(Readlink {
                path: path.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Resolve the real path for `path`.
    ///
    /// See [`Sftp::realpath`] for more information.
    pub async fn realpath(&self, path: impl Into<PathBuf>) -> anyhow::Result<PathBuf> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Realpath(Realpath {
                path: path.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Rename the filesystem object on the remote filesystem.
    ///
    /// See [`Sftp::rename`] for more information.
    pub async fn rename(
        &self,
        src: impl Into<PathBuf>,
        dst: impl Into<PathBuf>,
        opts: RenameOptions,
    ) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Rename(Rename {
                src: src.into(),
                dst: dst.into(),
                opts,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Remove a file on the remote filesystem.
    ///
    /// See [`Sftp::unlink`] for more information.
    pub async fn unlink(&self, file: impl Into<PathBuf>) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Sftp(SftpRequest::Unlink(Unlink {
                file: file.into(),
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) enum SftpRequest {
    OpenMode(OpenMode),
    Open(Open),
    Create(Create),
    Opendir(Opendir),
    Readdir(Readdir),
    Mkdir(Mkdir),
    Rmdir(Rmdir),
    Stat(Stat),
    Lstat(Lstat),
    Setstat(Setstat),
    Symlink(Symlink),
    Readlink(Readlink),
    Realpath(Realpath),
    Rename(Rename),
    Unlink(Unlink),

    /// Specialized type for file-based operations
    File(FileRequest),
}

#[derive(Debug)]
pub(crate) struct OpenMode {
    pub filename: PathBuf,
    pub opts: OpenOptions,
    pub reply: Sender<File>,
}

#[derive(Debug)]
pub(crate) struct Open {
    pub filename: PathBuf,
    pub reply: Sender<File>,
}

#[derive(Debug)]
pub(crate) struct Create {
    pub filename: PathBuf,
    pub reply: Sender<File>,
}

#[derive(Debug)]
pub(crate) struct Opendir {
    pub filename: PathBuf,
    pub reply: Sender<File>,
}

#[derive(Debug)]
pub(crate) struct Readdir {
    pub filename: PathBuf,
    pub reply: Sender<Vec<(PathBuf, Metadata)>>,
}

#[derive(Debug)]
pub(crate) struct Mkdir {
    pub filename: PathBuf,
    pub mode: i32,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct Rmdir {
    pub filename: PathBuf,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct Stat {
    pub filename: PathBuf,
    pub reply: Sender<Metadata>,
}

#[derive(Debug)]
pub(crate) struct Lstat {
    pub filename: PathBuf,
    pub reply: Sender<Metadata>,
}

#[derive(Debug)]
pub(crate) struct Setstat {
    pub filename: PathBuf,
    pub metadata: Metadata,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct Symlink {
    pub path: PathBuf,
    pub target: PathBuf,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct Readlink {
    pub path: PathBuf,
    pub reply: Sender<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct Realpath {
    pub path: PathBuf,
    pub reply: Sender<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct Rename {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub opts: RenameOptions,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct Unlink {
    pub file: PathBuf,
    pub reply: Sender<()>,
}
