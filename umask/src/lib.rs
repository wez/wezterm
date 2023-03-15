#[cfg(unix)]
use libc::{mode_t, umask};
#[cfg(unix)]
use std::sync::Mutex;

#[cfg(unix)]
lazy_static::lazy_static! {
static ref SAVED_UMASK: Mutex<Option<libc::mode_t>> = Mutex::new(None);
}

/// Unfortunately, novice unix users can sometimes be running
/// with an overly permissive umask so we take care to install
/// a more restrictive mask while we might be creating things
/// in the filesystem.
/// This struct locks down the umask for its lifetime, restoring
/// the prior umask when it is dropped.
pub struct UmaskSaver {
    #[cfg(unix)]
    mask: mode_t,
}

impl UmaskSaver {
    pub fn new() -> Self {
        let me = Self {
            #[cfg(unix)]
            mask: unsafe { umask(0o077) },
        };

        #[cfg(unix)]
        {
            SAVED_UMASK.lock().unwrap().replace(me.mask);
        }

        me
    }

    /// Retrieves the mask saved by a UmaskSaver, without
    /// having a reference to the UmaskSaver.
    /// This is only meaningful if a single UmaskSaver is
    /// used in a program.
    #[cfg(unix)]
    pub fn saved_umask() -> Option<mode_t> {
        *SAVED_UMASK.lock().unwrap()
    }
}

impl Drop for UmaskSaver {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            umask(self.mask);
            SAVED_UMASK.lock().unwrap().take();
        }
    }
}
