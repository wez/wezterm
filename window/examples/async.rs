use ::window::*;
use promise::spawn::spawn;
use std::rc::Rc;

struct MyWindow {
    allow_close: bool,
    cursor_pos: Point,
    dims: Dimensions,
}

impl Drop for MyWindow {
    fn drop(&mut self) {
        eprintln!("MyWindow dropped");
    }
}

async fn spawn_window() -> Result<(), Box<dyn std::error::Error>> {
    let (win, events) = Window::new_window("myclass", "the title", 800, 600, None).await?;

    let mut state = MyWindow {
        allow_close: false,
        cursor_pos: Point::new(100, 200),
        dims: Dimensions {
            pixel_width: 800,
            pixel_height: 600,
            dpi: 0,
        },
    };

    eprintln!("before show");
    win.show().await?;
    let gl = win.enable_opengl().await?;
    eprintln!("window is visible, do loop");

    while let Ok(event) = events.recv().await {
        match dbg!(event) {
            WindowEvent::CloseRequested => {
                eprintln!("can I close?");
                if state.allow_close {
                    win.close();
                } else {
                    state.allow_close = true;
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
                state.dims = dimensions;
            }
            WindowEvent::MouseEvent(event) => {
                state.cursor_pos = event.coords;
                win.invalidate();
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
                if gl.is_context_lost() {
                    eprintln!("opengl context was lost; should reinit");
                    break;
                }

                let mut frame = glium::Frame::new(
                    Rc::clone(&gl),
                    (
                        state.dims.pixel_width as u32,
                        state.dims.pixel_height as u32,
                    ),
                );

                use glium::Surface;
                frame.clear_color_srgb(0.25, 0.125, 0.375, 1.0);
                win.finish_frame(frame)?;
            }
            WindowEvent::Notification(_) | WindowEvent::FocusChanged(_) => {}
        }
    }

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
