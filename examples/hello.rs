extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::AnsiColor;
use termwiz::surface::{Change, Position, Surface};
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::{new_terminal, Terminal};

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut terminal = new_terminal(caps)?;
    terminal.set_raw_mode()?;

    let mut buf = BufferedTerminal::new(terminal)?;

    let mut block = Surface::new(5, 5);
    block.add_change(Change::ClearScreen(AnsiColor::Blue.into()));
    buf.draw_from_screen(&block, 10, 10);

    buf.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    buf.add_change("Hello world\r\n");
    buf.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Red.into(),
    )));
    buf.add_change("and in red here\r\n");
    buf.add_change(Change::CursorPosition {
        x: Position::Absolute(0),
        y: Position::Absolute(20),
    });

    buf.flush()?;

    Ok(())
}
