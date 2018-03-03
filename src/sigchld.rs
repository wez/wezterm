//! Helper for detecting SIGCHLD

use failure::Error;
use libc;
use std::io;
use std::mem;
use std::ptr;
use wakeup::GuiSender;

static mut EVENT_LOOP: Option<GuiSender<()>> = None;

extern "C" fn chld_handler(_signo: libc::c_int, _si: *const libc::siginfo_t, _: *const u8) {
    unsafe {
        match EVENT_LOOP.as_mut() {
            Some(wakeup) => {
                wakeup.send(()).ok();
            }
            None => (),
        }
    }
}

pub fn activate(wakeup: GuiSender<()>) -> Result<(), Error> {
    unsafe {
        EVENT_LOOP = Some(wakeup);

        let mut sa: libc::sigaction = mem::zeroed();
        sa.sa_sigaction = chld_handler as usize;
        sa.sa_flags = (libc::SA_SIGINFO | libc::SA_RESTART | libc::SA_NOCLDSTOP) as _;
        let res = libc::sigaction(libc::SIGCHLD, &sa, ptr::null_mut());
        if res == -1 {
            bail!("sigaction SIGCHLD failed: {:?}", io::Error::last_os_error());
        }

        Ok(())
    }
}
