#![cfg(all(unix, not(target_os = "macos")))]

use crate::connection::ConnectionOps;
#[cfg(feature = "wayland")]
use crate::os::wayland::connection::WaylandConnection;
#[cfg(feature = "wayland")]
use crate::os::wayland::window::WaylandWindow;
use crate::os::x11::connection::XConnection;
use crate::os::x11::window::XWindow;
use crate::screen::Screens;
use crate::{
    Appearance, Clipboard, MouseCursor, Rect, RequestedWindowGeometry, ScreenPoint, WindowEvent,
    WindowOps,
};
use async_trait::async_trait;
use config::ConfigHandle;
use promise::*;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use std::any::Any;
use std::rc::Rc;
use wezterm_font::FontConfiguration;

pub enum Connection {
    X11(Rc<XConnection>),
    #[cfg(feature = "wayland")]
    Wayland(Rc<WaylandConnection>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Window {
    X11(XWindow),
    #[cfg(feature = "wayland")]
    Wayland(WaylandWindow),
}

impl Connection {
    pub(crate) fn create_new() -> anyhow::Result<Connection> {
        #[cfg(feature = "wayland")]
        if config::configuration().enable_wayland {
            match WaylandConnection::create_new() {
                Ok(w) => {
                    log::debug!("Using wayland connection!");
                    return Ok(Connection::Wayland(Rc::new(w)));
                }
                Err(e) => {
                    log::debug!("Failed to init wayland: {}", e);
                }
            }
        }
        Ok(Connection::X11(XConnection::create_new()?))
    }

    pub async fn new_window<F>(
        &self,
        class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
        config: Option<&ConfigHandle>,
        font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        match self {
            Self::X11(_) => {
                XWindow::new_window(
                    class_name,
                    name,
                    geometry,
                    config,
                    font_config,
                    event_handler,
                )
                .await
            }
            #[cfg(feature = "wayland")]
            Self::Wayland(_) => {
                WaylandWindow::new_window(
                    class_name,
                    name,
                    geometry,
                    config,
                    font_config,
                    event_handler,
                )
                .await
            }
        }
    }

    pub(crate) fn x11(&self) -> Rc<XConnection> {
        match self {
            Self::X11(x) => Rc::clone(x),
            #[cfg(feature = "wayland")]
            _ => panic!("attempted to get x11 reference on non-x11 connection"),
        }
    }

    #[cfg(feature = "wayland")]
    pub(crate) fn wayland(&self) -> Rc<WaylandConnection> {
        match self {
            Self::Wayland(w) => Rc::clone(w),
            _ => panic!("attempted to get wayland reference on non-wayland connection"),
        }
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: Appearance) {
        log::trace!("Appearance changed to {appearance:?}");
        match self {
            Self::X11(x) => x.advise_of_appearance_change(appearance),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.advise_of_appearance_change(appearance),
        }
    }
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        match self {
            Self::X11(x) => x.terminate_message_loop(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.terminate_message_loop(),
        }
    }

    fn default_dpi(&self) -> f64 {
        match self {
            Self::X11(x) => x.default_dpi(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.default_dpi(),
        }
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        crate::os::xdg_desktop_portal::subscribe();
        match self {
            Self::X11(x) => x.run_message_loop(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.run_message_loop(),
        }
    }

    fn get_appearance(&self) -> Appearance {
        match self {
            Self::X11(x) => x.get_appearance(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.get_appearance(),
        }
    }

    fn beep(&self) {
        match self {
            Self::X11(x) => x.beep(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.beep(),
        }
    }

    fn screens(&self) -> anyhow::Result<Screens> {
        match self {
            Self::X11(x) => x.screens(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.screens(),
        }
    }
}

impl Window {
    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
        config: Option<&ConfigHandle>,
        font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        Connection::get()
            .unwrap()
            .new_window(
                class_name,
                name,
                geometry,
                config,
                font_config,
                event_handler,
            )
            .await
    }
}

unsafe impl HasRawDisplayHandle for Window {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        match self {
            Self::X11(x) => x.raw_display_handle(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.raw_display_handle(),
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self {
            Self::X11(x) => x.raw_window_handle(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.raw_window_handle(),
        }
    }
}

#[async_trait(?Send)]
impl WindowOps for Window {
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>> {
        match self {
            Self::X11(x) => x.enable_opengl().await,
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.enable_opengl().await,
        }
    }

    fn finish_frame(&self, frame: glium::Frame) -> anyhow::Result<()> {
        match self {
            Self::X11(x) => x.finish_frame(frame),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.finish_frame(frame),
        }
    }

    fn close(&self) {
        match self {
            Self::X11(x) => x.close(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.close(),
        }
    }
    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        match self {
            Self::X11(x) => x.notify(t),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.notify(t),
        }
    }

    fn hide(&self) {
        match self {
            Self::X11(x) => x.hide(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.hide(),
        }
    }

    fn toggle_fullscreen(&self) {
        match self {
            Self::X11(x) => x.toggle_fullscreen(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.toggle_fullscreen(),
        }
    }

    fn config_did_change(&self, config: &ConfigHandle) {
        match self {
            Self::X11(x) => x.config_did_change(config),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.config_did_change(config),
        }
    }

    fn show(&self) {
        match self {
            Self::X11(x) => x.show(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.show(),
        }
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        match self {
            Self::X11(x) => x.set_cursor(cursor),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_cursor(cursor),
        }
    }

    fn invalidate(&self) {
        match self {
            Self::X11(x) => x.invalidate(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.invalidate(),
        }
    }

    fn set_resize_increments(&self, x: u16, y: u16) {
        match self {
            Self::X11(x11) => x11.set_resize_increments(x, y),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_resize_increments(x, y),
        }
    }

    fn set_title(&self, title: &str) {
        match self {
            Self::X11(x) => x.set_title(title),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_title(title),
        }
    }

    fn set_icon(&self, image: crate::bitmaps::Image) {
        match self {
            Self::X11(x) => x.set_icon(image),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_icon(image),
        }
    }

    fn maximize(&self) {
        match self {
            Self::X11(x) => x.maximize(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.maximize(),
        }
    }

    fn restore(&self) {
        match self {
            Self::X11(x) => x.restore(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.restore(),
        }
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        match self {
            Self::X11(x) => x.set_inner_size(width, height),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_inner_size(width, height),
        }
    }

    fn request_drag_move(&self) {
        match self {
            Self::X11(x) => x.request_drag_move(),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.request_drag_move(),
        }
    }

    fn set_window_drag_position(&self, coords: ScreenPoint) {
        match self {
            Self::X11(x) => x.set_window_drag_position(coords),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_window_drag_position(coords),
        }
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        match self {
            Self::X11(x) => x.set_window_position(coords),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_window_position(coords),
        }
    }

    fn set_text_cursor_position(&self, cursor: Rect) {
        match self {
            Self::X11(x) => x.set_text_cursor_position(cursor),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_text_cursor_position(cursor),
        }
    }

    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        match self {
            Self::X11(x) => x.get_clipboard(clipboard),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.get_clipboard(clipboard),
        }
    }
    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        match self {
            Self::X11(x) => x.set_clipboard(clipboard, text),
            #[cfg(feature = "wayland")]
            Self::Wayland(w) => w.set_clipboard(clipboard, text),
        }
    }
}
