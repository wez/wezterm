// macOS has a broken poll(2) implementation, so we introduce a layer to deal with that here

use libc::pollfd;
use std::time::Duration;

#[cfg(not(target_os = "macos"))]
pub fn poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize, std::io::Error> {
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
        Err(std::io::Error::last_os_error())
    } else {
        Ok(poll_result as usize)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libc::{fd_set, timeval, FD_ISSET, FD_SET, FD_SETSIZE, FD_ZERO, POLLERR, POLLIN, POLLOUT};
    use std::os::unix::io::RawFd;

    struct FdSet {
        set: fd_set,
    }

    fn check_fd(fd: RawFd) {
        assert!(fd >= 0);
        assert!((fd as usize) < FD_SETSIZE);
    }

    impl FdSet {
        pub fn new() -> Self {
            unsafe {
                let mut set = std::mem::uninitialized();
                FD_ZERO(&mut set);
                Self { set }
            }
        }

        pub fn add(&mut self, fd: RawFd) {
            check_fd(fd);
            unsafe {
                FD_SET(fd, &mut self.set);
            }
        }

        pub fn contains(&mut self, fd: RawFd) -> bool {
            check_fd(fd);
            unsafe { FD_ISSET(fd, &mut self.set) }
        }
    }

    fn materialize<'a>(set: &'a mut Option<FdSet>) -> &'a mut FdSet {
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

    pub fn poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize, std::io::Error> {
        let mut read_set = None;
        let mut write_set = None;
        let mut exception_set = None;
        let mut nfds = 0;

        for item in pfd.iter_mut() {
            item.revents = 0;

            nfds = nfds.max(item.fd);

            if item.events & POLLIN != 0 {
                materialize(&mut read_set).add(item.fd);
            }
            if item.events & POLLOUT != 0 {
                materialize(&mut write_set).add(item.fd);
            }
            materialize(&mut exception_set).add(item.fd);
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
            Err(std::io::Error::last_os_error())
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
pub use macos::poll;
