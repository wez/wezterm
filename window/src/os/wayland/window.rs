// TODO: change this
#![allow(dead_code, unused)]

use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::anyhow;
use config::configuration;
use config::ConfigHandle;
use promise::Future;
use raw_window_handle::HasRawDisplayHandle;
use raw_window_handle::HasRawWindowHandle;
use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::DecorationMode;
use smithay_client_toolkit::shell::xdg::window::WindowDecorations as Decorations;
use wayland_client::globals::GlobalList;
use wezterm_font::FontConfiguration;
use wezterm_input_types::WindowDecorations;

use crate::wayland::WaylandConnection;
use crate::Clipboard;
use crate::ConnectionOps;
use crate::MouseCursor;
use crate::RequestedWindowGeometry;
use crate::Window;
use crate::WindowEvent;
use crate::WindowOps;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WaylandWindow(usize);

impl WaylandWindow {
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
        log::trace!("Creating a window");
        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };

        let conn = WaylandConnection::get()
            .ok_or_else(|| {
                anyhow!(
                    "new_window must be called on the gui thread after Connection:init has succeed",
                )
            })?
            .wayland();

        let window_id = conn.next_window_id();
        // let pending_event = Arc::new(Mutex::new(PendingEvent::default()));

        // let (pending_first_configure, wait_configure) = async_channel::bounded(1);

        let qh = conn.event_queue.borrow().handle();
        let globals = conn.globals.borrow();

        let compositor = CompositorState::bind(&globals, &qh)?;
        let surface = compositor.create_surface(&qh);

        let xdg_shell = XdgShell::bind(&globals, &qh)?;
        let window = xdg_shell.create_window(
            surface,
            Decorations::RequestServer,
            &qh,
        );

        window.set_app_id(class_name.to_string());
        // TODO: investigate the resizable thing
        // window.set_resizable(true);
        window.set_title(name.to_string());
        let decorations = config.window_decorations;

        let decor_mode = if decorations == WindowDecorations::NONE {
            None
        } else if decorations == WindowDecorations::default() {
            Some(DecorationMode::Server)
        } else {
            Some(DecorationMode::Client)
        };
        window.request_decoration_mode(decor_mode);

        // TODO: I don't know anything about the frame thing
        //         window.set_frame_config(ConceptConfig {

        window.set_min_size(Some((32, 32)));

        // 
        // TODO:
        // let copy_and_paste = CopyAndPaste::create();
        // let pending_mouse = PendingMouse::create(window_id, &copy_and_paste);

        // conn.pointer.borrow().add_window(&surface, &pending_mouse);

        // TODO: WindowInner

        let window_handle = Window::Wayland(WaylandWindow(window_id));

        // TODO: assign window inner
        //


        // window.set_decorate(if decorations == WindowDecorations::NONE {
        //     Decorations::None
        // } else if decorations == WindowDecorations::default() {
        //     Decorations::FollowServer
        // } else {
        //     // SCTK/Wayland don't allow more nuance than "decorations are hidden",
        //     // so if we have a mixture of things, then we need to force our
        //     // client side decoration rendering.
        //     Decorations::ClientSide
        // });

        window.commit();

        Ok(window_handle)
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

#[derive(Default, Clone, Debug)]
struct PendingEvent {
    close: bool,
    had_configure_event: bool,
    configure: Option<(u32, u32)>,
    dpi: Option<i32>,
    // window_state: Option<WindowState>,
}
