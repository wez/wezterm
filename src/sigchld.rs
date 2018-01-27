//! Helper for detecting SIGCHLD

use failure::Error;
use libc;
use mio::{Poll, PollOpt, Ready, Token};
use mio::event::Evented;
use mio::unix::EventedFd;
use std::io;
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr;


pub struct ChildWaiter {
    fd: RawFd,
}

impl Drop for ChildWaiter {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl ChildWaiter {
    pub fn new() -> Result<ChildWaiter, Error> {
        unsafe {
            let mut mask: libc::sigset_t = mem::zeroed();
            libc::sigaddset(&mut mask, libc::SIGCHLD);
            let res = libc::sigprocmask(libc::SIG_BLOCK, &mut mask, ptr::null_mut());
            if res == -1 {
                bail!(
                    "sigprocmask BLOCK SIGCHLD failed: {:?}",
                    io::Error::last_os_error()
                );
            }

            let fd = libc::signalfd(-1, &mask, libc::SFD_NONBLOCK | libc::SFD_CLOEXEC);
            if fd == -1 {
                bail!("signalfd SIGCHLD failed: {:?}", io::Error::last_os_error());
            }

            Ok(ChildWaiter { fd })
        }
    }

    pub fn read_one(&self) -> Result<u32, Error> {
        const BUFSIZE: usize = mem::size_of::<libc::signalfd_siginfo>();
        let mut buf = [0u8; BUFSIZE];
        let res = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if res == BUFSIZE as isize {
            let siginfo: libc::signalfd_siginfo = unsafe { mem::transmute(buf) };
            Ok(siginfo.ssi_pid)
        } else {
            bail!("signalfd read failed: {:?}", io::Error::last_os_error());
        }
    }
}

/// Glue for working with mio
impl Evented for ChildWaiter {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.fd).deregister(poll)
    }
}
