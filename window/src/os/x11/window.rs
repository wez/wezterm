use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::os::xkeysyms;
use crate::os::{Connection, Window};
use crate::{
    Appearance, Clipboard, DeadKeyStatus, Dimensions, MouseButtons, MouseCursor, MouseEvent,
    MouseEventKind, MousePress, Point, Rect, ScreenPoint, WindowDecorations, WindowEvent,
    WindowEventSender, WindowOps, WindowState,
};
use anyhow::{anyhow, Context as _};
use async_trait::async_trait;
use config::ConfigHandle;
use promise::{Future, Promise};
use raw_window_handle::unix::XcbHandle;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::any::Any;
use std::convert::TryInto;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use wezterm_font::FontConfiguration;
use wezterm_input_types::{KeyCode, KeyEvent, Modifiers};
use xcb_util::icccm::SizeHints;

#[derive(Default)]
struct CopyAndPaste {
    clipboard_owned: Option<String>,
    primary_selection_owned: Option<String>,
    clipboard_request: Option<Promise<String>>,
    selection_request: Option<Promise<String>>,
    time: u32,
}

impl CopyAndPaste {
    fn clipboard(&self, clipboard: Clipboard) -> &Option<String> {
        match clipboard {
            Clipboard::PrimarySelection => &self.primary_selection_owned,
            Clipboard::Clipboard => &self.clipboard_owned,
        }
    }

    fn clipboard_mut(&mut self, clipboard: Clipboard) -> &mut Option<String> {
        match clipboard {
            Clipboard::PrimarySelection => &mut self.primary_selection_owned,
            Clipboard::Clipboard => &mut self.clipboard_owned,
        }
    }

    fn request_mut(&mut self, clipboard: Clipboard) -> &mut Option<Promise<String>> {
        match clipboard {
            Clipboard::PrimarySelection => &mut self.selection_request,
            Clipboard::Clipboard => &mut self.clipboard_request,
        }
    }
}

pub(crate) struct XWindowInner {
    window_id: xcb::xproto::Window,
    conn: Weak<XConnection>,
    events: WindowEventSender,
    width: u16,
    height: u16,
    dpi: f64,
    cursors: CursorInfo,
    copy_and_paste: CopyAndPaste,
    config: ConfigHandle,
    appearance: Appearance,
    title: String,
    has_focus: bool,
    last_cursor_position: Rect,
    invalidated: bool,
    paint_throttled: bool,
    pending: Vec<WindowEvent>,
}

impl Drop for XWindowInner {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.upgrade() {
            xcb::destroy_window(conn.conn(), self.window_id);
        }
    }
}

unsafe impl HasRawWindowHandle for XWindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Xcb(XcbHandle {
            window: self.window_id,
            connection: self.conn.upgrade().unwrap().get_raw_conn() as *mut _,
            ..XcbHandle::empty()
        })
    }
}

impl XWindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let conn = self.conn();

        let gl_state = match conn.gl_connection.borrow().as_ref() {
            None => crate::egl::GlState::create(
                Some(conn.conn.get_raw_dpy() as *const _),
                self.window_id as *mut _,
            ),
            Some(glconn) => crate::egl::GlState::create_with_existing_connection(
                glconn,
                self.window_id as *mut _,
            ),
        };

        // Don't chain on the end of the above to avoid borrowing gl_connection twice.
        let gl_state = gl_state.map(Rc::new).and_then(|state| unsafe {
            conn.gl_connection
                .borrow_mut()
                .replace(Rc::clone(state.get_connection()));
            Ok(glium::backend::Context::new(
                Rc::clone(&state),
                true,
                if cfg!(debug_assertions) {
                    glium::debug::DebugCallbackBehavior::DebugMessageOnError
                } else {
                    glium::debug::DebugCallbackBehavior::Ignore
                },
            )?)
        })?;

        Ok(gl_state)
    }

    /// Add a region to the list of exposed/damaged/dirty regions.
    /// Note that a window resize will likely invalidate the entire window.
    /// If the new region intersects with the prior region, then we expand
    /// it to encompass both.  This avoids bloating the list with a series
    /// of increasing rectangles when resizing larger or smaller.
    fn expose(&mut self, _x: u16, _y: u16, _width: u16, _height: u16) {
        self.queue_pending(WindowEvent::NeedRepaint);
    }

    fn do_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        self.events.dispatch(WindowEvent::MouseEvent(event));
        Ok(())
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) -> anyhow::Result<()> {
        self.cursors.set_cursor(self.window_id, cursor)
    }

    fn check_dpi_and_synthesize_resize(&mut self) {
        let conn = self.conn();
        let dpi = conn.default_dpi();

        if dpi != self.dpi {
            log::trace!(
                "dpi changed from {} -> {}, so synthesize a resize",
                dpi,
                self.dpi
            );
            self.dpi = dpi;
            self.events.dispatch(WindowEvent::Resized {
                dimensions: Dimensions {
                    pixel_width: self.width as usize,
                    pixel_height: self.height as usize,
                    dpi: self.dpi as usize,
                },
                window_state: self.get_window_state().unwrap_or(WindowState::default()),
                live_resizing: false,
            });
        }
    }

    fn queue_pending(&mut self, event: WindowEvent) {
        self.pending.push(event);
    }

    pub fn dispatch_pending_events(&mut self) -> anyhow::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }

        let mut need_paint = false;
        let mut resize = None;

        for event in self.pending.drain(..) {
            match event {
                WindowEvent::NeedRepaint => {
                    if need_paint {
                        log::trace!("coalesce a repaint");
                    }
                    need_paint = true;
                }
                e @ WindowEvent::Resized { .. } => {
                    if resize.is_some() {
                        log::trace!("coalesce a resize");
                    }
                    resize.replace(e);
                }
                e => {
                    self.events.dispatch(e);
                }
            }
        }

        if let Some(resize) = resize.take() {
            self.events.dispatch(resize);
        }

        if need_paint {
            if self.paint_throttled {
                self.invalidated = true;
            } else {
                self.invalidated = false;
                self.events.dispatch(WindowEvent::NeedRepaint);

                self.paint_throttled = true;
                let window_id = self.window_id;
                let max_fps = self.config.max_fps;
                promise::spawn::spawn(async move {
                    async_io::Timer::after(std::time::Duration::from_millis(1000 / max_fps as u64))
                        .await;
                    XConnection::with_window_inner(window_id, |inner| {
                        inner.paint_throttled = false;
                        if inner.invalidated {
                            inner.invalidate();
                        }
                        Ok(())
                    });
                })
                .detach();
            }
        }

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: &xcb::GenericEvent) -> anyhow::Result<()> {
        let r = event.response_type() & 0x7f;
        let conn = self.conn();
        match r {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
                self.expose(expose.x(), expose.y(), expose.width(), expose.height());
            }
            xcb::CONFIGURE_NOTIFY => {
                self.update_ime_position();

                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                let width = cfg.width();
                let height = cfg.height();
                let dpi = conn.default_dpi();

                if width == self.width && height == self.height && dpi == self.dpi {
                    // Effectively unchanged; perhaps it was simply moved?
                    // Do nothing!
                    return Ok(());
                }

                self.width = width;
                self.height = height;
                self.dpi = dpi;

                let dimensions = Dimensions {
                    pixel_width: self.width as usize,
                    pixel_height: self.height as usize,
                    dpi: self.dpi as usize,
                };

                self.queue_pending(WindowEvent::Resized {
                    dimensions,
                    window_state: self.get_window_state().unwrap_or(WindowState::default()),
                    // Assume that we're live resizing: we don't know for sure,
                    // but it seems like a reasonable assumption
                    live_resizing: true,
                });
            }
            xcb::KEY_PRESS | xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                self.copy_and_paste.time = key_press.time();
                conn.keyboard.process_key_event(key_press, &mut self.events);
            }

            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };

                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    coords: Point::new(
                        motion.event_x().try_into().unwrap(),
                        motion.event_y().try_into().unwrap(),
                    ),
                    screen_coords: ScreenPoint::new(
                        motion.root_x().try_into().unwrap(),
                        motion.root_y().try_into().unwrap(),
                    ),
                    modifiers: xkeysyms::modifiers_from_state(motion.state()),
                    mouse_buttons: MouseButtons::default(),
                };
                self.do_mouse_event(event)?;
            }
            xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };
                self.copy_and_paste.time = button_press.time();

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

                        // Ideally this would be configurable, but it's currently a bit
                        // awkward to configure this layer, so let's just improve the
                        // default for now!
                        const LINES_PER_TICK: i16 = 5;

                        MouseEventKind::VertWheel(if b == 4 {
                            LINES_PER_TICK
                        } else {
                            -LINES_PER_TICK
                        })
                    }
                    _ => {
                        eprintln!("button {} is not implemented", button_press.detail());
                        return Ok(());
                    }
                };

                let event = MouseEvent {
                    kind,
                    coords: Point::new(
                        button_press.event_x().try_into().unwrap(),
                        button_press.event_y().try_into().unwrap(),
                    ),
                    screen_coords: ScreenPoint::new(
                        button_press.root_x().try_into().unwrap(),
                        button_press.root_y().try_into().unwrap(),
                    ),
                    modifiers: xkeysyms::modifiers_from_state(button_press.state()),
                    mouse_buttons: MouseButtons::default(),
                };
                self.do_mouse_event(event)?;
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };

                if msg.data().data32()[0] == conn.atom_delete() {
                    self.events.dispatch(WindowEvent::CloseRequested);
                }
            }
            xcb::DESTROY_NOTIFY => {
                self.events.dispatch(WindowEvent::Destroyed);
                conn.windows.borrow_mut().remove(&self.window_id);
            }
            xcb::SELECTION_CLEAR => {
                self.selection_clear(unsafe { xcb::cast_event(event) })?;
            }
            xcb::SELECTION_REQUEST => {
                self.selection_request(unsafe { xcb::cast_event(event) })?;
            }
            xcb::SELECTION_NOTIFY => {
                self.selection_notify(unsafe { xcb::cast_event(event) })?;
            }
            xcb::PROPERTY_NOTIFY => {
                let msg: &xcb::PropertyNotifyEvent = unsafe { xcb::cast_event(event) };

                /*
                if let Ok(reply) = xcb::xproto::get_atom_name(&conn, msg.atom()).get_reply() {
                    log::info!(
                        "PropertyNotifyEvent atom={} {} xsel={}",
                        msg.atom(),
                        reply.name(),
                        conn.atom_xsel_data
                    );
                }
                */

                log::trace!(
                    "PropertyNotifyEvent atom={} xsel={}",
                    msg.atom(),
                    conn.atom_xsel_data
                );

                if msg.atom() == conn.atom_gtk_edge_constraints {
                    // "_GTK_EDGE_CONSTRAINTS" property is changed when the
                    // accessibility settings change the text size and thus
                    // the dpi.  We use this as a way to detect dpi changes
                    // when running under gnome.
                    conn.update_xrm();
                    self.check_dpi_and_synthesize_resize();
                    let appearance = conn.get_appearance();
                    if appearance != self.appearance {
                        self.appearance = appearance;
                        self.events
                            .dispatch(WindowEvent::AppearanceChanged(appearance));
                    }
                }
            }
            xcb::FOCUS_IN => {
                self.has_focus = true;
                self.update_ime_position();
                log::trace!("Calling focus_change(true)");
                self.events.dispatch(WindowEvent::FocusChanged(true));
            }
            xcb::FOCUS_OUT => {
                self.has_focus = false;
                log::trace!("Calling focus_change(false)");
                self.events.dispatch(WindowEvent::FocusChanged(false));
            }
            _ => {
                eprintln!("unhandled: {:x}", r);
            }
        }

        Ok(())
    }

    pub fn dispatch_ime_compose_status(&mut self, status: DeadKeyStatus) {
        self.events
            .dispatch(WindowEvent::AdviseDeadKeyStatus(status));
    }

    pub fn dispatch_ime_text(&mut self, text: &str) {
        let key_event = KeyEvent {
            key: KeyCode::Composed(text.into()),
            modifiers: Modifiers::NONE,
            repeat_count: 1,
            key_is_down: true,
            raw: None,
        }
        .normalize_shift();
        self.events.dispatch(WindowEvent::KeyEvent(key_event));
    }

    /// If we own the selection, make sure that the X server reflects
    /// that and vice versa.
    fn update_selection_owner(&mut self, clipboard: Clipboard) {
        let conn = self.conn();
        let selection = match clipboard {
            Clipboard::PrimarySelection => xcb::ATOM_PRIMARY,
            Clipboard::Clipboard => conn.atom_clipboard,
        };
        let current_owner = xcb::get_selection_owner(&conn, selection)
            .get_reply()
            .unwrap()
            .owner();
        if self.copy_and_paste.clipboard(clipboard).is_none() && current_owner == self.window_id {
            // We don't have a selection but X thinks we do; disown it!
            xcb::set_selection_owner(&conn, xcb::NONE, selection, self.copy_and_paste.time);
        } else if self.copy_and_paste.clipboard(clipboard).is_some()
            && current_owner != self.window_id
        {
            // We have the selection but X doesn't think we do; assert it!
            xcb::set_selection_owner(&conn, self.window_id, selection, self.copy_and_paste.time);
        }
        conn.flush();
    }

    fn selection_atom_to_clipboard(&self, atom: xcb::Atom) -> Option<Clipboard> {
        if atom == xcb::ATOM_PRIMARY {
            Some(Clipboard::PrimarySelection)
        } else if atom == self.conn().atom_clipboard {
            Some(Clipboard::Clipboard)
        } else {
            None
        }
    }

    fn selection_clear(&mut self, request: &xcb::SelectionClearEvent) -> anyhow::Result<()> {
        if let Some(clipboard) = self.selection_atom_to_clipboard(request.selection()) {
            self.copy_and_paste.clipboard_mut(clipboard).take();
            self.copy_and_paste.request_mut(clipboard).take();
            self.update_selection_owner(clipboard);
        }

        Ok(())
    }

    /// A selection request is made to us after we've announced that we own the selection
    /// and when another client wants to copy it.
    fn selection_request(&mut self, request: &xcb::SelectionRequestEvent) -> anyhow::Result<()> {
        let conn = self.conn();
        log::trace!(
            "SEL: time={} owner={} requestor={} selection={} target={} property={}",
            request.time(),
            request.owner(),
            request.requestor(),
            request.selection(),
            request.target(),
            request.property()
        );
        log::trace!(
            "XSEL={}, UTF8={} PRIMARY={} clip={}",
            conn.atom_xsel_data,
            conn.atom_utf8_string,
            xcb::ATOM_PRIMARY,
            conn.atom_clipboard,
        );

        let selprop = if request.target() == conn.atom_targets {
            // They want to know which targets we support
            let atoms: [u32; 1] = [conn.atom_utf8_string];
            xcb::xproto::change_property(
                &conn,
                xcb::xproto::PROP_MODE_REPLACE as u8,
                request.requestor(),
                request.property(),
                xcb::xproto::ATOM_ATOM,
                32, /* 32-bit atom value */
                &atoms,
            );

            // let the requestor know that we set their property
            request.property()
        } else if request.target() == conn.atom_utf8_string
            || request.target() == xcb::xproto::ATOM_STRING
        {
            if let Some(clipboard) = self.selection_atom_to_clipboard(request.selection()) {
                // We'll accept requests for UTF-8 or STRING data.
                // We don't and won't do any conversion from UTF-8 to
                // whatever STRING represents; let's just assume that
                // the other end is going to handle it correctly.
                if let Some(text) = self.copy_and_paste.clipboard(clipboard) {
                    xcb::xproto::change_property(
                        &conn,
                        xcb::xproto::PROP_MODE_REPLACE as u8,
                        request.requestor(),
                        request.property(),
                        request.target(),
                        8, /* 8-bit string data */
                        text.as_bytes(),
                    );
                    // let the requestor know that we set their property
                    request.property()
                } else {
                    // We have no clipboard so there is nothing to report
                    xcb::NONE
                }
            } else {
                xcb::NONE
            }
        } else {
            // We didn't support their request, so there is nothing
            // we can report back to them.
            xcb::NONE
        };
        log::trace!("responding with selprop={}", selprop);

        xcb::xproto::send_event(
            &conn,
            true,
            request.requestor(),
            0,
            &xcb::xproto::SelectionNotifyEvent::new(
                request.time(),
                request.requestor(),
                request.selection(),
                request.target(),
                selprop, // the disposition from the operation above
            ),
        );

        Ok(())
    }

    fn selection_notify(&mut self, selection: &xcb::SelectionNotifyEvent) -> anyhow::Result<()> {
        let conn = self.conn();

        log::trace!(
            "SELECTION_NOTIFY received selection={} (prim={} clip={}) target={} property={}",
            selection.selection(),
            xcb::ATOM_PRIMARY,
            conn.atom_clipboard,
            selection.target(),
            selection.property()
        );

        if let Some(clipboard) = self.selection_atom_to_clipboard(selection.selection()) {
            if selection.property() != xcb::NONE {
                match xcb_util::icccm::get_text_property(
                    &conn,
                    selection.requestor(),
                    selection.property(),
                )
                .get_reply()
                {
                    Ok(prop) => {
                        if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take()
                        {
                            promise.ok(prop.name().to_owned());
                        }
                        xcb::delete_property(&conn, self.window_id, conn.atom_xsel_data);
                    }
                    Err(err) => {
                        log::error!("clipboard: err while getting clipboard property: {:?}", err);
                    }
                }
            } else if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take() {
                promise.ok("".to_owned());
            }
        }
        Ok(())
    }

    fn get_window_state(&self) -> anyhow::Result<WindowState> {
        let conn = self.conn();

        let reply = xcb::xproto::get_property(
            &conn,
            false,
            self.window_id,
            conn.atom_net_wm_state,
            xcb::xproto::ATOM_ATOM,
            0,
            1024,
        )
        .get_reply()?;

        let state = reply.value::<u32>();
        let mut window_state = WindowState::default();

        for &s in state {
            if s == conn.atom_state_fullscreen {
                window_state |= WindowState::FULL_SCREEN;
            } else if s == conn.atom_state_maximized_vert || s == conn.atom_state_maximized_horz {
                window_state |= WindowState::MAXIMIZED;
            } else if s == conn.atom_state_hidden {
                window_state |= WindowState::HIDDEN;
            }
        }

        Ok(window_state)
    }

    fn set_fullscreen_hint(&mut self, enable: bool) -> anyhow::Result<()> {
        let conn = self.conn();

        let net_wm_state = xcb::intern_atom(conn.conn(), false, "_NET_WM_STATE")
            .get_reply()?
            .atom();
        let net_wm_state_fullscreen =
            xcb::intern_atom(conn.conn(), false, "_NET_WM_STATE_FULLSCREEN")
                .get_reply()?
                .atom();

        let data: [u32; 5] = [if enable { 1 } else { 0 }, net_wm_state_fullscreen, 0, 0, 0];

        // Ask window manager to change our fullscreen state
        xcb::xproto::send_event(
            &conn,
            true,
            conn.root,
            xcb::xproto::EVENT_MASK_SUBSTRUCTURE_REDIRECT
                | xcb::xproto::EVENT_MASK_SUBSTRUCTURE_NOTIFY,
            &xcb::xproto::ClientMessageEvent::new(
                32,
                self.window_id,
                net_wm_state,
                xcb::ClientMessageData::from_data32(data),
            ),
        );
        self.adjust_decorations(self.config.window_decorations)?;

        Ok(())
    }

    #[allow(clippy::identity_op)]
    fn adjust_decorations(&mut self, decorations: WindowDecorations) -> anyhow::Result<()> {
        // Set the motif hints to disable decorations.
        // See https://stackoverflow.com/a/1909708
        #[repr(C)]
        struct MwmHints {
            flags: u32,
            functions: u32,
            decorations: u32,
            input_mode: i32,
            status: u32,
        }

        const HINTS_DECORATIONS: u32 = 1 << 1;
        const FUNC_ALL: u32 = 1 << 0;
        const FUNC_RESIZE: u32 = 1 << 1;
        // const HINTS_FUNCTIONS: u32 = 1 << 0;
        const FUNC_MOVE: u32 = 1 << 2;
        const FUNC_MINIMIZE: u32 = 1 << 3;
        const FUNC_MAXIMIZE: u32 = 1 << 4;
        const FUNC_CLOSE: u32 = 1 << 5;

        let decorations = if decorations == WindowDecorations::TITLE | WindowDecorations::RESIZE {
            FUNC_ALL
        } else if decorations == WindowDecorations::RESIZE {
            FUNC_RESIZE
        } else if decorations == WindowDecorations::TITLE {
            FUNC_MOVE | FUNC_MINIMIZE | FUNC_MAXIMIZE | FUNC_CLOSE
        } else if decorations == WindowDecorations::NONE {
            0
        } else {
            FUNC_ALL
        };

        let hints = MwmHints {
            flags: HINTS_DECORATIONS,
            functions: 0,
            decorations,
            input_mode: 0,
            status: 0,
        };

        let conn = self.conn();

        let hints_slice =
            unsafe { std::slice::from_raw_parts(&hints as *const _ as *const u32, 5) };

        let atom = xcb::intern_atom(conn.conn(), false, "_MOTIF_WM_HINTS")
            .get_reply()?
            .atom();
        xcb::change_property(
            conn.conn(),
            xcb::PROP_MODE_REPLACE as u8,
            self.window_id,
            atom,
            atom,
            32,
            hints_slice,
        );
        Ok(())
    }

    fn conn(&self) -> Rc<XConnection> {
        self.conn.upgrade().expect("XConnection to be alive")
    }
}

/// A Window!
#[derive(Debug, Clone)]
pub struct XWindow(xcb::xproto::Window);

impl XWindow {
    pub(crate) fn from_id(id: xcb::xproto::Window) -> Self {
        Self(id)
    }

    /// Create a new window on the specified screen with the specified
    /// dimensions
    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        config: Option<&ConfigHandle>,
        _font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };
        let conn = Connection::get()
            .ok_or_else(|| {
                anyhow!(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .x11();

        let mut events = WindowEventSender::new(event_handler);

        let window_id;
        let window = {
            let setup = conn.conn().get_setup();
            let screen = setup
                .roots()
                .nth(conn.screen_num() as usize)
                .ok_or_else(|| anyhow!("no screen?"))?;

            window_id = conn.conn().generate_id();

            let color_map_id = conn.conn().generate_id();
            xcb::create_colormap_checked(
                conn.conn(),
                xcb::COLORMAP_ALLOC_NONE as _,
                color_map_id,
                screen.root(),
                conn.visual.visual_id(),
            )
            .request_check()
            .context("create_colormap_checked")?;

            xcb::create_window_checked(
                conn.conn(),
                conn.depth,
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
                &[
                    (
                        xcb::CW_EVENT_MASK,
                        xcb::EVENT_MASK_EXPOSURE
                            | xcb::EVENT_MASK_FOCUS_CHANGE
                            | xcb::EVENT_MASK_KEY_PRESS
                            | xcb::EVENT_MASK_BUTTON_PRESS
                            | xcb::EVENT_MASK_BUTTON_RELEASE
                            | xcb::EVENT_MASK_POINTER_MOTION
                            | xcb::EVENT_MASK_BUTTON_MOTION
                            | xcb::EVENT_MASK_KEY_RELEASE
                            | xcb::EVENT_MASK_PROPERTY_CHANGE
                            | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                    ),
                    // We have to specify both a border pixel color and a colormap
                    // when specifying a depth that doesn't match the root window in
                    // order to avoid a BadMatch
                    (xcb::CW_BORDER_PIXEL, 0),
                    (xcb::CW_COLORMAP, color_map_id),
                ],
            )
            .request_check()
            .context("xcb::create_window_checked")?;

            events.assign_window(Window::X11(XWindow::from_id(window_id)));

            let appearance = conn.get_appearance();

            Arc::new(Mutex::new(XWindowInner {
                title: String::new(),
                appearance,
                window_id,
                conn: Rc::downgrade(&conn),
                events,
                width: width.try_into()?,
                height: height.try_into()?,
                dpi: conn.default_dpi(),
                copy_and_paste: CopyAndPaste::default(),
                cursors: CursorInfo::new(&conn),
                config: config.clone(),
                has_focus: false,
                last_cursor_position: Rect::default(),
                paint_throttled: false,
                invalidated: false,
                pending: vec![],
            }))
        };

        xcb_util::icccm::set_wm_class(&*conn, window_id, class_name, class_name);
        xcb_util::ewmh::set_wm_pid(conn.ewmh_conn(), window_id, unsafe {
            libc::getpid() as u32
        });

        xcb::change_property(
            &*conn,
            xcb::PROP_MODE_REPLACE as u8,
            window_id,
            conn.atom_protocols,
            4,
            32,
            &[conn.atom_delete],
        );

        window
            .lock()
            .unwrap()
            .adjust_decorations(config.window_decorations)?;

        let window_handle = Window::X11(XWindow::from_id(window_id));

        conn.windows.borrow_mut().insert(window_id, window);

        window_handle.set_title(name);
        window_handle.show();

        Ok(window_handle)
    }
}

impl XWindowInner {
    fn close(&mut self) {
        // Remove the window from the map now, as GL state
        // requires that it is able to make_current() in its
        // Drop impl, and that cannot succeed after we've
        // destroyed the window at the X11 level.
        self.conn().windows.borrow_mut().remove(&self.window_id);
        xcb::destroy_window(self.conn().conn(), self.window_id);
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        xcb::map_window(self.conn().conn(), self.window_id);
    }

    fn invalidate(&mut self) {
        self.queue_pending(WindowEvent::NeedRepaint);
        self.dispatch_pending_events().ok();
    }

    fn toggle_fullscreen(&mut self) {
        let fullscreen = match self.get_window_state() {
            Ok(f) => f.contains(WindowState::FULL_SCREEN),
            Err(err) => {
                log::error!("Failed to determine fullscreen state: {}", err);
                return;
            }
        };
        self.set_fullscreen_hint(!fullscreen).ok();
    }

    fn config_did_change(&mut self, config: &ConfigHandle) {
        self.config = config.clone();
        let _ = self.adjust_decorations(config.window_decorations);
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        // We ask the window manager to move the window for us so that
        // we don't have to deal with adjusting for the frame size.
        // Note that neither this technique or the configure_window
        // approach below will successfully move a window running
        // under the crostini environment on a chromebook :-(
        let conn = self.conn();
        xcb_util::ewmh::request_move_resize_window(
            conn.ewmh_conn(),
            conn.screen_num,
            self.window_id,
            xcb::xproto::GRAVITY_STATIC,
            1, // normal program
            xcb_util::ewmh::MOVE_RESIZE_MOVE
                | xcb_util::ewmh::MOVE_RESIZE_WINDOW_X
                | xcb_util::ewmh::MOVE_RESIZE_WINDOW_Y,
            coords.x as u32,
            coords.y as u32,
            self.width as u32,
            self.height as u32,
        );
    }

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        if title == self.title {
            return;
        }
        self.title = title.to_string();
        // Ideally, we'd simply call this:
        // xcb_util::icccm::set_wm_name(self.conn().conn(), self.window_id, title);
        // However, it uses ATOM_STRING internally, rather than UTF8_STRING
        // and will mangle non-ascii characters in the title, so we call the
        // underlying function for ourslves:

        unsafe {
            xcb_util::ffi::icccm::xcb_icccm_set_wm_name(
                self.conn().conn().get_raw_conn(),
                self.window_id,
                self.conn().atom_utf8_string,
                8,
                title.len() as u32,
                title.as_bytes().as_ptr() as *const _,
            );
        }
    }

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        if self.last_cursor_position == cursor {
            return;
        }
        self.last_cursor_position = cursor;
        self.update_ime_position();
    }

    fn update_ime_position(&mut self) {
        if !self.has_focus {
            return;
        }
        self.conn().ime.borrow_mut().update_pos(
            self.window_id,
            self.last_cursor_position.min_x() as i16,
            (self.last_cursor_position.max_y() + self.last_cursor_position.height()) as i16,
        );
    }

    fn set_icon(&mut self, image: &dyn BitmapImage) {
        let (width, height) = image.image_dimensions();

        // https://specifications.freedesktop.org/wm-spec/wm-spec-1.3.html#idm44927025355360
        // says that this is an array of 32bit ARGB data.
        // The first two elements are width, height, with the remainder
        // being the the row data, left-to-right, top-to-bottom.
        let mut icon_data = Vec::with_capacity((2 + (width * height)) * 4);
        icon_data.push(width as u32);
        icon_data.push(height as u32);
        icon_data.extend_from_slice(image.pixels());

        xcb_util::ewmh::set_wm_icon(
            self.conn().ewmh_conn(),
            xcb::PROP_MODE_REPLACE as u8,
            self.window_id,
            &icon_data,
        );
    }

    fn set_resize_increments(&mut self, x: u16, y: u16) -> anyhow::Result<()> {
        let mut hints = SizeHints::empty();
        hints = hints.resize(x.into(), y.into());
        xcb_util::icccm::set_wm_normal_hints(&self.conn(), self.window_id, &hints.build());

        Ok(())
    }
}

unsafe impl HasRawWindowHandle for XWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let conn = Connection::get().expect("raw_window_handle only callable on main thread");
        let handle = conn
            .x11()
            .window_by_id(self.0)
            .expect("window handle invalid!?");

        let inner = handle.lock().unwrap();
        inner.raw_window_handle()
    }
}

#[async_trait(?Send)]
impl WindowOps for XWindow {
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let window = self.0;
        promise::spawn::spawn(async move {
            if let Some(handle) = Connection::get().unwrap().x11().window_by_id(window) {
                let mut inner = handle.lock().unwrap();
                inner.enable_opengl()
            } else {
                anyhow::bail!("invalid window");
            }
        })
        .await
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        XConnection::with_window_inner(self.0, move |inner| {
            inner
                .events
                .dispatch(WindowEvent::Notification(Box::new(t)));
            Ok(())
        });
    }

    fn close(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        });
    }

    fn hide(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        });
    }

    fn toggle_fullscreen(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.toggle_fullscreen();
            Ok(())
        });
    }

    fn config_did_change(&self, config: &ConfigHandle) {
        let config = config.clone();
        XConnection::with_window_inner(self.0, move |inner| {
            inner.config_did_change(&config);
            Ok(())
        });
    }

    fn show(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        });
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        XConnection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        });
    }

    fn invalidate(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        });
    }

    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        });
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        XConnection::with_window_inner(self.0, move |inner| {
            xcb::configure_window(
                inner.conn().conn(),
                inner.window_id,
                &[
                    (xcb::CONFIG_WINDOW_WIDTH as u16, width as u32),
                    (xcb::CONFIG_WINDOW_HEIGHT as u16, height as u32),
                ],
            );
            Ok(())
        });
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        });
    }

    fn set_text_cursor_position(&self, cursor: Rect) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        });
    }

    fn set_icon(&self, image: Image) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_icon(&image);
            Ok(())
        });
    }

    fn set_resize_increments(&self, x: u16, y: u16) {
        XConnection::with_window_inner(self.0, move |inner| {
            if let Err(err) = inner.set_resize_increments(x, y) {
                log::error!("set_resize_increments failed: {:#}", err);
            }
            Ok(())
        });
    }

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let mut promise = Some(promise);
        XConnection::with_window_inner(self.0, move |inner| {
            let mut promise = promise.take().unwrap();
            if let Some(text) = inner.copy_and_paste.clipboard(clipboard) {
                promise.ok(text.to_owned());

                // Cancel any outstanding promise from the other branch
                // below.
                inner.copy_and_paste.request_mut(clipboard).take();
            } else {
                log::debug!("prepare promise, time={}", inner.copy_and_paste.time);
                inner.copy_and_paste.request_mut(clipboard).replace(promise);
                let conn = inner.conn();
                // Find the owner and ask them to send us the buffer
                xcb::convert_selection(
                    &conn,
                    inner.window_id,
                    // Note that under xwayland, access to the primary selection is
                    // forbidden by default citing a security concern.
                    match clipboard {
                        Clipboard::Clipboard => conn.atom_clipboard,
                        Clipboard::PrimarySelection => xcb::ATOM_PRIMARY,
                    },
                    conn.atom_utf8_string,
                    conn.atom_xsel_data,
                    inner.copy_and_paste.time,
                );
            }
            Ok(())
        });

        future
    }

    /// Set some text in the clipboard
    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner
                .copy_and_paste
                .clipboard_mut(clipboard)
                .replace(text.clone());
            inner.update_selection_owner(clipboard);
            Ok(())
        });
    }
}
