//! This example shows how to make a basic widget that accumulates
//! text input and renders it to the screen
#![allow(unused)]
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::{AnsiColor, ColorAttribute};
use termwiz::input::*;
use termwiz::surface::Change;
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::{new_terminal, Terminal};
#[cfg(feature = "widgets")]
use termwiz::widgets::*;
use termwiz::Error;

/// This is a widget for our application
struct MainScreen<'a> {
    /// Holds the input text that we wish the widget to display
    text: &'a mut String,
}

impl<'a> MainScreen<'a> {
    /// Initialize the widget with the input text
    pub fn new(text: &'a mut String) -> Self {
        Self { text }
    }
}

#[cfg(feature = "widgets")]
impl<'a> Widget for MainScreen<'a> {
    fn process_event(&mut self, event: &WidgetEvent, _args: &mut UpdateArgs) -> bool {
        match event {
            WidgetEvent::Input(InputEvent::Key(KeyEvent {
                key: KeyCode::Char(c),
                ..
            })) => self.text.push(*c),
            WidgetEvent::Input(InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            })) => {
                self.text.push_str("\r\n");
            }
            WidgetEvent::Input(InputEvent::Paste(s)) => {
                self.text.push_str(&s);
            }
            _ => {}
        }

        true // handled it all
    }

    /// Draw ourselves into the surface provided by RenderArgs
    fn render(&mut self, args: &mut RenderArgs) {
        args.surface.add_change(Change::ClearScreen(
            ColorAttribute::TrueColorWithPaletteFallback(
                (0x31, 0x1B, 0x92).into(),
                AnsiColor::Black.into(),
            ),
        ));
        args.surface
            .add_change(Change::Attribute(AttributeChange::Foreground(
                ColorAttribute::TrueColorWithPaletteFallback(
                    (0xB3, 0x88, 0xFF).into(),
                    AnsiColor::Purple.into(),
                ),
            )));
        let dims = args.surface.dimensions();
        args.surface
            .add_change(format!("🤷 surface size is {:?}\r\n", dims));
        args.surface.add_change(self.text.clone());

        // Place the cursor at the end of the text.
        // A more advanced text editing widget would manage the
        // cursor position differently.
        *args.cursor = CursorShapeAndPosition {
            coords: args.surface.cursor_position().into(),
            shape: termwiz::surface::CursorShape::SteadyBar,
            ..Default::default()
        };
    }

    fn get_size_constraints(&self) -> layout::Constraints {
        layout::Constraints::with_fixed_width_height(80, 24)
    }
}

#[cfg(feature = "widgets")]
fn main() -> Result<(), Error> {
    // Start with an empty string; typing into the app will
    // update this string.
    let mut typed_text = String::new();

    {
        // Create a terminal and put it into full screen raw mode
        let caps = Capabilities::new_from_env()?;
        let mut buf = BufferedTerminal::new(new_terminal(caps)?)?;
        buf.terminal().set_raw_mode()?;
        buf.terminal().enter_alternate_screen()?;

        // Set up the UI
        let mut ui = Ui::new();

        ui.set_root(MainScreen::new(&mut typed_text));

        loop {
            ui.process_event_queue()?;

            // After updating and processing all of the widgets, compose them
            // and render them to the screen.
            if ui.render_to_screen(&mut buf)? {
                // We have more events to process immediately; don't block waiting
                // for input below, but jump to the top of the loop to re-run the
                // updates.
                continue;
            }
            // Compute an optimized delta to apply to the terminal and display it
            buf.flush()?;

            // Wait for user input
            match buf.terminal().poll_input(None) {
                Ok(Some(InputEvent::Resized { rows, cols })) => {
                    // FIXME: this is working around a bug where we don't realize
                    // that we should redraw everything on resize in BufferedTerminal.
                    buf.add_change(Change::ClearScreen(Default::default()));
                    buf.resize(cols, rows);
                }
                Ok(Some(input)) => match input {
                    InputEvent::Key(KeyEvent {
                        key: KeyCode::Escape,
                        ..
                    }) => {
                        // Quit the app when escape is pressed
                        break;
                    }
                    input @ _ => {
                        // Feed input into the Ui
                        ui.queue_event(WidgetEvent::Input(input));
                    }
                },
                Ok(None) => {}
                Err(e) => {
                    print!("{:?}\r\n", e);
                    break;
                }
            }
        }
    }

    // After we've stopped the full screen raw terminal,
    // print out the final edited value of the input text.
    println!("The text you entered: {}", typed_text);

    Ok(())
}

#[cfg(not(feature = "widgets"))]
fn main() {
    println!("recompile with --features widgets");
}
