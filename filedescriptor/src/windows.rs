use crate::{
    AsRawFileDescriptor, FileDescriptor, FromRawFileDescriptor, IntoRawFileDescriptor, OwnedHandle,
    Pipe,
};
use failure::{bail, Fallible};
use std::io::{self, Error as IoError};
use std::os::windows::prelude::*;
use std::ptr;
use winapi::um::fileapi::*;
use winapi::um::handleapi::*;
use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
use winapi::um::namedpipeapi::CreatePipe;
use winapi::um::processthreadsapi::*;
use winapi::um::winnt::HANDLE;

/// `RawFileDescriptor` is a platform independent type alias for the
/// underlying platform file descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
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

unsafe impl Send for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE as _ && !self.handle.is_null() {
            unsafe { CloseHandle(self.handle as _) };
        }
    }
}

impl FromRawHandle for OwnedHandle {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        OwnedHandle { handle }
    }
}

impl OwnedHandle {
    #[inline]
    pub(crate) fn dup_impl<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        let handle = f.as_raw_file_descriptor();
        if handle == INVALID_HANDLE_VALUE as _ || handle.is_null() {
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
                0, // not inheritable
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
}

impl AsRawHandle for OwnedHandle {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

impl IntoRawHandle for OwnedHandle {
    fn into_raw_handle(self) -> RawHandle {
        let handle = self.handle;
        std::mem::forget(self);
        handle
    }
}

impl FileDescriptor {
    #[inline]
    pub(crate) fn as_stdio_impl(&self) -> Fallible<std::process::Stdio> {
        let duped = self.handle.try_clone()?;
        let handle = duped.into_raw_handle();
        let stdio = unsafe { std::process::Stdio::from_raw_handle(handle) };
        Ok(stdio)
    }
}

impl IntoRawHandle for FileDescriptor {
    fn into_raw_handle(self) -> RawHandle {
        self.handle.into_raw_handle()
    }
}

impl AsRawHandle for FileDescriptor {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}

impl FromRawHandle for FileDescriptor {
    unsafe fn from_raw_handle(handle: RawHandle) -> FileDescriptor {
        Self {
            handle: OwnedHandle::from_raw_handle(handle),
        }
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

impl Pipe {
    pub fn new() -> Fallible<Pipe> {
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: 0,
        };
        let mut read: HANDLE = INVALID_HANDLE_VALUE as _;
        let mut write: HANDLE = INVALID_HANDLE_VALUE as _;
        if unsafe { CreatePipe(&mut read, &mut write, &mut sa, 0) } == 0 {
            bail!("CreatePipe failed: {}", IoError::last_os_error());
        }
        Ok(Pipe {
            read: FileDescriptor {
                handle: OwnedHandle { handle: read as _ },
            },
            write: FileDescriptor {
                handle: OwnedHandle { handle: write as _ },
            },
        })
    }
}
