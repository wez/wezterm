use ::window::configuration::WindowConfiguration;
use config::{configuration, ConfigHandle};
use std::sync::Arc;

/// An instance that always returns the global configuration values
pub struct ConfigBridge;

/// Returns the config from the given config handle
pub struct ConfigInstance(pub ConfigHandle);

impl ConfigInstance {
    pub fn new(h: ConfigHandle) -> Arc<dyn WindowConfiguration + Send + Sync> {
        let s = Self(h);
        Arc::new(s)
    }
}

impl WindowConfiguration for ConfigInstance {
    fn use_ime(&self) -> bool {
        self.0.use_ime
    }

    fn use_dead_keys(&self) -> bool {
        self.0.use_dead_keys
    }

    fn send_composed_key_when_left_alt_is_pressed(&self) -> bool {
        self.0.send_composed_key_when_left_alt_is_pressed
    }

    fn send_composed_key_when_right_alt_is_pressed(&self) -> bool {
        self.0.send_composed_key_when_right_alt_is_pressed
    }

    fn treat_left_ctrlalt_as_altgr(&self) -> bool {
        self.0.treat_left_ctrlalt_as_altgr
    }

    fn enable_wayland(&self) -> bool {
        self.0.enable_wayland
    }

    fn prefer_egl(&self) -> bool {
        self.0.prefer_egl
    }

    fn prefer_swrast(&self) -> bool {
        #[cfg(windows)]
        {
            if crate::os::windows::is_running_in_rdp_session() {
                // Using OpenGL in RDP has problematic behavior upon
                // disconnect, so we force the use of software rendering.
                log::trace!("Running in an RDP session, use SWRAST");
                return true;
            }
        }
        self.0.front_end == config::FrontEndSelection::Software
    }

    fn native_macos_fullscreen_mode(&self) -> bool {
        self.0.native_macos_fullscreen_mode
    }

    fn window_background_opacity(&self) -> f32 {
        self.0.window_background_opacity
    }

    fn decorations(&self) -> ::window::WindowDecorations {
        self.0.window_decorations
    }
}

fn global() -> ConfigInstance {
    ConfigInstance(configuration())
}

impl WindowConfiguration for ConfigBridge {
    fn use_ime(&self) -> bool {
        global().use_ime()
    }

    fn use_dead_keys(&self) -> bool {
        global().use_dead_keys()
    }

    fn send_composed_key_when_left_alt_is_pressed(&self) -> bool {
        global().send_composed_key_when_left_alt_is_pressed()
    }

    fn send_composed_key_when_right_alt_is_pressed(&self) -> bool {
        global().send_composed_key_when_right_alt_is_pressed()
    }

    fn treat_left_ctrlalt_as_altgr(&self) -> bool {
        global().treat_left_ctrlalt_as_altgr()
    }

    fn enable_wayland(&self) -> bool {
        global().enable_wayland()
    }

    fn prefer_egl(&self) -> bool {
        global().prefer_egl()
    }

    fn prefer_swrast(&self) -> bool {
        global().prefer_swrast()
    }

    fn native_macos_fullscreen_mode(&self) -> bool {
        global().native_macos_fullscreen_mode()
    }

    fn window_background_opacity(&self) -> f32 {
        global().window_background_opacity()
    }

    fn decorations(&self) -> ::window::WindowDecorations {
        global().decorations()
    }
}
