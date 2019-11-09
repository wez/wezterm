use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::{
    Color, Dimensions, KeyEvent, MouseButtons, MouseCursor, MouseEvent, MouseEventKind, MousePress,
    Operator, PaintContext, Point, Rect, Size, WindowCallbacks, WindowOps, WindowOpsMut,
};
use failure::Fallible;
use std::any::Any;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub(crate) struct WindowInner {
    window_id: xcb::xproto::Window,
    conn: Rc<Connection>,
    callbacks: Box<dyn WindowCallbacks>,
    window_context: Context,
    width: u16,
    height: u16,
    expose: VecDeque<Rect>,
    paint_all: bool,
    buffer_image: BufferImage,
    cursor: Option<MouseCursor>,
    #[cfg(feature = "opengl")]
    gl_state: Option<Rc<glium::backend::Context>>,
}

fn enclosing_boundary_with(a: &Rect, b: &Rect) -> Rect {
    let left = a.min_x().min(b.min_x());
    let right = a.max_x().max(b.max_x());

    let top = a.min_y().min(b.min_y());
    let bottom = a.max_y().max(b.max_y());

    Rect::new(Point::new(left, top), Size::new(right - left, bottom - top))
}

impl Drop for WindowInner {
    fn drop(&mut self) {
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
}

struct X11GraphicsContext<'a> {
    buffer: &'a mut dyn BitmapImage,
}

impl<'a> PaintContext for X11GraphicsContext<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        self.buffer.clear_rect(rect, color)
    }

    fn clear(&mut self, color: Color) {
        self.buffer.clear(color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.buffer.image_dimensions();
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
        self.buffer
            .draw_image(dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        self.buffer.draw_line(start, end, color, operator);
    }
}

impl WindowInner {
    pub fn paint(&mut self) -> Fallible<()> {
        let window_dimensions =
            Rect::from_size(Size::new(self.width as isize, self.height as isize));

        if self.paint_all {
            self.paint_all = false;
            self.expose.clear();
            self.expose.push_back(window_dimensions);
        } else if self.expose.is_empty() {
            return Ok(());
        }

        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = self.gl_state.as_ref() {
                self.expose.clear();

                let mut frame = glium::Frame::new(
                    Rc::clone(&gl_context),
                    (u32::from(self.width), u32::from(self.height)),
                );

                self.callbacks.paint_opengl(&mut frame);
                frame.finish()?;
                return Ok(());
            }
        }

        let (buf_width, buf_height) = self.buffer_image.image_dimensions();
        if buf_width != self.width.into() || buf_height != self.height.into() {
            // Window was resized, so we need to update our buffer
            self.buffer_image = BufferImage::new(
                &self.conn,
                self.window_id,
                self.width as usize,
                self.height as usize,
            );
        }

        for rect in self.expose.drain(..) {
            // Clip the rectangle to the current window size.
            // It can be larger than the window size in the case where we are working
            // through a series of resize exposures during a live resize, and we're
            // now sized smaller then when we queued the exposure.
            let rect = Rect::new(
                rect.origin,
                Size::new(
                    rect.size.width.min(self.width as isize),
                    rect.size.height.min(self.height as isize),
                ),
            );

            let mut context = X11GraphicsContext {
                buffer: &mut self.buffer_image,
            };

            self.callbacks.paint(&mut context);

            match &self.buffer_image {
                BufferImage::Shared(ref im) => {
                    self.window_context.copy_area(
                        im,
                        rect.origin.x as i16,
                        rect.origin.y as i16,
                        &self.window_id,
                        rect.origin.x as i16,
                        rect.origin.y as i16,
                        rect.size.width as u16,
                        rect.size.height as u16,
                    );
                }
                BufferImage::Image(ref buffer) => {
                    if rect == window_dimensions {
                        self.window_context.put_image(0, 0, buffer);
                    } else {
                        let mut im =
                            Image::new(rect.size.width as usize, rect.size.height as usize);

                        im.draw_image(Point::new(0, 0), Some(rect), buffer, Operator::Source);

                        self.window_context.put_image(
                            rect.origin.x as i16,
                            rect.origin.y as i16,
                            &im,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Add a region to the list of exposed/damaged/dirty regions.
    /// Note that a window resize will likely invalidate the entire window.
    /// If the new region intersects with the prior region, then we expand
    /// it to encompass both.  This avoids bloating the list with a series
    /// of increasing rectangles when resizing larger or smaller.
    fn expose(&mut self, x: u16, y: u16, width: u16, height: u16) {
        let expose = Rect::new(
            Point::new(x as isize, y as isize),
            Size::new(width as isize, height as isize),
        );
        if let Some(prior) = self.expose.back_mut() {
            if prior.intersects(&expose) {
                *prior = enclosing_boundary_with(&prior, &expose);
                return;
            }
        }
        self.expose.push_back(expose);
    }

    fn do_mouse_event(&mut self, event: &MouseEvent) -> Fallible<()> {
        self.callbacks
            .mouse_event(&event, &Window::from_id(self.window_id));
        Ok(())
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) -> Fallible<()> {
        if cursor == self.cursor {
            return Ok(());
        }

        let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
            // `/usr/include/X11/cursorfont.h`
            MouseCursor::Arrow => 132,
            MouseCursor::Hand => 58,
            MouseCursor::Text => 152,
        };

        let cursor_id: xcb::ffi::xcb_cursor_t = self.conn.generate_id();
        xcb::create_glyph_cursor(
            &self.conn,
            cursor_id,
            self.conn.cursor_font_id,
            self.conn.cursor_font_id,
            id_no,
            id_no + 1,
            0xffff,
            0xffff,
            0xffff,
            0,
            0,
            0,
        );

        xcb::change_window_attributes(
            &self.conn,
            self.window_id,
            &[(xcb::ffi::XCB_CW_CURSOR, cursor_id)],
        );

        xcb::free_cursor(&self.conn, cursor_id);

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: &xcb::GenericEvent) -> Fallible<()> {
        let r = event.response_type() & 0x7f;
        match r {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
                self.expose(expose.x(), expose.y(), expose.width(), expose.height());
            }
            xcb::CONFIGURE_NOTIFY => {
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                self.width = cfg.width();
                self.height = cfg.height();
                self.callbacks.resize(Dimensions {
                    pixel_width: self.width as usize,
                    pixel_height: self.height as usize,
                    dpi: 96,
                })
            }
            xcb::KEY_PRESS | xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                if let Some((code, mods)) = self.conn.keyboard.process_key_event(key_press) {
                    let key = KeyEvent {
                        key: code,
                        raw_key: None,
                        modifiers: mods,
                        repeat_count: 1,
                        key_is_down: r == xcb::KEY_PRESS,
                    };
                    self.callbacks
                        .key_event(&key, &Window::from_id(self.window_id));
                }
            }

            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };

                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    x: motion.event_x().max(0) as u16,
                    y: motion.event_y().max(0) as u16,
                    modifiers: xkeysyms::modifiers_from_state(motion.state()),
                    mouse_buttons: MouseButtons::default(),
                };
                self.do_mouse_event(&event)?;
            }
            xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };

                let kind = match button_press.detail() {
                    b @ 1..=3 => {
                        let button = match b {
                            1 => MousePress::Left,
                            2 => MousePress::Middle,
                            3 => MousePress::Right,
                            _ => unreachable!(),
                        };
                        if r == xcb::BUTTON_PRESS {
                            MouseEventKind::Press(button)
                        } else {
                            MouseEventKind::Release(button)
                        }
                    }
                    b @ 4..=5 => {
                        if r == xcb::BUTTON_RELEASE {
                            return Ok(());
                        }
                        MouseEventKind::VertWheel(if b == 4 { 1 } else { -1 })
                    }
                    _ => {
                        eprintln!("button {} is not implemented", button_press.detail());
                        return Ok(());
                    }
                };

                let event = MouseEvent {
                    kind,
                    x: button_press.event_x().max(0) as u16,
                    y: button_press.event_y().max(0) as u16,
                    modifiers: xkeysyms::modifiers_from_state(button_press.state()),
                    mouse_buttons: MouseButtons::default(),
                };
                self.do_mouse_event(&event)?;
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
                if msg.data().data32()[0] == self.conn.atom_delete() && self.callbacks.can_close() {
                    xcb::destroy_window(self.conn.conn(), self.window_id);
                }
            }
            xcb::DESTROY_NOTIFY => {
                self.callbacks.destroy();
                self.conn.windows.borrow_mut().remove(&self.window_id);
            }
            _ => {
                eprintln!("unhandled: {:x}", r);
            }
        }

        Ok(())
    }
}

/// A Window!
#[derive(Debug, Clone)]
pub struct Window(xcb::xproto::Window);

impl Window {
    pub(crate) fn from_id(id: xcb::xproto::Window) -> Self {
        Self(id)
    }

    /// Create a new window on the specified screen with the specified
    /// dimensions
    pub fn new_window(
        _class_name: &str,
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

        let window_id;
        let window = {
            let setup = conn.conn().get_setup();
            let screen = setup
                .roots()
                .nth(conn.screen_num() as usize)
                .ok_or_else(|| failure::err_msg("no screen?"))?;

            window_id = conn.conn().generate_id();

            xcb::create_window_checked(
                conn.conn(),
                xcb::COPY_FROM_PARENT as u8,
                window_id,
                screen.root(),
                // x, y
                0,
                0,
                // width, height
                width.try_into()?,
                height.try_into()?,
                // border width
                0,
                xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
                conn.visual.visual_id(), // screen.root_visual(),
                &[(
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE
                        | xcb::EVENT_MASK_KEY_PRESS
                        | xcb::EVENT_MASK_BUTTON_PRESS
                        | xcb::EVENT_MASK_BUTTON_RELEASE
                        | xcb::EVENT_MASK_POINTER_MOTION
                        | xcb::EVENT_MASK_BUTTON_MOTION
                        | xcb::EVENT_MASK_KEY_RELEASE
                        | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                )],
            )
            .request_check()?;

            let window_context = Context::new(&conn, &window_id);

            let buffer_image = BufferImage::new(&conn, window_id, width, height);

            Arc::new(Mutex::new(WindowInner {
                window_id,
                conn: Rc::clone(&conn),
                callbacks,
                window_context,
                width: width.try_into()?,
                height: height.try_into()?,
                expose: VecDeque::new(),
                paint_all: true,
                buffer_image,
                cursor: None,
                #[cfg(feature = "opengl")]
                gl_state: None,
            }))
        };

        xcb::change_property(
            &*conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            conn.atom_protocols,
            4,
            32,
            &[conn.atom_delete],
        );

        let window_handle = Window::from_id(window_id);

        window.lock().unwrap().callbacks.created(&window_handle);

        conn.windows.borrow_mut().insert(window_id, window.clone());

        window_handle.set_title(name);
        window_handle.show();

        Ok(window_handle)
    }
}

impl Drawable for Window {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.0
    }
}

impl WindowOpsMut for WindowInner {
    fn close(&mut self) {
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        xcb::map_window(self.conn.conn(), self.window_id);
    }
    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        WindowInner::set_cursor(self, cursor).unwrap();
    }
    fn invalidate(&mut self) {
        self.paint_all = true;
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        xcb::configure_window(
            self.conn.conn(),
            self.window_id,
            &[
                (xcb::CONFIG_WINDOW_WIDTH as u16, width as u32),
                (xcb::CONFIG_WINDOW_HEIGHT as u16, height as u32),
            ],
        );
    }

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        xcb_util::icccm::set_wm_name(self.conn.conn(), self.window_id, title);
    }
}

impl WindowOps for Window {
    fn close(&self) {
        Connection::with_window_inner(self.0, |inner| inner.close());
    }
    fn hide(&self) {
        Connection::with_window_inner(self.0, |inner| inner.hide());
    }
    fn show(&self) {
        Connection::with_window_inner(self.0, |inner| inner.show());
    }
    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        Connection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
        });
    }
    fn invalidate(&self) {
        Connection::with_window_inner(self.0, |inner| inner.invalidate());
    }
    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| inner.set_title(&title));
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        Connection::with_window_inner(self.0, move |inner| inner.set_inner_size(width, height));
    }

    fn apply<F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps)>(&self, func: F)
    where
        Self: Sized,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);
            func(inner.callbacks.as_any(), &window);
        });
    }

    #[cfg(feature = "opengl")]
    fn enable_opengl<
        F: Send
            + 'static
            + Fn(
                &mut dyn Any,
                &dyn WindowOps,
                failure::Fallible<std::rc::Rc<glium::backend::Context>>,
            ),
    >(
        &self,
        func: F,
    ) where
        Self: Sized,
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

            func(inner.callbacks.as_any(), &window, gl_state);
        });
    }
}
