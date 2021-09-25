use super::{FileStat, SessionRequest, SessionSender, SftpRequest};
use smol::{
    channel::{bounded, Sender},
    future::FutureExt,
};
use std::{
    fmt,
    future::Future,
    io,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) type FileId = usize;

/// A file handle to an SFTP connection.
pub struct File {
    pub(crate) file_id: FileId,
    tx: Option<SessionSender>,
    state: FileState,
}

#[derive(Default)]
struct FileState {
    f_read: Option<Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + Send + Sync + 'static>>>,
    f_write: Option<Pin<Box<dyn Future<Output = io::Result<usize>> + Send + Sync + 'static>>>,
    f_flush: Option<Pin<Box<dyn Future<Output = io::Result<()>> + Send + Sync + 'static>>>,
    f_close: Option<Pin<Box<dyn Future<Output = io::Result<()>> + Send + Sync + 'static>>>,
}

#[derive(Debug)]
pub(crate) enum FileRequest {
    Write(WriteFile),
    Read(ReadFile),
    Close(CloseFile),
    Flush(FlushFile),
    Setstat(SetstatFile),
    Stat(StatFile),
    Readdir(ReaddirFile),
    Fsync(FsyncFile),
}

#[derive(Debug)]
pub(crate) struct WriteFile {
    pub file_id: FileId,
    pub data: Vec<u8>,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct ReadFile {
    pub file_id: FileId,
    pub max_bytes: usize,
    pub reply: Sender<Vec<u8>>,
}

#[derive(Debug)]
pub(crate) struct CloseFile {
    pub file_id: FileId,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct FlushFile {
    pub file_id: FileId,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct SetstatFile {
    pub file_id: FileId,
    pub stat: FileStat,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub(crate) struct StatFile {
    pub file_id: FileId,
    pub reply: Sender<FileStat>,
}

#[derive(Debug)]
pub(crate) struct ReaddirFile {
    pub file_id: FileId,
    pub reply: Sender<(PathBuf, FileStat)>,
}

#[derive(Debug)]
pub(crate) struct FsyncFile {
    pub file_id: FileId,
    pub reply: Sender<()>,
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File")
            .field("file_id", &self.file_id)
            .finish()
    }
}

impl Drop for File {
    /// Attempts to close the file that exists in the dedicated ssh2 thread
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let (reply, _) = bounded(1);
            let _ = tx.try_send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Close(
                CloseFile {
                    file_id: self.file_id,
                    reply,
                },
            ))));
        }
    }
}

impl File {
    pub(crate) fn new(file_id: FileId) -> Self {
        Self {
            file_id,
            tx: None,
            state: Default::default(),
        }
    }

    pub(crate) fn initialize_sender(&mut self, sender: SessionSender) {
        self.tx.replace(sender);
    }

    /// Set the metadata for this handle.
    ///
    /// See [`ssh2::File::setstat`] for more information.
    pub async fn setstat(&self, stat: FileStat) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::File(
                FileRequest::Setstat(SetstatFile {
                    file_id: self.file_id,
                    stat,
                    reply,
                }),
            )))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Get the metadata for this handle.
    ///
    /// See [`ssh2::File::stat`] for more information.
    pub async fn stat(&self) -> anyhow::Result<FileStat> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Stat(
                StatFile {
                    file_id: self.file_id,
                    reply,
                },
            ))))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Reads a block of data from a handle and returns file entry information for the next entry,
    /// if any.
    ///
    /// Note that this provides raw access to the readdir function from libssh2. This will return
    /// an error when there are no more files to read, and files such as . and .. will be included
    /// in the return values.
    ///
    /// Also note that the return paths will not be absolute paths, they are the filenames of the
    /// files in this directory.
    ///
    /// See [`ssh2::File::readdir`] for more information.
    pub async fn readdir(&self) -> anyhow::Result<(PathBuf, FileStat)> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::File(
                FileRequest::Readdir(ReaddirFile {
                    file_id: self.file_id,
                    reply,
                }),
            )))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// This function causes the remote server to synchronize the file data and metadata to disk
    /// (like fsync(2)).
    ///
    /// See [`ssh2::File::fsync`] for more information.
    pub async fn fsync(&self) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Fsync(
                FsyncFile {
                    file_id: self.file_id,
                    reply,
                },
            ))))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }
}

impl smol::io::AsyncRead for File {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        async fn read(tx: SessionSender, file_id: usize, len: usize) -> io::Result<Vec<u8>> {
            inner_read(tx, file_id, len)
                .await
                .map_err(|x| io::Error::new(io::ErrorKind::Other, x))
        }
        let tx = self.tx.as_ref().unwrap().clone();
        let file_id = self.file_id;

        let poll = self
            .state
            .f_read
            .get_or_insert_with(|| Box::pin(read(tx, file_id, buf.len())))
            .poll(cx);

        if poll.is_ready() {
            self.state.f_read.take();
        }

        match poll {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(x)) => Poll::Ready(Err(x)),
            Poll::Ready(Ok(data)) => {
                let n = data.len();
                (&mut buf[..n]).copy_from_slice(&data[..n]);
                Poll::Ready(Ok(n))
            }
        }
    }
}

impl smol::io::AsyncWrite for File {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        async fn write(tx: SessionSender, file_id: usize, buf: Vec<u8>) -> io::Result<usize> {
            let n = buf.len();
            inner_write(tx, file_id, buf)
                .await
                .map(|_| n)
                .map_err(|x| io::Error::new(io::ErrorKind::Other, x))
        }

        let tx = self.tx.as_ref().unwrap().clone();
        let file_id = self.file_id;

        let poll = self
            .state
            .f_write
            .get_or_insert_with(|| Box::pin(write(tx, file_id, buf.to_vec())))
            .poll(cx);

        if poll.is_ready() {
            self.state.f_write.take();
        }

        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        async fn flush(tx: SessionSender, file_id: usize) -> io::Result<()> {
            inner_flush(tx, file_id)
                .await
                .map_err(|x| io::Error::new(io::ErrorKind::Other, x))
        }

        let tx = self.tx.as_ref().unwrap().clone();
        let file_id = self.file_id;

        let poll = self
            .state
            .f_flush
            .get_or_insert_with(|| Box::pin(flush(tx, file_id)))
            .poll(cx);

        if poll.is_ready() {
            self.state.f_flush.take();
        }

        poll
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        async fn close(tx: SessionSender, file_id: usize) -> io::Result<()> {
            inner_close(tx, file_id)
                .await
                .map_err(|x| io::Error::new(io::ErrorKind::Other, x))
        }

        let tx = self.tx.as_ref().unwrap().clone();
        let file_id = self.file_id;

        let poll = self
            .state
            .f_close
            .get_or_insert_with(|| Box::pin(close(tx, file_id)))
            .poll(cx);

        if poll.is_ready() {
            self.state.f_close.take();
        }

        poll
    }
}

/// Writes some bytes to the file.
async fn inner_write(tx: SessionSender, file_id: usize, data: Vec<u8>) -> anyhow::Result<()> {
    let (reply, rx) = bounded(1);
    tx.send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Write(
        WriteFile {
            file_id,
            data,
            reply,
        },
    ))))
    .await?;
    let result = rx.recv().await?;
    Ok(result)
}

/// Reads some bytes from the file, returning a vector of bytes read.
///
/// If the vector is empty, this indicates that there are no more bytes
/// to read at the moment.
async fn inner_read(
    tx: SessionSender,
    file_id: usize,
    max_bytes: usize,
) -> anyhow::Result<Vec<u8>> {
    let (reply, rx) = bounded(1);
    tx.send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Read(
        ReadFile {
            file_id,
            max_bytes,
            reply,
        },
    ))))
    .await?;
    let result = rx.recv().await?;
    Ok(result)
}

/// Flushes the remote file
async fn inner_flush(tx: SessionSender, file_id: usize) -> anyhow::Result<()> {
    let (reply, rx) = bounded(1);
    tx.send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Flush(
        FlushFile { file_id, reply },
    ))))
    .await?;
    let result = rx.recv().await?;
    Ok(result)
}

/// Closes the handle to the remote file
async fn inner_close(tx: SessionSender, file_id: usize) -> anyhow::Result<()> {
    let (reply, rx) = bounded(1);
    tx.send(SessionRequest::Sftp(SftpRequest::File(FileRequest::Close(
        CloseFile { file_id, reply },
    ))))
    .await?;
    let result = rx.recv().await?;
    Ok(result)
}
