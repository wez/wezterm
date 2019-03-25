use std::io::{self, Error as IoError};
use std::os::windows::raw::HANDLE;
use std::ptr;
use winapi::um::fileapi::*;
use winapi::um::handleapi::*;
use winapi::um::processthreadsapi::*;

#[derive(Debug)]
pub struct OwnedHandle {
    pub handle: HANDLE,
}
unsafe impl Send for OwnedHandle {}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

impl OwnedHandle {
    pub fn new(handle: HANDLE) -> Self {
        Self { handle }
    }
    pub fn try_clone(&self) -> Result<Self, IoError> {
        if self.handle == INVALID_HANDLE_VALUE || self.handle.is_null() {
            return Ok(OwnedHandle {
                handle: self.handle,
            });
        }

        let proc = unsafe { GetCurrentProcess() };
        let mut duped = INVALID_HANDLE_VALUE;
        let ok = unsafe {
            DuplicateHandle(
                proc,
                self.handle as *mut _,
                proc,
                &mut duped,
                0,
                0,
                winapi::um::winnt::DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(OwnedHandle {
                handle: duped as *mut _,
            })
        }
    }
}

impl io::Read for OwnedHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut num_read = 0;
        let ok = unsafe {
            ReadFile(
                self.handle as *mut _,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut num_read,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(num_read as usize)
        }
    }
}

impl io::Write for OwnedHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let mut num_wrote = 0;
        let ok = unsafe {
            WriteFile(
                self.handle as *mut _,
                buf.as_ptr() as *const _,
                buf.len() as u32,
                &mut num_wrote,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(num_wrote as usize)
        }
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}
