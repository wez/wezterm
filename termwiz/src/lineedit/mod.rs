use crate::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use crate::surface::{Change, Position};
use crate::terminal::Terminal;
use failure::Fallible;
use unicode_segmentation::GraphemeCursor;

pub struct LineEditor<T: Terminal> {
    terminal: T,
    line: String,
    cursor: usize,
}

impl<T: Terminal> LineEditor<T> {
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            line: String::new(),
            cursor: 0,
        }
    }

    fn render(&mut self) -> Fallible<()> {
        self.terminal.render(&[
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::NoChange,
            },
            Change::ClearToEndOfScreen(Default::default()),
            Change::Text(self.line.clone()),
            Change::CursorPosition {
                x: Position::Absolute(self.cursor),
                y: Position::NoChange,
            },
        ])?;
        Ok(())
    }

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
