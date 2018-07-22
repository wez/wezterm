extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::AnsiColor;
use termwiz::surface::{Change, Surface};
use termwiz::terminal::{self, Terminal};

#[cfg(unix)]
fn get_terminal(caps: Capabilities) -> Result<impl Terminal, failure::Error> {
    terminal::UnixTerminal::new(caps)
}

#[cfg(windows)]
fn get_terminal(caps: Capabilities) -> Result<impl Terminal, failure::Error> {
    terminal::WindowsTerminal::new(caps)
}

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut terminal = get_terminal(caps)?;
    terminal.set_raw_mode()?;

    let size = terminal.get_screen_size()?;
    let mut screen = Surface::new(size.cols as usize, size.rows as usize);

    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    screen.add_change("Hello world\r\n");
    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Red.into(),
    )));
    screen.add_change("and in red here\r\n");

    let (_seq, changes) = screen.get_changes(0);
    terminal.render(&changes)?;
    //println!("changes: {:?}", changes);
    println!("size: {:?}", size);

    Ok(())
}
