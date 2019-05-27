use failure::Fallible;
use termwiz::cell::AttributeChange;
use termwiz::color::{AnsiColor, ColorAttribute, RgbColor};
use termwiz::lineedit::*;

#[derive(Default)]
struct Host {
    history: BasicHistory,
}

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

    fn history(&mut self) -> &mut History {
        &mut self.history
    }
}

fn main() -> Fallible<()> {
    println!("Type `exit` to quit this example");
    let mut editor = line_editor()?;

    let mut host = Host::default();
    loop {
        if let Some(line) = editor.read_line(&mut host)? {
            println!("read line: {:?}", line);
            if line == "exit" {
                break;
            }

            host.history().add(&line);
        }
    }

    Ok(())
}
