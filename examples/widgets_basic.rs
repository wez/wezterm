//! This example shows how to make a basic widget that accumulates
//! text input and renders it to the screen
extern crate failure;
#[cfg(unix)]
extern crate libc;
extern crate termwiz;

use failure::Error;
use std::cell::Cell;
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
    cursor: Cell<ParentRelativeCoords>,
}

#[cfg(unix)]
fn catch_winch() {
    extern "C" fn dummy(_: libc::c_int) {
        // This signal handler only exists so that we can knock
        // ourselves out of a read syscall with EINTR.
    }
    unsafe {
        // note: SA_RESTART is cleared by this; we don't want
        // SA_RESTART enabled; we want SIGWINCH to generate an
        // EINTR when it occurs.
        use std::mem;
        use std::ptr;
        let mut sig: libc::sigaction = mem::zeroed();
        sig.sa_sigaction = dummy as usize;
        libc::sigaction(libc::SIGWINCH, &sig, ptr::null_mut());
    }
}

impl WidgetImpl for MainScreen {
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

    fn render_to_surface(&self, surface: &mut Surface) {
        surface.add_change(Change::ClearScreen(AnsiColor::Blue.into()));
        let dims = surface.dimensions();
        surface.add_change(format!("surface size is {:?}\r\n", dims));
        surface.add_change(self.buf.clone());
        // Allow the surface rendering code to figure out where the
        // cursor ends up, then stash a copy of that information for
        // later retrieval by get_cursor_shape_and_position().
        let (x, y) = surface.cursor_position();
        self.cursor.set(ParentRelativeCoords::new(x, y));
    }

    fn get_cursor_shape_and_position(&self) -> CursorShapeAndPosition {
        CursorShapeAndPosition {
            coords: self.cursor.get(),
            shape: termwiz::surface::CursorShape::SteadyBar,
            ..Default::default()
        }
    }
}

fn main() -> Result<(), Error> {
    #[cfg(unix)]
    catch_winch();

    let caps = Capabilities::new_from_env()?;

    let mut buf = BufferedTerminal::new(new_terminal(caps)?)?;
    buf.terminal().set_raw_mode()?;

    let mut screen = Screen::new(Widget::new(Box::new(MainScreen::default())));

    screen.render_to_screen(&mut buf)?;
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

        if buf.check_for_resize()? {
            buf.add_change(Change::ClearScreen(Default::default()));
        }
        screen.render_to_screen(&mut buf)?;
        buf.flush()?;
    }

    Ok(())
}
