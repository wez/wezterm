use crate::server::UnixStream;
use crossbeam_channel::{unbounded as channel, Receiver, Sender, TryRecvError};
use failure::{format_err, Fallible};
use filedescriptor::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::TcpStream;

pub trait ReadAndWrite: std::io::Read + std::io::Write + Send + AsPollFd {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()>;
    fn has_read_buffered(&self) -> bool;
}
impl ReadAndWrite for UnixStream {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()> {
        self.set_nonblocking(non_blocking)?;
        Ok(())
    }
    fn has_read_buffered(&self) -> bool {
        false
    }
}
impl ReadAndWrite for native_tls::TlsStream<std::net::TcpStream> {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()> {
        self.get_ref().set_nonblocking(non_blocking)?;
        Ok(())
    }
    fn has_read_buffered(&self) -> bool {
        self.buffered_read_size().unwrap_or(0) != 0
    }
}

pub struct PollableSender<T> {
    sender: Sender<T>,
    write: RefCell<FileDescriptor>,
}

impl<T> PollableSender<T> {
    pub fn send(&self, item: T) -> Fallible<()> {
        self.write.borrow_mut().write(b"x")?;
        self.sender.send(item).map_err(|e| format_err!("{}", e))?;
        Ok(())
    }
}

impl<T> Clone for PollableSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            write: RefCell::new(
                self.write
                    .borrow()
                    .try_clone()
                    .expect("failed to clone PollableSender fd"),
            ),
        }
    }
}

pub struct PollableReceiver<T> {
    receiver: Receiver<T>,
    read: RefCell<FileDescriptor>,
}

impl<T> PollableReceiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let item = self.receiver.try_recv()?;
        let mut byte = [0u8];
        self.read.borrow_mut().read(&mut byte).ok();
        Ok(item)
    }
}

impl<T> AsPollFd for PollableReceiver<T> {
    fn as_poll_fd(&self) -> pollfd {
        self.read.borrow().as_raw_file_descriptor().as_poll_fd()
    }
}

/// A channel that can be polled together with a socket.
/// This uses the self-pipe trick but with a unix domain
/// socketpair.
/// In theory this should also work on windows, but will require
/// windows 10 w/unix domain socket support.
pub fn pollable_channel<T>() -> Fallible<(PollableSender<T>, PollableReceiver<T>)> {
    let (sender, receiver) = channel();
    let (write, read) = socketpair()?;
    Ok((
        PollableSender {
            sender,
            write: RefCell::new(FileDescriptor::new(write)),
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

impl AsPollFd for RawFileDescriptor {
    fn as_poll_fd(&self) -> pollfd {
        pollfd {
            fd: *self,
            events: POLLIN | POLLERR,
            revents: 0,
        }
    }
}

impl AsPollFd for native_tls::TlsStream<TcpStream> {
    fn as_poll_fd(&self) -> pollfd {
        self.get_ref().as_raw_file_descriptor().as_poll_fd()
    }
}

impl AsPollFd for UnixStream {
    fn as_poll_fd(&self) -> pollfd {
        self.as_raw_file_descriptor().as_poll_fd()
    }
}

pub fn poll_for_read(pfd: &mut [pollfd]) {
    poll(pfd, None).ok();
}
