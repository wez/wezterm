use std::sync::{Arc, Mutex};

pub trait WindowConfiguration {
    fn use_ime(&self) -> bool {
        false
    }

    fn use_dead_keys(&self) -> bool {
        true
    }

    fn enable_wayland(&self) -> bool {
        true
    }

    fn prefer_egl(&self) -> bool {
        true
    }

    fn native_macos_fullscreen_mode(&self) -> bool {
        false
    }
}

lazy_static::lazy_static! {
    static ref CONFIG: Mutex<Arc<dyn WindowConfiguration + Send + Sync>> = default_config();
}

pub(crate) fn config() -> Arc<dyn WindowConfiguration + Send + Sync> {
    Arc::clone(&CONFIG.lock().unwrap())
}

fn default_config() -> Mutex<Arc<dyn WindowConfiguration + Send + Sync>> {
    struct DefConfig;
    impl WindowConfiguration for DefConfig {}
    Mutex::new(Arc::new(DefConfig))
}

pub fn set_configuration<C: WindowConfiguration + Send + Sync + 'static>(c: C) {
    let mut global_config = CONFIG.lock().unwrap();
    *global_config = Arc::new(c);
}
