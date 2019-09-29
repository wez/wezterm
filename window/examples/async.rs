#![cfg_attr(feature="async_await", feature(async_await))]

use ::window::*;
use failure::Fallible;
use std::any::Any;

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
    }

    fn resize(&mut self, dims: Dimensions) {
        eprintln!("resize {:?}", dims);
    }

    fn key_event(&mut self, key: &KeyEvent, ctx: &dyn WindowOps) -> bool {
        eprintln!("{:?}", key);
        ctx.set_cursor(Some(MouseCursor::Text));
        false
    }

    fn mouse_event(&mut self, event: &MouseEvent, ctx: &dyn WindowOps) {
        self.cursor_pos = (event.x, event.y);
        ctx.invalidate();
        ctx.set_cursor(Some(MouseCursor::Arrow));

        if event.kind == MouseEventKind::Press(MousePress::Left) {
            eprintln!("{:?}", event);
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(feature="async_await")]
async fn spawn_window() -> Result<(), Box<dyn std::error::Error>> {
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

    eprintln!("here I am");
    win.show();
    eprintln!("and here");
    win.apply(|myself, _win| {
        eprintln!("doing apply");
        if let Some(myself) = myself.downcast_ref::<MyWindow>() {
            eprintln!(
                "got myself; allow_close={}, cursor_pos:{:?}",
                myself.allow_close, myself.cursor_pos
            );
        }
    });
    eprintln!("done with spawn_window");
    Ok(())
}

#[cfg(feature="async_await")]
fn main() -> Fallible<()> {
    let conn = Connection::init()?;
    conn.spawn_task(async {
        eprintln!("running this async block");
        spawn_window().await.ok();
    });
    conn.run_message_loop()
}

#[cfg(not(feature="async_await"))]
fn main() {
    eprintln!("Run me with rust 1.39 or later, or a current nightly build");
    eprintln!("   cargo +nightly run --example async --features async_await");
}
