use crate::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use crate::surface::{Change, Position};
use crate::terminal::Terminal;
use failure::Fallible;

pub struct LineEditor<T: Terminal> {
    terminal: T,
    line: String,
}

impl<T: Terminal> LineEditor<T> {
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            line: String::new(),
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
        self.render()?;
        while let Some(event) = self.terminal.poll_input(None)? {
            match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Enter,
                    modifiers: Modifiers::NONE,
                }) => {
                    break;
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    modifiers: Modifiers::NONE,
                }) => {
                    self.line.push(c);
                }
                _ => {}
            }
            self.render()?;
        }
        Ok(self.line.clone())
    }
}
