use crate::Pipes;
use failure::{bail, Fallible};
use std::io::{self, Error as IoError};
use std::os::windows::prelude::*;
use std::os::windows::raw::HANDLE;
use std::ptr;
use winapi::um::fileapi::*;
use winapi::um::handleapi::*;
use winapi::um::namedpipeapi::CreatePipe;
use winapi::um::processthreadsapi::*;

pub trait AsRawFileDescriptor: AsRawHandle {}

impl<T: AsRawHandle> AsRawFileDescriptor for T {}

#[derive(Debug)]
pub struct OwnedHandle {
    handle: HANDLE,
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

    pub fn try_clone(&self) -> Fallible<Self> {
        dup_handle(self.handle)
    }
}

impl AsRawHandle for OwnedHandle {
    fn as_raw_handle(&self) -> HANDLE {
        self.handle
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    handle: OwnedHandle,
}

fn dup_handle(handle: HANDLE) -> Fallible<OwnedHandle> {
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Ok(OwnedHandle::new(handle));
    }

    let proc = unsafe { GetCurrentProcess() };
    let mut duped = INVALID_HANDLE_VALUE;
    let ok = unsafe {
        DuplicateHandle(
            proc,
            handle as *mut _,
            proc,
            &mut duped,
            0,
            0,
            winapi::um::winnt::DUPLICATE_SAME_ACCESS,
        )
    };
    if ok == 0 {
        Err(IoError::last_os_error().into())
    } else {
        Ok(OwnedHandle {
            handle: duped as *mut _,
        })
    }
}

pub fn dup<F: AsRawFileDescriptor>(f: F) -> Fallible<FileDescriptor> {
    dup_handle(f.as_raw_handle()).map(|handle| FileDescriptor { handle })
}

impl FileDescriptor {
    pub fn new(handle: HANDLE) -> Self {
        Self {
            handle: OwnedHandle::new(handle),
        }
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        self.handle
            .try_clone()
            .map(|handle| FileDescriptor { handle })
    }
    pub fn as_stdio(&self) -> Fallible<std::process::Stdio> {
        let duped = self.handle.try_clone()?;
        let handle = duped.handle;
        let stdio = unsafe { std::process::Stdio::from_raw_handle(handle) };
        std::mem::forget(duped); // don't drop; stdio now owns it
        Ok(stdio)
    }

    pub fn pipe() -> Fallible<Pipes> {
        let mut read: HANDLE = INVALID_HANDLE_VALUE;
        let mut write: HANDLE = INVALID_HANDLE_VALUE;
        if unsafe { CreatePipe(&mut read, &mut write, ptr::null_mut(), 0) } == 0 {
            bail!("CreatePipe failed: {}", IoError::last_os_error());
        }
        Ok(Pipes {
            read: FileDescriptor {
                handle: OwnedHandle::new(read),
            },
            write: FileDescriptor {
                handle: OwnedHandle::new(write),
            },
        })
    }
    pub fn dup<F: AsRawFileDescriptor>(f: F) -> Fallible<Self> {
        dup(f)
    }
}

impl io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut num_read = 0;
        let ok = unsafe {
            ReadFile(
                self.handle.as_raw_handle() as *mut _,
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

impl io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        let mut num_wrote = 0;
        let ok = unsafe {
            WriteFile(
                self.handle.as_raw_handle() as *mut _,
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
