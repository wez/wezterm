use super::*;

/// Represents the host of the terminal.
/// Provides a means for sending data to the connected pty,
/// and for operating on the clipboard
pub trait TerminalHost {
    /// Returns an object that can be used to send data to the
    /// slave end of the associated pty.
    fn writer(&mut self) -> &mut std::io::Write;

    /// Returns the current clipboard contents
    fn get_clipboard(&mut self) -> Result<String, Error>;

    /// Adjust the contents of the clipboard
    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error>;

    /// Change the title of the window
    fn set_title(&mut self, title: &str);
}

pub struct Terminal {
    /// The terminal model/state
    state: TerminalState,
    /// Baseline terminal escape sequence parser
    parser: vte::Parser,
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

/// When the terminal parser needs to convey a response
/// back to the caller, this enum holds that response
#[derive(Debug, Clone)]
pub(crate) enum AnswerBack {
    /// Some data to send back to the application on
    /// the slave end of the pty.
    WriteToPty(Vec<u8>),
    /// The application has requested that we change
    /// the terminal title, and here it is.
    TitleChanged(String),
}

impl Terminal {
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Terminal {
        Terminal {
            state: TerminalState::new(physical_rows, physical_cols, scrollback_size),
            parser: vte::Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes of input.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B, host: &mut TerminalHost) {
        let bytes = bytes.as_ref();
        for b in bytes.iter() {
            self.parser.advance(&mut self.state, *b);
        }
        if let Some(answerback) = self.state.drain_answerback() {
            for answer in answerback {
                match answer {
                    AnswerBack::WriteToPty(response) => {
                        host.writer().write(&response).ok(); // discard error
                    }
                    AnswerBack::TitleChanged(title) => {
                        host.set_title(&title);
                    }
                }
            }
        }
    }
}
