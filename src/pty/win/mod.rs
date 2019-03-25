use crate::pty::{ChildTrait, ExitStatus};
use std::io::{Error as IoError, Result as IoResult};
use winapi::shared::minwindef::DWORD;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::*;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;

pub mod conpty;
pub mod winpty;

pub mod ownedhandle;

use ownedhandle::OwnedHandle;

#[derive(Debug)]
pub struct Child {
    proc: OwnedHandle,
}

impl ChildTrait for Child {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        Child::try_wait(self)
    }
    fn kill(&mut self) -> IoResult<()> {
        Child::kill(self)
    }
    fn wait(&mut self) -> IoResult<ExitStatus> {
        Child::wait(self)
    }
}

impl Child {
    pub fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        let mut status: DWORD = 0;
        let res = unsafe { GetExitCodeProcess(self.proc.handle, &mut status) };
        if res != 0 {
            if status == STILL_ACTIVE {
                Ok(None)
            } else {
                Ok(Some(ExitStatus::with_exit_code(status)))
            }
        } else {
            Ok(None)
        }
    }

    pub fn kill(&mut self) -> IoResult<()> {
        unsafe {
            TerminateProcess(self.proc.handle, 1);
        }
        self.wait()?;
        Ok(())
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
            Ok(ExitStatus::with_exit_code(status))
        } else {
            Err(IoError::last_os_error())
        }
    }
}
