//! This example shows how to use `BufferedTerminal` to queue
//! up changes and then flush them.  `BufferedTerminal` enables
//! optimizing the output sequence to update the screen, which is
//! important on links with poor connectivity.
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::AnsiColor;
use termwiz::surface::Change;
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::{new_terminal, Terminal};
use termwiz::Error;

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut terminal = new_terminal(caps)?;
    terminal.set_raw_mode()?;

    let mut buf = BufferedTerminal::new(terminal)?;

    buf.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Maroon.into(),
    )));
    buf.add_change("Hello world\r\n");
    buf.add_change(Change::Attribute(AttributeChange::Foreground(
        AnsiColor::Red.into(),
    )));
    buf.add_change("and in red here\r\n");

    buf.flush()?;

    Ok(())
}
