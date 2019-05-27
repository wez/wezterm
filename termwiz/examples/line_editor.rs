use failure::Fallible;
use termwiz::caps::Capabilities;
use termwiz::lineedit::LineEditor;
use termwiz::terminal::new_terminal;

fn main() -> Fallible<()> {
    let caps = Capabilities::new_from_env()?;
    let terminal = new_terminal(caps)?;
    let mut editor = LineEditor::new(terminal);

    let line = editor.read_line()?;
    println!("read line: {}", line);

    Ok(())
}
