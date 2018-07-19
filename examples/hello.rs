extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::AnsiColor;
use termwiz::render::terminfo::TerminfoRenderer;
use termwiz::render::Renderer;
use termwiz::screen::{Change, Screen};
use termwiz::terminal::{Terminal, UnixTerminal};

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;
    let renderer = TerminfoRenderer::new(caps);

    let mut terminal = UnixTerminal::new()?;
    terminal.set_raw_mode()?;

    let size = terminal.get_screen_size()?;
    let mut screen = Screen::new(size.cols as usize, size.rows as usize);

    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    screen.add_change("Hello world\r\n");

    let (_seq, changes) = screen.get_changes(0);
    let _end_attr = renderer.render_to(&CellAttributes::default(), &changes, &mut terminal);
    //println!("changes: {:?}", changes);
    println!("size: {:?}", size);

    Ok(())
}
