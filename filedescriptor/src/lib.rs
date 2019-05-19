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
/// management, the `from_raw_file_descrptor` function is marked as unsafe
/// to indicate that care must be taken by the caller to ensure that it
/// is used appropriately.
pub trait FromRawFileDescriptor {
    unsafe fn from_raw_file_descrptor(fd: RawFileDescriptor) -> Self;
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
    /// representable as the systemm `RawFileDescriptor` type and return an
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
    /// representable as the systemm `RawFileDescriptor` type and return a
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
pub struct Pipe {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}
