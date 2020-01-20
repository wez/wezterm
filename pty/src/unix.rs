//! Working with pseudo-terminals

use crate::{Child, CommandBuilder, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use anyhow::{bail, Error};
use filedescriptor::FileDescriptor;
use libc::{self, winsize};
use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::ptr;

#[derive(Default)]
pub struct UnixPtySystem {}

impl PtySystem for UnixPtySystem {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair> {
        let mut master: RawFd = -1;
        let mut slave: RawFd = -1;

        let mut size = winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: size.pixel_width,
            ws_ypixel: size.pixel_height,
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

        let master = UnixMasterPty {
            fd: unsafe { FileDescriptor::from_raw_fd(master) },
        };
        let slave = UnixSlavePty {
            fd: unsafe { FileDescriptor::from_raw_fd(slave) },
        };

        // Ensure that these descriptors will get closed when we execute
        // the child process.  This is done after constructing the Pty
        // instances so that we ensure that the Ptys get drop()'d if
        // the cloexec() functions fail (unlikely!).
        cloexec(master.fd.as_raw_fd())?;
        cloexec(slave.fd.as_raw_fd())?;

        Ok(PtyPair {
            master: Box::new(master),
            slave: Box::new(slave),
        })
    }
}

/// Represents the master end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
pub struct UnixMasterPty {
    fd: FileDescriptor,
}

/// Represents the slave end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
pub struct UnixSlavePty {
    fd: FileDescriptor,
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

impl SlavePty for UnixSlavePty {
    fn spawn_command(&self, builder: CommandBuilder) -> Result<Box<dyn Child>, Error> {
        let mut cmd = builder.as_command()?;

        unsafe {
            cmd.stdin(self.as_stdio()?)
                .stdout(self.as_stdio()?)
                .stderr(self.as_stdio()?)
                .pre_exec(move || {
                    // Clean up a few things before we exec the program
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
                })
        };

        let mut child = cmd.spawn()?;

        // Ensure that we close out the slave fds that Child retains;
        // they are not what we need (we need the master side to reference
        // them) and won't work in the usual way anyway.
        // In practice these are None, but it seems best to be move them
        // out in case the behavior of Command changes in the future.
        child.stdin.take();
        child.stdout.take();
        child.stderr.take();

        Ok(Box::new(child))
    }
}

impl UnixSlavePty {
    /// Helper for setting up a Command instance
    fn as_stdio(&self) -> Result<Stdio, Error> {
        let dup = self.fd.try_clone()?;
        Ok(unsafe { Stdio::from_raw_fd(dup.into_raw_fd()) })
    }
}

impl MasterPty for UnixMasterPty {
    fn resize(&self, size: PtySize) -> Result<(), Error> {
        let ws_size = winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: size.pixel_width,
            ws_ypixel: size.pixel_height,
        };

        if unsafe { libc::ioctl(self.fd.as_raw_fd(), libc::TIOCSWINSZ, &ws_size as *const _) } != 0
        {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                io::Error::last_os_error()
            );
        }

        Ok(())
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(self.fd.as_raw_fd(), libc::TIOCGWINSZ, &mut size as *mut _) } != 0 {
            bail!(
                "failed to ioctl(TIOCGWINSZ): {:?}",
                io::Error::last_os_error()
            );
        }
        Ok(PtySize {
            rows: size.ws_row,
            cols: size.ws_col,
            pixel_width: size.ws_xpixel,
            pixel_height: size.ws_ypixel,
        })
    }

    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error> {
        let fd = self.fd.try_clone()?;
        Ok(Box::new(fd))
    }
}

impl io::Write for UnixMasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.fd.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.fd.flush()
    }
}
