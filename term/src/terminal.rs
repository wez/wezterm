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

/// Represents an instance of a terminal emulator.
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
    /// Construct a new Terminal.
    /// `physical_rows` and `physical_cols` describe the dimensions
    /// of the visible portion of the terminal display in terms of
    /// the number of text cells.
    ///
    /// `pixel_width` and `pixel_height` describe the dimensions of
    /// that same visible area but in pixels.
    ///
    /// `term_program` and `term_version` are required to identify
    /// the host terminal program; they are used to respond to the
    /// terminal identification sequence `\033[>q`.
    ///
    /// `writer` is anything that implements `std::io::Write`; it
    /// is used to send input to the connected program; both keyboard
    /// and mouse input is encoded and written to that stream, as
    /// are answerback responses to a number of escape sequences.
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        pixel_width: usize,
        pixel_height: usize,
        config: Arc<dyn TerminalConfiguration>,
        term_program: &str,
        term_version: &str,
        // writing to the writer sends data to input of the pty
        writer: Box<dyn std::io::Write>,
    ) -> Terminal {
        Terminal {
            state: TerminalState::new(
                physical_rows,
                physical_cols,
                pixel_height,
                pixel_width,
                config,
                term_program,
                term_version,
                writer,
            ),
            parser: Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes from the output
    /// of the associated program.
    /// The slice is not required to be a complete sequence of escape
    /// characters; it is valid to feed in chunks of data as they arrive.
    /// The output is parsed and applied to the terminal model.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) {
        let bytes = bytes.as_ref();

        let mut performer = Performer::new(&mut self.state);

        self.parser.parse(bytes, |action| performer.perform(action));
    }
}
