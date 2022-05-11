use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::os::{xkeysyms, Connection, Window};
use crate::{
    Appearance, Clipboard, DeadKeyStatus, Dimensions, MouseButtons, MouseCursor, MouseEvent,
    MouseEventKind, MousePress, Point, Rect, RequestedWindowGeometry, ScreenPoint,
    WindowDecorations, WindowEvent, WindowEventSender, WindowOps, WindowState,
};
use anyhow::{anyhow, Context as _};
use async_trait::async_trait;
use config::{ConfigHandle, DimensionContext, GeometryOrigin};
use promise::{Future, Promise};
use raw_window_handle::unix::XcbHandle;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::any::Any;
use std::collections::HashMap;
use std::convert::TryInto;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use wezterm_font::FontConfiguration;
use wezterm_input_types::{KeyCode, KeyEvent, Modifiers};
use xcb::x::{Atom, PropMode};
use xcb::{Event, Xid};

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
    window_id: xcb::x::Window,
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
    has_focus: Option<bool>,
    last_cursor_position: Rect,
    invalidated: bool,
    paint_throttled: bool,
    pending: Vec<WindowEvent>,
    dispatched_any_resize: bool,
}

impl Drop for XWindowInner {
    fn drop(&mut self) {
        if self.window_id != xcb::x::Window::none() {
            if let Some(conn) = self.conn.upgrade() {
                conn.send_request(&xcb::x::DestroyWindow {
                    window: self.window_id,
                });
            }
        }
    }
}

unsafe impl HasRawWindowHandle for XWindowInner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Xcb(XcbHandle {
            window: self.window_id.resource_id(),
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
                self.window_id.resource_id() as *mut _,
            ),
            Some(glconn) => crate::egl::GlState::create_with_existing_connection(
                glconn,
                self.window_id.resource_id() as *mut _,
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
            self.dispatched_any_resize = true;
            self.events.dispatch(resize);
        }

        if need_paint {
            if self.paint_throttled {
                self.invalidated = true;
            } else {
                self.invalidated = false;

                if self.has_focus.is_none() {
                    log::trace!(
                        "About to paint, but we've never received a FOCUS_IN/FOCUS_OUT \
                         event; querying WM to determine focus state"
                    );

                    let focus = self
                        .conn()
                        .wait_for_reply(self.conn().send_request(&xcb::x::GetInputFocus {}))?;
                    let focused = focus.focus() == self.window_id;
                    log::trace!("Do I have focus? {}", focused);
                    self.has_focus.replace(focused);
                    self.events.dispatch(WindowEvent::FocusChanged(focused));
                }

                if !self.dispatched_any_resize {
                    self.dispatched_any_resize = true;

                    log::trace!(
                        "About to paint, but we've never dispatched a Resized \
                         event, and thus never received a CONFIGURE_NOTIFY; \
                         querying WM for geometry"
                    );
                    let geom = self.conn().wait_for_reply(self.conn().send_request(
                        &xcb::x::GetGeometry {
                            drawable: xcb::x::Drawable::Window(self.window_id),
                        },
                    ))?;
                    log::trace!(
                        "geometry is {}x{} vs. our initial {}x{}",
                        geom.width(),
                        geom.height(),
                        self.width,
                        self.height
                    );

                    self.width = geom.width();
                    self.height = geom.height();

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

    fn button_event(
        &mut self,
        pressed: bool,
        time: xcb::x::Timestamp,
        detail: xcb::x::Button,
        event_x: i16,
        event_y: i16,
        root_x: i16,
        root_y: i16,
        state: xcb::x::KeyButMask,
    ) -> anyhow::Result<()> {
        self.copy_and_paste.time = time;

        let kind = match detail {
            b @ 1..=3 => {
                let button = match b {
                    1 => MousePress::Left,
                    2 => MousePress::Middle,
                    3 => MousePress::Right,
                    _ => unreachable!(),
                };
                if pressed {
                    MouseEventKind::Press(button)
                } else {
                    MouseEventKind::Release(button)
                }
            }
            b @ 4..=5 => {
                if !pressed {
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
                eprintln!("button {} is not implemented", detail);
                return Ok(());
            }
        };

        let event = MouseEvent {
            kind,
            coords: Point::new(event_x.try_into().unwrap(), event_y.try_into().unwrap()),
            screen_coords: ScreenPoint::new(root_x.try_into().unwrap(), root_y.try_into().unwrap()),
            modifiers: xkeysyms::modifiers_from_state(state.bits()),
            mouse_buttons: MouseButtons::default(),
        };
        self.do_mouse_event(event)
    }

    pub fn dispatch_event(&mut self, event: &Event) -> anyhow::Result<()> {
        let conn = self.conn();
        match event {
            Event::X(xcb::x::Event::Expose(expose)) => {
                self.expose(expose.x(), expose.y(), expose.width(), expose.height());
            }
            Event::X(xcb::x::Event::ConfigureNotify(cfg)) => {
                self.update_ime_position();

                let width = cfg.width();
                let height = cfg.height();
                let dpi = conn.default_dpi();

                if width == self.width && height == self.height && dpi == self.dpi {
                    // Effectively unchanged; perhaps it was simply moved?
                    // Do nothing!
                    log::trace!("Ignoring CONFIGURE_NOTIFY because width,height,dpi are unchanged");
                    return Ok(());
                }

                log::trace!(
                    "CONFIGURE_NOTIFY: width {} -> {}, height {} -> {}, dpi {} -> {}",
                    self.width,
                    width,
                    self.height,
                    height,
                    self.dpi,
                    dpi
                );

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
            Event::X(xcb::x::Event::KeyPress(key_press)) => {
                self.copy_and_paste.time = key_press.time();
                conn.keyboard
                    .process_key_press_event(key_press, &mut self.events);
            }
            Event::X(xcb::x::Event::KeyRelease(key_release)) => {
                self.copy_and_paste.time = key_release.time();
                conn.keyboard
                    .process_key_release_event(key_release, &mut self.events);
            }
            Event::X(xcb::x::Event::MotionNotify(motion)) => {
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
                    modifiers: xkeysyms::modifiers_from_state(motion.state().bits()),
                    mouse_buttons: MouseButtons::default(),
                };
                self.do_mouse_event(event)?;
            }
            Event::X(xcb::x::Event::ButtonPress(e)) => {
                self.button_event(
                    true,
                    e.time(),
                    e.detail(),
                    e.event_x(),
                    e.event_y(),
                    e.root_x(),
                    e.root_y(),
                    e.state(),
                )?;
            }
            Event::X(xcb::x::Event::ButtonRelease(e)) => {
                self.button_event(
                    false,
                    e.time(),
                    e.detail(),
                    e.event_x(),
                    e.event_y(),
                    e.root_x(),
                    e.root_y(),
                    e.state(),
                )?;
            }
            Event::X(xcb::x::Event::ClientMessage(msg)) => {
                use xcb::x::ClientMessageData;
                match msg.data() {
                    ClientMessageData::Data32(data) => {
                        if data[0] == conn.atom_delete().resource_id() {
                            self.events.dispatch(WindowEvent::CloseRequested);
                        }
                    }
                    ClientMessageData::Data8(_) | ClientMessageData::Data16(_) => {}
                }
            }
            Event::X(xcb::x::Event::DestroyNotify(_)) => {
                self.events.dispatch(WindowEvent::Destroyed);
                conn.windows.borrow_mut().remove(&self.window_id);
            }
            Event::X(xcb::x::Event::SelectionClear(e)) => {
                self.selection_clear(e)?;
            }
            Event::X(xcb::x::Event::SelectionRequest(e)) => {
                self.selection_request(e)?;
            }
            Event::X(xcb::x::Event::SelectionNotify(e)) => {
                self.selection_notify(e)?;
            }
            Event::X(xcb::x::Event::PropertyNotify(msg)) => {
                /*
                if let Ok(reply) = xcb::x::get_atom_name(&conn, msg.atom()).get_reply() {
                    log::info!(
                        "PropertyNotifyEvent atom={} {} xsel={}",
                        msg.atom(),
                        reply.name(),
                        conn.atom_xsel_data
                    );
                }
                */

                log::trace!(
                    "PropertyNotifyEvent atom={:?} xsel={:?}",
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
            Event::X(xcb::x::Event::FocusIn(_)) => {
                self.has_focus.replace(true);
                self.update_ime_position();
                log::trace!("Calling focus_change(true)");
                self.events.dispatch(WindowEvent::FocusChanged(true));
            }
            Event::X(xcb::x::Event::FocusOut(_)) => {
                self.has_focus.replace(false);
                log::trace!("Calling focus_change(false)");
                self.events.dispatch(WindowEvent::FocusChanged(false));
            }
            Event::X(xcb::x::Event::LeaveNotify(_)) => {
                self.events.dispatch(WindowEvent::MouseLeave);
            }
            _ => {
                eprintln!("unhandled: {:?}", event);
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
    fn update_selection_owner(&mut self, clipboard: Clipboard) -> anyhow::Result<()> {
        let conn = self.conn();
        let selection = match clipboard {
            Clipboard::PrimarySelection => xcb::x::ATOM_PRIMARY,
            Clipboard::Clipboard => conn.atom_clipboard,
        };
        let current_owner = conn
            .wait_for_reply(conn.send_request(&xcb::x::GetSelectionOwner { selection }))
            .unwrap()
            .owner();
        if self.copy_and_paste.clipboard(clipboard).is_none() && current_owner == self.window_id {
            // We don't have a selection but X thinks we do; disown it!
            conn.send_request(&xcb::x::SetSelectionOwner {
                owner: xcb::x::Window::none(),
                selection,
                time: self.copy_and_paste.time,
            });
        } else if self.copy_and_paste.clipboard(clipboard).is_some()
            && current_owner != self.window_id
        {
            // We have the selection but X doesn't think we do; assert it!
            conn.send_request(&xcb::x::SetSelectionOwner {
                owner: self.window_id,
                selection,
                time: self.copy_and_paste.time,
            });
        }
        conn.flush().context("flushing after updating selection")?;
        Ok(())
    }

    fn selection_atom_to_clipboard(&self, atom: Atom) -> Option<Clipboard> {
        if atom == xcb::x::ATOM_PRIMARY {
            Some(Clipboard::PrimarySelection)
        } else if atom == self.conn().atom_clipboard {
            Some(Clipboard::Clipboard)
        } else {
            None
        }
    }

    fn selection_clear(&mut self, request: &xcb::x::SelectionClearEvent) -> anyhow::Result<()> {
        if let Some(clipboard) = self.selection_atom_to_clipboard(request.selection()) {
            self.copy_and_paste.clipboard_mut(clipboard).take();
            self.copy_and_paste.request_mut(clipboard).take();
            self.update_selection_owner(clipboard)?;
        }

        Ok(())
    }

    /// A selection request is made to us after we've announced that we own the selection
    /// and when another client wants to copy it.
    fn selection_request(&mut self, request: &xcb::x::SelectionRequestEvent) -> anyhow::Result<()> {
        let conn = self.conn();
        log::trace!(
            "SEL: time={} owner={:?} requestor={:?} selection={:?} target={:?} property={:?}",
            request.time(),
            request.owner(),
            request.requestor(),
            request.selection(),
            request.target(),
            request.property()
        );
        log::trace!(
            "XSEL={:?}, UTF8={:?} PRIMARY={:?} clip={:?}",
            conn.atom_xsel_data,
            conn.atom_utf8_string,
            xcb::x::ATOM_PRIMARY,
            conn.atom_clipboard,
        );

        let selprop = if request.target() == conn.atom_targets {
            // They want to know which targets we support
            let atoms: [Atom; 1] = [conn.atom_utf8_string];
            conn.send_request(&xcb::x::ChangeProperty {
                mode: PropMode::Replace,
                window: request.requestor(),
                property: request.property(),
                r#type: xcb::x::ATOM_ATOM,
                data: &atoms,
            });

            // let the requestor know that we set their property
            request.property()
        } else if request.target() == conn.atom_utf8_string
            || request.target() == xcb::x::ATOM_STRING
        {
            if let Some(clipboard) = self.selection_atom_to_clipboard(request.selection()) {
                // We'll accept requests for UTF-8 or STRING data.
                // We don't and won't do any conversion from UTF-8 to
                // whatever STRING represents; let's just assume that
                // the other end is going to handle it correctly.
                if let Some(text) = self.copy_and_paste.clipboard(clipboard) {
                    conn.send_request(&xcb::x::ChangeProperty {
                        mode: PropMode::Replace,
                        window: request.requestor(),
                        property: request.property(),
                        r#type: request.target(),
                        data: text.as_bytes(),
                    });
                    // let the requestor know that we set their property
                    request.property()
                } else {
                    // We have no clipboard so there is nothing to report
                    xcb::x::ATOM_NONE
                }
            } else {
                xcb::x::ATOM_NONE
            }
        } else {
            // We didn't support their request, so there is nothing
            // we can report back to them.
            xcb::x::ATOM_NONE
        };
        log::trace!("responding with selprop={:?}", selprop);

        conn.send_request(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(request.requestor()),
            event_mask: xcb::x::EventMask::empty(),
            event: &xcb::x::SelectionNotifyEvent::new(
                request.time(),
                request.requestor(),
                request.selection(),
                request.target(),
                selprop, // the disposition from the operation above
            ),
        });

        Ok(())
    }

    fn selection_notify(&mut self, selection: &xcb::x::SelectionNotifyEvent) -> anyhow::Result<()> {
        let conn = self.conn();

        log::trace!(
            "SELECTION_NOTIFY received selection={:?} (prim={:?} clip={:?}) target={:?} property={:?} utf8={:?}",
            selection.selection(),
            xcb::x::ATOM_PRIMARY,
            conn.atom_clipboard,
            selection.target(),
            selection.property(),
            self.conn().atom_utf8_string,
        );

        if let Some(clipboard) = self.selection_atom_to_clipboard(selection.selection()) {
            if selection.property() != xcb::x::ATOM_NONE
                // Restrict to strictly UTF-8 to avoid crashing; see
                // <https://github.com/meh/rust-xcb-util/issues/21>
                && selection.target() == self.conn().atom_utf8_string
            {
                match conn.wait_for_reply(conn.send_request(&xcb::x::GetProperty {
                    delete: false,
                    window: selection.requestor(),
                    property: selection.property(),
                    r#type: self.conn().atom_utf8_string,
                    long_offset: 0,
                    long_length: u32::max_value(),
                })) {
                    Ok(prop) => {
                        if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take()
                        {
                            promise.ok(String::from_utf8_lossy(prop.value()).to_string());
                        }
                        conn.send_request(&xcb::x::DeleteProperty {
                            window: self.window_id,
                            property: conn.atom_xsel_data,
                        });
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

        let reply = conn.wait_for_reply(conn.send_request(&xcb::x::GetProperty {
            delete: false,
            window: self.window_id,
            property: conn.atom_net_wm_state,
            r#type: xcb::x::ATOM_ATOM,
            long_offset: 0,
            long_length: 1024,
        }))?;

        let state = reply.value::<u32>();
        let mut window_state = WindowState::default();

        for &s in state {
            if s == conn.atom_state_fullscreen.resource_id() {
                window_state |= WindowState::FULL_SCREEN;
            } else if s == conn.atom_state_maximized_vert.resource_id()
                || s == conn.atom_state_maximized_horz.resource_id()
            {
                window_state |= WindowState::MAXIMIZED;
            } else if s == conn.atom_state_hidden.resource_id() {
                window_state |= WindowState::HIDDEN;
            }
        }

        Ok(window_state)
    }

    fn set_fullscreen_hint(&mut self, enable: bool) -> anyhow::Result<()> {
        let conn = self.conn();
        let data: [u32; 5] = [
            if enable { 1 } else { 0 },
            conn.atom_state_fullscreen.resource_id(),
            0,
            0,
            0,
        ];

        // Ask window manager to change our fullscreen state
        conn.send_request(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(conn.root),
            event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
                | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
            event: &xcb::x::ClientMessageEvent::new(
                self.window_id,
                conn.atom_net_wm_state,
                xcb::x::ClientMessageData::Data32(data),
            ),
        });
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

        conn.conn().send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: conn.atom_motif_wm_hints,
            r#type: conn.atom_motif_wm_hints,
            data: hints_slice,
        });
        Ok(())
    }

    fn conn(&self) -> Rc<XConnection> {
        self.conn.upgrade().expect("XConnection to be alive")
    }
}

/// A Window!
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct XWindow(xcb::x::Window);

impl PartialOrd for XWindow {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.resource_id().partial_cmp(&other.0.resource_id())
    }
}

impl Ord for XWindow {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.resource_id().cmp(&other.0.resource_id())
    }
}

impl XWindow {
    pub(crate) fn from_id(id: xcb::x::Window) -> Self {
        Self(id)
    }

    /// Create a new window on the specified screen with the specified
    /// dimensions
    pub async fn new_window<F>(
        class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
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

        let needs_reposition = geometry.x.is_some() && geometry.y.is_some();
        let ResolvedGeometry {
            x,
            y,
            width,
            height,
        } = resolve_geometry(&conn, geometry)?;

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
            conn.check_request(conn.conn().send_request_checked(&xcb::x::CreateColormap {
                alloc: xcb::x::ColormapAlloc::None,
                mid: color_map_id,
                window: screen.root(),
                visual: conn.visual.visual_id(),
            }))
            .context("create_colormap_checked")?;

            conn.check_request(conn.conn().send_request_checked(&xcb::x::CreateWindow {
                depth: conn.depth,
                wid: window_id,
                parent: screen.root(),
                x,
                y,
                width: width.try_into()?,
                height: height.try_into()?,
                border_width: 0,
                class: xcb::x::WindowClass::InputOutput,
                visual: conn.visual.visual_id(),
                value_list: &[
                    // We have to specify both a border pixel color and a colormap
                    // when specifying a depth that doesn't match the root window in
                    // order to avoid a BadMatch
                    xcb::x::Cw::BorderPixel(0),
                    xcb::x::Cw::EventMask(
                        xcb::x::EventMask::EXPOSURE
                            | xcb::x::EventMask::FOCUS_CHANGE
                            | xcb::x::EventMask::KEY_PRESS
                            | xcb::x::EventMask::BUTTON_PRESS
                            | xcb::x::EventMask::BUTTON_RELEASE
                            | xcb::x::EventMask::POINTER_MOTION
                            | xcb::x::EventMask::LEAVE_WINDOW
                            | xcb::x::EventMask::BUTTON_MOTION
                            | xcb::x::EventMask::KEY_RELEASE
                            | xcb::x::EventMask::PROPERTY_CHANGE
                            | xcb::x::EventMask::STRUCTURE_NOTIFY,
                    ),
                    xcb::x::Cw::Colormap(color_map_id),
                ],
            }))
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
                cursors: CursorInfo::new(&config, &conn),
                config: config.clone(),
                has_focus: None,
                last_cursor_position: Rect::default(),
                paint_throttled: false,
                invalidated: false,
                pending: vec![],
                dispatched_any_resize: false,
            }))
        };

        // WM_CLASS is encoded as the class and instance name,
        // null terminated
        let mut class_string = class_name.as_bytes().to_vec();
        class_string.push(0);
        class_string.extend_from_slice(class_name.as_bytes());
        class_string.push(0);

        conn.send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: xcb::x::ATOM_WM_CLASS,
            r#type: xcb::x::ATOM_STRING,
            data: &class_string,
        });

        conn.send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: conn.atom_net_wm_pid,
            r#type: xcb::x::ATOM_CARDINAL,
            data: &[unsafe { libc::getpid() as u32 }],
        });

        conn.send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: conn.atom_protocols,
            r#type: xcb::x::ATOM_ATOM,
            data: &[conn.atom_delete],
        });

        window
            .lock()
            .unwrap()
            .adjust_decorations(config.window_decorations)?;

        let window_handle = Window::X11(XWindow::from_id(window_id));

        conn.windows.borrow_mut().insert(window_id, window);

        window_handle.set_title(name);
        window_handle.show();

        // Some window managers will ignore the x,y that we set during window
        // creation, so we ask them again once the window is mapped
        if needs_reposition {
            window_handle.set_window_position(ScreenPoint::new(x.into(), y.into()));
        }

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
        self.conn().conn().send_request(&xcb::x::DestroyWindow {
            window: self.window_id,
        });
        // Ensure that we don't try to destroy the window twice,
        // otherwise the rust xcb bindings will generate a
        // fatal error!
        self.window_id = xcb::x::Window::none();
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        self.conn().conn().send_request(&xcb::x::MapWindow {
            window: self.window_id,
        });
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

        conn.send_request(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(conn.root),
            event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
                | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
            event: &xcb::x::ClientMessageEvent::new(
                self.window_id,
                conn.atom_net_move_resize_window,
                xcb::x::ClientMessageData::Data32([
                    xcb::x::Gravity::Static as u32 |
            1<<12 | // normal program
            xcb_util::MOVE_RESIZE_MOVE
                | xcb_util::MOVE_RESIZE_WINDOW_X
                | xcb_util::MOVE_RESIZE_WINDOW_Y,
                    coords.x as u32,
                    coords.y as u32,
                    self.width as u32,
                    self.height as u32,
                ]),
            ),
        });
    }

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        if title == self.title {
            return;
        }
        self.title = title.to_string();

        let conn = self.conn();

        conn.send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: xcb::x::ATOM_WM_NAME,
            r#type: conn.atom_utf8_string,
            data: title.as_bytes(),
        });

        // Also set EWMH _NET_WM_NAME, as some clients don't correctly
        // fall back to reading WM_NAME
        conn.send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: conn.atom_net_wm_name,
            r#type: conn.atom_utf8_string,
            data: title.as_bytes(),
        });
    }

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        if self.last_cursor_position == cursor {
            return;
        }
        self.last_cursor_position = cursor;
        self.update_ime_position();
    }

    fn update_ime_position(&mut self) {
        if !self.has_focus.unwrap_or(false) {
            return;
        }
        self.conn().ime.borrow_mut().update_pos(
            self.window_id,
            self.last_cursor_position.min_x() as i16,
            self.last_cursor_position.max_y() as i16,
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
        // `BitmapImage` is rgba32, so we need to munge to get argb32.
        // We also need to put the data into big endian format.
        for pixel in image.pixels() {
            let [r, g, b, a] = pixel.to_ne_bytes();
            icon_data.push(u32::from_be_bytes([a, r, g, b]));
        }

        self.conn().send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: self.conn().atom_net_wm_icon,
            r#type: xcb::x::ATOM_CARDINAL,
            data: &icon_data,
        });
    }

    fn set_resize_increments(&mut self, x: u16, y: u16) -> anyhow::Result<()> {
        use xcb_util::*;
        let hints = xcb_size_hints_t {
            flags: XCB_ICCCM_SIZE_HINT_P_RESIZE_INC,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            min_width: 0,
            min_height: 0,
            max_width: 0,
            max_height: 0,
            width_inc: x.into(),
            height_inc: y.into(),
            min_aspect_num: 0,
            min_aspect_den: 0,
            max_aspect_num: 0,
            max_aspect_den: 0,
            base_width: 0,
            base_height: 0,
            win_gravity: 0,
        };

        let data = unsafe {
            std::slice::from_raw_parts(
                &hints as *const _ as *const u32,
                std::mem::size_of::<xcb_size_hints_t>() / 4,
            )
        };

        self.conn().send_request(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: xcb::x::ATOM_WM_SIZE_HINTS,
            r#type: xcb::x::ATOM_CARDINAL,
            data,
        });

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
            inner.conn().conn().send_request(&xcb::x::ConfigureWindow {
                window: inner.window_id,
                value_list: &[
                    xcb::x::ConfigWindow::Width(width as u32),
                    xcb::x::ConfigWindow::Height(height as u32),
                ],
            });
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
                conn.send_request(&xcb::x::ConvertSelection {
                    requestor: inner.window_id,
                    selection: match clipboard {
                        Clipboard::Clipboard => conn.atom_clipboard,
                        Clipboard::PrimarySelection => xcb::x::ATOM_PRIMARY,
                    },
                    target: conn.atom_utf8_string,
                    property: conn.atom_xsel_data,
                    time: inner.copy_and_paste.time,
                });
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
            inner.update_selection_owner(clipboard)?;
            Ok(())
        });
    }
}

#[derive(Debug)]
struct ResolvedGeometry {
    x: i16,
    y: i16,
    width: usize,
    height: usize,
}

fn resolve_geometry(
    conn: &XConnection,
    geometry: RequestedWindowGeometry,
) -> anyhow::Result<ResolvedGeometry> {
    let bounds = if conn.has_randr {
        let res = conn
            .conn()
            .wait_for_reply(
                conn.conn()
                    .send_request(&xcb::randr::GetScreenResources { window: conn.root }),
            )
            .context("get_screen_resources")?;

        let mut virtual_screen: Rect = euclid::rect(0, 0, 0, 0);
        let mut main_screen: Rect = euclid::rect(0, 0, 0, 0);
        let mut by_name = HashMap::new();

        for &o in res.outputs() {
            let info = conn
                .conn()
                .wait_for_reply(conn.conn().send_request(&xcb::randr::GetOutputInfo {
                    output: o,
                    config_timestamp: res.config_timestamp(),
                }))
                .context("get_output_info")?;
            let name = String::from_utf8_lossy(info.name()).to_string();
            let c = info.crtc();
            if let Ok(cinfo) =
                conn.conn()
                    .wait_for_reply(conn.conn().send_request(&xcb::randr::GetCrtcInfo {
                        crtc: c,
                        config_timestamp: res.config_timestamp(),
                    }))
            {
                let bounds = euclid::rect(
                    cinfo.x() as isize,
                    cinfo.y() as isize,
                    cinfo.width() as isize,
                    cinfo.height() as isize,
                );
                virtual_screen = virtual_screen.union(&bounds);
                if bounds.origin.x == 0 && bounds.origin.y == 0 {
                    main_screen = bounds;
                }
                by_name.insert(name, bounds);
            }
        }
        log::trace!("{:?}", by_name);
        log::trace!("virtual: {:?}", virtual_screen);
        log::trace!("main: {:?}", main_screen);

        match geometry.origin {
            GeometryOrigin::ScreenCoordinateSystem => virtual_screen,
            GeometryOrigin::MainScreen => main_screen,
            GeometryOrigin::ActiveScreen => {
                // TODO: find focused window and resolve it!
                // Maybe something like <https://stackoverflow.com/a/43666928/149111>
                // but ported to Rust?
                main_screen
            }
            GeometryOrigin::Named(name) => match by_name.get(&name) {
                Some(bounds) => bounds.clone(),
                None => {
                    log::error!(
                        "Requested display {} was not found; available displays are: {:?}. \
                             Using primary display instead",
                        name,
                        by_name,
                    );
                    main_screen
                }
            },
        }
    } else {
        euclid::rect(0, 0, 65535, 65535)
    };

    let dpi = conn.default_dpi();
    let width_context = DimensionContext {
        dpi: dpi as f32,
        pixel_max: bounds.width() as f32,
        pixel_cell: bounds.width() as f32,
    };
    let height_context = DimensionContext {
        dpi: dpi as f32,
        pixel_max: bounds.height() as f32,
        pixel_cell: bounds.height() as f32,
    };
    let width = geometry.width.evaluate_as_pixels(width_context) as usize;
    let height = geometry.height.evaluate_as_pixels(height_context) as usize;
    let x = geometry
        .x
        .map(|x| x.evaluate_as_pixels(width_context) as i16 + bounds.origin.x as i16)
        .unwrap_or(0);
    let y = geometry
        .y
        .map(|y| y.evaluate_as_pixels(height_context) as i16 + bounds.origin.y as i16)
        .unwrap_or(0);

    Ok(ResolvedGeometry {
        x,
        y,
        width,
        height,
    })
}
