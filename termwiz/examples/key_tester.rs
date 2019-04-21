extern crate failure;
extern crate termwiz;

use failure::Error;
use termwiz::caps::Capabilities;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use termwiz::terminal::{new_terminal, Blocking, Terminal};

const CTRL_C: KeyEvent = KeyEvent {
    key: KeyCode::Char('C'),
    modifiers: Modifiers::CTRL,
};

fn main() -> Result<(), Error> {
    let caps = Capabilities::new_from_env()?;
    let mut terminal = new_terminal(caps)?;
    terminal.set_raw_mode()?;

    while let Some(event) = terminal.poll_input(Blocking::Wait)? {
        print!("{:?}\r\n", event);
        if event == InputEvent::Key(CTRL_C) {
            break;
        }
    }

    Ok(())
}
