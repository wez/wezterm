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

#[cfg(not(target_os = "linux"))]
static mut WRITE_END: RawFd = -1;

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

#[cfg(not(target_os = "linux"))]
extern "C" fn chld_handler(_signo: libc::c_int, si: *const libc::siginfo_t, _: *const u8) {
    unsafe {
        let pid = (*si).si_pid;
        libc::write(
            WRITE_END,
            &pid as *const _ as *const libc::c_void,
            mem::size_of::<libc::pid_t>(),
        );
    }
}

impl ChildWaiter {
    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<ChildWaiter, Error> {
        unsafe {
            let mut pipe: [RawFd; 2] = [-1, -1];
            let res = libc::pipe(pipe.as_mut_ptr());
            if res == -1 {
                bail!("pipe failed: {:?}", io::Error::last_os_error());
            }

            WRITE_END = pipe[1];

            let mut sa: libc::sigaction = mem::zeroed();
            sa.sa_sigaction = chld_handler as usize;
            sa.sa_flags = (libc::SA_SIGINFO | libc::SA_RESTART | libc::SA_NOCLDSTOP) as _;
            let res = libc::sigaction(libc::SIGCHLD, &sa, ptr::null_mut());
            if res == -1 {
                bail!("sigaction SIGCHLD failed: {:?}", io::Error::last_os_error());
            }

            Ok(Self { fd: pipe[0] })
        }
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(not(target_os = "linux"))]
    pub fn read_one(&self) -> Result<u32, Error> {
        let mut pid: libc::pid_t = 0;
        let res = unsafe {
            libc::read(
                self.fd,
                &mut pid as *mut _ as *mut libc::c_void,
                mem::size_of::<libc::pid_t>(),
            )
        };
        if res == mem::size_of::<libc::pid_t>() as isize {
            Ok(pid as u32)
        } else {
            bail!("signalfd read failed: {:?}", io::Error::last_os_error());
        }
    }

    #[cfg(target_os = "linux")]
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
