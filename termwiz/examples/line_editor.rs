use failure::{err_msg, Fallible};
use termwiz::caps::{Capabilities, ProbeHintsBuilder};
use termwiz::lineedit::LineEditor;
use termwiz::terminal::new_terminal;

fn main() -> Fallible<()> {
    // Disable mouse input in the line editor
    let hints = ProbeHintsBuilder::new_from_env()
        .mouse_reporting(Some(false))
        .build()
        .map_err(err_msg)?;
    let caps = Capabilities::new_with_hints(hints)?;
    let terminal = new_terminal(caps)?;
    let mut editor = LineEditor::new(terminal);

    let line = editor.read_line()?;
    println!("read line: {}", line);

    Ok(())
}
