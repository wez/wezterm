extern crate termwiz;
#[macro_use]
extern crate failure;

use failure::Error;
use std::io::stdout;
use termwiz::caps::Capabilities;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::AnsiColor;
use termwiz::render::terminfo::TerminfoRenderer;
use termwiz::render::Renderer;
use termwiz::screen::{Change, Screen};
use termwiz::terminal::{Terminal, UnixTerminal};

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut terminal = UnixTerminal::new()?;
    terminal.set_raw_mode()?;

    let renderer = TerminfoRenderer::new(caps);

    // TODO: obtain the size via termios
    let mut screen = Screen::new(20, 2);

    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    screen.add_change("Hello world\r\n");

    let (seq, changes) = screen.get_changes(0);
    let end_attr = renderer.render_to(&CellAttributes::default(), &changes, &mut terminal);
    //println!("changes: {:?}", changes);
    Ok(())
}
