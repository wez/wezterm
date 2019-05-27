use failure::Fallible;
use termwiz::lineedit::line_editor;

fn main() -> Fallible<()> {
    let mut editor = line_editor()?;

    let line = editor.read_line()?;
    println!("read line: {:?}", line);

    Ok(())
}
