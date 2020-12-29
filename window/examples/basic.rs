use ::window::*;
use glium::{program, uniform};
use std::any::Any;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}
glium::implement_vertex!(Vertex, position, color);

struct MyWindow {
    allow_close: bool,
    cursor_pos: Point,
    gl: Option<GlState>,
}

struct GlState {
    vb: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
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

    fn created(
        &mut self,
        _window: &Window,
        context: std::rc::Rc<glium::backend::Context>,
    ) -> anyhow::Result<()> {
        let vb = glium::VertexBuffer::new(
            &context,
            &[
                Vertex {
                    position: [-0.5, -0.5],
                    color: [0.0, 1.0, 0.0],
                },
                Vertex {
                    position: [0.0, 0.5],
                    color: [0.0, 0.0, 1.0],
                },
                Vertex {
                    position: [0.5, -0.5],
                    color: [1.0, 0.0, 0.0],
                },
            ],
        )?;

        let index_buffer = glium::IndexBuffer::new(
            &context,
            glium::index::PrimitiveType::TrianglesList,
            &[0u16, 1, 2],
        )?;

        let program = glium::program!(&context,
                                      140 => {
            vertex: "
                #version 140
                uniform mat4 matrix;
                in vec2 position;
                in vec3 color;
                out vec3 vColor;
                void main() {
                    gl_Position = vec4(position, 0.0, 1.0) * matrix;
                    vColor = color;
                }
            ",

            fragment: "
                #version 140
                in vec3 vColor;
                out vec4 f_color;
                void main() {
                    f_color = vec4(vColor, 1.0);
                }
            "
        },
        )?;

        self.gl.replace(GlState {
            vb,
            index_buffer,
            program,
        });

        Ok(())
    }

    fn paint(&mut self, frame: &mut glium::Frame) {
        // Window contents are gray in opengl mode
        use glium::Surface;
        frame.clear_color_srgb(0.25, 0.125, 0.375, 1.0);

        let uniforms = uniform! {
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0f32]
            ]
        };

        // drawing a frame
        let gl = self.gl.as_ref().unwrap();
        frame
            .draw(
                &gl.vb,
                &gl.index_buffer,
                &gl.program,
                &uniforms,
                &Default::default(),
            )
            .unwrap();
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
        eprintln!("{:?}", event);
        self.cursor_pos = event.coords;
        ctx.invalidate();
        ctx.set_cursor(Some(MouseCursor::Arrow));

        if event.kind == MouseEventKind::Press(MousePress::Left) {
            // spawn_window().unwrap();
        }
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
            cursor_pos: Point::new(100, 200),
            gl: None,
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
