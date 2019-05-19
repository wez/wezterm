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

pub struct Pipes {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}
