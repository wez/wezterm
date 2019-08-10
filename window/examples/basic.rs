use ::window::*;
use failure::Fallible;

struct MyWindow {
    allow_close: bool,
}

impl Drop for MyWindow {
    fn drop(&mut self) {
        eprintln!("MyWindow dropped");
    }
}

impl WindowCallbacks for MyWindow {
    fn can_close(&mut self) -> bool {
        eprintln!("can I close?");
        if self.allow_close {
            true
        } else {
            self.allow_close = true;
            false
        }
    }

    fn destroy(&mut self) {
        eprintln!("destroy was called!");
        Connection::get().unwrap().terminate_message_loop();
    }

    fn paint(&mut self, context: &mut dyn PaintContext) {
        context.clear(Color::rgb(0x40, 0x20, 0x60));
    }

    fn resize(&mut self, dims: Dimensions) {
        eprintln!("resize {:?}", dims);
    }

    fn key_event(&mut self, key: &KeyEvent) -> bool {
        eprintln!("{:?}", key);
        false
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
    drop(win);

    conn.run_message_loop()
}
