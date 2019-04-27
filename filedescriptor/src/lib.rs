use failure::{bail, Fallible};
use std::os::unix::prelude::*;

#[cfg(unix)]
pub trait AsRawFileDescriptor: AsRawFd {}

#[cfg(unix)]
impl<T: AsRawFd> AsRawFileDescriptor for T {}

#[cfg(windows)]
pub trait AsRawFileDescriptor: AsRawHandle {}

#[cfg(windows)]
impl<T: AsRawHandle> AsRawFileDescriptor for T {}

#[derive(Debug)]
pub struct FileDescriptor {
    fd: RawFd,
}

impl std::io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl std::io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::write(self.fd, buf.as_ptr() as *const _, buf.len()) };
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

impl Drop for FileDescriptor {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsRawFd for &FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

fn dup_fd(fd: RawFd) -> Fallible<FileDescriptor> {
    let duped = unsafe { libc::dup(fd) };
    if duped == -1 {
        bail!(
            "dup of fd {} failed: {:?}",
            fd,
            std::io::Error::last_os_error()
        )
    } else {
        let mut owned = FileDescriptor { fd: duped };
        owned.cloexec()?;
        Ok(owned)
    }
}

pub struct Pipes {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}

pub fn dup<F: AsRawFileDescriptor>(f: F) -> Fallible<FileDescriptor> {
    #[cfg(unix)]
    dup_fd(f.as_raw_fd())
}

impl FileDescriptor {
    pub fn dup<F: AsRawFileDescriptor>(f: F) -> Fallible<Self> {
        dup(f)
    }

    pub fn clone(&self) -> Fallible<FileDescriptor> {
        dup(self)
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
            let mut read = FileDescriptor { fd: fds[0] };
            let mut write = FileDescriptor { fd: fds[1] };
            read.cloexec()?;
            write.cloexec()?;
            Ok(Pipes { read, write })
        }
    }

    /// Helper function to set the close-on-exec flag for a raw descriptor
    fn cloexec(&mut self) -> Fallible<()> {
        let flags = unsafe { libc::fcntl(self.fd, libc::F_GETFD) };
        if flags == -1 {
            bail!(
                "fcntl to read flags failed: {:?}",
                std::io::Error::last_os_error()
            );
        }
        let result = unsafe { libc::fcntl(self.fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result == -1 {
            bail!(
                "fcntl to set CLOEXEC failed: {:?}",
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }

    pub fn as_stdio(&self) -> Fallible<std::process::Stdio> {
        let duped = dup_fd(self.fd)?;
        let fd = duped.fd;
        let stdio = unsafe { std::process::Stdio::from_raw_fd(fd) };
        std::mem::forget(duped); // don't drop; stdio now owns it
        Ok(stdio)
    }
}
