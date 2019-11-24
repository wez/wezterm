//! Bridge our gui config into the terminal crate configuration

use crate::config::configuration;

#[derive(Debug)]
pub struct TermConfig;

impl term::TerminalConfiguration for TermConfig {
    fn scrollback_size(&self) -> usize {
        configuration().scrollback_lines
    }
}
