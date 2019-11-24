//! Bridge our gui config into the terminal crate configuration

use crate::config::configuration;
use termwiz::hyperlink::Rule as HyperlinkRule;

#[derive(Debug)]
pub struct TermConfig;

impl term::TerminalConfiguration for TermConfig {
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
}
