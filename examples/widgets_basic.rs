extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::color::AnsiColor;
use termwiz::input::*;
use termwiz::surface::{Change, Surface};
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::Blocking;
use termwiz::terminal::{new_terminal, Terminal};
use termwiz::widgets::*;

#[derive(Default)]
struct MainScreen {
    buf: String,
}

impl WidgetImpl for MainScreen {
    fn set_widget_id(&mut self, _id: WidgetId) {}
    fn process_event(&mut self, event: &WidgetEvent) -> EventDisposition {
        match event {
            WidgetEvent::Input(InputEvent::Key(KeyEvent {
                key: KeyCode::Char(c),
                ..
            })) => {
                self.buf.push(*c);
                EventDisposition::Stop
            }
            WidgetEvent::Input(InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            })) => {
                self.buf.push_str("\r\n");
                EventDisposition::Stop
            }
            _ => EventDisposition::Propagate,
        }
    }

    fn get_size_constraints(&self) -> SizeConstraints {
        SizeConstraints::default()
    }

    fn render_to_surface(&self, surface: &mut Surface) {
        surface.add_change(Change::ClearScreen(AnsiColor::Blue.into()));
        surface.add_change(self.buf.clone());
    }
}

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;

    let mut buf = BufferedTerminal::new(new_terminal(caps)?)?;
    buf.terminal().set_raw_mode()?;

    let mut screen = Screen::new(Widget::new(Box::new(MainScreen::default())));

    screen.render_to_screen(&mut buf);
    buf.flush()?;

    loop {
        match buf.terminal().poll_input(Blocking::Wait) {
            Ok(Some(input)) => match input {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                }) => {
                    break;
                }
                input @ _ => {
                    screen.route_event(&WidgetEvent::Input(input));
                }
            },
            Ok(None) => {}
            Err(e) => {
                print!("{:?}\r\n", e);
                break;
            }
        }

        screen.render_to_screen(&mut buf);
        buf.flush()?;
    }

    Ok(())
}
