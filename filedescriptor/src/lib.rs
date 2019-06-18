//! The purpose of this crate is to make it a bit more ergonomic for portable
//! applications that need to work with the platform level `RawFd` and
//! `RawHandle` types.
//!
//! Rather than conditionally using `RawFd` and `RawHandle`, the `FileDescriptor`
//! type can be used to manage ownership, duplicate, read and write.
//!
//! ## FileDescriptor
//!
//! This is a bit of a contrived example, but demonstrates how to avoid
//! the conditional code that would otherwise be required to deal with
//! calling `as_raw_fd` and `as_raw_handle`:
//!
//! ```
//! use filedescriptor::{FileDescriptor, FromRawFileDescriptor};
//! use failure::Fallible;
//! use std::io::Write;
//!
//! fn get_stdout() -> Fallible<FileDescriptor> {
//!   let stdout = std::io::stdout();
//!   let handle = stdout.lock();
//!   FileDescriptor::dup(&handle)
//! }
//!
//! fn print_something() -> Fallible<()> {
//!    get_stdout()?.write(b"hello")?;
//!    Ok(())
//! }
//! ```
//!
//! ## Pipe
//! The `Pipe` type makes it more convenient to create a pipe and manage
//! the lifetime of both the read and write ends of that pipe.
//!
//! ```
//! use filedescriptor::Pipe;
//! use std::io::{Read,Write};
//! use failure::Error;
//!
//! let mut pipe = Pipe::new()?;
//! pipe.write.write(b"hello")?;
//! drop(pipe.write);
//!
//! let mut s = String::new();
//! pipe.read.read_to_string(&mut s)?;
//! assert_eq!(s, "hello");
//! # Ok::<(), Error>(())
//! ```
use failure::Fallible;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use crate::windows::*;

/// `AsRawFileDescriptor` is a platform independent trait for returning
/// a non-owning reference to the underlying platform file descriptor
/// type.
pub trait AsRawFileDescriptor {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor;
}

/// `IntoRawFileDescriptor` is a platform independent trait for converting
/// an instance into the underlying platform file descriptor type.
pub trait IntoRawFileDescriptor {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor;
}

/// `FromRawFileDescriptor` is a platform independent trait for creating
/// an instance from the underlying platform file descriptor type.
/// Because the platform file descriptor type has no inherent ownership
/// management, the `from_raw_file_descriptor` function is marked as unsafe
/// to indicate that care must be taken by the caller to ensure that it
/// is used appropriately.
pub trait FromRawFileDescriptor {
    unsafe fn from_raw_file_descriptor(fd: RawFileDescriptor) -> Self;
}

/// `OwnedHandle` allows managing the lifetime of the platform `RawFileDescriptor`
/// type.  It is exposed in the interface of this crate primarily for convenience
/// on Windows where the system handle type is used for a variety of objects
/// that don't support reading and writing.
#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawFileDescriptor,
}

impl OwnedHandle {
    /// Create a new handle from some object that is convertible into
    /// the system `RawFileDescriptor` type.  This consumes the parameter
    /// and replaces it with an `OwnedHandle` instance.
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        Self {
            handle: f.into_raw_file_descriptor(),
        }
    }

    /// Attempt to duplicate the underlying handle and return an
    /// `OwnedHandle` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn try_clone(&self) -> Fallible<Self> {
        Self::dup(self)
    }

    /// Attempt to duplicate the underlying handle from an object that is
    /// representable as the system `RawFileDescriptor` type and return an
    /// `OwnedHandle` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        Self::dup_impl(f)
    }
}

/// `FileDescriptor` is a thin wrapper on top of the `OwnedHandle` type that
/// exposes the ability to Read and Write to the platform `RawFileDescriptor`.
///
/// This is a bit of a contrived example, but demonstrates how to avoid
/// the conditional code that would otherwise be required to deal with
/// calling `as_raw_fd` and `as_raw_handle`:
///
/// ```
/// use filedescriptor::{FileDescriptor, FromRawFileDescriptor};
/// use failure::Fallible;
/// use std::io::Write;
///
/// fn get_stdout() -> Fallible<FileDescriptor> {
///   let stdout = std::io::stdout();
///   let handle = stdout.lock();
///   FileDescriptor::dup(&handle)
/// }
///
/// fn print_something() -> Fallible<()> {
///    get_stdout()?.write(b"hello")?;
///    Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FileDescriptor {
    handle: OwnedHandle,
}

impl FileDescriptor {
    /// Create a new descriptor from some object that is convertible into
    /// the system `RawFileDescriptor` type.  This consumes the parameter
    /// and replaces it with a `FileDescriptor` instance.
    pub fn new<F: IntoRawFileDescriptor>(f: F) -> Self {
        let handle = OwnedHandle::new(f);
        Self { handle }
    }

    /// Attempt to duplicate the underlying handle from an object that is
    /// representable as the system `RawFileDescriptor` type and return a
    /// `FileDescriptor` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn dup<F: AsRawFileDescriptor>(f: &F) -> Fallible<Self> {
        OwnedHandle::dup(f).map(|handle| Self { handle })
    }

    /// Attempt to duplicate the underlying handle and return a
    /// `FileDescriptor` wrapped around the duplicate.  Since the duplication
    /// requires kernel resources that may not be available, this is a
    /// potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub fn try_clone(&self) -> Fallible<Self> {
        self.handle.try_clone().map(|handle| Self { handle })
    }

    /// A convenience method for creating a `std::process::Stdio` object
    /// to be used for eg: redirecting the stdio streams of a child
    /// process.  The `Stdio` is created using a duplicated handle so
    /// that the source handle remains alive.
    pub fn as_stdio(&self) -> Fallible<std::process::Stdio> {
        self.as_stdio_impl()
    }
}

/// Represents the readable and writable ends of a pair of descriptors
/// connected via a kernel pipe.
///
/// ```
/// use filedescriptor::Pipe;
/// use std::io::{Read,Write};
/// use failure::Error;
///
/// let mut pipe = Pipe::new()?;
/// pipe.write.write(b"hello")?;
/// drop(pipe.write);
///
/// let mut s = String::new();
/// pipe.read.read_to_string(&mut s)?;
/// assert_eq!(s, "hello");
/// # Ok::<(), Error>(())
/// ```
pub struct Pipe {
    /// The readable end of the pipe
    pub read: FileDescriptor,
    /// The writable end of the pipe
    pub write: FileDescriptor,
}

use std::time::Duration;

/// Examines a set of FileDescriptors to see if some of them are ready for I/O,
/// or if certain events have occurred on them.
///
/// This uses the system native readiness checking mechanism, which on Windows
/// means that it does NOT use IOCP and that this only works with sockets on
/// Windows.  If you need IOCP then the `mio` crate is recommended for a much
/// more scalable solution.
///
/// On macOS, the `poll(2)` implementation has problems when used with eg: pty
/// descriptors, so this implementation of poll uses the `select(2)` interface
/// under the covers.  That places a limit on the maximum file descriptor value
/// that can be passed to poll.  If a file descriptor is out of range then an
/// error will returned.  This limitation could potentially be lifted in the
/// future.
///
/// If `duration` is `None`, then `poll` will block until any of the requested
/// events are ready.  Otherwise, `duration` specifies how long to wait for
/// readiness before giving up.
///
/// The return value is the number of entries that were satisfied; `0` means
/// that none were ready after waiting for the specified duration.
///
/// The `pfd` array is mutated and the `revents` field is updated to indicate
/// which of the events were received.
pub fn poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Fallible<usize> {
    poll_impl(pfd, duration)
}
