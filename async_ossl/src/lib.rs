use openssl::ssl::SslStream;
use std::net::TcpStream;

#[cfg(unix)]
pub trait AsRawDesc: std::os::unix::io::AsRawFd {}
#[cfg(windows)]
pub trait AsRawDesc: std::os::windows::io::AsRawSocket {}

#[derive(Debug)]
pub struct AsyncSslStream {
    s: SslStream<TcpStream>,
}

unsafe impl async_io::IoSafe for AsyncSslStream {}

impl AsyncSslStream {
    pub fn new(s: SslStream<TcpStream>) -> Self {
        Self { s }
    }
}

#[cfg(unix)]
impl std::os::fd::AsFd for AsyncSslStream {
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        self.s.get_ref().as_fd()
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for AsyncSslStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.s.get_ref().as_raw_fd()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawSocket for AsyncSslStream {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        self.s.get_ref().as_raw_socket()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsSocket for AsyncSslStream {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket {
        self.s.get_ref().as_socket()
    }
}

impl AsRawDesc for AsyncSslStream {}

impl std::io::Read for AsyncSslStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.s.read(buf)
    }
}

impl std::io::Write for AsyncSslStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.s.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.s.flush()
    }
}
