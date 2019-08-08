use failure::Fallible;
use window::os::windows::window::*;

struct MyWindow {
    allow_close: bool,
}

impl WindowCallbacks for MyWindow {
    fn can_close(&mut self) -> bool {
        eprintln!("can I close?");
        if self.allow_close {
            terminate_message_loop();
            true
        } else {
            self.allow_close = true;
            false
        }
    }
}

fn main() -> Fallible<()> {
    let win = Window::new_window(
        "myclass",
        "the title",
        800,
        600,
        Box::new(MyWindow { allow_close: false }),
    )?;

    win.show();

    run_message_loop()
}
