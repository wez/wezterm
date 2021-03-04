use ::window::configuration::WindowConfiguration;
use config::configuration;

pub struct ConfigBridge;

impl WindowConfiguration for ConfigBridge {
    fn use_ime(&self) -> bool {
        configuration().use_ime
    }

    fn use_dead_keys(&self) -> bool {
        configuration().use_dead_keys
    }

    fn send_composed_key_when_left_alt_is_pressed(&self) -> bool {
        configuration().send_composed_key_when_left_alt_is_pressed
    }

    fn send_composed_key_when_right_alt_is_pressed(&self) -> bool {
        configuration().send_composed_key_when_right_alt_is_pressed
    }

    fn enable_wayland(&self) -> bool {
        configuration().enable_wayland
    }

    fn prefer_egl(&self) -> bool {
        configuration().prefer_egl
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
        configuration().front_end == config::FrontEndSelection::Software
    }

    fn native_macos_fullscreen_mode(&self) -> bool {
        configuration().native_macos_fullscreen_mode
    }

    fn window_background_opacity(&self) -> f32 {
        configuration().window_background_opacity
    }

    fn decorations(&self) -> ::window::WindowDecorations {
        use ::config::WindowDecorations as CWD;
        use ::window::WindowDecorations as WD;
        match configuration().window_decorations {
            CWD::Full => WD::Full,
            CWD::None => WD::None,
        }
    }
}
