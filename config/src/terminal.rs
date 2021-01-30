//! Bridge our gui config into the terminal crate configuration

use crate::configuration;
use termwiz::hyperlink::Rule as HyperlinkRule;
use wezterm_term::color::ColorPalette;

#[derive(Debug)]
pub struct TermConfig;

impl wezterm_term::TerminalConfiguration for TermConfig {
    fn generation(&self) -> usize {
        configuration().generation()
    }

    fn scrollback_size(&self) -> usize {
        configuration().scrollback_lines
    }

    fn hyperlink_rules(&self) -> (usize, Vec<HyperlinkRule>) {
        let config = configuration();
        (config.generation(), config.hyperlink_rules.clone())
    }

    fn enable_csi_u_key_encoding(&self) -> bool {
        configuration().enable_csi_u_key_encoding
    }

    fn color_palette(&self) -> ColorPalette {
        let config = configuration();

        config.resolved_palette.clone().into()
    }

    fn alternate_buffer_wheel_scroll_speed(&self) -> u8 {
        configuration().alternate_buffer_wheel_scroll_speed
    }
}
