use promise::Future;
use std::any::Any;
pub mod bitmaps;
pub mod color;
pub mod configuration;
pub mod connection;
pub mod os;
mod spawn;
mod timerlist;

use configuration::{config, WindowConfigHandle};

#[cfg(target_os = "macos")]
pub const DEFAULT_DPI: f64 = 72.0;
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_DPI: f64 = 96.0;

mod egl;

pub use bitmaps::{BitmapImage, Image};
pub use color::Color;
pub use connection::*;
pub use glium;
pub use os::*;
pub use wezterm_input_types::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clipboard {
    Clipboard,
    PrimarySelection,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::Clipboard
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub dpi: usize,
}

pub type Rect = euclid::Rect<isize, PixelUnit>;
pub type Size = euclid::Size2D<isize, PixelUnit>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseCursor {
    Arrow,
    Hand,
    Text,
    SizeUpDown,
    SizeLeftRight,
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

    /// Called when window gains/loses focus
    fn focus_change(&mut self, focused: bool) {}

    /// Called when the window has opengl mode enabled and the window
    /// contents need painting.
    fn paint(&mut self, frame: &mut glium::Frame) {
        use glium::Surface;
        frame.clear_color_srgb(0.25, 0.125, 0.375, 1.0);
    }

    /// Called if the opengl context is lost
    fn opengl_context_lost(&mut self, _window: &dyn WindowOps) -> anyhow::Result<()> {
        Ok(())
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
    fn created(
        &mut self,
        _window: &Window,
        _context: std::rc::Rc<glium::backend::Context>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

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
    fn apply<R, F: Send + 'static + FnMut(&mut dyn Any, &dyn WindowOps) -> anyhow::Result<R>>(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static;

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String>;

    /// Set some text in the clipboard
    fn set_clipboard(&self, clipboard: Clipboard, text: String) -> Future<()>;

    /// Set the icon for the window.
    /// Depending on the system this may be shown in its titlebar
    /// and/or in the task manager/task switcher
    fn set_icon(&self, _image: Image) -> Future<()> {
        Future::ok(())
    }

    fn toggle_fullscreen(&self) -> Future<()> {
        Future::ok(())
    }

    fn config_did_change(&self, _config: &WindowConfigHandle) -> Future<()> {
        Future::ok(())
    }
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
    fn set_inner_size(&mut self, width: usize, height: usize);

    /// inform the windowing system of the current textual
    /// cursor input location.  This is used primarily for
    /// the platform specific input method editor
    fn set_text_cursor_position(&mut self, _cursor: Rect) {}

    /// Changes the location of the window on the screen.
    /// The coordinates are of the top left pixel of the
    /// client area.
    fn set_window_position(&self, _coords: ScreenPoint) {}

    /// Set the icon for the window.
    /// Depending on the system this may be shown in its titlebar
    /// and/or in the task manager/task switcher
    fn set_icon(&mut self, _image: &dyn BitmapImage) {}

    fn toggle_fullscreen(&mut self) {}

    fn config_did_change(&mut self, _config: &WindowConfigHandle) {}
}
