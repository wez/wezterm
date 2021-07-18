use ::window::*;
use promise::spawn::spawn;
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_font::FontConfiguration;

struct MyWindow {
    allow_close: bool,
    cursor_pos: Point,
    dims: Dimensions,
    gl: Option<Rc<glium::backend::Context>>,
}

impl Drop for MyWindow {
    fn drop(&mut self) {
        eprintln!("MyWindow dropped");
    }
}

impl MyWindow {
    fn dispatch(&mut self, event: WindowEvent, win: &Window) {
        match dbg!(event) {
            WindowEvent::CloseRequested => {
                eprintln!("can I close?");
                if self.allow_close {
                    win.close();
                } else {
                    self.allow_close = true;
                }
            }
            WindowEvent::Destroyed => {
                eprintln!("destroy was called!");
                Connection::get().unwrap().terminate_message_loop();
            }
            WindowEvent::Resized {
                dimensions,
                is_full_screen,
            } => {
                eprintln!("resize {:?} is_full_screen={}", dimensions, is_full_screen);
                self.dims = dimensions;
            }
            WindowEvent::MouseEvent(event) => {
                self.cursor_pos = event.coords;
                // win.invalidate();
                win.set_cursor(Some(MouseCursor::Arrow));

                if event.kind == MouseEventKind::Press(MousePress::Left) {
                    eprintln!("{:?}", event);
                }
            }
            WindowEvent::KeyEvent(key) => {
                eprintln!("{:?}", key);
                win.set_cursor(Some(MouseCursor::Text));
                win.default_key_processing(key);
            }
            WindowEvent::NeedRepaint => {
                if let Some(gl) = self.gl.as_mut() {
                    if gl.is_context_lost() {
                        eprintln!("opengl context was lost; should reinit");
                        return;
                    }

                    let mut frame = glium::Frame::new(
                        Rc::clone(&gl),
                        (self.dims.pixel_width as u32, self.dims.pixel_height as u32),
                    );

                    use glium::Surface;
                    frame.clear_color_srgb(0.25, 0.125, 0.375, 1.0);
                    win.finish_frame(frame).unwrap();
                }
            }
            WindowEvent::AppearanceChanged(_)
            | WindowEvent::Notification(_)
            | WindowEvent::FocusChanged(_) => {}
        }
    }
}

async fn spawn_window() -> Result<(), Box<dyn std::error::Error>> {
    let fontconfig = Rc::new(FontConfiguration::new(
        None,
        ::window::default_dpi() as usize,
    )?);

    let state = Rc::new(RefCell::new(MyWindow {
        allow_close: false,
        cursor_pos: Point::new(100, 200),
        dims: Dimensions {
            pixel_width: 800,
            pixel_height: 600,
            dpi: 0,
        },
        gl: None,
    }));

    let cb_state = Rc::clone(&state);
    let win = Window::new_window(
        "myclass",
        "the title",
        800,
        600,
        None,
        fontconfig,
        move |event, window| {
            let mut state = cb_state.borrow_mut();
            state.dispatch(event, window)
        },
    )
    .await?;

    eprintln!("before show");
    win.show();
    let gl = win.enable_opengl().await?;

    state.borrow_mut().gl.replace(gl);
    win.invalidate();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let conn = Connection::init()?;
    spawn(async {
        eprintln!("running this async block");
        dbg!(spawn_window().await).ok();
        eprintln!("end of async block");
    })
    .detach();
    conn.run_message_loop()
}
