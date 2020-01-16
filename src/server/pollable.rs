use crate::server::UnixStream;
use anyhow::Error;
use crossbeam_channel::{unbounded as channel, Receiver, Sender, TryRecvError};
use filedescriptor::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

pub trait ReadAndWrite: std::io::Read + std::io::Write + Send + AsPollFd {
    fn set_non_blocking(&self, non_blocking: bool) -> anyhow::Result<()>;
    fn has_read_buffered(&self) -> bool;
}
impl ReadAndWrite for UnixStream {
    fn set_non_blocking(&self, non_blocking: bool) -> anyhow::Result<()> {
        self.set_nonblocking(non_blocking)?;
        Ok(())
    }
    fn has_read_buffered(&self) -> bool {
        false
    }
}
impl ReadAndWrite for native_tls::TlsStream<std::net::TcpStream> {
    fn set_non_blocking(&self, non_blocking: bool) -> anyhow::Result<()> {
        self.get_ref().set_nonblocking(non_blocking)?;
        Ok(())
    }
    fn has_read_buffered(&self) -> bool {
        self.buffered_read_size().unwrap_or(0) != 0
    }
}

#[cfg(any(feature = "openssl", unix))]
impl ReadAndWrite for openssl::ssl::SslStream<std::net::TcpStream> {
    fn set_non_blocking(&self, non_blocking: bool) -> anyhow::Result<()> {
        self.get_ref().set_nonblocking(non_blocking)?;
        Ok(())
    }
    fn has_read_buffered(&self) -> bool {
        self.ssl().pending() != 0
    }
}

pub struct PollableSender<T> {
    sender: Sender<T>,
    write: Arc<Mutex<FileDescriptor>>,
}

impl<T: Send + Sync + 'static> PollableSender<T> {
    pub fn send(&self, item: T) -> anyhow::Result<()> {
        // Attempt to write to the pipe; if it fails due to
        // being full, that's fine: it means that the other end
        // is going to be signalled already so they won't miss
        // anything if our write doesn't happen.
        self.write.lock().unwrap().write_all(b"x").ok();
        self.sender.send(item).map_err(Error::msg)?;
        Ok(())
    }
}

impl<T> Clone for PollableSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            write: self.write.clone(),
        }
    }
}

pub struct PollableReceiver<T> {
    receiver: Receiver<T>,
    read: RefCell<FileDescriptor>,
}

impl<T> PollableReceiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        // try to drain the pipe.
        // We do this regardless of whether we popped an item
        // so that we avoid being in a perpetually signalled state.
        let mut byte = [0u8; 64];
        self.read.borrow_mut().read(&mut byte).ok();

        Ok(self.receiver.try_recv()?)
    }
}

impl<T> AsPollFd for PollableReceiver<T> {
    fn as_poll_fd(&self) -> pollfd {
        self.read.borrow().as_socket_descriptor().as_poll_fd()
    }
}

/// A channel that can be polled together with a socket.
/// This uses the self-pipe trick but with a unix domain
/// socketpair.
/// In theory this should also work on windows, but will require
/// windows 10 w/unix domain socket support.
pub fn pollable_channel<T>() -> anyhow::Result<(PollableSender<T>, PollableReceiver<T>)> {
    let (sender, receiver) = channel();
    let (mut write, mut read) = socketpair()?;

    write.set_non_blocking(true)?;
    read.set_non_blocking(true)?;

    Ok((
        PollableSender {
            sender,
            write: Arc::new(Mutex::new(FileDescriptor::new(write))),
        },
        PollableReceiver {
            receiver,
            read: RefCell::new(FileDescriptor::new(read)),
        },
    ))
}

pub trait AsPollFd {
    fn as_poll_fd(&self) -> pollfd;
}

impl AsPollFd for SocketDescriptor {
    fn as_poll_fd(&self) -> pollfd {
        pollfd {
            fd: *self,
            events: POLLIN,
            revents: 0,
        }
    }
}

impl AsPollFd for native_tls::TlsStream<TcpStream> {
    fn as_poll_fd(&self) -> pollfd {
        self.get_ref().as_socket_descriptor().as_poll_fd()
    }
}

#[cfg(any(feature = "openssl", unix))]
impl AsPollFd for openssl::ssl::SslStream<TcpStream> {
    fn as_poll_fd(&self) -> pollfd {
        self.get_ref().as_socket_descriptor().as_poll_fd()
    }
}

impl AsPollFd for UnixStream {
    fn as_poll_fd(&self) -> pollfd {
        self.as_socket_descriptor().as_poll_fd()
    }
}

pub fn poll_for_read(pfd: &mut [pollfd]) {
    if let Err(e) = poll(pfd, None) {
        log::error!("poll failed for {}", e);
    }
}
