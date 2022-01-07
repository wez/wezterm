use std::path::{Path, PathBuf};

/// There's a lot more code in this windows module than I thought I would need
/// to write.  Ostensibly, we could get away with making a symlink by taking
/// the SessionName environment variable, combining it with the class name
/// and using a symlink to point to the actual path.
/// Symlinks are problematic on Windows, and the SessionName environment
/// variable may not be set.
/// It's a bit of a chore to resolve the name, and then it would be more
/// of a chore to manage the symlink.
/// What this module does is logically equivalent to the above, except
/// that it creates a piece of shared memory in the per-desktop namespace.
/// While there is a lot of code in here, it is simpler overall because
/// the naming is managed by the OS, as well as automatically removing
/// the name from the namespace when there are no more references to it.
#[cfg(windows)]
mod windows {
    use anyhow::Context;
    use super::*;
    use std::io::Error as IoError;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::memoryapi::{
        CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    };
    use winapi::um::synchapi::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
    use winapi::um::winbase::{INFINITE, WAIT_OBJECT_0};
    use winapi::um::winnt::{HANDLE, PAGE_READWRITE};

    const MAX_NAME: usize = 1024;

    /// Keeps the published name alive for the duration of the process.
    pub struct NameHolder {
        _mapping: FileMapping,
        _view: MappedView,
    }

    /// A Windows file mapping
    struct FileMapping {
        name: String,
        handle: HANDLE,
        size: usize,
    }

    impl Drop for FileMapping {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.handle) };
        }
    }

    impl FileMapping {
        /// Create a new or open an existing mapping with the specified name/size
        pub fn create(name: &str, size: usize) -> anyhow::Result<Self> {
            let wide_name = wide_string(&name);

            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    std::ptr::null_mut(),
                    PAGE_READWRITE,
                    0,
                    size as _,
                    wide_name.as_ptr(),
                )
            };
            if handle.is_null() {
                return Err(IoError::last_os_error())
                    .with_context(|| format!("creating shared memory with name {}", name));
            }
            Ok(Self {
                name: name.to_string(),
                handle,
                size,
            })
        }

        /// Attempt to open an existing mapping
        pub fn open(name: &str, size: usize) -> anyhow::Result<Self> {
            let wide_name = wide_string(&name);

            let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide_name.as_ptr()) };
            if handle.is_null() {
                return Err(IoError::last_os_error())
                    .with_context(|| format!("creating shared memory with name {}", name));
            }
            Ok(Self {
                name: name.to_string(),
                handle,
                size,
            })
        }

        /// Map the mapping into the process address space
        pub fn map(&self) -> anyhow::Result<MappedView> {
            let buf =
                unsafe { MapViewOfFile(self.handle, FILE_MAP_ALL_ACCESS, 0, 0, self.size as _) };
            if buf.is_null() {
                return Err(IoError::last_os_error()).with_context(|| {
                    format!("mapping view of shared memory with name {}", self.name)
                });
            }
            Ok(MappedView {
                buf: buf as _,
                size: self.size,
            })
        }
    }

    /// A mutex that can be used to coordinate between processes
    struct NamedMutex {
        handle: HANDLE,
    }
    impl Drop for NamedMutex {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    impl NamedMutex {
        /// Create a mutex with the specified name
        pub fn new(name: &str) -> anyhow::Result<Self> {
            let wide_name = wide_string(name);
            let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, wide_name.as_ptr()) };
            if handle.is_null() {
                return Err(IoError::last_os_error())
                    .with_context(|| format!("creating mutex name {}", name));
            }
            Ok(Self { handle })
        }

        /// Acquire the mutex, and perform `func` while the mutex is held.
        /// Once `func` returns, the mutex is released.
        /// Returns the result of `func`.
        pub fn with_lock<F, T>(&self, func: F) -> anyhow::Result<T>
        where
            F: FnOnce() -> anyhow::Result<T>,
        {
            let res = unsafe { WaitForSingleObject(self.handle, INFINITE) };
            if res != WAIT_OBJECT_0 {
                return Err(IoError::last_os_error()).context("acquire mutex");
            }

            let res = func();
            unsafe { ReleaseMutex(self.handle) };
            res
        }
    }

    /// A materialized view of a mapping
    struct MappedView {
        buf: *mut u8,
        size: usize,
    }

    impl Drop for MappedView {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(self.buf as _);
            }
        }
    }

    impl MappedView {
        fn slice_mut(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.buf, self.size) }
        }

        fn slice(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.buf, self.size) }
        }
    }

    impl NameHolder {
        /// Computes the names of the objects; they use Local scoped
        /// names so that we have one per desktop, rather than one
        /// system wide.
        fn compute_names(class_name: &str) -> (String, String) {
            let mutex_name = format!("Local\\wezterm-sock-mutex-{}", class_name);
            let map_name = format!("Local\\wezterm-sock-{}", class_name);
            (mutex_name, map_name)
        }

        /// Publish path as the path for class_name.
        pub fn new(path: &Path, class_name: &str) -> anyhow::Result<Self> {
            let (mutex_name, map_name) = Self::compute_names(class_name);
            let path = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("path has no file_name!?"))?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("path is not UTF8!"))?
                .to_string();

            let mutex = NamedMutex::new(&mutex_name)?;
            mutex.with_lock(|| {
                let mapping = FileMapping::create(&map_name, MAX_NAME)?;
                let mut view = mapping.map()?;

                let target_slice = view.slice_mut();
                let len = path.len();

                target_slice[0..len].copy_from_slice(path.as_bytes());
                target_slice[len] = 0;

                log::info!("published gui path as {}", path);

                Ok(Self {
                    _mapping: mapping,
                    _view: view,
                })
            })
        }

        /// Resolve the existing path for class_name
        pub fn resolve(class_name: &str) -> anyhow::Result<PathBuf> {
            let (mutex_name, map_name) = Self::compute_names(class_name);
            let mutex = NamedMutex::new(&mutex_name)?;
            mutex.with_lock(|| {
                let mapping = FileMapping::open(&map_name, MAX_NAME)?;
                let view = mapping.map()?;

                let source_slice = view.slice();
                let len = source_slice
                    .iter()
                    .position(|&c| c == 0)
                    .ok_or_else(|| anyhow::anyhow!("shared memory is not NUL terminated!"))?;

                let path = std::str::from_utf8(&source_slice[0..len])
                    .context("reading path from shared memory")?;

                let path: PathBuf = path.into();

                Ok(path)
            })
        }
    }

    /// Convert a rust string to a windows wide string
    fn wide_string(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[allow(dead_code)]
mod fallback {
    use super::*;
    pub struct NameHolder {}
    impl NameHolder {
        pub fn new(_path: &Path, _class_name: &str) -> anyhow::Result<Self> {
            anyhow::bail!("Sock path publishing not implemented on this system");
        }

        pub fn resolve(_class_name: &str) -> anyhow::Result<PathBuf> {
            anyhow::bail!("Sock path publishing not implemented on this system");
        }
    }
}

#[cfg(windows)]
pub use self::windows::NameHolder;

#[cfg(not(windows))]
pub use self::fallback::NameHolder;

/// Unconditionally update the published path to match the provided path,
/// even if there is a running instance with a legitimate published path.
pub fn publish_gui_sock_path(path: &Path, class_name: &str) -> anyhow::Result<NameHolder> {
    NameHolder::new(path, class_name)
}

/// Resolve the last published path for `class_name`.
/// If successful, there is NO guarantee that the returned path references
/// a running instance; it is just the last published path.
pub fn resolve_gui_sock_path(class_name: &str) -> anyhow::Result<PathBuf> {
    NameHolder::resolve(class_name)
}
