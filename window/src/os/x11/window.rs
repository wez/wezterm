use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::os::{xkeysyms, Connection, Window};
use crate::{
    Appearance, Clipboard, DeadKeyStatus, Dimensions, MouseButtons, MouseCursor, MouseEvent,
    MouseEventKind, MousePress, Point, Rect, RequestedWindowGeometry, ResizeIncrement,
    ResolvedGeometry, ScreenPoint, ScreenRect, WindowDecorations, WindowEvent, WindowEventSender,
    WindowOps, WindowState,
};
use anyhow::{anyhow, Context as _};
use async_trait::async_trait;
use config::ConfigHandle;
use promise::{Future, Promise};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle, XcbDisplayHandle, XcbWindowHandle,
};
use std::any::Any;
use std::convert::TryInto;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use url::Url;
use wezterm_font::FontConfiguration;
use wezterm_input_types::{KeyCode, KeyEvent, KeyboardLedStatus, Modifiers};
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

struct DragAndDrop {
    src_window: Option<xcb::x::Window>,
    src_types: Vec<Atom>,
    src_action: Atom,
    time: u32,
    target_type: Atom,
    target_action: Atom,
}

impl Default for DragAndDrop {
    fn default() -> DragAndDrop {
        DragAndDrop {
            src_window: None,
            src_types: Vec::new(),
            src_action: xcb::x::ATOM_NONE,
            time: 0,
            target_type: xcb::x::ATOM_NONE,
            target_action: xcb::x::ATOM_NONE,
        }
    }
}

pub(crate) struct XWindowInner {
    pub window_id: xcb::x::Window,
    pub child_id: xcb::x::Window,
    conn: Weak<XConnection>,
    pub events: WindowEventSender,
    width: u16,
    height: u16,
    last_wm_state: WindowState,
    dpi: f64,
    cursors: CursorInfo,
    copy_and_paste: CopyAndPaste,
    drag_and_drop: DragAndDrop,
    config: ConfigHandle,
    appearance: Appearance,
    title: String,
    pub has_focus: Option<bool>,
    verify_focus: bool,
    last_cursor_position: Rect,
    invalidated: bool,
    paint_throttled: bool,
    pending: Vec<WindowEvent>,
    sure_about_geometry: bool,
    current_mouse_event: Option<MouseEvent>,
    window_drag_position: Option<ScreenPoint>,
    dragging: bool,
    outstanding_configure_requests: usize,
    pending_finished_resizes: usize,
}

/// <https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm46409506331616>
const _NET_WM_MOVERESIZE_MOVE: u32 = 8;
const _NET_WM_MOVERESIZE_CANCEL: u32 = 11;

impl Drop for XWindowInner {
    fn drop(&mut self) {
        if self.window_id != xcb::x::Window::none() {
            if let Some(conn) = self.conn.upgrade() {
                self.conn()
                    .conn()
                    .flush()
                    .context("flush pending requests prior to issuing DestroyWindow")
                    .ok();
                conn.send_request_no_reply_log(&xcb::x::DestroyWindow {
                    window: self.child_id,
                });
                conn.send_request_no_reply_log(&xcb::x::DestroyWindow {
                    window: self.window_id,
                });
            }
        }
    }
}

impl HasDisplayHandle for XWindowInner {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        if let Some(conn) = self.conn.upgrade() {
            let handle =
                XcbDisplayHandle::new(NonNull::new(conn.conn.get_raw_conn() as _), conn.screen_num);
            unsafe { Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(handle))) }
        } else {
            Err(HandleError::Unavailable)
        }
    }
}

impl HasWindowHandle for XWindowInner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let mut handle =
            XcbWindowHandle::new(NonZeroU32::new(self.child_id.resource_id()).expect("non-zero"));
        handle.visual_id = NonZeroU32::new(self.conn.upgrade().unwrap().visual.visual_id());
        unsafe { Ok(WindowHandle::borrow_raw(RawWindowHandle::Xcb(handle))) }
    }
}

impl XWindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let conn = self.conn();

        let gl_state = match conn.gl_connection.borrow().as_ref() {
            None => crate::egl::GlState::create(
                Some(conn.conn.get_raw_dpy() as *const _),
                self.child_id.resource_id() as *mut _,
            ),
            Some(glconn) => crate::egl::GlState::create_with_existing_connection(
                glconn,
                self.child_id.resource_id() as *mut _,
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
    fn expose(&mut self, x: u16, y: u16, width: u16, height: u16, count: u16) {
        log::trace!("expose: {x},{y} {width}x{height} ({count} expose events follow this one)");
        let max_x = x.saturating_add(width);
        let max_y = y.saturating_add(height);
        if max_x > self.width || max_y > self.height {
            log::trace!(
                "flagging geometry as unsure because exposed region is larger than known geom"
            );
            self.sure_about_geometry = false;
        }
        self.queue_pending(WindowEvent::NeedRepaint);
    }

    fn cancel_drag(&mut self) -> bool {
        if self.dragging {
            log::debug!("cancel_drag");
            self.net_wm_moveresize(0, 0, _NET_WM_MOVERESIZE_CANCEL, 0);
            self.dragging = false;
            if let Some(event) = self.current_mouse_event.take() {
                self.do_mouse_event(MouseEvent {
                    kind: MouseEventKind::Release(MousePress::Left),
                    ..event
                })
                .ok();
            }
            return true;
        }
        false
    }

    fn do_mouse_event(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        if self.cancel_drag() {
            return Ok(());
        }
        self.current_mouse_event.replace(event.clone());
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
            self.last_wm_state = self.get_window_state().unwrap_or(WindowState::default());
            self.events.dispatch(WindowEvent::Resized {
                dimensions: Dimensions {
                    pixel_width: self.width as usize,
                    pixel_height: self.height as usize,
                    dpi: self.dpi as usize,
                },
                window_state: self.last_wm_state,
                live_resizing: false,
            });
        }
    }

    fn queue_pending(&mut self, event: WindowEvent) {
        self.pending.push(event);
    }

    fn resize_child(&self, width: u32, height: u32) {
        self.conn()
            .send_request_no_reply_log(&xcb::x::ConfigureWindow {
                window: self.child_id,
                value_list: &[
                    xcb::x::ConfigWindow::Width(width as u32),
                    xcb::x::ConfigWindow::Height(height as u32),
                ],
            });
        // send_request_no_reply_log() is synchronous, so no further synchronization required
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
            self.sure_about_geometry = true;
            self.events.dispatch(resize);
        }

        // These SetInnerSizeCompleted events need to be dispatched after the
        // above Resized events because a resize cannot finish before it occurs.
        while self.pending_finished_resizes > 0 {
            self.events.dispatch(WindowEvent::SetInnerSizeCompleted);
            self.pending_finished_resizes -= 1;
        }

        if need_paint {
            if self.paint_throttled {
                self.invalidated = true;
            } else {
                self.invalidated = false;

                if self.verify_focus || self.has_focus.is_none() {
                    log::trace!("About to paint, but we're unsure about focus; querying!");

                    let focus = self
                        .conn()
                        .send_and_wait_request(&xcb::x::GetInputFocus {})?;
                    let focused = focus.focus() == self.window_id;
                    log::trace!(
                        "Do I {:?} have focus? result={}, I thought {:?}",
                        self.window_id,
                        focused,
                        self.has_focus
                    );
                    if Some(focused) != self.has_focus {
                        self.has_focus.replace(focused);
                        self.events.dispatch(WindowEvent::FocusChanged(focused));
                    }

                    self.verify_focus = false;
                }

                if !self.sure_about_geometry {
                    self.sure_about_geometry = true;

                    log::trace!(
                        "About to paint, but we're unsure about geometry; querying window_id {:?}!",
                        self.window_id
                    );
                    let geom = self
                        .conn()
                        .send_and_wait_request(&xcb::x::GetGeometry {
                            drawable: xcb::x::Drawable::Window(self.window_id),
                        })
                        .context("querying geometry")?;
                    log::trace!(
                        "geometry is {}x{} vs. our initial {}x{}",
                        geom.width(),
                        geom.height(),
                        self.width,
                        self.height
                    );

                    let window_state = self.get_window_state().unwrap_or(WindowState::default());

                    if self.width != geom.width()
                        || self.height != geom.height()
                        || self.last_wm_state != window_state
                    {
                        self.resize_child(geom.width() as u32, geom.height() as u32);

                        self.width = geom.width();
                        self.height = geom.height();
                        self.last_wm_state = window_state;

                        self.events.dispatch(WindowEvent::Resized {
                            dimensions: Dimensions {
                                pixel_width: self.width as usize,
                                pixel_height: self.height as usize,
                                dpi: self.dpi as usize,
                            },
                            window_state,
                            live_resizing: false,
                        });
                    }
                }

                self.events.dispatch(WindowEvent::NeedRepaint);

                self.paint_throttled = true;
                let window_id = self.window_id;
                let max_fps = self.config.max_fps;
                promise::spawn::spawn(async move {
                    async_io::Timer::after(std::time::Duration::from_millis(1000 / max_fps as u64))
                        .await;
                    XConnection::with_window_inner(window_id, move |inner| {
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

        if self.cancel_drag() {
            log::debug!("cancel drag due to button {detail} {state:?}");
            return Ok(());
        }

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
                log::trace!("button {} is not implemented", detail);
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

    fn configure_notify(&mut self, source: &str, width: u16, height: u16) -> anyhow::Result<()> {
        let conn = self.conn();

        self.update_ime_position();

        let mut dpi = conn.default_dpi();

        if !self.config.dpi_by_screen.is_empty() {
            let coords = conn
                .send_and_wait_request(&xcb::x::TranslateCoordinates {
                    src_window: self.window_id,
                    dst_window: conn.root,
                    src_x: 0,
                    src_y: 0,
                })
                .context("querying window coordinates")?;
            let screens = conn.get_cached_screens()?;
            let window_rect: ScreenRect = euclid::rect(
                coords.dst_x().into(),
                coords.dst_y().into(),
                width as isize,
                height as isize,
            );
            let screen = screens
                .by_name
                .values()
                .filter_map(|screen| {
                    screen
                        .rect
                        .intersection(&window_rect)
                        .map(|r| (screen, r.area()))
                })
                .max_by_key(|s| s.1)
                .ok_or_else(|| anyhow::anyhow!("window is not in any screen"))?
                .0;

            if let Some(value) = self.config.dpi_by_screen.get(&screen.name).copied() {
                dpi = value;
            } else if let Some(value) = self.config.dpi {
                dpi = value;
            }
        }

        if width == self.width && height == self.height && dpi == self.dpi {
            // Effectively unchanged; perhaps it was simply moved?
            // Do nothing!
            log::trace!(
                "Ignoring {source} ({width}x{height} dpi={dpi}) \
                                 because width,height,dpi are unchanged",
            );
            return Ok(());
        }

        self.resize_child(width as u32, height as u32);

        log::trace!(
            "{source}: width {} -> {}, height {} -> {}, dpi {} -> {}",
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
        self.last_wm_state = self.get_window_state().unwrap_or(WindowState::default());

        let dimensions = Dimensions {
            pixel_width: self.width as usize,
            pixel_height: self.height as usize,
            dpi: self.dpi as usize,
        };

        self.queue_pending(WindowEvent::Resized {
            dimensions,
            window_state: self.last_wm_state,
            // Assume that we're live resizing: we don't know for sure,
            // but it seems like a reasonable assumption
            live_resizing: true,
        });
        Ok(())
    }

    fn xdnd_event(&mut self, msgtype: Atom, data: &[u32]) -> anyhow::Result<()> {
        use xcb::XidNew;
        let conn = self.conn();
        let msgtype_name = conn.atom_name(msgtype);
        let srcwin = unsafe { xcb::x::Window::new(data[0]) };
        if msgtype == conn.atom_xdndenter {
            self.drag_and_drop.src_window = Some(srcwin);
            let moretypes = data[1] & 0x01 != 0;
            let xdndversion = data[1] >> 24 as u8;
            log::trace!("ClientMessage {msgtype_name}, Version {xdndversion}, more than 3 types: {moretypes}");
            if !moretypes {
                self.drag_and_drop.src_types = data[2..]
                    .into_iter()
                    .filter(|&&x| x != 0)
                    .map(|&x| unsafe { Atom::new(x) })
                    .collect();
            } else {
                self.drag_and_drop.src_types =
                    match conn.send_and_wait_request(&xcb::x::GetProperty {
                        delete: false,
                        window: srcwin,
                        property: conn.atom_xdndtypelist,
                        r#type: xcb::x::ATOM_ATOM,
                        long_offset: 0,
                        long_length: u32::max_value(),
                    }) {
                        Ok(prop) => prop.value::<Atom>().to_vec(),
                        Err(err) => {
                            log::error!(
                                "xdnd: unable to get type list from source window: {:?}",
                                err
                            );
                            Vec::<Atom>::new()
                        }
                    };
            }
            self.drag_and_drop.target_type = xcb::x::ATOM_NONE;
            for t in [
                conn.atom_texturilist,
                conn.atom_xmozurl,
                conn.atom_utf8_string,
            ] {
                if self.drag_and_drop.src_types.contains(&t) {
                    self.drag_and_drop.target_type = t;
                    break;
                }
            }
            for t in &self.drag_and_drop.src_types {
                log::trace!("types offered: {}", conn.atom_name(*t));
            }
            log::trace!(
                "selected: {}",
                conn.atom_name(self.drag_and_drop.target_type)
            );
        } else if self.drag_and_drop.src_window != Some(srcwin) {
            log::error!("ClientMessage {msgtype_name} received, but no Xdnd in progress or source window mismatch");
        } else if msgtype == conn.atom_xdndposition {
            self.drag_and_drop.time = data[3];
            let (x, y) = (data[2] >> 16 as u16, data[2] as u16);
            self.drag_and_drop.src_action = unsafe { Atom::new(data[4]) };
            self.drag_and_drop.target_action = conn.atom_xdndactioncopy;
            log::trace!(
                "ClientMessage {msgtype_name}, ({x}, {y}), timestamp: {}, action: {}",
                self.drag_and_drop.time,
                conn.atom_name(self.drag_and_drop.src_action)
            );
            conn.send_request_no_reply_log(&xcb::x::SendEvent {
                propagate: false,
                destination: xcb::x::SendEventDest::Window(srcwin),
                event_mask: xcb::x::EventMask::empty(),
                event: &xcb::x::ClientMessageEvent::new(
                    srcwin,
                    conn.atom_xdndstatus,
                    xcb::x::ClientMessageData::Data32([
                        self.window_id.resource_id(),
                        2 | (self.drag_and_drop.target_type != xcb::x::ATOM_NONE) as u32,
                        0,
                        0,
                        self.drag_and_drop.target_action.resource_id(),
                    ]),
                ),
            });
        } else if msgtype == conn.atom_xdndleave {
            self.drag_and_drop.src_window = None;
            log::trace!("ClientMessage {msgtype_name}");
        } else if msgtype == conn.atom_xdnddrop {
            self.drag_and_drop.time = data[2];
            log::trace!(
                "ClientMessage {msgtype_name}, timestamp: {}",
                self.drag_and_drop.time
            );
            if self.drag_and_drop.target_type != xcb::x::ATOM_NONE {
                conn.send_request_no_reply_log(&xcb::x::ConvertSelection {
                    requestor: self.window_id,
                    selection: conn.atom_xdndselection,
                    target: self.drag_and_drop.target_type,
                    property: conn.atom_xsel_data,
                    time: self.drag_and_drop.time,
                });
            } else {
                log::warn!("XdndDrop received, but no target type selected. Ignoring.");
                conn.send_request_no_reply_log(&xcb::x::SendEvent {
                    propagate: false,
                    destination: xcb::x::SendEventDest::Window(srcwin),
                    event_mask: xcb::x::EventMask::empty(),
                    event: &xcb::x::ClientMessageEvent::new(
                        srcwin,
                        conn.atom_xdndfinished,
                        xcb::x::ClientMessageData::Data32([
                            self.window_id.resource_id(),
                            0,
                            0,
                            0,
                            0,
                        ]),
                    ),
                });
            }
        }
        return Ok(());
    }

    pub fn dispatch_event(&mut self, event: &Event) -> anyhow::Result<()> {
        let conn = self.conn();
        match event {
            Event::X(xcb::x::Event::Expose(expose)) => {
                self.expose(
                    expose.x(),
                    expose.y(),
                    expose.width(),
                    expose.height(),
                    expose.count(),
                );
            }
            Event::Present(xcb::present::Event::ConfigureNotify(cfg)) => {
                self.configure_notify("Present::ConfigureNotify", cfg.width(), cfg.height())?;
            }
            Event::X(xcb::x::Event::ConfigureNotify(cfg)) => {
                self.configure_notify("X::ConfigureNotify", cfg.width(), cfg.height())?;
                if self.outstanding_configure_requests > 0 {
                    self.outstanding_configure_requests -= 1;
                    self.pending_finished_resizes += 1;
                }
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
                let type_atom_name = conn.atom_name(msg.r#type());
                use xcb::x::ClientMessageData;
                use xcb::XidNew;
                let xdnd_msgtype_atoms = [
                    conn.atom_xdndenter,
                    conn.atom_xdndposition,
                    conn.atom_xdndstatus,
                    conn.atom_xdndleave,
                    conn.atom_xdnddrop,
                    conn.atom_xdndfinished,
                ];
                if xdnd_msgtype_atoms.contains(&msg.r#type()) {
                    if let ClientMessageData::Data32(data) = msg.data() {
                        self.xdnd_event(msg.r#type(), &data)?;
                    } else {
                        log::warn!("Received ClientMessage {type_atom_name} with wrong format");
                    }
                } else if msg.r#type() == conn.atom_protocols {
                    if let ClientMessageData::Data32(data) = msg.data() {
                        let protocol_atom = unsafe { Atom::new(data[0]) };
                        log::trace!(
                            "ClientMessage {type_atom_name}/{}",
                            conn.atom_name(protocol_atom)
                        );
                        if protocol_atom == conn.atom_delete {
                            self.events.dispatch(WindowEvent::CloseRequested);
                        }
                    } else {
                        log::warn!("Received ClientMessage {type_atom_name} with wrong format");
                    }
                }
            }
            Event::X(xcb::x::Event::DestroyNotify(_)) => {
                self.events.dispatch(WindowEvent::Destroyed);
                conn.windows.borrow_mut().remove(&self.window_id);
                conn.child_to_parent_id.borrow_mut().remove(&self.child_id);
            }
            Event::X(xcb::x::Event::SelectionClear(e)) => {
                if let Err(err) = self.selection_clear(e) {
                    log::error!("Error handling SelectionClear: {err:#}");
                }
            }
            Event::X(xcb::x::Event::SelectionRequest(e)) => {
                if let Err(err) = self.selection_request(e) {
                    // Don't propagate this, as it is not worth exiting the program over it.
                    // <https://github.com/wez/wezterm/pull/6135>
                    log::error!("Error handling SelectionRequest: {err:#}");
                }
            }
            Event::X(xcb::x::Event::SelectionNotify(e)) => {
                if let Err(err) = self.selection_notify(e) {
                    log::error!("Error handling SelectionNotify: {err:#}");
                }
            }
            Event::X(xcb::x::Event::PropertyNotify(msg)) => {
                let atom_name = conn.atom_name(msg.atom());
                log::trace!("PropertyNotifyEvent {atom_name}");

                if msg.atom() == conn.atom_gtk_edge_constraints {
                    // "_GTK_EDGE_CONSTRAINTS" property is changed when the
                    // accessibility settings change the text size and thus
                    // the dpi.  We use this as a way to detect dpi changes
                    // when running under gnome.
                    conn.update_xrm();
                    self.check_dpi_and_synthesize_resize();
                    let appearance = conn.get_appearance();
                    self.appearance_changed(appearance);
                }

                if msg.atom() == conn.atom_net_wm_state {
                    // Change in window state should be accompanied by
                    // a Configure Notify but not all WMs send these
                    // events consistently/at all/in the same order.
                    self.sure_about_geometry = false;
                    self.verify_focus = true;
                }
            }
            Event::X(xcb::x::Event::FocusIn(e)) => {
                if !matches!(e.detail(), xcb::x::NotifyDetail::Pointer) {
                    self.focus_changed(true);
                }
            }
            Event::X(xcb::x::Event::FocusOut(e)) => {
                if !matches!(e.detail(), xcb::x::NotifyDetail::Pointer) {
                    self.focus_changed(false);
                }
            }
            Event::X(xcb::x::Event::LeaveNotify(_)) => {
                self.events.dispatch(WindowEvent::MouseLeave);
            }
            _ => {
                log::warn!("unhandled: {:?}", event);
            }
        }

        Ok(())
    }

    pub(crate) fn appearance_changed(&mut self, appearance: Appearance) {
        if appearance != self.appearance {
            self.appearance = appearance;
            self.events
                .dispatch(WindowEvent::AppearanceChanged(appearance));
        }
    }

    fn focus_changed(&mut self, focused: bool) {
        log::trace!("focus_changed {focused}, flagging geometry as unsure");
        self.sure_about_geometry = false;
        if self.has_focus != Some(focused) {
            self.has_focus.replace(focused);
            self.update_ime_position();
            log::trace!("Calling focus_change({focused})");
            self.events.dispatch(WindowEvent::FocusChanged(focused));
        }
    }

    pub fn dispatch_ime_compose_status(&mut self, status: DeadKeyStatus) {
        self.events
            .dispatch(WindowEvent::AdviseDeadKeyStatus(status));
    }

    pub fn dispatch_ime_text(&mut self, text: &str) {
        let key_event = KeyEvent {
            key: KeyCode::Composed(text.into()),
            leds: KeyboardLedStatus::empty(),
            modifiers: Modifiers::NONE,
            repeat_count: 1,
            key_is_down: true,
            raw: None,
        }
        .normalize_shift()
        .resurface_positional_modifier_key();
        self.events.dispatch(WindowEvent::KeyEvent(key_event));
        // Since we just composed, synthesize a cleared status, as we
        // are not guaranteed to receive an event notification to
        // trigger dispatch_ime_compose_status() above.
        // <https://github.com/wez/wezterm/issues/4841>
        self.events
            .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
    }

    /// If we own the selection, make sure that the X server reflects
    /// that and vice versa.
    fn update_selection_owner(&mut self, clipboard: Clipboard) -> anyhow::Result<()> {
        let window_id = self.window_id;
        let conn = self.conn();
        let selection = match clipboard {
            Clipboard::PrimarySelection => xcb::x::ATOM_PRIMARY,
            Clipboard::Clipboard => conn.atom_clipboard,
        };
        let current_owner = conn
            .send_and_wait_request(&xcb::x::GetSelectionOwner { selection })
            .unwrap()
            .owner();

        let we_own_it = self.copy_and_paste.clipboard(clipboard).is_some();

        if !we_own_it && current_owner == window_id {
            log::trace!(
                "SEL: window_id={window_id:?} X thinks we own selection, \
                        but we don't: tell it to clear it"
            );
            // We don't have a selection but X thinks we do; disown it!
            conn.send_request_no_reply(&xcb::x::SetSelectionOwner {
                owner: xcb::x::Window::none(),
                selection,
                time: self.copy_and_paste.time,
            })?;
        } else if we_own_it {
            log::trace!(
                "SEL: window_id={window_id:?} currently owned by \
                 {current_owner:?}, tell X we now own it"
            );
            // We have the selection but X doesn't think we do; assert it!
            conn.send_request_no_reply(&xcb::x::SetSelectionOwner {
                owner: self.window_id,
                selection,
                time: self.copy_and_paste.time,
            })?;
        } else {
            log::trace!(
                "SEL: window_id={window_id:?} current_owner={current_owner:?} \
                owned={we_own_it}"
            );
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
        let window_id = self.window_id;
        log::debug!("SEL: window_id={window_id:?} {:?}", request);
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
        let window_id = self.window_id;
        log::trace!("SEL: window_id={window_id:?} {:?}", request);
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
            log::trace!("SEL: window_id={window_id:?} requestor wants supported targets");
            conn.send_request_no_reply(&xcb::x::ChangeProperty {
                mode: PropMode::Replace,
                window: request.requestor(),
                property: request.property(),
                r#type: xcb::x::ATOM_ATOM,
                data: &atoms,
            })?;

            // let the requestor know that we set their property
            request.property()
        } else if request.target() == conn.atom_utf8_string
            || request.target() == xcb::x::ATOM_STRING
        {
            log::trace!("SEL: window_id={window_id:?} requestor wants string data");
            if let Some(clipboard) = self.selection_atom_to_clipboard(request.selection()) {
                // We'll accept requests for UTF-8 or STRING data.
                // We don't and won't do any conversion from UTF-8 to
                // whatever STRING represents; let's just assume that
                // the other end is going to handle it correctly.
                if let Some(text) = self.copy_and_paste.clipboard(clipboard) {
                    conn.send_request_no_reply(&xcb::x::ChangeProperty {
                        mode: PropMode::Replace,
                        window: request.requestor(),
                        property: request.property(),
                        r#type: request.target(),
                        data: text.as_bytes(),
                    })?;
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
        log::trace!(
            "SEL: window_id={window_id:?} responding with selprop={:?}",
            selprop
        );

        conn.send_request_no_reply(&xcb::x::SendEvent {
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
        })?;

        Ok(())
    }

    fn selection_notify(&mut self, selection: &xcb::x::SelectionNotifyEvent) -> anyhow::Result<()> {
        let conn = self.conn();
        let window_id = self.window_id;
        let selection_name = conn.atom_name(selection.selection());
        let target_name = conn.atom_name(selection.target());

        log::trace!(
            "SEL: window_id={window_id:?} SELECTION_NOTIFY received {selection:?} \
            selection.selection={selection_name} selection.target={target_name}"
        );

        if let Some(clipboard) = self.selection_atom_to_clipboard(selection.selection()) {
            if selection.property() == xcb::x::ATOM_NONE {
                if selection.target() == conn.atom_utf8_string {
                    log::trace!(
                        "SEL: window_id={window_id:?} -> UTF-8 selection data \
                         available, requesting STRING instead"
                    );
                    conn.send_request_no_reply_log(&xcb::x::ConvertSelection {
                        requestor: window_id,
                        selection: selection.selection(),
                        target: xcb::x::ATOM_STRING,
                        property: conn.atom_xsel_data,
                        time: self.copy_and_paste.time,
                    });
                    return Ok(());
                }

                if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take() {
                    log::trace!(
                        "SEL: window_id={window_id:?} -> no compatible selection data \
                         available, fulfil promise with empty string"
                    );
                    promise.ok("".to_owned());
                    return Ok(());
                }
                log::trace!(
                    "SEL: window_id={window_id:?} -> no compatible selection data \
                     available, and no promise. weird!"
                );

                return Ok(());
            }

            match conn.send_and_wait_request(&xcb::x::GetProperty {
                delete: false,
                window: selection.requestor(),
                property: selection.property(),
                r#type: selection.target(),
                long_offset: 0,
                long_length: u32::max_value(),
            }) {
                Ok(prop) => {
                    if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take() {
                        fn latin1_to_string(s: &[u8]) -> String {
                            s.iter().map(|&c| c as char).collect()
                        }

                        let data = if selection.target() == xcb::x::ATOM_STRING {
                            latin1_to_string(prop.value())
                        } else {
                            // selection.target() is probably == conn.atom_utf8_string,
                            // because we only ever ask for either STRING or UTF8_STRING.
                            // If it isn't, we'll just try to convert it anyway.
                            String::from_utf8_lossy(prop.value()).to_string()
                        };

                        promise.ok(data);
                    }

                    conn.send_request_no_reply(&xcb::x::DeleteProperty {
                        window: self.window_id,
                        property: conn.atom_xsel_data,
                    })?;
                }
                Err(err) => {
                    log::error!("clipboard: err while getting clipboard property: {:?}", err);
                    if let Some(mut promise) = self.copy_and_paste.request_mut(clipboard).take() {
                        promise.ok("".to_owned());
                    }
                }
            }
        } else if selection.selection() == conn.atom_xdndselection
            && selection.property() == conn.atom_xsel_data
        {
            if let Some(srcwin) = self.drag_and_drop.src_window {
                match conn.send_and_wait_request(&xcb::x::GetProperty {
                    delete: true,
                    window: selection.requestor(),
                    property: selection.property(),
                    r#type: selection.target(),
                    long_offset: 0,
                    long_length: u32::max_value(),
                }) {
                    Ok(prop) => {
                        if selection.target() == conn.atom_utf8_string {
                            let text = String::from_utf8_lossy(prop.value()).to_string();
                            self.events.dispatch(WindowEvent::DroppedString(text));
                        } else if selection.target() == conn.atom_xmozurl {
                            let data = decode_dropped_url_string(prop.value());
                            let urls = parse_xmozurl_list(&data);
                            self.events.dispatch(WindowEvent::DroppedUrl(urls));
                        } else if selection.target() == conn.atom_texturilist {
                            let paths = parse_texturi_list(prop.value());
                            self.events.dispatch(WindowEvent::DroppedFile(paths));
                        }
                    }
                    Err(err) => {
                        log::error!("clipboard: err while getting clipboard property: {err:#}");
                    }
                }
                conn.send_request_no_reply_log(&xcb::x::SendEvent {
                    propagate: false,
                    destination: xcb::x::SendEventDest::Window(srcwin),
                    event_mask: xcb::x::EventMask::empty(),
                    event: &xcb::x::ClientMessageEvent::new(
                        srcwin,
                        conn.atom_xdndfinished,
                        xcb::x::ClientMessageData::Data32([
                            window_id.resource_id(),
                            1,
                            self.drag_and_drop.target_action.resource_id(),
                            0,
                            0,
                        ]),
                    ),
                });
            } else {
                log::warn!("No Xdnd in progress, but received Xdnd selection. Ignoring.");
            }
        } else {
            log::trace!("SEL: window_id={window_id:?} unknown selection {selection_name}");
        }
        Ok(())
    }

    fn get_window_state(&self) -> anyhow::Result<WindowState> {
        let conn = self.conn();

        let reply = conn.send_and_wait_request(&xcb::x::GetProperty {
            delete: false,
            window: self.window_id,
            property: conn.atom_net_wm_state,
            r#type: xcb::x::ATOM_ATOM,
            long_offset: 0,
            long_length: 1024,
        })?;

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

    fn set_wm_state(
        &mut self,
        action: NetWmStateAction,
        atom: Atom,
        atom2: Option<Atom>,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        let data: [u32; 5] = [
            action as u32,
            atom.resource_id(),
            atom2.map(|a| a.resource_id()).unwrap_or(0),
            0,
            0,
        ];

        // Ask window manager to change our fullscreen state
        conn.send_request_no_reply(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(conn.root),
            event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
                | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
            event: &xcb::x::ClientMessageEvent::new(
                self.window_id,
                conn.atom_net_wm_state,
                xcb::x::ClientMessageData::Data32(data),
            ),
        })?;
        conn.flush()?;
        self.adjust_decorations(self.config.window_decorations)?;

        Ok(())
    }

    fn set_maximized_hint(&mut self, enable: bool) -> anyhow::Result<()> {
        self.set_wm_state(
            NetWmStateAction::with_bool(enable),
            self.conn().atom_state_maximized_vert,
            Some(self.conn().atom_state_maximized_horz),
        )
    }

    fn set_fullscreen_hint(&mut self, enable: bool) -> anyhow::Result<()> {
        self.set_wm_state(
            NetWmStateAction::with_bool(enable),
            self.conn().atom_state_fullscreen,
            None,
        )
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
        } else if decorations == WindowDecorations::RESIZE
            || decorations == WindowDecorations::INTEGRATED_BUTTONS
            || decorations == WindowDecorations::INTEGRATED_BUTTONS | WindowDecorations::RESIZE
        {
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

        conn.send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: conn.atom_motif_wm_hints,
            r#type: conn.atom_motif_wm_hints,
            data: hints_slice,
        })?;
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

        let ResolvedGeometry {
            x,
            y,
            width,
            height,
        } = conn.resolve_geometry(geometry);

        let mut events = WindowEventSender::new(event_handler);

        let window_id;
        let child_id;
        let window = {
            let setup = conn.conn().get_setup();
            let screen = setup
                .roots()
                .nth(conn.screen_num() as usize)
                .ok_or_else(|| anyhow!("no screen?"))?;

            window_id = conn.conn().generate_id();
            child_id = conn.conn().generate_id();

            let color_map_id = conn.conn().generate_id();
            conn.send_request_no_reply(&xcb::x::CreateColormap {
                alloc: xcb::x::ColormapAlloc::None,
                mid: color_map_id,
                window: screen.root(),
                visual: conn.visual.visual_id(),
            })
            .context("create_colormap_checked")?;

            conn.send_request_no_reply(&xcb::x::CreateWindow {
                depth: conn.depth,
                wid: window_id,
                parent: screen.root(),
                x: x.unwrap_or(0).try_into()?,
                y: y.unwrap_or(0).try_into()?,
                width: width.try_into()?,
                height: height.try_into()?,
                border_width: 0,
                class: xcb::x::WindowClass::InputOutput,
                visual: conn.visual.visual_id(),
                value_list: &[
                    // We have to specify both a border pixel color and a colormap
                    // when specifying a depth that doesn't match the root window in
                    // order to avoid a BadMatch
                    xcb::x::Cw::BackPixel(0), // transparent background
                    xcb::x::Cw::BorderPixel(screen.black_pixel()),
                    xcb::x::Cw::EventMask(
                        xcb::x::EventMask::FOCUS_CHANGE
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
            })
            .context("xcb::create_window_checked")?;

            conn.send_request_no_reply(&xcb::x::CreateWindow {
                depth: conn.depth,
                wid: child_id,
                parent: window_id,
                x: 0,
                y: 0,
                width: width.try_into()?,
                height: height.try_into()?,
                border_width: 0,
                class: xcb::x::WindowClass::InputOutput,
                visual: conn.visual.visual_id(),
                value_list: &[
                    // We have to specify both a border pixel color and a colormap
                    // when specifying a depth that doesn't match the root window in
                    // order to avoid a BadMatch
                    xcb::x::Cw::BackPixel(0), // transparent background
                    xcb::x::Cw::BorderPixel(screen.black_pixel()),
                    xcb::x::Cw::BitGravity(xcb::x::Gravity::NorthWest),
                    xcb::x::Cw::EventMask(xcb::x::EventMask::EXPOSURE),
                    xcb::x::Cw::Colormap(color_map_id),
                ],
            })
            .context("xcb::create_window_checked")?;

            conn.send_request_no_reply(&xcb::x::MapWindow { window: child_id })
                .context("xcb::map_window")?;

            events.assign_window(Window::X11(XWindow::from_id(window_id)));

            let appearance = conn.get_appearance();

            Arc::new(Mutex::new(XWindowInner {
                title: String::new(),
                appearance,
                window_id,
                child_id,
                conn: Rc::downgrade(&conn),
                events,
                width: width.try_into()?,
                height: height.try_into()?,
                dpi: conn.default_dpi(),
                copy_and_paste: CopyAndPaste::default(),
                drag_and_drop: DragAndDrop::default(),
                cursors: CursorInfo::new(&config, &conn),
                config: config.clone(),
                has_focus: None,
                verify_focus: true,
                last_cursor_position: Rect::default(),
                paint_throttled: false,
                last_wm_state: WindowState::default(),
                invalidated: false,
                pending: vec![],
                sure_about_geometry: false,
                current_mouse_event: None,
                window_drag_position: None,
                dragging: false,
                outstanding_configure_requests: 0,
                pending_finished_resizes: 0,
            }))
        };

        // WM_CLASS is encoded as the class and instance name,
        // null terminated
        let mut class_string = class_name.as_bytes().to_vec();
        class_string.push(0);
        class_string.extend_from_slice(class_name.as_bytes());
        class_string.push(0);

        conn.send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: xcb::x::ATOM_WM_CLASS,
            r#type: xcb::x::ATOM_STRING,
            data: &class_string,
        })?;

        conn.send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: conn.atom_net_wm_pid,
            r#type: xcb::x::ATOM_CARDINAL,
            data: &[unsafe { libc::getpid() as u32 }],
        })?;

        conn.send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: conn.atom_protocols,
            r#type: xcb::x::ATOM_ATOM,
            data: &[conn.atom_delete],
        })?;

        conn.send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: window_id,
            property: conn.atom_xdndaware,
            r#type: xcb::x::ATOM_ATOM,
            data: &[5u32],
        })?;

        window
            .lock()
            .unwrap()
            .adjust_decorations(config.window_decorations)?;

        let window_handle = Window::X11(XWindow::from_id(window_id));

        conn.windows.borrow_mut().insert(window_id, window);
        conn.child_to_parent_id
            .borrow_mut()
            .insert(child_id, window_id);

        window_handle.set_title(name);
        // Before we map the window, flush to ensure that all of the other properties
        // have been applied to it.
        // This is a speculative fix for this race condition issue:
        // <https://github.com/wez/wezterm/issues/2155>
        conn.flush().context("flushing before mapping window")?;
        window_handle.show();

        // Some window managers will ignore the x,y that we set during window
        // creation, so we ask them again once the window is mapped
        if let (Some(x), Some(y)) = (x, y) {
            window_handle.set_window_position(ScreenPoint::new(x.try_into()?, y.try_into()?));
        }

        if conn
            .active_extensions()
            .any(|e| e == xcb::Extension::Present)
        {
            let event_id = conn.generate_id();
            conn.send_request_no_reply(&xcb::present::SelectInput {
                eid: event_id,
                window: window_id,
                event_mask: xcb::present::EventMask::CONFIGURE_NOTIFY,
            })
            .context("Present::SelectInput")?;
        }

        Ok(window_handle)
    }
}

impl XWindowInner {
    fn close(&mut self) {
        let conn = self.conn();
        conn.flush()
            .context("flush pending requests prior to issuing DestroyWindow")
            .ok();
        // Remove the window from the map now, as GL state
        // requires that it is able to make_current() in its
        // Drop impl, and that cannot succeed after we've
        // destroyed the window at the X11 level.
        self.conn().windows.borrow_mut().remove(&self.window_id);
        self.conn()
            .child_to_parent_id
            .borrow_mut()
            .remove(&self.child_id);

        // Unmap the window first: calling DestroyWindow here may race
        // with some requests made either by EGL or the IME, but I haven't
        // been able to pin down the source.
        // We'll destroy the window in a couple of seconds
        conn.send_request_no_reply_log(&xcb::x::UnmapWindow {
            window: self.window_id,
        });

        conn.send_request_no_reply_log(&xcb::x::DestroyWindow {
            window: self.child_id,
        });

        // Arrange to destroy the window after a couple of seconds; that
        // should give whatever stuff is still referencing the window
        // to finish and avoid triggering a protocol error.
        // I don't really like this as a solution :-/
        // <https://github.com/wez/wezterm/issues/2198>
        let window = self.window_id;
        promise::spawn::spawn(async move {
            async_io::Timer::after(std::time::Duration::from_secs(2)).await;
            let conn = Connection::get().unwrap().x11();
            log::trace!("close sending DestroyWindow for {:?}", window);
            conn.send_request_no_reply_log(&xcb::x::DestroyWindow { window });
        })
        .detach();
        // Ensure that we don't try to destroy the window twice,
        // otherwise the rust xcb bindings will generate a
        // fatal error!
        log::trace!("clear out self.window_id");
        self.window_id = xcb::x::Window::none();
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        self.conn().send_request_no_reply_log(&xcb::x::MapWindow {
            window: self.window_id,
        });
    }

    fn focus(&mut self) {
        let conn = self.conn();
        conn.send_request_no_reply_log(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(conn.root),
            event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
                | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
            event: &xcb::x::ClientMessageEvent::new(
                self.window_id,
                conn.atom_net_active_window,
                xcb::x::ClientMessageData::Data32([
                    1,
                    // You'd think that self.copy_and_paste.time would
                    // be the thing to use, but Mutter ignored this request
                    // until I switched to CURRENT_TIME
                    xcb::x::CURRENT_TIME,
                    0,
                    0,
                    0,
                ]),
            ),
        });

        if let Err(err) = conn.flush() {
            log::error!("Error flushing: {err:#}");
        }
    }

    fn invalidate(&mut self) {
        self.queue_pending(WindowEvent::NeedRepaint);
        self.dispatch_pending_events().ok();
    }

    fn maximize(&mut self) {
        if let Err(err) = self.set_maximized_hint(true) {
            log::error!("Failed to maximize: {err:#}");
        }
    }

    fn restore(&mut self) {
        if let Err(err) = self.set_maximized_hint(false) {
            log::error!("Failed to restore: {err:#}");
        }
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
        let dpi_changed =
            self.config.dpi != config.dpi || self.config.dpi_by_screen != config.dpi_by_screen;
        self.config = config.clone();
        let _ = self.adjust_decorations(config.window_decorations);

        if dpi_changed {
            let _ = self.configure_notify("config reload", self.width, self.height);
        }
    }

    fn net_wm_moveresize(&mut self, x_root: u32, y_root: u32, direction: u32, button: u32) {
        let source_indication = 1;
        let conn = self.conn();

        if !conn
            .supported
            .borrow()
            .contains(&conn.atom_net_wm_moveresize)
        {
            log::debug!("WM doesn't support _NET_WM_MOVERESIZE");
            return;
        }

        log::debug!("net_wm_moveresize {x_root},{y_root} direction={direction} button={button}");

        if direction != _NET_WM_MOVERESIZE_CANCEL {
            // Tell the server to ungrab. Even though we haven't explicitly
            // grabbed it in our application code, there's an implicit grab
            // as part of a mouse drag and the moveresize will do nothing
            // if we don't ungrab it.
            conn.send_request_no_reply_log(&xcb::x::UngrabPointer {
                time: self.copy_and_paste.time,
            });
            // Flag to ourselves that we are dragging.
            // This is also used to gate the fallback of calling
            // set_window_position in case the WM doesn't support
            // _NET_WM_MOVERESIZE and we returned early above.
            self.dragging = true;
        }

        conn.send_request_no_reply_log(&xcb::x::SendEvent {
            propagate: true,
            destination: xcb::x::SendEventDest::Window(conn.root),
            event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
                | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
            event: &xcb::x::ClientMessageEvent::new(
                self.window_id,
                conn.atom_net_wm_moveresize,
                xcb::x::ClientMessageData::Data32([
                    x_root,
                    y_root,
                    direction,
                    button,
                    source_indication,
                ]),
            ),
        });
        conn.flush().context("flush moveresize").ok();
    }

    fn request_drag_move(&mut self) -> anyhow::Result<()> {
        let pos = self.window_drag_position.unwrap_or_default();

        let x_root = pos.x as u32;
        let y_root = pos.y as u32;
        let button = 1; // Left

        self.net_wm_moveresize(x_root, y_root, _NET_WM_MOVERESIZE_MOVE, button);
        Ok(())
    }

    fn set_window_position(&mut self, coords: ScreenPoint) {
        if self.dragging {
            return;
        }

        // We ask the window manager to move the window for us so that
        // we don't have to deal with adjusting for the frame size.
        // Note that neither this technique or the configure_window
        // approach below will successfully move a window running
        // under the crostini environment on a chromebook :-(
        let conn = self.conn();

        conn.send_request_no_reply_log(&xcb::x::SendEvent {
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

        conn.send_request_no_reply_log(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: xcb::x::ATOM_WM_NAME,
            r#type: conn.atom_utf8_string,
            data: title.as_bytes(),
        });

        // Also set EWMH _NET_WM_NAME, as some clients don't correctly
        // fall back to reading WM_NAME
        conn.send_request_no_reply_log(&xcb::x::ChangeProperty {
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

        self.conn()
            .send_request_no_reply_log(&xcb::x::ChangeProperty {
                mode: PropMode::Replace,
                window: self.window_id,
                property: self.conn().atom_net_wm_icon,
                r#type: xcb::x::ATOM_CARDINAL,
                data: &icon_data,
            });
    }

    fn set_resize_increments(&mut self, incr: ResizeIncrement) -> anyhow::Result<()> {
        use xcb_util::*;
        let hints = xcb_size_hints_t {
            flags: XCB_ICCCM_SIZE_HINT_P_MIN_SIZE
                | XCB_ICCCM_SIZE_HINT_P_RESIZE_INC
                | XCB_ICCCM_SIZE_HINT_BASE_SIZE,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            min_width: (incr.base_width + incr.x).into(),
            min_height: (incr.base_height + incr.y).into(),
            max_width: 0,
            max_height: 0,
            width_inc: incr.x.into(),
            height_inc: incr.y.into(),
            min_aspect_num: 0,
            min_aspect_den: 0,
            max_aspect_num: 0,
            max_aspect_den: 0,
            base_width: incr.base_width.into(),
            base_height: incr.base_height.into(),
            win_gravity: 0,
        };

        let data = unsafe {
            std::slice::from_raw_parts(
                &hints as *const _ as *const u32,
                std::mem::size_of::<xcb_size_hints_t>() / 4,
            )
        };

        self.conn().send_request_no_reply(&xcb::x::ChangeProperty {
            mode: PropMode::Replace,
            window: self.window_id,
            property: xcb::x::ATOM_WM_NORMAL_HINTS,
            r#type: xcb::x::ATOM_WM_SIZE_HINTS,
            data,
        })?;

        Ok(())
    }
}

impl HasDisplayHandle for XWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let conn = Connection::get()
            .expect("display_handle only callable on main thread")
            .x11();
        let handle = XcbDisplayHandle::new(NonNull::new(conn.get_raw_conn() as _), conn.screen_num);

        unsafe { Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(handle))) }
    }
}

impl HasWindowHandle for XWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let conn = Connection::get().expect("window_handle only callable on main thread");
        let handle = conn
            .x11()
            .window_by_id(self.0)
            .expect("window handle invalid!?");

        let inner = handle.lock().unwrap();
        let handle = inner.window_handle()?;
        unsafe { Ok(WindowHandle::borrow_raw(handle.as_raw())) }
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

    fn maximize(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.maximize();
            Ok(())
        });
    }

    fn restore(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.restore();
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

    fn focus(&self) {
        XConnection::with_window_inner(self.0, |inner| {
            inner.focus();
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
            inner
                .conn()
                .send_request_no_reply_log(&xcb::x::ConfigureWindow {
                    window: inner.window_id,
                    value_list: &[
                        xcb::x::ConfigWindow::Width(width as u32),
                        xcb::x::ConfigWindow::Height(height as u32),
                    ],
                });
            inner.resize_child(width as u32, height as u32);
            inner.outstanding_configure_requests += 1;
            Ok(())
        });
    }

    fn request_drag_move(&self) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.request_drag_move()?;
            Ok(())
        });
    }

    fn set_window_drag_position(&self, coords: ScreenPoint) {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.window_drag_position.replace(coords);
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

    fn set_resize_increments(&self, incr: ResizeIncrement) {
        XConnection::with_window_inner(self.0, move |inner| {
            if let Err(err) = inner.set_resize_increments(incr) {
                log::error!("set_resize_increments failed: {:#}", err);
            }
            Ok(())
        });
    }

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        let window_id = self.0;
        log::trace!("SEL: window_id={window_id:?} Window::get_clipboard {clipboard:?} called");
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let mut promise = Some(promise);

        XConnection::with_window_inner(window_id, move |inner| {
            // In theory, we could simply consult inner.copy_and_paste to see
            // if we think we own the clipboard, but there are some situations
            // where the selection owner moves between two wezterm windows
            // where we don't receive a SELECTION_NOTIFY in time to correctly
            // invalidate that state, so we always ask the X server to for
            // the selection, even if it is a little slower.
            // <https://github.com/wez/wezterm/issues/2110>
            let promise = promise.take().unwrap();
            log::debug!(
                "SEL: window_id={window_id:?} Window::get_clipboard: \
                        {clipboard:?}, prepare promise, time={}",
                inner.copy_and_paste.time
            );
            inner.copy_and_paste.request_mut(clipboard).replace(promise);
            let conn = inner.conn();
            // Find the owner and ask them to send us the buffer
            conn.send_request_no_reply_log(&xcb::x::ConvertSelection {
                requestor: inner.window_id,
                selection: match clipboard {
                    Clipboard::Clipboard => conn.atom_clipboard,
                    Clipboard::PrimarySelection => xcb::x::ATOM_PRIMARY,
                },
                target: conn.atom_utf8_string,
                property: conn.atom_xsel_data,
                time: inner.copy_and_paste.time,
            });
            Ok(())
        });

        future
    }

    /// Set some text in the clipboard
    fn set_clipboard(&self, clipboard: Clipboard, text: String) {
        let window_id = self.0;
        XConnection::with_window_inner(window_id, move |inner| {
            log::trace!(
                "SEL: window_id={window_id:?} now owns selection \
                for {clipboard:?} {text:?}"
            );
            inner
                .copy_and_paste
                .clipboard_mut(clipboard)
                .replace(text.clone());
            inner.update_selection_owner(clipboard)?;
            Ok(())
        });
    }
}

fn parse_texturi_list(url_list: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(url_list)
        .lines()
        .filter_map(|line| {
            if line.starts_with('#') || line.trim().is_empty() {
                // text/uri-list: Any lines beginning with the '#' character
                // are comment lines and are ignored during processing
                return None;
            }
            let url = Url::parse(line)
                .map_err(|err| {
                    log::error!("Error parsing dropped file line {line} as url: {err:#}");
                })
                .ok()?;
            url.to_file_path()
                .map_err(|_| {
                    log::error!("Error converting url {url:?} from line {line} to pathbuf");
                })
                .ok()
        })
        .collect()
}

fn parse_xmozurl_list(url_list: &str) -> Vec<Url> {
    url_list
        .lines()
        .step_by(2)
        .filter_map(|line| {
            // the lines alternate between the urls and their titles
            Url::parse(line)
                .map_err(|err| {
                    log::error!("Error parsing dropped file line {line} as url: {err:#}");
                })
                .ok()
        })
        .collect()
}

/// Data may be UTF16 in either byte order, or UTF8
fn decode_dropped_url_string(raw: &[u8]) -> String {
    if raw.len() >= 2 && ((raw[0], raw[1]) == (0xfe, 0xff) || (raw[0] != 0x00 && raw[1] == 0x00)) {
        String::from_utf16_lossy(
            raw.chunks_exact(2)
                .map(|x: &[u8]| u16::from(x[1]) << 8 | u16::from(x[0]))
                .collect::<Vec<u16>>()
                .as_slice(),
        )
    } else if raw.len() >= 2
        && ((raw[0], raw[1]) == (0xff, 0xfe) || (raw[0] == 0x00 && raw[1] != 0x00))
    {
        String::from_utf16_lossy(
            raw.chunks_exact(2)
                .map(|x: &[u8]| u16::from(x[0]) << 8 | u16::from(x[1]))
                .collect::<Vec<u16>>()
                .as_slice(),
        )
    } else {
        String::from_utf8_lossy(raw).to_string()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
enum NetWmStateAction {
    Remove = 0,
    Add = 1,
    #[allow(dead_code)]
    Toggle = 2,
}

impl NetWmStateAction {
    fn with_bool(enable: bool) -> Self {
        if enable {
            Self::Add
        } else {
            Self::Remove
        }
    }
}
