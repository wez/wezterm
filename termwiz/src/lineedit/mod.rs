//! The `LineEditor` struct provides line editing facilities similar
//! to those in the unix shell.
//!
//! ```no_run
//! use failure::Fallible;
//! use termwiz::lineedit::line_editor;
//!
//! fn main() -> Fallible<()> {
//!     let mut editor = line_editor()?;
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
//! Ctrl-C        | Cancel the line editor
//! Ctrl-F, Right | Move cursor one grapheme to the right
//! Ctrl-H, Backspace | Delete the grapheme to the left of the cursor
//! Ctrl-J, Ctrl-M, Enter | Finish line editing and accept the current line
//! Ctrl-K        | Delete from cursor to end of line
//! Ctrl-L        | Move the cursor to the top left, clear screen and repaint
//! Ctrl-W        | Delete word leading up to cursor
//! Alt-b, Alt-Left | Move the cursor backwards one word
//! Alt-f, Alt-Right | Move the cursor forwards one word
use crate::caps::{Capabilities, ProbeHintsBuilder};
use crate::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use crate::surface::{Change, Position};
use crate::terminal::{new_terminal, Terminal};
use failure::{err_msg, Fallible};
use unicode_segmentation::GraphemeCursor;
use unicode_width::UnicodeWidthStr;

mod actions;
pub use actions::{Action, Movement, RepeatCount};

/// The `LineEditor` struct provides line editing facilities similar
/// to those in the unix shell.
/// ```no_run
/// use failure::Fallible;
/// use termwiz::lineedit::line_editor;
///
/// fn main() -> Fallible<()> {
///     let mut editor = line_editor()?;
///
///     let line = editor.read_line()?;
///     println!("read line: {}", line);
///
///     Ok(())
/// }
/// ```
pub struct LineEditor<T: Terminal> {
    terminal: T,
    line: String,
    /// byte index into the UTF-8 string data of the insertion
    /// point.  This is NOT the number of graphemes!
    cursor: usize,
}

impl<T: Terminal> LineEditor<T> {
    /// Create a new line editor.
    /// In most cases, you'll want to use the `line_editor` function,
    /// because it creates a `Terminal` instance with the recommended
    /// settings, but if you need to decompose that for some reason,
    /// this snippet shows the recommended way to create a line
    /// editor:
    ///
    /// ```no_run
    /// use termwiz::caps::{Capabilities, ProbeHintsBuilder};
    /// use termwiz::terminal::new_terminal;
    /// use failure::err_msg;
    /// // Disable mouse input in the line editor
    /// let hints = ProbeHintsBuilder::new_from_env()
    ///     .mouse_reporting(Some(false))
    ///     .build()
    ///     .map_err(err_msg)?;
    /// let caps = Capabilities::new_with_hints(hints)?;
    /// let terminal = new_terminal(caps)?;
    /// # Ok::<(), failure::Error>(())
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
    /// Returns Ok(None) if the editor was cancelled eg: via CTRL-C.
    pub fn read_line(&mut self) -> Fallible<Option<String>> {
        self.terminal.set_raw_mode()?;
        let res = self.read_line_impl();
        self.terminal.set_cooked_mode()?;
        println!();
        res
    }

    fn resolve_action(&self, event: &InputEvent) -> Option<Action> {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('C'),
                modifiers: Modifiers::CTRL,
            }) => Some(Action::Cancel),
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
            }) => Some(Action::AcceptLine),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('H'),
                modifiers: Modifiers::CTRL,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Backspace,
                modifiers: Modifiers::NONE,
            }) => Some(Action::Kill(Movement::BackwardChar(1))),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('B'),
                modifiers: Modifiers::CTRL,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::LeftArrow,
                modifiers: Modifiers::NONE,
            }) => Some(Action::Move(Movement::BackwardChar(1))),

            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('W'),
                modifiers: Modifiers::CTRL,
            }) => Some(Action::Kill(Movement::BackwardWord(1))),

            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('b'),
                modifiers: Modifiers::ALT,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::LeftArrow,
                modifiers: Modifiers::ALT,
            }) => Some(Action::Move(Movement::BackwardWord(1))),

            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('f'),
                modifiers: Modifiers::ALT,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::RightArrow,
                modifiers: Modifiers::ALT,
            }) => Some(Action::Move(Movement::ForwardWord(1))),

            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('A'),
                modifiers: Modifiers::CTRL,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Home,
                modifiers: Modifiers::NONE,
            }) => Some(Action::Move(Movement::StartOfLine)),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('E'),
                modifiers: Modifiers::CTRL,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::End,
                modifiers: Modifiers::NONE,
            }) => Some(Action::Move(Movement::EndOfLine)),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('F'),
                modifiers: Modifiers::CTRL,
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::RightArrow,
                modifiers: Modifiers::NONE,
            }) => Some(Action::Move(Movement::ForwardChar(1))),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char(c),
                modifiers: Modifiers::NONE,
            }) => Some(Action::InsertChar(1, *c)),
            InputEvent::Paste(text) => Some(Action::InsertText(1, text.clone())),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('L'),
                modifiers: Modifiers::CTRL,
            }) => Some(Action::Repaint),
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('K'),
                modifiers: Modifiers::CTRL,
            }) => Some(Action::Kill(Movement::EndOfLine)),
            _ => None,
        }
    }

    /// Compute the cursor position after applying movement
    fn eval_movement(&self, movement: Movement) -> usize {
        match movement {
            Movement::BackwardChar(rep) => {
                let mut position = self.cursor;
                for _ in 0..rep {
                    let mut cursor = GraphemeCursor::new(position, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.prev_boundary(&self.line, 0) {
                        position = pos;
                    } else {
                        break;
                    }
                }
                position
            }
            Movement::BackwardWord(rep) => {
                let char_indices: Vec<(usize, char)> = self.line.char_indices().collect();
                if char_indices.is_empty() {
                    return self.cursor;
                }
                let mut char_position = char_indices
                    .iter()
                    .position(|(idx, _)| *idx == self.cursor)
                    .unwrap_or(char_indices.len() - 1);

                for _ in 0..rep {
                    if char_position == 0 {
                        break;
                    }

                    let mut found = None;
                    for prev in (0..char_position - 1).rev() {
                        if char_indices[prev].1.is_whitespace() {
                            found = Some(prev + 1);
                            break;
                        }
                    }

                    char_position = found.unwrap_or(0);
                }
                char_indices[char_position].0
            }
            Movement::ForwardWord(rep) => {
                let char_indices: Vec<(usize, char)> = self.line.char_indices().collect();
                if char_indices.is_empty() {
                    return self.cursor;
                }
                let mut char_position = char_indices
                    .iter()
                    .position(|(idx, _)| *idx == self.cursor)
                    .unwrap_or(char_indices.len());

                for _ in 0..rep {
                    // Skip any non-whitespace characters
                    while char_position < char_indices.len()
                        && !char_indices[char_position].1.is_whitespace()
                    {
                        char_position += 1;
                    }

                    // Skip any whitespace characters
                    while char_position < char_indices.len()
                        && char_indices[char_position].1.is_whitespace()
                    {
                        char_position += 1;
                    }

                    // We are now on the start of the next word
                }
                char_indices
                    .get(char_position)
                    .map(|(i, _)| *i)
                    .unwrap_or(self.line.len())
            }
            Movement::ForwardChar(rep) => {
                let mut position = self.cursor;
                for _ in 0..rep {
                    let mut cursor = GraphemeCursor::new(position, self.line.len(), false);
                    if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                        position = pos;
                    } else {
                        break;
                    }
                }
                position
            }
            Movement::StartOfLine => 0,
            Movement::EndOfLine => {
                let mut cursor = GraphemeCursor::new(self.line.len() - 1, self.line.len(), false);
                if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                    pos
                } else {
                    self.cursor
                }
            }
        }
    }

    fn kill_text(&mut self, movement: Movement) {
        let new_cursor = self.eval_movement(movement);

        let (lower, upper) = if new_cursor < self.cursor {
            (new_cursor, self.cursor)
        } else {
            (self.cursor, new_cursor)
        };

        self.line.replace_range(lower..upper, "");

        // Clamp to the line length, otherwise a kill to end of line
        // command will leave the cursor way off beyond the end of
        // the line.
        self.cursor = new_cursor.min(self.line.len());
    }

    fn read_line_impl(&mut self) -> Fallible<Option<String>> {
        self.line.clear();
        self.cursor = 0;

        self.render()?;
        while let Some(event) = self.terminal.poll_input(None)? {
            match self.resolve_action(&event) {
                Some(Action::Cancel) => return Ok(None),
                Some(Action::AcceptLine) => break,
                Some(Action::Kill(movement)) => self.kill_text(movement),
                Some(Action::Move(movement)) => self.cursor = self.eval_movement(movement),
                Some(Action::InsertChar(rep, c)) => {
                    for _ in 0..rep {
                        self.line.insert(self.cursor, c);
                        let mut cursor = GraphemeCursor::new(self.cursor, self.line.len(), false);
                        if let Ok(Some(pos)) = cursor.next_boundary(&self.line, 0) {
                            self.cursor = pos;
                        }
                    }
                }
                Some(Action::InsertText(rep, text)) => {
                    for _ in 0..rep {
                        self.line.insert_str(self.cursor, &text);
                        self.cursor += text.len();
                    }
                }
                Some(Action::Repaint) => {
                    self.terminal
                        .render(&[Change::ClearScreen(Default::default())])?;
                }
                _ => {}
            }
            self.render()?;
        }
        Ok(Some(self.line.clone()))
    }
}

/// Create a `Terminal` with the recommended settings, and use that
/// to create a `LineEditor` instance.
pub fn line_editor() -> Fallible<LineEditor<impl Terminal>> {
    let hints = ProbeHintsBuilder::new_from_env()
        .mouse_reporting(Some(false))
        .build()
        .map_err(err_msg)?;
    let caps = Capabilities::new_with_hints(hints)?;
    let terminal = new_terminal(caps)?;
    Ok(LineEditor::new(terminal))
}
