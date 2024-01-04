// TODO: change this
#![allow(dead_code, unused)]

use std::any::Any;
use std::rc::Rc;

use config::ConfigHandle;
use promise::Future;
use raw_window_handle::HasRawDisplayHandle;
use raw_window_handle::HasRawWindowHandle;
use wezterm_font::FontConfiguration;

use crate::Clipboard;
use crate::MouseCursor;
use crate::RequestedWindowGeometry;
use crate::Window;
use crate::WindowEvent;
use crate::WindowOps;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
    pub async fn new_window<F>(
        _class_name: &str,
        _name: &str,
        _geometry: RequestedWindowGeometry,
        _config: Option<&ConfigHandle>,
        _font_config: Rc<FontConfiguration>,
        _event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        log::debug!("Creating a window");
        todo!("WaylandWindow::new_window")
    }
}

impl WindowOps for WaylandWindow {
    #[doc = r" Show a hidden window"]
    fn show(&self) {
        todo!()
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        todo!()
    }

    #[doc = r" Setup opengl for rendering"]
    #[must_use]
    #[allow(clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn enable_opengl<'life0, 'async_trait>(
        &'life0 self,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = anyhow::Result<Rc<glium::backend::Context>>>
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    #[doc = r" Hide a visible window"]
    fn hide(&self) {
        todo!()
    }

    #[doc = r" Schedule the window to be closed"]
    fn close(&self) {
        todo!()
    }

    #[doc = r" Change the cursor"]
    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        todo!()
    }

    #[doc = r" Invalidate the window so that the entire client area will"]
    #[doc = r" be repainted shortly"]
    fn invalidate(&self) {
        todo!()
    }

    #[doc = r" Change the titlebar text for the window"]
    fn set_title(&self, title: &str) {
        todo!()
    }

    #[doc = r" Resize the inner or client area of the window"]
    fn set_inner_size(&self, width: usize, height: usize) {
        todo!()
    }

    #[doc = r" Initiate textual transfer from the clipboard"]
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        todo!()
    }

    #[doc = r" Set some text in the clipboard"]
    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        todo!()
    }
}

unsafe impl HasRawDisplayHandle for WaylandWindow {
    fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        todo!()
    }
}

unsafe impl HasRawWindowHandle for WaylandWindow {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        todo!()
    }
}
