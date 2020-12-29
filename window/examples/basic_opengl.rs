use ::window::*;
use std::any::Any;

struct MyWindow {
    allow_close: bool,
    cursor_pos: (u16, u16),
}

impl WindowCallbacks for MyWindow {
    fn destroy(&mut self) {
        Connection::get().unwrap().terminate_message_loop();
    }

    fn paint(&mut self, context: &mut dyn PaintContext) {
        // Window contents are black in software mode
        context.clear(Color::rgb(0x0, 0x0, 0x0));
    }

    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        // Window contents are gray in opengl mode
        use glium::Surface;
        frame.clear_color_srgb(0.15, 0.15, 0.15, 1.0);
    }

    fn opengl_initialize(
        &mut self,
        _window: &dyn WindowOps,
        context: anyhow::Result<std::rc::Rc<glium::backend::Context>>,
    ) -> anyhow::Result<()> {
        match context {
            Ok(_ctx) => eprintln!("opengl enabled!"),
            Err(err) => eprintln!("opengl fail: {}", err),
        };
        Ok(())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

fn spawn_window() -> anyhow::Result<()> {
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
    win.apply(|myself, _win| {
        if let Some(myself) = myself.downcast_ref::<MyWindow>() {
            eprintln!(
                "got myself; allow_close={}, cursor_pos:{:?}",
                myself.allow_close, myself.cursor_pos
            );
        }
        Ok(())
    });
    Ok(())
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let conn = Connection::init()?;
    spawn_window()?;
    conn.run_message_loop()
}
