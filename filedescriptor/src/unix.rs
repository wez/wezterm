use crate::{AsRawFileDescriptor, FromRawFileDescriptor, IntoRawFileDescriptor, Pipes};
use failure::{bail, Fallible};
use std::os::unix::prelude::*;

pub type RawFileDescriptor = RawFd;

impl<T: AsRawFd> AsRawFileDescriptor for T {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor {
        self.as_raw_fd()
    }
}

impl<T: IntoRawFd> IntoRawFileDescriptor for T {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor {
        self.into_raw_fd()
    }
}

impl<T: FromRawFd> FromRawFileDescriptor for T {
    unsafe fn from_raw_file_descrptor(fd: RawFileDescriptor) -> Self {
        Self::from_raw_fd(fd)
    }
}

#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawFileDescriptor,
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.handle);
        }
    }
}

impl AsRawFd for OwnedHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.handle
    }
}

impl IntoRawFd for OwnedHandle {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.handle;
        std::mem::forget(self);
        fd
    }
}

impl FromRawFd for OwnedHandle {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { handle: fd }
    }
}

impl OwnedHandle {
    /// Helper function to set the close-on-exec flag for a raw descriptor
    fn cloexec(&mut self) -> Fallible<()> {
        let flags = unsafe { libc::fcntl(self.handle, libc::F_GETFD) };
        if flags == -1 {
            bail!(
                "fcntl to read flags failed: {:?}",
                std::io::Error::last_os_error()
            );
        }
        let result = unsafe { libc::fcntl(self.handle, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result == -1 {
            bail!(
                "fcntl to set CLOEXEC failed: {:?}",
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }

    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        Self {
            handle: f.into_raw_file_descriptor(),
        }
    }

    pub fn dup<F: AsRawFileDescriptor>(fd: &F) -> Fallible<Self> {
        let fd = fd.as_raw_file_descriptor();
        let duped = unsafe { libc::dup(fd) };
        if duped == -1 {
            bail!(
                "dup of fd {} failed: {:?}",
                fd,
                std::io::Error::last_os_error()
            )
        } else {
            let mut owned = OwnedHandle { handle: duped };
            owned.cloexec()?;
            Ok(owned)
        }
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        Self::dup(self)
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    fd: OwnedHandle,
}

impl std::io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::read(self.fd.handle, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl std::io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::write(self.fd.handle, buf.as_ptr() as *const _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for FileDescriptor {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for FileDescriptor {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            fd: OwnedHandle::from_raw_fd(fd),
        }
    }
}

impl FileDescriptor {
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let fd = OwnedHandle::new(f);
        Self { fd }
    }

    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        OwnedHandle::dup(f).map(|fd| Self { fd })
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        self.fd.try_clone().map(|fd| Self { fd })
    }

    pub fn pipe() -> Fallible<Pipes> {
        let mut fds = [-1i32; 2];
        let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if res == -1 {
            bail!(
                "failed to create a pipe: {:?}",
                std::io::Error::last_os_error()
            )
        } else {
            let mut read = FileDescriptor {
                fd: OwnedHandle { handle: fds[0] },
            };
            let mut write = FileDescriptor {
                fd: OwnedHandle { handle: fds[1] },
            };
            read.fd.cloexec()?;
            write.fd.cloexec()?;
            Ok(Pipes { read, write })
        }
    }

    pub fn as_stdio(&self) -> Fallible<std::process::Stdio> {
        let duped = OwnedHandle::dup(self)?;
        let fd = duped.into_raw_fd();
        let stdio = unsafe { std::process::Stdio::from_raw_fd(fd) };
        Ok(stdio)
    }
}
