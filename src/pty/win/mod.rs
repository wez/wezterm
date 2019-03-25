use std::io::{Error as IoError, Result as IoResult};
use winapi::shared::minwindef::DWORD;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::*;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;

pub mod cmdline;

#[cfg(not(feature = "use-winpty"))]
pub mod conpty;
#[cfg(feature = "use-winpty")]
pub mod winpty;

pub mod ownedhandle;

use ownedhandle::OwnedHandle;

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[derive(Debug)]
pub struct Child {
    proc: OwnedHandle,
}

impl Child {
    pub fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        let mut status: DWORD = 0;
        let res = unsafe { GetExitCodeProcess(self.proc.handle, &mut status) };
        if res != 0 {
            if status == STILL_ACTIVE {
                Ok(None)
            } else {
                Ok(Some(ExitStatus { status }))
            }
        } else {
            Ok(None)
        }
    }

    pub fn kill(&mut self) -> IoResult<ExitStatus> {
        unsafe {
            TerminateProcess(self.proc.handle, 1);
        }
        self.wait()
    }

    pub fn wait(&mut self) -> IoResult<ExitStatus> {
        if let Ok(Some(status)) = self.try_wait() {
            return Ok(status);
        }
        unsafe {
            WaitForSingleObject(self.proc.handle, INFINITE);
        }
        let mut status: DWORD = 0;
        let res = unsafe { GetExitCodeProcess(self.proc.handle, &mut status) };
        if res != 0 {
            Ok(ExitStatus { status })
        } else {
            Err(IoError::last_os_error())
        }
    }
}

#[derive(Debug)]
pub struct ExitStatus {
    status: DWORD,
}
