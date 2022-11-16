use async_trait::async_trait;
use bitflags::bitflags;
use config::{ConfigHandle, Dimension, GeometryOrigin};
use promise::Future;
use std::any::Any;
use std::path::PathBuf;
use std::rc::Rc;
use thiserror::Error;
pub mod bitmaps;
pub use wezterm_color_types as color;
mod configuration;
pub mod connection;
pub mod os;
pub mod screen;
mod spawn;

pub use raw_window_handle;

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

pub type ULength = euclid::Length<usize, PixelUnit>;
pub type Rect = euclid::Rect<isize, PixelUnit>;
pub type RectF = euclid::Rect<f32, PixelUnit>;
pub type Size = euclid::Size2D<isize, PixelUnit>;
pub type SizeF = euclid::Size2D<f32, PixelUnit>;
pub type ScreenRect = euclid::Rect<isize, ScreenPixelUnit>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseCursor {
    Arrow,
    Hand,
    Text,
    SizeUpDown,
    SizeLeftRight,
}

/// Represents the preferred appearance of the windowing
/// environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appearance {
    /// Standard dark-text-on-light-background presentation
    Light,
    /// Dark mode, with predominantly dark or muted colors
    Dark,
    /// dark-text-on-light-background, but in a higher contrast
    /// more accesible palette
    LightHighContrast,
    /// darker background but with higher contrast than regular
    /// dark mode
    DarkHighContrast,
}

impl std::string::ToString for Appearance {
    fn to_string(&self) -> String {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::LightHighContrast => "LightHighContrast",
            Self::DarkHighContrast => "DarkHighContrast",
        }
        .to_string()
    }
}

bitflags! {
    #[derive(Default)]
    pub struct WindowState: u8 {
        /// Occupies the whole screen; cannot be resized while in this state.
        const FULL_SCREEN = 1<<1;
        /// Maximized along either or both of horizontal or vertical dimensions;
        /// cannot be resized while in this state.
        const MAXIMIZED = 1<<2;
        /// Minimized or in some kind of off-screen state. Cannot be repainted
        /// while in this state.
        const HIDDEN = 1<<3;
    }
}

impl WindowState {
    pub fn can_resize(self) -> bool {
        !self.intersects(Self::FULL_SCREEN | Self::MAXIMIZED)
    }

    pub fn can_paint(self) -> bool {
        !self.contains(Self::HIDDEN)
    }
}

#[derive(Debug, Clone)]
pub enum WindowKeyEvent {
    RawKeyEvent(RawKeyEvent),
    KeyEvent(KeyEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadKeyStatus {
    /// Not in a dead key processing hold
    None,
    /// Holding until composition is done; the string is the uncommitted
    /// composition text to show as a placeholder
    Composing(String),
}

#[derive(Debug)]
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
        window_state: WindowState,
        live_resizing: bool,
    },

    /// Called when the window has been invalidated and needs to
    /// be repainted
    NeedRepaint,

    /// Called when the window gains/loses focus
    FocusChanged(bool),

    AdviseDeadKeyStatus(DeadKeyStatus),

    /// Called to handle a raw key event, prior to any dead key,
    /// keymap composition or other higher level treatment.
    /// If you handle this key event, you must call
    /// event.set_handled() to prevent additional processing.
    RawKeyEvent(RawKeyEvent),

    /// Called to handle a key event.
    KeyEvent(KeyEvent),

    MouseEvent(MouseEvent),
    MouseLeave,

    AppearanceChanged(Appearance),

    Notification(Box<dyn Any + Send + Sync>),

    // Called when the files are being dragged into the window
    DraggedFile(Vec<PathBuf>),

    // Called when the files are dropped into the window
    DroppedFile(Vec<PathBuf>),
}

pub struct WindowEventSender {
    handler: Box<dyn FnMut(WindowEvent, &Window)>,
    window: Option<Window>,
}

impl WindowEventSender {
    pub fn new<F: 'static + FnMut(WindowEvent, &Window)>(handler: F) -> Self {
        Self {
            handler: Box::new(handler),
            window: None,
        }
    }

    pub(crate) fn assign_window(&mut self, window: Window) {
        self.window.replace(window);
    }

    pub fn dispatch(&mut self, event: WindowEvent) {
        if let Some(window) = self.window.as_ref() {
            log::trace!("{:?}", event);
            (self.handler)(event, window);
        }
    }
}

#[derive(Debug, Error)]
#[error("Graphics drivers lost context")]
pub struct GraphicsDriversLostContext {}

#[async_trait(?Send)]
pub trait WindowOps {
    /// Show a hidden window
    fn show(&self);

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized;

    /// Setup opengl for rendering
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>>;
    /// Advise the window that a frame is finished
    fn finish_frame(&self, frame: glium::Frame) -> anyhow::Result<()> {
        frame.finish()?;
        Ok(())
    }

    /// Hide a visible window
    fn hide(&self);

    /// Schedule the window to be closed
    fn close(&self);

    /// Change the cursor
    fn set_cursor(&self, cursor: Option<MouseCursor>);

    /// Invalidate the window so that the entire client area will
    /// be repainted shortly
    fn invalidate(&self);

    /// Change the titlebar text for the window
    fn set_title(&self, title: &str);

    /// Resize the inner or client area of the window
    fn set_inner_size(&self, width: usize, height: usize);

    /// Requests the windowing system to start a window drag.
    ///
    /// This is only implemented on backends that handle
    /// window movement on the server side (Wayland).
    fn request_drag_move(&self) {}

    /// Signal to the windowing system that the mouse is over
    /// a window dragging area.
    ///
    /// This is only implemented on backends that need to
    /// know if the mouse is in a drag area to handle the
    /// click before forwarding the event (Windows).
    fn set_window_drag_position(&self, _coords: ScreenPoint) {}

    /// Changes the location of the window on the screen.
    /// The coordinates are of the top left pixel of the
    /// client area.
    ///
    /// This is only implemented on backends that allow
    /// windows to move themselves (not Wayland).
    fn set_window_position(&self, _coords: ScreenPoint) {}

    /// inform the windowing system of the current textual
    /// cursor input location.  This is used primarily for
    /// the platform specific input method editor
    fn set_text_cursor_position(&self, _cursor: Rect) {}

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String>;

    /// Set some text in the clipboard
    fn set_clipboard(&self, clipboard: Clipboard, text: String);

    /// Set the icon for the window.
    /// Depending on the system this may be shown in its titlebar
    /// and/or in the task manager/task switcher
    fn set_icon(&self, _image: Image) {}

    fn maximize(&self) {}
    fn restore(&self) {}

    fn toggle_fullscreen(&self) {}

    fn config_did_change(&self, _config: &config::ConfigHandle) {}

    /// Configure the Window so that the desktop environment
    /// will constrain resizes so that they are multiples of
    /// the x and y values specified.
    /// This may not be supported or respected by the desktop
    /// environment.
    fn set_resize_increments(&self, _x: u16, _y: u16) {}

    fn get_os_parameters(
        &self,
        _config: &ConfigHandle,
        _window_state: WindowState,
    ) -> anyhow::Result<Option<os::parameters::Parameters>> {
        Ok(None)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RequestedWindowGeometry {
    pub width: Dimension,
    pub height: Dimension,
    pub x: Option<Dimension>,
    pub y: Option<Dimension>,
    /// Specifies basis for evaluating x/y coords.
    /// Also applies to width/height when computing % based dimensions
    pub origin: GeometryOrigin,
}

#[derive(Debug, Clone)]
pub struct ResolvedGeometry {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: usize,
    pub height: usize,
}
