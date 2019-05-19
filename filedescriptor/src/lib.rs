use failure::Fallible;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use crate::windows::*;

pub trait AsRawFileDescriptor {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor;
}

pub trait IntoRawFileDescriptor {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor;
}

pub trait FromRawFileDescriptor {
    unsafe fn from_raw_file_descrptor(fd: RawFileDescriptor) -> Self;
}

#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawFileDescriptor,
}

impl OwnedHandle {
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        Self {
            handle: f.into_raw_file_descriptor(),
        }
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        Self::dup(self)
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    handle: OwnedHandle,
}

pub struct Pipes {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}

impl FileDescriptor {
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = OwnedHandle::new(f);
        Self { handle }
    }

    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        OwnedHandle::dup(f).map(|handle| Self { handle })
    }

    pub fn try_clone(&self) -> Fallible<Self> {
        self.handle.try_clone().map(|handle| Self { handle })
    }
}
