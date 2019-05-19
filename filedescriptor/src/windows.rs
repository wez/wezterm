use crate::{AsRawFileDescriptor, FromRawFileDescriptor, IntoRawFileDescriptor, Pipes};
use failure::{bail, Fallible};
use std::io::{self, Error as IoError};
use std::os::windows::prelude::*;
use std::os::windows::raw::HANDLE;
use std::ptr;
use winapi::um::fileapi::*;
use winapi::um::handleapi::*;
use winapi::um::namedpipeapi::CreatePipe;
use winapi::um::processthreadsapi::*;

pub type RawFileDescriptor = RawHandle;

impl<T: AsRawHandle> AsRawFileDescriptor for T {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor {
        self.as_raw_handle()
    }
}

impl<T: IntoRawHandle> IntoRawFileDescriptor for T {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor {
        self.into_raw_handle()
    }
}

impl<T: FromRawHandle> FromRawFileDescriptor for T {
    unsafe fn from_raw_file_descrptor(handle: RawHandle) -> Self {
        Self::from_raw_handle(handle)
    }
}

#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawHandle,
}

unsafe impl Send for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE && !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

impl FromRawHandle for OwnedHandle {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        OwnedHandle { handle }
    }
}

impl OwnedHandle {
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = f.into_raw_file_descriptor();
        Self { handle }
    }

    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        dup_handle(f.as_raw_file_descriptor())
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

impl IntoRawHandle for OwnedHandle {
    fn into_raw_handle(self) -> HANDLE {
        let handle = self.handle;
        std::mem::forget(self);
        handle
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    handle: OwnedHandle,
}

fn dup_handle(handle: HANDLE) -> Fallible<OwnedHandle> {
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Ok(OwnedHandle { handle });
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

impl FromRawHandle for FileDescriptor {
    unsafe fn from_raw_handle(handle: RawHandle) -> FileDescriptor {
        Self {
            handle: OwnedHandle::from_raw_handle(handle),
        }
    }
}

impl FileDescriptor {
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = OwnedHandle::new(f);
        Self { handle }
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        self.handle
            .try_clone()
            .map(|handle| FileDescriptor { handle })
    }

    pub fn as_stdio(&self) -> Fallible<std::process::Stdio> {
        let duped = self.handle.try_clone()?;
        let handle = duped.into_raw_handle();
        let stdio = unsafe { std::process::Stdio::from_raw_handle(handle) };
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
                handle: OwnedHandle { handle: read },
            },
            write: FileDescriptor {
                handle: OwnedHandle { handle: write },
            },
        })
    }

    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        dup_handle(f.as_raw_file_descriptor()).map(|handle| FileDescriptor { handle })
    }
}

impl IntoRawHandle for FileDescriptor {
    fn into_raw_handle(self) -> HANDLE {
        self.handle.into_raw_handle()
    }
}

impl AsRawHandle for FileDescriptor {
    fn as_raw_handle(&self) -> HANDLE {
        self.handle.as_raw_handle()
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
