//! Working with pseudo-terminals

use failure::Error;
use libc::{self, winsize};
use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::ptr;

pub use std::process::{Child, Command, ExitStatus};

#[derive(Debug)]
pub struct OwnedFd {
    fd: RawFd,
}

impl OwnedFd {
    fn try_clone(&self) -> Result<Self, Error> {
        // Note that linux has a variant of the dup syscall that can set
        // the CLOEXEC flag at dup time.  We could use that here but the
        // additional code complexity isn't worth it: it's just a couple
        // of syscalls at startup to do it the portable way below.
        let new_fd = unsafe { libc::dup(self.fd) };
        if new_fd == -1 {
            bail!("dup of pty fd failed: {:?}", io::Error::last_os_error())
        }
        let new_fd = OwnedFd { fd: new_fd };
        cloexec(new_fd.as_raw_fd())?;
        Ok(new_fd)
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl IntoRawFd for OwnedFd {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.fd;
        mem::forget(self);
        fd
    }
}

impl FromRawFd for OwnedFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl io::Read for OwnedFd {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let size = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl io::Write for OwnedFd {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let size = unsafe { libc::write(self.fd, buf.as_ptr() as *const _, buf.len()) };
        if size == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

/// Represents the master end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
pub struct MasterPty {
    fd: OwnedFd,
}

/// Represents the slave end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
pub struct SlavePty {
    fd: OwnedFd,
}

/// Helper function to set the close-on-exec flag for a raw descriptor
fn cloexec(fd: RawFd) -> Result<(), Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        bail!(
            "fcntl to read flags failed: {:?}",
            io::Error::last_os_error()
        );
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result == -1 {
        bail!(
            "fcntl to set CLOEXEC failed: {:?}",
            io::Error::last_os_error()
        );
    }
    Ok(())
}

#[allow(dead_code)]
fn clear_nonblocking(fd: RawFd) -> Result<(), Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags == -1 {
        bail!(
            "fcntl to read flags failed: {:?}",
            io::Error::last_os_error()
        );
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
    if result == -1 {
        bail!(
            "fcntl to set NONBLOCK failed: {:?}",
            io::Error::last_os_error()
        );
    }
    Ok(())
}

#[allow(dead_code)]
fn set_nonblocking(fd: RawFd) -> Result<(), Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags == -1 {
        bail!(
            "fcntl to read flags failed: {:?}",
            io::Error::last_os_error()
        );
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result == -1 {
        bail!(
            "fcntl to set NONBLOCK failed: {:?}",
            io::Error::last_os_error()
        );
    }
    Ok(())
}

/// Create a new Pty instance with the window size set to the specified
/// dimensions.  Returns a (master, slave) Pty pair.  The master side
/// is used to drive the slave side.
pub fn openpty(
    num_rows: u16,
    num_cols: u16,
    pixel_width: u16,
    pixel_height: u16,
) -> Result<(MasterPty, SlavePty), Error> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;

    let mut size = winsize {
        ws_row: num_rows,
        ws_col: num_cols,
        ws_xpixel: pixel_width,
        ws_ypixel: pixel_height,
    };

    let result = unsafe {
        // BSDish systems may require mut pointers to some args
        #[cfg_attr(feature = "cargo-clippy", allow(clippy::unnecessary_mut_passed))]
        libc::openpty(
            &mut master,
            &mut slave,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut size,
        )
    };

    if result != 0 {
        bail!("failed to openpty: {:?}", io::Error::last_os_error());
    }

    let master = MasterPty {
        fd: OwnedFd { fd: master },
    };
    let slave = SlavePty {
        fd: OwnedFd { fd: slave },
    };

    // Ensure that these descriptors will get closed when we execute
    // the child process.  This is done after constructing the Pty
    // instances so that we ensure that the Ptys get drop()'d if
    // the cloexec() functions fail (unlikely!).
    cloexec(master.fd.as_raw_fd())?;
    cloexec(slave.fd.as_raw_fd())?;

    Ok((master, slave))
}

impl SlavePty {
    /// Helper for setting up a Command instance
    fn as_stdio(&self) -> Result<Stdio, Error> {
        let dup = self.fd.try_clone()?;
        Ok(unsafe { Stdio::from_raw_fd(dup.into_raw_fd()) })
    }

    /// this method prepares a Command builder to spawn a process with the Pty
    /// set up to be the controlling terminal, and then spawns the command.
    /// This method consumes the slave Pty instance and the Command builder
    /// instance so that the associated file descriptors are closed.
    /// The `cmd` parameter is set up to reference the slave
    /// Pty for its stdio streams, as well as to establish itself as the session
    /// leader.
    pub fn spawn_command(self, mut cmd: Command) -> Result<Child, Error> {
        cmd.stdin(self.as_stdio()?)
            .stdout(self.as_stdio()?)
            .stderr(self.as_stdio()?)
            .before_exec(move || {
                // Clean up a few things before we exec the program
                unsafe {
                    // Clear out any potentially problematic signal
                    // dispositions that we might have inherited
                    for signo in &[
                        libc::SIGCHLD,
                        libc::SIGHUP,
                        libc::SIGINT,
                        libc::SIGQUIT,
                        libc::SIGTERM,
                        libc::SIGALRM,
                    ] {
                        libc::signal(*signo, libc::SIG_DFL);
                    }

                    // Establish ourselves as a session leader.
                    if libc::setsid() == -1 {
                        return Err(io::Error::last_os_error());
                    }

                    // Clippy wants us to explicitly cast TIOCSCTTY using
                    // type::from(), but the size and potentially signedness
                    // are system dependent, which is why we're using `as _`.
                    // Suppress this lint for this section of code.
                    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_lossless))]
                    {
                        // Set the pty as the controlling terminal.
                        // Failure to do this means that delivery of
                        // SIGWINCH won't happen when we resize the
                        // terminal, among other undesirable effects.
                        if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                }
            });

        let mut child = cmd.spawn()?;

        // Ensure that we close out the slave fds that Child retains;
        // they are not what we need (we need the master side to reference
        // them) and won't work in the usual way anyway.
        // In practice these are None, but it seems best to be move them
        // out in case the behavior of Command changes in the future.
        child.stdin.take();
        child.stdout.take();
        child.stderr.take();

        Ok(child)
    }
}

impl MasterPty {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    pub fn resize(
        &self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        let size = winsize {
            ws_row: num_rows,
            ws_col: num_cols,
            ws_xpixel: pixel_width,
            ws_ypixel: pixel_height,
        };

        if unsafe { libc::ioctl(self.fd.as_raw_fd(), libc::TIOCSWINSZ, &size as *const _) } != 0 {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                io::Error::last_os_error()
            );
        }

        Ok(())
    }

    pub fn get_size(&self) -> Result<winsize, Error> {
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(self.fd.as_raw_fd(), libc::TIOCGWINSZ, &mut size as *mut _) } != 0 {
            bail!(
                "failed to ioctl(TIOCGWINSZ): {:?}",
                io::Error::last_os_error()
            );
        }
        Ok(size)
    }

    pub fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error> {
        let fd = self.fd.try_clone()?;
        Ok(Box::new(fd))
    }
}

impl io::Write for MasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.fd.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.fd.flush()
    }
}
