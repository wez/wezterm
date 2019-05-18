#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use crate::windows::*;

pub struct Pipes {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}
