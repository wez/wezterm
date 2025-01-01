//! Keeps track of the number of user-initiated activities
use crate::Mux;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNT: AtomicUsize = AtomicUsize::new(0);

/// Create and hold on to an Activity while you are processing
/// the direct result of a user initiated action, such as preparing
/// to open a window.
/// Once you have opened the window, drop the activity.
/// The activity is used to keep the frontend alive even if there
/// may be no windows present in the mux.
pub struct Activity {}

impl Activity {
    pub fn new() -> Self {
        COUNT.fetch_add(1, Ordering::SeqCst);
        Self {}
    }

    pub fn count() -> usize {
        COUNT.load(Ordering::SeqCst)
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        COUNT.fetch_sub(1, Ordering::SeqCst);

        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get();
            mux.prune_dead_windows();
        })
        .detach();
    }
}
