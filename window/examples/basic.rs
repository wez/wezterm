use ::window::*;
use failure::Fallible;

struct MyWindow {
    allow_close: bool,
    cursor_pos: (u16, u16),
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
        // Pick a purple background color
        context.clear(Color::rgb(0x40, 0x20, 0x60));

        // This line doesn't need anti-aliasing
        context.draw_line(0, 0, 100, 100, Color::rgb(0xff, 0xff, 0xff), Operator::Over);

        // This shallower line should need some
        context.draw_line(
            100,
            0,
            200,
            120,
            Color::rgb(0xff, 0x80, 0xff),
            Operator::Over,
        );

        context.draw_line(
            0,
            0,
            self.cursor_pos.0 as isize,
            self.cursor_pos.1 as isize,
            Color::rgb(0xff, 0xff, 0x80),
            Operator::Over,
        );
    }

    fn resize(&mut self, dims: Dimensions) {
        eprintln!("resize {:?}", dims);
    }

    fn key_event(&mut self, key: &KeyEvent, ctx: &mut WindowContext) -> bool {
        eprintln!("{:?}", key);
        ctx.set_cursor(Some(MouseCursor::Text));
        false
    }

    fn mouse_event(&mut self, event: &MouseEvent, ctx: &mut WindowContext) {
        eprintln!("{:?}", event);
        self.cursor_pos = (event.x, event.y);
        ctx.invalidate();
        ctx.set_cursor(Some(MouseCursor::Arrow));

        if event.kind == MouseEventKind::Press(MousePress::Left) {
            spawn_window().unwrap();
        }
    }
}

fn spawn_window() -> Fallible<()> {
    let win = Window::new_window(
        "myclass",
        "the title",
        800,
        600,
        Box::new(MyWindow {
            allow_close: false,
            cursor_pos: (100, 200),
        }),
    )?;

    win.show();
    Ok(())
}

fn main() -> Fallible<()> {
    let conn = Connection::init()?;
    spawn_window()?;
    conn.run_message_loop()
}
