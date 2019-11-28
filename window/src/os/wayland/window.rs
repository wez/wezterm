use crate::bitmaps::BitmapImage;
use crate::color::Color;
use crate::connection::ConnectionOps;
use crate::{
    Connection, Dimensions, MouseCursor, Operator, PaintContext, Point, Rect, ScreenPoint,
    WindowCallbacks, WindowOps, WindowOpsMut,
};
use failure::Fallible;
use promise::Future;
use smithay_client_toolkit as toolkit;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use toolkit::reexports::client::protocol::wl_seat::WlSeat;
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::reexports::client::NewProxy;
use toolkit::utils::DoubleMemPool;
use toolkit::window::Event;

struct MyTheme;
use toolkit::window::ButtonState;
impl toolkit::window::Theme for MyTheme {
    fn get_primary_color(&self, _active: bool) -> [u8; 4] {
        [0xff, 0x80, 0x80, 0x80]
    }

    fn get_secondary_color(&self, _active: bool) -> [u8; 4] {
        [0xff, 0x60, 0x60, 0x60]
    }

    fn get_close_button_color(&self, _status: ButtonState) -> [u8; 4] {
        [0xff, 0xff, 0xff, 0xff]
    }
    fn get_maximize_button_color(&self, _status: ButtonState) -> [u8; 4] {
        [0xff, 0xff, 0xff, 0xff]
    }
    fn get_minimize_button_color(&self, _status: ButtonState) -> [u8; 4] {
        [0xff, 0xff, 0xff, 0xff]
    }
}

pub struct WindowInner {
    window_id: usize,
    callbacks: Box<dyn WindowCallbacks>,
    surface: WlSurface,
    seat: WlSeat,
    window: toolkit::window::Window<toolkit::window::ConceptFrame>,
    pool: DoubleMemPool,
    dimensions: (u32, u32),
}

pub struct Window(usize);

impl Window {
    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> Fallible<Window> {
        let conn = Connection::get().ok_or_else(|| {
            failure::err_msg(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
        })?;

        let window_id = conn.next_window_id();

        let surface = conn
            .environment
            .borrow_mut()
            .compositor
            .create_surface(NewProxy::implement_dummy)
            .map_err(|_| failure::err_msg("new_window: failed to create a surface"))?;

        let dimensions = (width as u32, height as u32);
        let mut window = toolkit::window::Window::<toolkit::window::ConceptFrame>::init_from_env(
            &*conn.environment.borrow(),
            surface.clone(),
            dimensions,
            move |evt| {
                Connection::with_window_inner(window_id, move |inner| {
                    inner.handle_event(evt.clone());
                    Ok(())
                });
            },
        )
        .map_err(|e| failure::format_err!("Failed to create window: {}", e))?;

        window.set_app_id(class_name.to_string());
        window.set_decorate(true);
        window.set_resizable(true);
        window.set_theme(MyTheme {});

        let pool = DoubleMemPool::new(&conn.environment.borrow().shm, || {})?;

        let seat = conn
            .environment
            .borrow()
            .manager
            .instantiate_range(1, 6, NewProxy::implement_dummy)
            .map_err(|_| failure::format_err!("Failed to create seat"))?;
        window.new_seat(&seat);

        let inner = Rc::new(RefCell::new(WindowInner {
            window_id,
            callbacks,
            surface,
            seat,
            window,
            pool,
            dimensions,
        }));

        let window_handle = Window(window_id);

        conn.windows.borrow_mut().insert(window_id, inner.clone());

        inner.borrow_mut().callbacks.created(&window_handle);

        Ok(window_handle)
    }
}

impl WindowInner {
    fn handle_event(&mut self, evt: Event) {
        match evt {
            Event::Close => {
                if self.callbacks.can_close() {
                    println!("FIXME: I should destroy all refs to the window now");
                }
            }
            Event::Refresh => {
                self.window.refresh();
                self.window.surface().commit();
            }
            Event::Configure { new_size, states } => {
                if let Some((w, h)) = new_size {
                    self.window.resize(w, h);
                    self.dimensions = (w, h);
                }
                self.window.refresh();
                self.do_paint().unwrap();
            }
        }
    }

    fn do_paint(&mut self) -> Fallible<()> {
        let pool = match self.pool.pool() {
            Some(pool) => pool,
            None => {
                // Buffer still in use by server; retry later
                return Ok(());
            }
        };

        pool.resize((4 * self.dimensions.0 * self.dimensions.1) as usize)?;

        let mut context = MmapImage {
            mmap: pool.mmap(),
            dimensions: (self.dimensions.0 as usize, self.dimensions.1 as usize),
        };
        self.callbacks.paint(&mut context);

        let buffer = pool.buffer(
            0,
            self.dimensions.0 as i32,
            self.dimensions.1 as i32,
            4 * self.dimensions.0 as i32,
            toolkit::reexports::client::protocol::wl_shm::Format::Argb8888,
        );

        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.commit();
        self.window.refresh();

        Ok(())
    }
}

struct MmapImage<'a> {
    mmap: &'a mut memmap::MmapMut,
    dimensions: (usize, usize),
}

impl<'a> BitmapImage for MmapImage<'a> {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.mmap.as_ptr()
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.mmap.as_mut_ptr()
    }

    fn image_dimensions(&self) -> (usize, usize) {
        self.dimensions
    }
}

impl<'a> PaintContext for MmapImage<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        BitmapImage::clear_rect(self, rect, color)
    }

    fn clear(&mut self, color: Color) {
        BitmapImage::clear(self, color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: 96,
        }
    }

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        BitmapImage::draw_image(self, dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        BitmapImage::draw_line(self, start, end, color, operator);
    }
}

impl WindowOps for Window {
    fn close(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn hide(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        })
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps) -> Fallible<R>>(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);
            func(inner.callbacks.as_any(), &window)
        })
    }

    #[cfg(feature = "opengl")]
    fn enable_opengl<
        R,
        F: Send
            + 'static
            + Fn(
                &mut dyn Any,
                &dyn WindowOps,
                failure::Fallible<std::rc::Rc<glium::backend::Context>>,
            ) -> failure::Fallible<R>,
    >(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);

            let gl_state = crate::egl::GlState::create(
                Some(inner.conn.display as *const _),
                inner.window_id as *mut _,
            )
            .map(Rc::new)
            .and_then(|state| unsafe {
                Ok(glium::backend::Context::new(
                    Rc::clone(&state),
                    true,
                    if cfg!(debug_assertions) {
                        glium::debug::DebugCallbackBehavior::DebugMessageOnError
                    } else {
                        glium::debug::DebugCallbackBehavior::Ignore
                    },
                )?)
            });

            inner.gl_state = gl_state.as_ref().map(Rc::clone).ok();

            func(inner.callbacks.as_any(), &window, gl_state)
        })
    }
}

impl WindowOpsMut for WindowInner {
    fn close(&mut self) {}
    fn hide(&mut self) {}
    fn show(&mut self) {
        let conn = Connection::get().unwrap();

        if !conn.environment.borrow().shell.needs_configure() {
            self.do_paint().unwrap();
        } else {
            self.window.refresh();
        }
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {}

    fn invalidate(&mut self) {
        self.window
            .surface()
            .damage(0, 0, self.dimensions.0 as i32, self.dimensions.1 as i32);
    }

    fn set_inner_size(&self, width: usize, height: usize) {}

    fn set_window_position(&self, coords: ScreenPoint) {}

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        self.window.set_title(title.to_string());
    }
}
