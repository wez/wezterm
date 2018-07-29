//! This example shows how to make a basic widget that accumulates
//! text input and renders it to the screen
extern crate failure;
extern crate termwiz;

use failure::Error;
use std::borrow::Cow;
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::{AnsiColor, ColorAttribute, RgbColor};
use termwiz::input::*;
use termwiz::surface::Change;
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::Blocking;
use termwiz::terminal::{new_terminal, Terminal};
use termwiz::widgets::*;

/// This is a widget for our application
struct MainScreen<'a> {
    /// Holds the input text that we wish the widget to display
    text: &'a str,
}

impl<'a> MainScreen<'a> {
    /// Initialize the widget with the input text
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }
}

impl<'a> Widget for MainScreen<'a> {
    /// This particular widget doesn't require the Ui to hold any state
    /// for it, as the Event always returns any changed text, and we
    /// always pass in that text on the next loop iteration.
    type State = ();

    /// Our `update_state` method will return Some(text) if the result
    /// of processing input changed the input text, or None if the
    /// input text is unchanged.
    type Event = Option<String>;

    /// We have no State to initialize
    fn init_state(&self) {}

    /// Process any input events and potentially update the returned text.
    /// Returns Some(text) if it was edited, else None.
    fn update_state(&self, args: &mut WidgetUpdate<Self>) -> Option<String> {
        // We use Cow here to defer making a clone of the input
        // text until the input events require it.
        let mut text = Cow::Borrowed(self.text);

        for event in args.events() {
            match event {
                WidgetEvent::Input(InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    ..
                })) => text.to_mut().push(c),
                WidgetEvent::Input(InputEvent::Key(KeyEvent {
                    key: KeyCode::Enter,
                    ..
                })) => {
                    text.to_mut().push_str("\r\n");
                }
                WidgetEvent::Input(InputEvent::Paste(s)) => {
                    text.to_mut().push_str(&s);
                }
                _ => {}
            }
        }

        // Now that we've updated the text, let's render it to
        // the display.  Get a reference to the surface.
        let mut surface = args.surface_mut();

        surface.add_change(Change::ClearScreen(
            ColorAttribute::TrueColorWithPaletteFallback(
                RgbColor::new(0x31, 0x1B, 0x92),
                AnsiColor::Black.into(),
            ),
        ));
        surface.add_change(Change::Attribute(AttributeChange::Foreground(
            ColorAttribute::TrueColorWithPaletteFallback(
                RgbColor::new(0xB3, 0x88, 0xFF),
                AnsiColor::Purple.into(),
            ),
        )));
        let dims = surface.dimensions();
        surface.add_change(format!("🤷 surface size is {:?}\r\n", dims));
        surface.add_change(text.clone());

        // Place the cursor at the end of the text.
        // A more advanced text editing widget would manage the
        // cursor position differently.
        *args.cursor_mut() = CursorShapeAndPosition {
            coords: surface.cursor_position().into(),
            shape: termwiz::surface::CursorShape::SteadyBar,
            ..Default::default()
        };

        // Return the new text if it changed, else None.
        if text != self.text {
            // updated!
            Some(text.into_owned())
        } else {
            // unchanged
            None
        }
    }
}

fn main() -> Result<(), Error> {
    // Start with an empty string; typing into the app will
    // update this string.
    let mut typed_text = String::new();

    {
        // Create a terminal and put it into full screen raw mode
        let caps = Capabilities::new_from_env()?;
        let mut buf = BufferedTerminal::new(new_terminal(caps)?)?;
        buf.terminal().set_raw_mode()?;

        // Set up the UI
        let mut ui = Ui::new();
        // Assign an id to associate state with the MainScreen widget.
        // We don't happen to retain anything in this example, but we
        // still need to assign an id.
        let main_id = WidgetId::new();

        loop {
            // Create the MainScreen and place it into the Ui.
            // When `build_ui_root` is called, the Ui machinery will
            // invoke `MainScreen::update_state` to process any input
            // events.
            // Because we've defined MainScreen to return Some(String)
            // when the input text is edited, the effect of these next
            // few lines is to have `MainScreen` take `typed_text`, apply
            // any keyboard events to it, display it, and then update
            // `typed_text` to hold the changed value.
            // `MainScreen` doesn't live for longer then the scope of this
            // short block.
            if let Some(updated) = MainScreen::new(&typed_text).build_ui_root(main_id, &mut ui) {
                typed_text = updated;
            }

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
            match buf.terminal().poll_input(Blocking::Wait) {
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
