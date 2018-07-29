//! This example shows how to make a basic widget that accumulates
//! text input and renders it to the screen
extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::cell::AttributeChange;
use termwiz::color::{AnsiColor, ColorAttribute, RgbColor};
use termwiz::input::*;
use termwiz::surface::Change;
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::Blocking;
use termwiz::terminal::{new_terminal, Terminal};
use termwiz::widgets::*;

#[derive(Default)]
struct MainScreen {}

impl Widget2 for MainScreen {
    type State = String;
    type Event = ();

    fn init_state(&self) -> String {
        String::new()
    }

    fn update_state(&self, args: &mut WidgetUpdate<Self>) {
        let mut text = args.state_mut();
        for event in args.events() {
            match event {
                WidgetEvent::Input(InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    ..
                })) => text.push(c),
                WidgetEvent::Input(InputEvent::Key(KeyEvent {
                    key: KeyCode::Enter,
                    ..
                })) => {
                    text.push_str("\r\n");
                }
                WidgetEvent::Input(InputEvent::Paste(s)) => {
                    text.push_str(&s);
                }
                _ => {}
            }
        }

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

        *args.cursor_mut() = CursorShapeAndPosition {
            coords: surface.cursor_position().into(),
            shape: termwiz::surface::CursorShape::SteadyBar,
            ..Default::default()
        };
    }
}

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut buf = BufferedTerminal::new(new_terminal(caps)?)?;
    buf.terminal().set_raw_mode()?;

    let mut ui = Ui::new();

    let main_id = WidgetIdNr::new();
    ui.set_focus(main_id);

    loop {
        {
            MainScreen {}.build_ui_root(main_id, &mut ui);
        }
        ui.render_to_screen(&mut buf)?;
        buf.flush()?;

        match buf.terminal().poll_input(Blocking::Wait) {
            Ok(Some(InputEvent::Resized { rows, cols })) => {
                buf.add_change(Change::ClearScreen(Default::default()));
                buf.resize(cols, rows);
            }
            Ok(Some(input)) => match input {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                }) => {
                    break;
                }
                input @ _ => {
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

    Ok(())
}
