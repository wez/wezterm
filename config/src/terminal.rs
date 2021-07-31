//! Bridge our gui config into the terminal crate configuration

use crate::{configuration, ConfigHandle};
use std::sync::Mutex;
use termwiz::hyperlink::Rule as HyperlinkRule;
use wezterm_term::color::ColorPalette;

#[derive(Debug)]
pub struct TermConfig {
    config: Mutex<Option<ConfigHandle>>,
}

impl TermConfig {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(None),
        }
    }

    pub fn with_config(config: ConfigHandle) -> Self {
        Self {
            config: Mutex::new(Some(config)),
        }
    }

    pub fn set_config(&self, config: ConfigHandle) {
        self.config.lock().unwrap().replace(config);
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

    fn hyperlink_rules(&self) -> (usize, Vec<HyperlinkRule>) {
        let config = self.configuration();
        (config.generation(), config.hyperlink_rules.clone())
    }

    fn enable_csi_u_key_encoding(&self) -> bool {
        self.configuration().enable_csi_u_key_encoding
    }

    fn color_palette(&self) -> ColorPalette {
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
}
