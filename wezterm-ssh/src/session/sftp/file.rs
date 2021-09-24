use super::{
    CloseFile, FlushFile, ReadFile, SessionRequest, SessionSender, SftpRequest, WriteFile,
};
use smol::channel::bounded;

pub(crate) type FileId = usize;

/// A file handle to an SFTP connection.
#[derive(Clone, Debug)]
pub struct File {
    pub(crate) file_id: FileId,
    pub(crate) tx: Option<SessionSender>,
}

impl smol::io::AsyncRead for File {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        use smol::future::FutureExt;
        async fn read(
            mut _self: std::pin::Pin<&mut File>,
            buf: &mut [u8],
        ) -> std::io::Result<usize> {
            let data = _self
                .read(buf.len())
                .await
                .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, x))?;
            let n = data.len();

            buf.copy_from_slice(&data[..n]);

            Ok(n)
        }

        Box::pin(read(self, buf)).poll(cx)
    }
}

impl smol::io::AsyncWrite for File {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        use smol::future::FutureExt;
        async fn write(mut _self: std::pin::Pin<&mut File>, buf: &[u8]) -> std::io::Result<usize> {
            _self
                .write(buf.to_vec())
                .await
                .map(|_| buf.len())
                .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, x))
        }

        Box::pin(write(self, buf)).poll(cx)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use smol::future::FutureExt;
        async fn flush(mut _self: std::pin::Pin<&mut File>) -> std::io::Result<()> {
            _self
                .flush()
                .await
                .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, x))
        }

        Box::pin(flush(self)).poll(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use smol::future::FutureExt;
        async fn close(mut _self: std::pin::Pin<&mut File>) -> std::io::Result<()> {
            _self
                .close()
                .await
                .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, x))
        }

        Box::pin(close(self)).poll(cx)
    }
}

impl File {
    /// Writes some bytes to the file.
    async fn write(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::WriteFile(WriteFile {
                file_id: self.file_id,
                data,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Reads some bytes from the file, returning a vector of bytes read.
    ///
    /// If the vector is empty, this indicates that there are no more bytes
    /// to read at the moment.
    async fn read(&mut self, max_bytes: usize) -> anyhow::Result<Vec<u8>> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::ReadFile(ReadFile {
                file_id: self.file_id,
                max_bytes,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Flushes the remote file
    async fn flush(&mut self) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::FlushFile(FlushFile {
                file_id: self.file_id,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }

    /// Closes the handle to the remote file
    async fn close(&mut self) -> anyhow::Result<()> {
        let (reply, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .send(SessionRequest::Sftp(SftpRequest::CloseFile(CloseFile {
                file_id: self.file_id,
                reply,
            })))
            .await?;
        let result = rx.recv().await?;
        Ok(result)
    }
}
