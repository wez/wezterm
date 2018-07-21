extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::AnsiColor;
#[cfg(unix)]
use termwiz::render::terminfo::TerminfoRenderer;
#[cfg(windows)]
use termwiz::render::windows::WindowsConsoleRenderer;
use termwiz::render::Renderer;
use termwiz::screen::{Change, Screen};
use termwiz::terminal::{self, Terminal};

#[cfg(unix)]
fn get_terminal() -> Result<impl Terminal, failure::Error> {
    terminal::UnixTerminal::new()
}

#[cfg(unix)]
fn get_renderer(caps: Capabilities) -> impl Renderer {
    TerminfoRenderer::new(caps)
}

#[cfg(windows)]
fn get_terminal() -> Result<impl Terminal, failure::Error> {
    terminal::WindowsTerminal::new()
}

#[cfg(windows)]
fn get_renderer(_caps: Capabilities) -> impl Renderer {
    WindowsConsoleRenderer::new()
}

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;
    let mut renderer = get_renderer(caps);

    let mut terminal = get_terminal()?;
    terminal.set_raw_mode()?;

    let size = terminal.get_screen_size()?;
    let mut screen = Screen::new(size.cols as usize, size.rows as usize);

    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    screen.add_change("Hello world\r\n");
    screen.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Red.into(),
    )));
    screen.add_change("and in red here\r\n");

    let (_seq, changes) = screen.get_changes(0);
    let _end_attr = renderer.render_to(&CellAttributes::default(), &changes, &mut terminal);
    //println!("changes: {:?}", changes);
    println!("size: {:?}", size);

    Ok(())
}
