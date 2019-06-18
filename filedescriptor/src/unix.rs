use crate::{
    AsRawFileDescriptor, FileDescriptor, FromRawFileDescriptor, IntoRawFileDescriptor, OwnedHandle,
    Pipe,
};
use failure::{bail, Fallible};
use std::os::unix::prelude::*;

/// `RawFileDescriptor` is a platform independent type alias for the
/// underlying platform file descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
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
    unsafe fn from_raw_file_descriptor(fd: RawFileDescriptor) -> Self {
        Self::from_raw_fd(fd)
    }
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

    fn non_atomic_dup(fd: RawFd) -> Fallible<Self> {
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

    #[inline]
    pub(crate) fn dup_impl<F: AsRawFileDescriptor>(fd: &F) -> Fallible<Self> {
        let fd = fd.as_raw_file_descriptor();
        let duped = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if duped == -1 {
            let err = std::io::Error::last_os_error();
            if let Some(libc::EINVAL) = err.raw_os_error() {
                // We may be running on eg: WSL or an old kernel that
                // doesn't support F_DUPFD_CLOEXEC; fall back.
                return Self::non_atomic_dup(fd);
            } else {
                bail!("dup of fd {} failed: {:?}", fd, err)
            }
        } else {
            Ok(OwnedHandle { handle: duped })
        }
    }
}

impl std::io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::read(self.handle.handle, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl std::io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let size = unsafe { libc::write(self.handle.handle, buf.as_ptr() as *const _, buf.len()) };
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
        self.handle.as_raw_fd()
    }
}

impl IntoRawFd for FileDescriptor {
    fn into_raw_fd(self) -> RawFd {
        self.handle.into_raw_fd()
    }
}

impl FromRawFd for FileDescriptor {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            handle: OwnedHandle::from_raw_fd(fd),
        }
    }
}

impl FileDescriptor {
    #[inline]
    pub(crate) fn as_stdio_impl(&self) -> Fallible<std::process::Stdio> {
        let duped = OwnedHandle::dup(self)?;
        let fd = duped.into_raw_fd();
        let stdio = unsafe { std::process::Stdio::from_raw_fd(fd) };
        Ok(stdio)
    }
}

impl Pipe {
    #[cfg(target_os = "linux")]
    pub fn new() -> Fallible<Pipe> {
        let mut fds = [-1i32; 2];
        let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if res == -1 {
            bail!(
                "failed to create a pipe: {:?}",
                std::io::Error::last_os_error()
            )
        } else {
            let read = FileDescriptor {
                handle: OwnedHandle { handle: fds[0] },
            };
            let write = FileDescriptor {
                handle: OwnedHandle { handle: fds[1] },
            };
            Ok(Pipe { read, write })
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Fallible<Pipe> {
        let mut fds = [-1i32; 2];
        let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if res == -1 {
            bail!(
                "failed to create a pipe: {:?}",
                std::io::Error::last_os_error()
            )
        } else {
            let mut read = FileDescriptor {
                handle: OwnedHandle { handle: fds[0] },
            };
            let mut write = FileDescriptor {
                handle: OwnedHandle { handle: fds[1] },
            };
            read.handle.cloexec()?;
            write.handle.cloexec()?;
            Ok(Pipe { read, write })
        }
    }
}

#[cfg(target_os = "linux")]
#[doc(hidden)]
pub fn socketpair_impl() -> Fallible<(FileDescriptor, FileDescriptor)> {
    let mut fds = [-1i32; 2];
    let res = unsafe {
        libc::socketpair(
            libc::PF_LOCAL,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if res == -1 {
        bail!(
            "failed to create a socketpair: {:?}",
            std::io::Error::last_os_error()
        )
    } else {
        let mut read = FileDescriptor {
            handle: OwnedHandle { handle: fds[0] },
        };
        let mut write = FileDescriptor {
            handle: OwnedHandle { handle: fds[1] },
        };
        Ok((read, write))
    }
}

#[cfg(not(target_os = "linux"))]
#[doc(hidden)]
pub fn socketpair_impl() -> Fallible<(FileDescriptor, FileDescriptor)> {
    let mut fds = [-1i32; 2];
    let res = unsafe { libc::socketpair(libc::PF_LOCAL, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    if res == -1 {
        bail!(
            "failed to create a socketpair: {:?}",
            std::io::Error::last_os_error()
        )
    } else {
        let mut read = FileDescriptor {
            handle: OwnedHandle { handle: fds[0] },
        };
        let mut write = FileDescriptor {
            handle: OwnedHandle { handle: fds[1] },
        };
        read.handle.cloexec()?;
        write.handle.cloexec()?;
        Ok((read, write))
    }
}

pub use libc::{pollfd, POLLERR, POLLHUP, POLLIN, POLLOUT};
use std::time::Duration;

#[cfg(not(target_os = "macos"))]
#[doc(hidden)]
pub fn poll_impl(pfd: &mut [pollfd], duration: Option<Duration>) -> Fallible<usize> {
    let poll_result = unsafe {
        libc::poll(
            pfd.as_mut_ptr(),
            pfd.len() as _,
            duration
                .map(|wait| wait.as_millis() as libc::c_int)
                .unwrap_or(-1),
        )
    };
    if poll_result < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(poll_result as usize)
    }
}

// macOS has a broken poll(2) implementation, so we introduce a layer to deal with that here
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libc::{fd_set, timeval, FD_ISSET, FD_SET, FD_SETSIZE, FD_ZERO, POLLERR, POLLIN, POLLOUT};
    use std::os::unix::io::RawFd;

    struct FdSet {
        set: fd_set,
    }

    #[inline]
    fn check_fd(fd: RawFd) -> Fallible<()> {
        failure::ensure!(fd >= 0, "illegal fd value");
        failure::ensure!(
            (fd as usize) < FD_SETSIZE,
            "fd value is too large to use with select(2) on macos"
        );
        Ok(())
    }

    impl FdSet {
        pub fn new() -> Self {
            unsafe {
                let mut set = std::mem::uninitialized();
                FD_ZERO(&mut set);
                Self { set }
            }
        }

        pub fn add(&mut self, fd: RawFd) -> Fallible<()> {
            check_fd(fd)?;
            unsafe {
                FD_SET(fd, &mut self.set);
            }
            Ok(())
        }

        pub fn contains(&mut self, fd: RawFd) -> bool {
            check_fd(fd).unwrap();
            unsafe { FD_ISSET(fd, &mut self.set) }
        }
    }

    fn materialize(set: &mut Option<FdSet>) -> &mut FdSet {
        set.get_or_insert_with(FdSet::new)
    }

    fn set_ptr(set: &mut Option<FdSet>) -> *mut fd_set {
        set.as_mut()
            .map(|s| &mut s.set as *mut _)
            .unwrap_or_else(std::ptr::null_mut)
    }

    fn is_set(set: &mut Option<FdSet>, fd: RawFd) -> bool {
        set.as_mut().map(|s| s.contains(fd)).unwrap_or(false)
    }

    pub fn poll_impl(pfd: &mut [pollfd], duration: Option<Duration>) -> Fallible<usize> {
        let mut read_set = None;
        let mut write_set = None;
        let mut exception_set = None;
        let mut nfds = 0;

        for item in pfd.iter_mut() {
            item.revents = 0;

            nfds = nfds.max(item.fd);

            if item.events & POLLIN != 0 {
                materialize(&mut read_set).add(item.fd)?;
            }
            if item.events & POLLOUT != 0 {
                materialize(&mut write_set).add(item.fd)?;
            }
            materialize(&mut exception_set).add(item.fd)?;
        }

        let mut timeout = duration.map(|d| timeval {
            tv_sec: d.as_secs() as _,
            tv_usec: d.as_micros() as _,
        });

        let res = unsafe {
            libc::select(
                nfds + 1,
                set_ptr(&mut read_set),
                set_ptr(&mut write_set),
                set_ptr(&mut exception_set),
                timeout
                    .as_mut()
                    .map(|t| t as *mut _)
                    .unwrap_or_else(std::ptr::null_mut),
            )
        };

        if res < 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            for item in pfd.iter_mut() {
                if is_set(&mut read_set, item.fd) {
                    item.revents |= POLLIN;
                }
                if is_set(&mut write_set, item.fd) {
                    item.revents |= POLLOUT;
                }
                if is_set(&mut exception_set, item.fd) {
                    item.revents |= POLLERR;
                }
            }

            Ok(res as usize)
        }
    }
}

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use macos::poll_impl;
