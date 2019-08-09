use ::window::*;
use failure::Fallible;

struct MyWindow {
    allow_close: bool,
}

impl WindowCallbacks for MyWindow {
    fn can_close(&mut self) -> bool {
        eprintln!("can I close?");
        if self.allow_close {
            Connection::get().unwrap().terminate_message_loop();
            true
        } else {
            self.allow_close = true;
            false
        }
    }
}

fn main() -> Fallible<()> {
    let conn = Connection::init()?;

    let win = Window::new_window(
        "myclass",
        "the title",
        800,
        600,
        Box::new(MyWindow { allow_close: false }),
    )?;

    win.show();

    conn.run_message_loop()
}
