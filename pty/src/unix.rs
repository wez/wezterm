//! Working with pseudo-terminals

use crate::{Child, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use anyhow::{bail, Context as _, Error};
use async_trait::async_trait;
use filedescriptor::FileDescriptor;
use libc::{self, winsize};
use mio::unix::EventedFd;
use mio::{self, Evented, PollOpt, Ready, Token};
use std::io;
use std::io::{Read, Write};
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};
use tokio::io::PollEvented;

#[derive(Default)]
pub struct UnixPtySystem {}

fn openpty(size: PtySize) -> anyhow::Result<(UnixMasterPty, UnixSlavePty)> {
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
        fd: PtyFd(unsafe { FileDescriptor::from_raw_fd(master) }),
    };
    let slave = UnixSlavePty {
        fd: PtyFd(unsafe { FileDescriptor::from_raw_fd(slave) }),
    };

    // Ensure that these descriptors will get closed when we execute
    // the child process.  This is done after constructing the Pty
    // instances so that we ensure that the Ptys get drop()'d if
    // the cloexec() functions fail (unlikely!).
    cloexec(master.fd.as_raw_fd())?;
    cloexec(slave.fd.as_raw_fd())?;

    Ok((master, slave))
}

impl PtySystem for UnixPtySystem {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair> {
        let (master, slave) = openpty(size)?;
        Ok(PtyPair {
            master: Box::new(master),
            slave: Box::new(slave),
        })
    }
}

#[async_trait(?Send)]
impl crate::awaitable::PtySystem for UnixPtySystem {
    async fn openpty(&self, size: PtySize) -> anyhow::Result<crate::awaitable::PtyPair> {
        let (mut master, mut slave) = openpty(size)?;

        master.fd.set_non_blocking(true)?;
        slave.fd.set_non_blocking(true)?;

        Ok(crate::awaitable::PtyPair {
            master: Box::pin(AwaitableMasterPty {
                io: PollEvented::new(master.fd)?,
            }),
            slave: Box::pin(AwaitableSlavePty {
                io: PollEvented::new(slave.fd)?,
            }),
        })
    }
}

struct PtyFd(pub FileDescriptor);
impl std::ops::Deref for PtyFd {
    type Target = FileDescriptor;
    fn deref(&self) -> &FileDescriptor {
        &self.0
    }
}
impl std::ops::DerefMut for PtyFd {
    fn deref_mut(&mut self) -> &mut FileDescriptor {
        &mut self.0
    }
}

impl Read for PtyFd {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        match self.0.read(buf) {
            Err(ref e)
                if e.kind() == io::ErrorKind::Other && e.raw_os_error() == Some(libc::EIO) =>
            {
                // EIO indicates that the slave pty has been closed.
                // Treat this as EOF so that std::io::Read::read_to_string
                // and similar functions gracefully terminate when they
                // encounter this condition
                Ok(0)
            }
            x => x,
        }
    }
}

impl PtyFd {
    fn resize(&self, size: PtySize) -> Result<(), Error> {
        let ws_size = winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: size.pixel_width,
            ws_ypixel: size.pixel_height,
        };

        if unsafe { libc::ioctl(self.0.as_raw_fd(), libc::TIOCSWINSZ, &ws_size as *const _) } != 0 {
            bail!(
                "failed to ioctl(TIOCSWINSZ): {:?}",
                io::Error::last_os_error()
            );
        }

        Ok(())
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        let mut size: winsize = unsafe { mem::zeroed() };
        if unsafe { libc::ioctl(self.0.as_raw_fd(), libc::TIOCGWINSZ, &mut size as *mut _) } != 0 {
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

    fn spawn_command(&self, builder: CommandBuilder) -> anyhow::Result<std::process::Child> {
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

        Ok(child)
    }
}

impl Evented for PtyFd {
    fn register(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> std::io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).deregister(poll)
    }
}

/// Represents the master end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
struct UnixMasterPty {
    fd: PtyFd,
}

/// Represents the slave end of a pty.
/// The file descriptor will be closed when the Pty is dropped.
struct UnixSlavePty {
    fd: PtyFd,
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
        Ok(Box::new(self.fd.spawn_command(builder)?))
    }
}

impl MasterPty for UnixMasterPty {
    fn resize(&self, size: PtySize) -> Result<(), Error> {
        self.fd.resize(size)
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        self.fd.get_size()
    }

    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, Error> {
        let fd = PtyFd(self.fd.try_clone()?);
        Ok(Box::new(fd))
    }
}

impl Write for UnixMasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.fd.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.fd.flush()
    }
}

struct AwaitableSlavePty {
    io: PollEvented<PtyFd>,
}

struct AwaitableMasterPty {
    io: PollEvented<PtyFd>,
}

#[derive(Debug)]
struct AwaitableChild {
    pid: libc::pid_t,
    waiting: Option<std::sync::mpsc::Receiver<anyhow::Result<ExitStatus>>>,
}

impl crate::awaitable::Child for AwaitableChild {
    fn kill(&mut self) -> std::io::Result<()> {
        let res = unsafe { libc::kill(self.pid, libc::SIGKILL) };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

impl std::future::Future for AwaitableChild {
    type Output = anyhow::Result<ExitStatus>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<anyhow::Result<ExitStatus>> {
        if self.waiting.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let pid = self.pid;
            let waker = cx.waker().clone();
            std::thread::spawn(move || loop {
                let mut status = 0;
                let reaped = unsafe { libc::waitpid(pid, &mut status, 0) };
                let err = std::io::Error::last_os_error();
                if reaped == pid {
                    let exit_code = if unsafe { libc::WIFEXITED(status) } {
                        unsafe { libc::WEXITSTATUS(status) as u32 }
                    } else {
                        1
                    };
                    tx.send(Ok(ExitStatus::with_exit_code(exit_code))).ok();
                    waker.wake();
                    return;
                }

                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }

                tx.send(Err(err).context("waitpid result")).ok();
                waker.wake();
                return;
            });
            self.waiting = Some(rx);
        }

        match self.waiting.as_mut().unwrap().try_recv() {
            Ok(status) => Poll::Ready(status),
            Err(std::sync::mpsc::TryRecvError::Empty) => Poll::Pending,
            Err(err) => Poll::Ready(Err(err).context("receiving process wait status")),
        }
    }
}

#[async_trait(?Send)]
impl crate::awaitable::SlavePty for AwaitableSlavePty {
    async fn spawn_command(
        &self,
        builder: CommandBuilder,
    ) -> anyhow::Result<Pin<Box<dyn crate::awaitable::Child>>> {
        let child = self.io.get_ref().spawn_command(builder)?;
        let pid = child.id() as libc::pid_t;
        Ok(Box::pin(AwaitableChild { pid, waiting: None }))
    }
}

#[async_trait(?Send)]
impl crate::awaitable::MasterPty for AwaitableMasterPty {
    async fn resize(&self, size: PtySize) -> Result<(), Error> {
        self.io.get_ref().resize(size)
    }

    async fn get_size(&self) -> Result<PtySize, Error> {
        self.io.get_ref().get_size()
    }

    fn try_clone_reader(&self) -> anyhow::Result<Pin<Box<dyn tokio::io::AsyncRead + Send>>> {
        let mut fd = self.io.get_ref().try_clone()?;
        fd.set_non_blocking(true)?;
        Ok(Box::pin(AwaitableMasterPty {
            io: PollEvented::new(PtyFd(fd))?,
        }))
    }
}

impl AwaitableMasterPty {
    fn poll_write_impl(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if Poll::Pending == self.io.poll_write_ready(cx)? {
            return Poll::Pending;
        }

        match self.io.get_mut().0.write(buf) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_write_ready(cx)?;
                Poll::Pending
            }
            x => Poll::Ready(x),
        }
    }
}

impl tokio::io::AsyncWrite for AwaitableMasterPty {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.poll_write_impl(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl tokio::io::AsyncRead for AwaitableMasterPty {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        poll_read_impl(&mut self.io, cx, buf)
    }
}

fn poll_read_impl(
    io: &mut PollEvented<PtyFd>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
) -> Poll<io::Result<usize>> {
    if Poll::Pending == io.poll_read_ready(cx, Ready::readable())? {
        return Poll::Pending;
    }

    match io.get_mut().read(buf) {
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            io.clear_read_ready(cx, Ready::readable())?;
            Poll::Pending
        }
        x => Poll::Ready(x),
    }
}

impl tokio::io::AsyncRead for AwaitableSlavePty {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        poll_read_impl(&mut self.io, cx, buf)
    }
}
