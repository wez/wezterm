//! Helper for detecting SIGCHLD

use failure::Error;
use glium::glutin::EventsLoopProxy;
use libc;
use std::io;
use std::mem;
use std::ptr;

static mut EVENT_LOOP: Option<EventsLoopProxy> = None;

extern "C" fn chld_handler(_signo: libc::c_int, _si: *const libc::siginfo_t, _: *const u8) {
    unsafe {
        match EVENT_LOOP.as_mut() {
            Some(proxy) => {
                proxy.wakeup().ok();
            }
            None => (),
        }
    }
}

pub fn activate(proxy: EventsLoopProxy) -> Result<(), Error> {
    unsafe {
        EVENT_LOOP = Some(proxy);

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
