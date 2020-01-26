use super::*;
use std::sync::Arc;
use termwiz::escape::parser::Parser;

pub trait Clipboard {
    fn get_contents(&self) -> anyhow::Result<String>;
    fn set_contents(&self, data: Option<String>) -> anyhow::Result<()>;
}

impl Clipboard for Box<dyn Clipboard> {
    fn get_contents(&self) -> anyhow::Result<String> {
        self.as_ref().get_contents()
    }

    fn set_contents(&self, data: Option<String>) -> anyhow::Result<()> {
        self.as_ref().set_contents(data)
    }
}

/// Represents the host of the terminal.
/// Provides a means for sending data to the connected pty
pub trait TerminalHost {
    /// Returns an object that can be used to send data to the
    /// slave end of the associated pty.
    fn writer(&mut self) -> &mut dyn std::io::Write;
}

pub struct Terminal {
    /// The terminal model/state
    state: TerminalState,
    /// Baseline terminal escape sequence parser
    parser: Parser,
}

impl Deref for Terminal {
    type Target = TerminalState;

    fn deref(&self) -> &TerminalState {
        &self.state
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }
}

impl Terminal {
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        pixel_width: usize,
        pixel_height: usize,
        config: Arc<dyn TerminalConfiguration>,
    ) -> Terminal {
        Terminal {
            state: TerminalState::new(
                physical_rows,
                physical_cols,
                pixel_height,
                pixel_width,
                config,
            ),
            parser: Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes of input.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B, host: &mut dyn TerminalHost) {
        let bytes = bytes.as_ref();

        let mut performer = Performer::new(&mut self.state, host);

        self.parser.parse(bytes, |action| performer.perform(action));
    }
}
