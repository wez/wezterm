use failure::Fallible;
use termwiz::cell::AttributeChange;
use termwiz::color::{AnsiColor, ColorAttribute, RgbColor};
use termwiz::lineedit::{line_editor, LineEditorHost, OutputElement};

struct Host {}

impl LineEditorHost for Host {
    // Render the prompt with a darkslateblue background color if
    // the terminal supports true color, otherwise render it with
    // a navy blue ansi color.
    fn render_prompt(&self, prompt: &str) -> Vec<OutputElement> {
        vec![
            OutputElement::Attribute(AttributeChange::Background(
                ColorAttribute::TrueColorWithPaletteFallback(
                    RgbColor::from_named("darkslateblue").unwrap(),
                    AnsiColor::Navy.into(),
                ),
            )),
            OutputElement::Text(prompt.to_owned()),
        ]
    }
}

fn main() -> Fallible<()> {
    let mut editor = line_editor()?;

    let mut host = Host {};
    let line = editor.read_line(&mut host)?;
    println!("read line: {:?}", line);

    Ok(())
}
