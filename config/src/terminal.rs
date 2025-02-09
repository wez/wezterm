//! Bridge our gui config into the terminal crate configuration

use crate::{configuration, ConfigHandle, NewlineCanon};
use std::sync::Mutex;
use termwiz::cell::UnicodeVersion;
use wezterm_term::color::ColorPalette;
use wezterm_term::config::BidiMode;

#[derive(Debug)]
pub struct TermConfig {
    config: Mutex<Option<ConfigHandle>>,
    client_palette: Mutex<Option<ColorPalette>>,
}

impl TermConfig {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(None),
            client_palette: Mutex::new(None),
        }
    }

    pub fn with_config(config: ConfigHandle) -> Self {
        Self {
            config: Mutex::new(Some(config)),
            client_palette: Mutex::new(None),
        }
    }

    pub fn set_config(&self, config: ConfigHandle) {
        self.config.lock().unwrap().replace(config);
    }

    pub fn set_client_palette(&self, palette: ColorPalette) {
        self.client_palette.lock().unwrap().replace(palette);
    }

    fn configuration(&self) -> ConfigHandle {
        match self.config.lock().unwrap().as_ref() {
            Some(h) => h.clone(),
            None => configuration(),
        }
    }
}

impl wezterm_term::TerminalConfiguration for TermConfig {
    fn generation(&self) -> usize {
        self.configuration().generation()
    }

    fn scrollback_size(&self) -> usize {
        self.configuration().scrollback_lines
    }

    fn enable_csi_u_key_encoding(&self) -> bool {
        self.configuration().enable_csi_u_key_encoding
    }

    fn color_palette(&self) -> ColorPalette {
        let client_palette = self.client_palette.lock().unwrap();
        if let Some(p) = client_palette.as_ref().cloned() {
            return p;
        }
        let config = self.configuration();

        config.resolved_palette.clone().into()
    }

    fn alternate_buffer_wheel_scroll_speed(&self) -> u8 {
        self.configuration().alternate_buffer_wheel_scroll_speed
    }

    fn enq_answerback(&self) -> String {
        configuration().enq_answerback.clone()
    }

    fn enable_kitty_graphics(&self) -> bool {
        self.configuration().enable_kitty_graphics
    }

    fn enable_title_reporting(&self) -> bool {
        self.configuration().enable_title_reporting
    }

    fn enable_kitty_keyboard(&self) -> bool {
        self.configuration().enable_kitty_keyboard
    }

    fn enable_osc52_clipboard_reading(&self) -> bool {
        self.configuration().enable_osc52_clipboard_reading
    }

    fn canonicalize_pasted_newlines(&self) -> wezterm_term::config::NewlineCanon {
        match self.configuration().canonicalize_pasted_newlines {
            None => wezterm_term::config::NewlineCanon::default(),
            Some(NewlineCanon::None) => wezterm_term::config::NewlineCanon::None,
            Some(NewlineCanon::LineFeed) => wezterm_term::config::NewlineCanon::LineFeed,
            Some(NewlineCanon::CarriageReturn) => {
                wezterm_term::config::NewlineCanon::CarriageReturn
            }
            Some(NewlineCanon::CarriageReturnAndLineFeed) => {
                wezterm_term::config::NewlineCanon::CarriageReturnAndLineFeed
            }
        }
    }

    fn unicode_version(&self) -> UnicodeVersion {
        let config = self.configuration();
        UnicodeVersion {
            version: config.unicode_version,
            ambiguous_are_wide: config.treat_east_asian_ambiguous_width_as_wide,
        }
    }

    fn debug_key_events(&self) -> bool {
        self.configuration().debug_key_events
    }

    fn log_unknown_escape_sequences(&self) -> bool {
        self.configuration().log_unknown_escape_sequences
    }

    fn normalize_output_to_unicode_nfc(&self) -> bool {
        self.configuration().normalize_output_to_unicode_nfc
    }

    fn bidi_mode(&self) -> BidiMode {
        let config = self.configuration();
        BidiMode {
            enabled: config.bidi_enabled,
            hint: config.bidi_direction,
        }
    }
}
