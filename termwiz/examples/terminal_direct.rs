//! This example shows how to render `Change`s directly to
//! an instance of `Terminal`.  When used in this way, the
//! library performas no optimization on the change stream.
//! Consider using the `Surface` struct to enable optimization;
//! the `buffered_terminal.rs` example demonstrates a simple
//! way to enable optimizations.

use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::AnsiColor;
use termwiz::surface::Change;
use termwiz::terminal::{new_terminal, Terminal};
use termwiz::Error;

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;
    let mut terminal = new_terminal(caps)?;

    terminal.render(&[
        Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
        Change::Text("Hello world\r\n".into()),
        Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
        Change::Text("and in red here\r\n".into()),
    ])?;

    Ok(())
}
