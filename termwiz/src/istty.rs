//! Making it a little more convenient and safe to query whether
//! something is a terminal teletype or not.
//! This module defines the IsTty trait and the is_tty method to
//! return true if the item represents a terminal.
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::System::Console::{GetConsoleMode, CONSOLE_MODE};

/// Adds the is_tty method to types that might represent a terminal
pub trait IsTty {
    /// Returns true if the instance is a terminal teletype, false
    /// otherwise.
    fn is_tty(&self) -> bool;
}

/// On unix, the `isatty()` library function returns true if a file
/// descriptor is a terminal.  Let's implement `IsTty` for anything
/// that has an associated raw file descriptor.
#[cfg(unix)]
impl<S: AsRawFd> IsTty for S {
    fn is_tty(&self) -> bool {
        let fd = self.as_raw_fd();
        unsafe { libc::isatty(fd) == 1 }
    }
}

#[cfg(windows)]
impl<S: AsRawHandle> IsTty for S {
    fn is_tty(&self) -> bool {
        let mut mode = CONSOLE_MODE::default();
        unsafe { GetConsoleMode(HANDLE(self.as_raw_handle() as isize), &mut mode).is_ok() }
    }
}
