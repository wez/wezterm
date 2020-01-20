use crate::cmdbuilder::CommandBuilder;
use crate::win::conpty::ConPtySystem;
use crate::win::psuedocon::PsuedoCon;
use crate::win::readbuf::ReadBuffer;
use crate::PtySize;
use anyhow::anyhow;
use async_trait::async_trait;
use filedescriptor::{FileDescriptor, Pipe};
use std::io::{self, Error as IoError, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::os::windows::raw::HANDLE;
use std::pin::Pin;
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use winapi::um::namedpipeapi::PeekNamedPipe;
use winapi::um::wincon::COORD;

struct AwaitableConPtySlavePty {
    inner: Arc<Mutex<AwaitableInner>>,
}

struct AwaitableConPtyMasterPty {
    inner: Arc<Mutex<AwaitableInner>>,
}

enum WriteRequest {
    Data(Vec<u8>),
    Resize(PtySize, Waker, Sender<anyhow::Result<()>>),
}

struct AwaitableInner {
    con: Arc<PsuedoCon>,
    size: PtySize,
    write_tx: Sender<WriteRequest>,
    reader: Arc<Mutex<AwaitableReader>>,
}

/// PTYs on Windows are restricted to synchronous operation, so we cannot
/// simply use IOCP to manage our asynchronous reads or writes.
/// The AwaitableReader implements a little facade that allows us to
/// schedule a blocking read of a desirable size in a worker thread.
/// We can then use non-blocking calls on a channel to poll for completion.
struct AwaitableReader {
    wait_for_read: Sender<(Waker, usize)>,
    read_buffer: ReadBuffer,
    read_results_rx: Receiver<std::io::Result<Vec<u8>>>,
}

#[async_trait(?Send)]
impl crate::awaitable::SlavePty for AwaitableConPtySlavePty {
    async fn spawn_command(
        &self,
        cmd: CommandBuilder,
    ) -> anyhow::Result<Pin<Box<dyn crate::awaitable::Child>>> {
        let inner = self.inner.lock().unwrap();
        let child = inner.con.spawn_command(cmd)?;
        Ok(Box::pin(child))
    }
}

impl tokio::io::AsyncRead for AwaitableConPtyMasterPty {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.inner
            .lock()
            .unwrap()
            .reader
            .lock()
            .unwrap()
            .poll_read_impl(cx, buf)
    }
}

impl tokio::io::AsyncRead for AwaitableReaderArc {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.0.lock().unwrap().poll_read_impl(cx, buf)
    }
}

impl tokio::io::AsyncWrite for AwaitableConPtyMasterPty {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.inner.lock().unwrap().poll_write_impl(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[async_trait(?Send)]
impl crate::awaitable::MasterPty for AwaitableConPtyMasterPty {
    async fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        enum ResizeState {
            NotRequested,
            Waiting(Receiver<anyhow::Result<()>>),
            Done,
        }

        struct ResizeFuture {
            state: ResizeState,
            inner: Arc<Mutex<AwaitableInner>>,
            size: PtySize,
        }

        impl std::future::Future for ResizeFuture {
            type Output = anyhow::Result<()>;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                match std::mem::replace(&mut self.state, ResizeState::Done) {
                    ResizeState::NotRequested => {
                        let inner = self.inner.lock().unwrap();
                        let (tx, rx) = channel();
                        if let Err(err) = inner.write_tx.send(WriteRequest::Resize(
                            self.size.clone(),
                            cx.waker().clone(),
                            tx,
                        )) {
                            return Poll::Ready(Err(anyhow!(
                                "sending write request to pty failed: {}",
                                err
                            )));
                        }

                        drop(inner);
                        self.state = ResizeState::Waiting(rx);
                        Poll::Pending
                    }
                    ResizeState::Waiting(rx) => match rx.try_recv() {
                        Ok(res) => {
                            // We just successfully changed the size, so
                            // record the new size
                            self.inner.lock().unwrap().size = self.size;
                            Poll::Ready(res)
                        }
                        Err(TryRecvError::Empty) => {
                            self.state = ResizeState::Waiting(rx);
                            Poll::Pending
                        }
                        Err(err) => Poll::Ready(Err(anyhow!(
                            "receiving write results from pty failed: {}",
                            err
                        ))),
                    },
                    ResizeState::Done => Poll::Ready(Err(anyhow!("polling a completed future?"))),
                }
            }
        }

        let future = ResizeFuture {
            state: ResizeState::NotRequested,
            inner: Arc::clone(&self.inner),
            size,
        };
        future.await
    }

    async fn get_size(&self) -> anyhow::Result<PtySize> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.size.clone())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Pin<Box<dyn tokio::io::AsyncRead + Send>>> {
        let inner = self.inner.lock().unwrap();
        Ok(Box::pin(AwaitableReaderArc(inner.reader.clone())))
    }
}

#[async_trait(?Send)]
impl crate::awaitable::PtySystem for ConPtySystem {
    async fn openpty(&self, size: PtySize) -> anyhow::Result<crate::awaitable::PtyPair> {
        let stdin = Pipe::new()?;
        let stdout = Pipe::new()?;

        let con = PsuedoCon::new(
            COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            },
            stdin.read,
            stdout.write,
        )?;

        let master = AwaitableConPtyMasterPty {
            inner: Arc::new(Mutex::new(AwaitableInner::new(
                con,
                stdout.read,
                stdin.write,
                size,
            ))),
        };

        let slave = AwaitableConPtySlavePty {
            inner: master.inner.clone(),
        };

        Ok(crate::awaitable::PtyPair {
            master: Box::pin(master),
            slave: Box::pin(slave),
        })
    }
}

fn peek_pipe_len(pipe: HANDLE) -> std::io::Result<usize> {
    let mut bytes_avail = 0;

    let res = unsafe {
        PeekNamedPipe(
            pipe,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes_avail,
            std::ptr::null_mut(),
        )
    };
    if res == 0 {
        return Err(IoError::last_os_error());
    }

    Ok(bytes_avail as usize)
}

impl AwaitableInner {
    fn new(
        con: PsuedoCon,
        readable: FileDescriptor,
        writable: FileDescriptor,
        size: PtySize,
    ) -> Self {
        let con = Arc::new(con);
        let (write_tx, write_rx) = channel();
        let write_thread_con = Arc::clone(&con);
        std::thread::spawn(move || Self::writer_thread(write_rx, write_thread_con, writable));

        let (wait_for_read, wait_rx) = channel();

        let (read_results_tx, read_results_rx) = sync_channel(1);
        std::thread::spawn(move || {
            AwaitableReader::reader_thread(wait_rx, readable, read_results_tx)
        });

        Self {
            con,
            size,
            write_tx,
            reader: Arc::new(Mutex::new(AwaitableReader {
                wait_for_read,
                read_results_rx,
                read_buffer: ReadBuffer::new(),
            })),
        }
    }

    fn writer_thread(
        to_write: Receiver<WriteRequest>,
        con: Arc<PsuedoCon>,
        mut writable: FileDescriptor,
    ) {
        while let Ok(item) = to_write.recv() {
            match item {
                WriteRequest::Data(data) => {
                    if let Err(_err) = writable.write_all(&data) {
                        // FIXME: set errored flag?
                        // Right now we defer error detection to
                        // the read side
                    }
                }
                WriteRequest::Resize(size, waker, results) => {
                    let res = con.resize(COORD {
                        X: size.cols as i16,
                        Y: size.rows as i16,
                    });
                    let res = results.send(res);
                    waker.wake();
                    if res.is_err() {
                        break;
                    }
                }
            }
        }
    }

    pub fn poll_write_impl(
        &mut self,
        _cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        // The poll_write API has EAGAIN semantics which are too
        // awkward to emulate here.  We'll simply buffer up the
        // write and claim that it succeeded.
        let len = buf.len();
        if let Err(err) = self.write_tx.send(WriteRequest::Data(buf.to_vec())) {
            Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("unable to queue write ({}); treating as EOF", err),
            )))
        } else {
            Poll::Ready(Ok(len))
        }
    }
}

impl AwaitableReader {
    fn poll_read_impl(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        // Try to satisfy from the buffer
        let len = self.read_buffer.consume(buf);
        if len > 0 {
            return Poll::Ready(Ok(len));
        }

        match self.read_results_rx.try_recv() {
            // A successful read; store to the buffer and then return
            // an appropriate portion of it to our caller.
            Ok(Ok(recv_buf)) => {
                self.read_buffer.append(&recv_buf);

                let len = self.read_buffer.consume(buf);
                if len > 0 {
                    return Poll::Ready(Ok(len));
                }

                // Probably impossible, but if we get here, we'll fall
                // through below and request some more data to be read
            }

            // Map BrokenPipe errors to EOF for easier POSIX compatibility
            Ok(Err(err)) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                return Poll::Ready(Ok(0))
            }

            // Other errors are returned to the caller
            Ok(Err(err)) => return Poll::Ready(Err(err)),

            Err(TryRecvError::Empty) => {
                // There are no reasults ready to read yet.
                // We'll queue up a read below.
            }

            // There's a problem with the channel, most likely we're partially
            // destroyed: relay this as an EOF error (distinct from EOF) so
            // that it bubbles up as an actual error condition
            Err(err) => {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("unable to receive read results ({}); treating as EOF", err),
                )))
            }
        }

        // Ask the reader thread to do some IO for us
        match self.wait_for_read.send((cx.waker().clone(), buf.len())) {
            Ok(_) => Poll::Pending,
            Err(err) => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("unable to queue read ({}); treating as EOF", err),
            ))),
        }
    }

    fn reader_thread(
        wait_requests: Receiver<(Waker, usize)>,
        mut readable: FileDescriptor,
        read_results: SyncSender<std::io::Result<Vec<u8>>>,
    ) {
        while let Ok((waker, size)) = wait_requests.recv() {
            // If there is data already present in the pipe, we know that
            // our read will immediately succeed for that size, so prefer
            // to size our local vector to match.
            // If there is no data available to read, take the size from
            // the caller.
            let res = match peek_pipe_len(readable.as_raw_handle()) {
                Ok(avail) => {
                    let size = if avail == 0 { size } else { size.min(avail) };

                    let mut buf = vec![0u8; size];

                    readable.read(&mut buf).map(|size| {
                        buf.resize(size, 0);
                        buf
                    })
                }
                Err(err) => Err(err),
            };

            let broken = read_results.send(res).is_err();
            waker.wake();
            if broken {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct AwaitableReaderArc(pub Arc<Mutex<AwaitableReader>>);
