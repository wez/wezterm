//! The `LineEditor` struct provides line editing facilities similar
//! to those in the unix shell.
//! It is recommended that a direct `Terminal` instance be used to
//! construct the `LineEditor` (rather than a `BufferedTerminal`),
//! and to disable mouse input:
//!
//! ```
//! use failure::{err_msg, Fallible};
//! use termwiz::caps::{Capabilities, ProbeHintsBuilder};
//! use termwiz::lineedit::LineEditor;
//! use termwiz::terminal::new_terminal;
//!
//! fn main() -> Fallible<()> {
//!     // Disable mouse input in the line editor
//!     let hints = ProbeHintsBuilder::new_from_env()
//!         .mouse_reporting(Some(false))
//!         .build()
//!         .map_err(err_msg)?;
//!     let caps = Capabilities::new_with_hints(hints)?;
//!     let terminal = new_terminal(caps)?;
//!     let mut editor = LineEditor::new(terminal);
//!
//!     let line = editor.read_line()?;
//!     println!("read line: {}", line);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Key Bindings
//!
//! The following key bindings are supported:
//!
//! Keystroke     | Action
//! ---------     | ------
//! Ctrl-A, Home  | Move cursor to the beginning of the line
//! Ctrl-E, End   | Move cursor to the end of the line
//! Ctrl-B, Left  | Move cursor one grapheme to the left
//! Ctrl-F, Right | Move cursor one grapheme to the right
//! Ctrl-H, Backspace | Delete the grapheme to the left of the cursor
//! Ctrl-J, Ctrl-M, Enter | Finish line editing and accept the current line
use crate::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use crate::surface::{Change, Position};
use crate::terminal::Terminal;
use failure::Fallible;
use unicode_segmentation::GraphemeCursor;
use unicode_width::UnicodeWidthStr;

pub struct LineEditor<T: Terminal> {
    terminal: T,
    line: String,
    /// byte index into the UTF-8 string data of the insertion
    /// point.  This is NOT the number of graphemes!
    cursor: usize,
}

impl<T: Terminal> LineEditor<T> {
    /// Create a new line editor.
    /// It is recommended that the terminal be created this way:
    /// ```
    /// // Disable mouse input in the line editor
    /// let hints = ProbeHintsBuilder::new_from_env()
    ///     .mouse_reporting(Some(false))
    ///     .build()
    ///     .map_err(err_msg)?;
    /// let caps = Capabilities::new_with_hints(hints)?;
    /// let terminal = new_terminal(caps)?;
    /// ```
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            line: String::new(),
            cursor: 0,
        }
    }

    fn render(&mut self) -> Fallible<()> {
        // In order to position the terminal cursor at the right spot,
        // we need to compute how many graphemes away from the start of
        // the line the current insertion point is.  We can do this by
        // slicing into the string and requesting its unicode width.
        // It might feel more right to count the number of graphemes in
        // the string, but this doesn't render correctly for glyphs that
        // are double-width.  Nothing about unicode is easy :-/
        let grapheme_count = UnicodeWidthStr::width(&self.line[0..self.cursor]);
        self.terminal.render(&[
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::NoChange,
            },
            Change::ClearToEndOfScreen(Default::default()),
            Change::Text(self.line.clone()),
            Change::CursorPosition {
                x: Position::Absolute(grapheme_count),
                y: Position::NoChange,
            },
        ])?;
        Ok(())
    }

    /// Enter line editing mode.
    /// Control is not returned to the caller until a line has been
    /// accepted, or until an error is detected.
    pub fn read_line(&mut self) -> Fallible<String> {
        self.terminal.set_raw_mode()?;
        let res = self.read_line_impl();
        self.terminal.set_cooked_mode()?;
        println!();
        res
    }

    fn read_line_impl(&mut self) -> Fallible<String> {
        self.line.clear();
        self.cursor = 0;

        self.render()?;
        while let Some(event) = self.terminal.poll_input(None)? {
            match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('J'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('M'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Enter,
                    modifiers: Modifiers::NONE,
                }) => {
                    break;
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('H'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Backspace,
                    modifiers: Modifiers::NONE,
                }) => {
                    let mut cursor = GraphemeCursor::new(self.cursor, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.prev_boundary(&self.line, 0) {
                        self.line.remove(pos);
                        self.cursor = pos;
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('B'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::LeftArrow,
                    modifiers: Modifiers::NONE,
                }) => {
                    let mut cursor = GraphemeCursor::new(self.cursor, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.prev_boundary(&self.line, 0) {
                        self.cursor = pos;
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('A'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Home,
                    modifiers: Modifiers::NONE,
                }) => {
                    self.cursor = 0;
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('E'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::End,
                    modifiers: Modifiers::NONE,
                }) => {
                    let mut cursor =
                        GraphemeCursor::new(self.line.len() - 1, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                        self.cursor = pos;
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('F'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::RightArrow,
                    modifiers: Modifiers::NONE,
                }) => {
                    let mut cursor = GraphemeCursor::new(self.cursor, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                        self.cursor = pos;
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    modifiers: Modifiers::NONE,
                }) => {
                    self.line.insert(self.cursor, c);
                    let mut cursor = GraphemeCursor::new(self.cursor, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                        self.cursor = pos;
                    }
                }
                InputEvent::Paste(text) => {
                    self.line.insert_str(self.cursor, &text);
                    self.cursor += text.len();
                }
                _ => {}
            }
            self.render()?;
        }
        Ok(self.line.clone())
    }
}
