use async_trait::async_trait;
use promise::Future;
use std::rc::Rc;
use thiserror::Error;
pub mod bitmaps;
pub mod color;
mod configuration;
pub mod connection;
pub mod os;
mod spawn;
mod timerlist;

#[cfg(target_os = "macos")]
pub(crate) const DEFAULT_DPI: f64 = 72.0;
#[cfg(not(target_os = "macos"))]
pub(crate) const DEFAULT_DPI: f64 = 96.0;

pub fn default_dpi() -> f64 {
    match Connection::get() {
        Some(conn) => conn.default_dpi(),
        None => DEFAULT_DPI,
    }
}

mod egl;

pub use bitmaps::{BitmapImage, Image};
pub use connection::*;
pub use glium;
pub use os::*;
pub use wezterm_input_types::*;

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

#[derive(Debug, PartialEq, Eq)]
pub enum WindowEvent {
    /// Called when the window close button is clicked.
    /// The window closure is deferred and this event is
    /// sent to your application to decide whether it will
    /// really close the window.
    CloseRequested,

    /// Called when the window is being destroyed by the window system
    Destroyed,

    /// Called when the window has been resized
    Resized {
        dimensions: Dimensions,
        is_full_screen: bool,
    },

    /// Called when the window has been invalidated and needs to
    /// be repainted
    NeedRepaint,

    /// Called when the window gains/loses focus
    FocusChanged(bool),

    /// Called to handle a key event.
    /// If you didn't handle this event, then you must call
    /// window.default_key_processing(key) to allow the system to perform
    /// the default key handling.
    /// This is important on Windows for ALT keys to continue working
    /// correctly.
    KeyEvent(KeyEvent),

    MouseEvent(MouseEvent),
}

pub type WindowEventSender = async_channel::Sender<WindowEvent>;
pub type WindowEventReceiver = async_channel::Receiver<WindowEvent>;

#[derive(Debug, Error)]
#[error("Graphics drivers lost context")]
pub struct GraphicsDriversLostContext {}

#[async_trait(?Send)]
pub trait WindowOps {
    /// Show a hidden window
    fn show(&self) -> Future<()>;

    /// Setup opengl for rendering
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>>;
    /// Advise the window that a frame is finished
    fn finish_frame(&self, frame: glium::Frame) -> anyhow::Result<()> {
        frame.finish()?;
        Ok(())
    }

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

    fn default_key_processing(&self, _key: KeyEvent) {}

    /// Resize the inner or client area of the window
    fn set_inner_size(&self, width: usize, height: usize) -> Future<Dimensions>;

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

    fn config_did_change(&self, _config: &config::ConfigHandle) -> Future<()> {
        Future::ok(())
    }
}
