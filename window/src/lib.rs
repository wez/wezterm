use promise::Future;
use std::any::Any;
pub mod bitmaps;
pub mod color;
pub mod connection;
pub mod input;
pub mod os;
mod spawn;
mod tasks;
mod timerlist;

#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "windows"),
    feature = "opengl"
))]
mod egl;

#[cfg(feature = "opengl")]
pub use glium;

pub use bitmaps::BitmapImage;
pub use color::Color;
pub use connection::*;
pub use input::*;
pub use os::*;

/// Compositing operator.
/// We implement a small subset of possible compositing operators.
/// More information on these and their temrinology can be found
/// in the Cairo documentation here:
/// https://www.cairographics.org/operators/
#[derive(Debug, Clone, Copy)]
pub enum Operator {
    /// Apply the alpha channel of src and combine src with dest,
    /// according to the classic OVER composite operator
    Over,
    /// Ignore dest; take src as the result of the operation
    Source,
    /// Multiply src x dest.  The result is at least as dark as
    /// the darker of the two input colors.  This is used to
    /// apply a color tint.
    Multiply,
    /// Multiply src with the provided color, then apply the
    /// Over operator on the result with the dest as the dest.
    /// This is used to colorize the src and then blend the
    /// result into the destination.
    MultiplyThenOver(Color),
}

#[derive(Debug, Clone, Copy)]
pub struct Dimensions {
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub dpi: usize,
}

pub struct PixelUnit;
pub type Point = euclid::Point2D<isize, PixelUnit>;
pub type Rect = euclid::Rect<isize, PixelUnit>;
pub type Size = euclid::Size2D<isize, PixelUnit>;

pub struct ScreenPixelUnit;
pub type ScreenPoint = euclid::Point2D<isize, ScreenPixelUnit>;

pub trait PaintContext {
    fn get_dimensions(&self) -> Dimensions;

    /// Clear the entire context to the specified color
    fn clear(&mut self, color: Color) {
        let dims = self.get_dimensions();
        self.clear_rect(
            Rect::from_size(Size::new(
                dims.pixel_width as isize,
                dims.pixel_height as isize,
            )),
            color,
        );
    }

    /// Clear a rectangle to the specified color
    fn clear_rect(&mut self, rect: Rect, color: Color);

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    );

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCursor {
    Arrow,
    Hand,
    Text,
}

#[allow(unused_variables)]
pub trait WindowCallbacks: Any {
    /// Called when the window close button is clicked.
    /// Return true to allow the close to continue, false to
    /// prevent it from closing.
    fn can_close(&mut self) -> bool {
        true
    }

    /// Called when the window is being destroyed by the gui system
    fn destroy(&mut self) {}

    /// Called when the window is resized, or when the dpi has changed
    fn resize(&mut self, dimensions: Dimensions) {}

    /// Called when the window contents need painting.
    /// This is used only when the software renderer is enabled (which
    /// is the default).  When the window is set to opengl mode, the
    /// `paint_opengl` function is called instead.
    fn paint(&mut self, context: &mut dyn PaintContext) {
        context.clear(Color::rgb(0x20, 0x40, 0x60));
    }

    /// Called when the window has opengl mode enabled and the window
    /// contents need painting.
    #[cfg(feature = "opengl")]
    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        use glium::Surface;
        frame.clear_color(0.25, 0.125, 0.375, 1.0);
    }

    /// Called to handle a key event.
    /// If your window didn't handle the event, you must return false.
    /// This is particularly important for eg: ALT keys on windows,
    /// otherwise standard key assignments may not function in your window.
    fn key_event(&mut self, key: &KeyEvent, context: &dyn WindowOps) -> bool {
        false
    }

    fn mouse_event(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        context.set_cursor(Some(MouseCursor::Arrow));
    }

    /// Called when the window is created and allows the embedding
    /// app to reference the window and operate upon it.
    fn created(&mut self, window: &Window) {}

    /// An unfortunate bit of boilerplate; you need to provie an impl
    /// of this method that returns `self` in order for the downcast_ref
    /// method of the Any trait to be usable on WindowCallbacks.
    /// https://stackoverflow.com/q/46045298/149111 and others have
    /// some rationale on why Rust works this way.
    fn as_any(&mut self) -> &mut dyn Any;
}

pub trait WindowOps {
    /// Show a hidden window
    fn show(&self) -> Future<()>;

    /// Hide a visible window
    fn hide(&self) -> Future<()>;

    /// Schedule the window to be closed
    fn close(&self) -> Future<()>;

    /// Change the cursor
    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()>;

    /// Invalidate the window so that the entire client area will
    /// be repainted shortly
    fn invalidate(&self) -> Future<()>;

    /// Change the titlebar text for the window
    fn set_title(&self, title: &str) -> Future<()>;

    /// Resize the inner or client area of the window
    fn set_inner_size(&self, width: usize, height: usize) -> Future<()>;

    /// Changes the location of the window on the screen.
    /// The coordinates are of the top left pixel of the
    /// client area.
    fn set_window_position(&self, _coords: ScreenPoint) -> Future<()> {
        Future::ok(())
    }

    /// inform the windowing system of the current textual
    /// cursor input location.  This is used primarily for
    /// the platform specific input method editor
    fn set_text_cursor_position(&self, _cursor: Rect) -> Future<()> {
        Future::ok(())
    }

    /// Schedule a callback on the data associated with the window.
    /// The `Any` that is passed in corresponds to the WindowCallbacks
    /// impl you passed to `new_window`, pre-converted to Any so that
    /// you can `downcast_ref` or `downcast_mut` it and operate on it.
    fn apply<R, F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps) -> failure::Fallible<R>>(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static;

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
        R: Send + 'static;
}

pub trait WindowOpsMut {
    /// Show a hidden window
    fn show(&mut self);

    /// Hide a visible window
    fn hide(&mut self);

    /// Schedule the window to be closed
    fn close(&mut self);

    /// Change the cursor
    fn set_cursor(&mut self, cursor: Option<MouseCursor>);

    /// Invalidate the window so that the entire client area will
    /// be repainted shortly
    fn invalidate(&mut self);

    /// Change the titlebar text for the window
    fn set_title(&mut self, title: &str);

    /// Resize the inner or client area of the window
    fn set_inner_size(&self, width: usize, height: usize);

    /// inform the windowing system of the current textual
    /// cursor input location.  This is used primarily for
    /// the platform specific input method editor
    fn set_text_cursor_position(&mut self, _cursor: Rect) {}

    /// Changes the location of the window on the screen.
    /// The coordinates are of the top left pixel of the
    /// client area.
    fn set_window_position(&self, _coords: ScreenPoint) {}
}
