use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::os::xkeysyms;
use crate::os::{Connection, Window};
use crate::{
    Clipboard, Dimensions, MouseButtons, MouseCursor, MouseEvent, MouseEventKind, MousePress,
    Point, Rect, ScreenPoint, Size, WindowCallbacks, WindowOps, WindowOpsMut,
};
use anyhow::{anyhow, Context as _};
use promise::{Future, Promise};
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use xcb::ffi::xcb_cursor_t;

struct XcbCursor {
    id: xcb_cursor_t,
    conn: Weak<XConnection>,
}

impl Drop for XcbCursor {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.upgrade() {
            xcb::free_cursor(&conn.conn, self.id);
        }
    }
}

#[derive(Default)]
struct CopyAndPaste {
    owned: Option<String>,
    request: Option<Promise<String>>,
    time: u32,
}

pub(crate) struct XWindowInner {
    window_id: xcb::xproto::Window,
    conn: Weak<XConnection>,
    callbacks: Box<dyn WindowCallbacks>,
    width: u16,
    height: u16,
    expose: VecDeque<Rect>,
    paint_all: bool,
    cursor: Option<MouseCursor>,
    cursors: HashMap<Option<MouseCursor>, XcbCursor>,
    copy_and_paste: CopyAndPaste,
    gl_state: Option<Rc<glium::backend::Context>>,
}

fn enclosing_boundary_with(a: &Rect, b: &Rect) -> Rect {
    let left = a.min_x().min(b.min_x());
    let right = a.max_x().max(b.max_x());

    let top = a.min_y().min(b.min_y());
    let bottom = a.max_y().max(b.max_y());

    Rect::new(Point::new(left, top), Size::new(right - left, bottom - top))
}

impl Drop for XWindowInner {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.upgrade() {
            xcb::destroy_window(conn.conn(), self.window_id);
        }
    }
}

impl XWindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<()> {
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

        self.gl_state.replace(gl_state.clone());
        let window_handle = Window::X11(XWindow::from_id(self.window_id));
        self.callbacks.created(&window_handle, gl_state)
    }

    pub fn paint(&mut self) -> anyhow::Result<()> {
        if !self.paint_all && self.expose.is_empty() {
            return Ok(());
        }
        self.paint_all = false;
        self.expose.clear();

        if let Some(gl_context) = self.gl_state.as_ref() {
            if gl_context.is_context_lost() {
                log::error!("opengl context was lost; should reinit");
                drop(self.gl_state.take());
                self.enable_opengl()?;
                return self.paint();
            }

            let mut frame = glium::Frame::new(
                Rc::clone(&gl_context),
                (u32::from(self.width), u32::from(self.height)),
            );

            self.callbacks.paint(&mut frame);
            frame.finish()?;
        }

        Ok(())
    }

    /// Add a region to the list of exposed/damaged/dirty regions.
    /// Note that a window resize will likely invalidate the entire window.
    /// If the new region intersects with the prior region, then we expand
    /// it to encompass both.  This avoids bloating the list with a series
    /// of increasing rectangles when resizing larger or smaller.
    fn expose(&mut self, x: u16, y: u16, width: u16, height: u16) {
        let expose = Rect::new(
            Point::new(x as isize, y as isize),
            Size::new(width as isize, height as isize),
        );
        if let Some(prior) = self.expose.back_mut() {
            if prior.intersects(&expose) {
                *prior = enclosing_boundary_with(&prior, &expose);
                return;
            }
        }
        self.expose.push_back(expose);
    }

    fn do_mouse_event(&mut self, event: &MouseEvent) -> anyhow::Result<()> {
        self.callbacks
            .mouse_event(&event, &XWindow::from_id(self.window_id));
        Ok(())
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) -> anyhow::Result<()> {
        if cursor == self.cursor {
            return Ok(());
        }

        let conn = self.conn();

        let cursor_id = match self.cursors.get(&cursor) {
            Some(cursor) => cursor.id,
            None => {
                let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
                    // `/usr/include/X11/cursorfont.h`
                    MouseCursor::Arrow => 132,
                    MouseCursor::Hand => 58,
                    MouseCursor::Text => 152,
                    MouseCursor::SizeUpDown => 116,
                    MouseCursor::SizeLeftRight => 108,
                };

                let cursor_id: xcb::ffi::xcb_cursor_t = conn.generate_id();
                xcb::create_glyph_cursor(
                    &conn,
                    cursor_id,
                    conn.cursor_font_id,
                    conn.cursor_font_id,
                    id_no,
                    id_no + 1,
                    0xffff,
                    0xffff,
                    0xffff,
                    0,
                    0,
                    0,
                );

                self.cursors.insert(
                    cursor,
                    XcbCursor {
                        id: cursor_id,
                        conn: Rc::downgrade(&conn),
                    },
                );

                cursor_id
            }
        };

        xcb::change_window_attributes(
            &conn,
            self.window_id,
            &[(xcb::ffi::XCB_CW_CURSOR, cursor_id)],
        );

        self.cursor = cursor;

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
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                self.width = cfg.width();
                self.height = cfg.height();
                self.callbacks.resize(Dimensions {
                    pixel_width: self.width as usize,
                    pixel_height: self.height as usize,
                    dpi: crate::DEFAULT_DPI as usize,
                })
            }
            xcb::KEY_PRESS | xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                self.copy_and_paste.time = key_press.time();
                if let Some(key) = conn.keyboard.process_key_event(key_press) {
                    let key = key.normalize_shift();
                    self.callbacks
                        .key_event(&key, &XWindow::from_id(self.window_id));
                }
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
                self.do_mouse_event(&event)?;
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
                self.do_mouse_event(&event)?;
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
                if msg.data().data32()[0] == conn.atom_delete() && self.callbacks.can_close() {
                    xcb::destroy_window(conn.conn(), self.window_id);
                }
            }
            xcb::DESTROY_NOTIFY => {
                self.callbacks.destroy();
                conn.windows.borrow_mut().remove(&self.window_id);
            }
            xcb::SELECTION_CLEAR => {
                self.selection_clear()?;
            }
            xcb::SELECTION_REQUEST => {
                self.selection_request(unsafe { xcb::cast_event(event) })?;
            }
            xcb::SELECTION_NOTIFY => {
                self.selection_notify(unsafe { xcb::cast_event(event) })?;
            }
            xcb::PROPERTY_NOTIFY => {
                let msg: &xcb::PropertyNotifyEvent = unsafe { xcb::cast_event(event) };
                log::trace!(
                    "PropertyNotifyEvent atom={} xsel={}",
                    msg.atom(),
                    conn.atom_xsel_data
                );
            }
            xcb::FOCUS_IN => {
                log::trace!("Calling focus_change(true)");
                self.callbacks.focus_change(true);
            }
            xcb::FOCUS_OUT => {
                log::trace!("Calling focus_change(false)");
                self.callbacks.focus_change(false);
            }
            _ => {
                eprintln!("unhandled: {:x}", r);
            }
        }

        Ok(())
    }

    /// If we own the selection, make sure that the X server reflects
    /// that and vice versa.
    fn update_selection_owner(&mut self) {
        let conn = self.conn();
        for &selection in &[xcb::ATOM_PRIMARY, conn.atom_clipboard] {
            let current_owner = xcb::get_selection_owner(&conn, selection)
                .get_reply()
                .unwrap()
                .owner();
            if self.copy_and_paste.owned.is_none() && current_owner == self.window_id {
                // We don't have a selection but X thinks we do; disown it!
                xcb::set_selection_owner(&conn, xcb::NONE, selection, self.copy_and_paste.time);
            } else if self.copy_and_paste.owned.is_some() && current_owner != self.window_id {
                // We have the selection but X doesn't think we do; assert it!
                xcb::set_selection_owner(
                    &conn,
                    self.window_id,
                    selection,
                    self.copy_and_paste.time,
                );
            }
        }
        conn.flush();
    }

    fn selection_clear(&mut self) -> anyhow::Result<()> {
        self.copy_and_paste.owned.take();
        self.copy_and_paste.request.take();
        self.update_selection_owner();
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
            // We'll accept requests for UTF-8 or STRING data.
            // We don't and won't do any conversion from UTF-8 to
            // whatever STRING represents; let's just assume that
            // the other end is going to handle it correctly.
            if let Some(text) = self.copy_and_paste.owned.as_ref() {
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

        if (selection.selection() == xcb::ATOM_PRIMARY
            || selection.selection() == conn.atom_clipboard)
            && selection.property() != xcb::NONE
        {
            match xcb_util::icccm::get_text_property(
                &conn,
                selection.requestor(),
                selection.property(),
            )
            .get_reply()
            {
                Ok(prop) => {
                    if let Some(mut promise) = self.copy_and_paste.request.take() {
                        promise.ok(prop.name().to_owned());
                    }
                    xcb::delete_property(&conn, self.window_id, conn.atom_xsel_data);
                }
                Err(err) => {
                    log::error!("clipboard: err while getting clipboard property: {:?}", err);
                }
            }
        } else if let Some(mut promise) = self.copy_and_paste.request.take() {
            promise.ok("".to_owned());
        }
        Ok(())
    }

    fn is_fullscreen(&self) -> anyhow::Result<bool> {
        let conn = self.conn();

        let net_wm_state = xcb::intern_atom(conn.conn(), false, "_NET_WM_STATE")
            .get_reply()?
            .atom();
        let net_wm_state_fullscreen =
            xcb::intern_atom(conn.conn(), false, "_NET_WM_STATE_FULLSCREEN")
                .get_reply()?
                .atom();

        let reply = xcb::xproto::get_property(
            &conn,
            false,
            self.window_id,
            net_wm_state,
            xcb::xproto::ATOM_ATOM,
            0,
            1024,
        )
        .get_reply()?;

        let state = reply.value::<u32>();

        Ok(state
            .iter()
            .position(|&x| x == net_wm_state_fullscreen)
            .is_some())
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

        Ok(())
    }

    #[allow(dead_code, clippy::identity_op)]
    fn adjust_decorations(&mut self, enable: bool) -> anyhow::Result<()> {
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

        const HINTS_FUNCTIONS: u32 = 1 << 0;
        const HINTS_DECORATIONS: u32 = 1 << 1;
        const FUNC_ALL: u32 = 1 << 0;
        const FUNC_RESIZE: u32 = 1 << 1;
        const FUNC_MOVE: u32 = 1 << 2;
        const FUNC_MINIMIZE: u32 = 1 << 3;
        const FUNC_MAXIMIZE: u32 = 1 << 4;
        const FUNC_CLOSE: u32 = 1 << 5;

        let hints = MwmHints {
            flags: HINTS_DECORATIONS,
            functions: 0,
            decorations: if enable { FUNC_ALL } else { 0 },
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
    pub fn new_window(
        class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> anyhow::Result<Window> {
        let conn = Connection::get()
            .ok_or_else(|| {
                anyhow!(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
            })?
            .x11();

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

            Arc::new(Mutex::new(XWindowInner {
                window_id,
                conn: Rc::downgrade(&conn),
                callbacks,
                width: width.try_into()?,
                height: height.try_into()?,
                expose: VecDeque::new(),
                paint_all: true,
                copy_and_paste: CopyAndPaste::default(),
                cursor: None,
                cursors: HashMap::new(),
                gl_state: None,
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

        // window.lock().unwrap().disable_decorations()?;

        let window_handle = Window::X11(XWindow::from_id(window_id));

        window.lock().unwrap().enable_opengl()?;

        conn.windows.borrow_mut().insert(window_id, window);

        window_handle.set_title(name);
        window_handle.show();

        Ok(window_handle)
    }
}

impl Drawable for XWindow {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.0
    }
}

impl WindowOpsMut for XWindowInner {
    fn close(&mut self) {
        xcb::destroy_window(self.conn().conn(), self.window_id);
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        xcb::map_window(self.conn().conn(), self.window_id);
    }
    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        XWindowInner::set_cursor(self, cursor).unwrap();
    }
    fn invalidate(&mut self) {
        self.paint_all = true;
    }

    fn toggle_fullscreen(&mut self) {
        let fullscreen = match self.is_fullscreen() {
            Ok(f) => f,
            Err(err) => {
                log::error!("Failed to determine fullscreen state: {}", err);
                return;
            }
        };
        self.set_fullscreen_hint(!fullscreen).ok();
    }

    fn set_inner_size(&mut self, width: usize, height: usize) {
        xcb::configure_window(
            self.conn().conn(),
            self.window_id,
            &[
                (xcb::CONFIG_WINDOW_WIDTH as u16, width as u32),
                (xcb::CONFIG_WINDOW_HEIGHT as u16, height as u32),
            ],
        );
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
        xcb_util::icccm::set_wm_name(self.conn().conn(), self.window_id, title);
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
}

impl WindowOps for XWindow {
    fn close(&self) -> Future<()> {
        XConnection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn hide(&self) -> Future<()> {
        XConnection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        })
    }

    fn toggle_fullscreen(&self) -> Future<()> {
        XConnection::with_window_inner(self.0, |inner| {
            inner.toggle_fullscreen();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        XConnection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        })
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        XConnection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        XConnection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn set_icon(&self, image: Image) -> Future<()> {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.set_icon(&image);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + FnMut(&mut dyn Any, &dyn WindowOps) -> anyhow::Result<R>>(
        &self,
        mut func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        XConnection::with_window_inner(self.0, move |inner| {
            let window = XWindow(inner.window_id);
            func(inner.callbacks.as_any(), &window)
        })
    }

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self, clipboard: Clipboard) -> Future<String> {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let mut promise = Some(promise);
        XConnection::with_window_inner(self.0, move |inner| {
            let mut promise = promise.take().unwrap();
            if let Some(text) = inner.copy_and_paste.owned.as_ref() {
                promise.ok(text.to_owned());

                // Cancel any outstanding promise from the other branch
                // below.
                inner.copy_and_paste.request.take();
            } else {
                log::debug!("prepare promise, time={}", inner.copy_and_paste.time);
                inner.copy_and_paste.request.replace(promise);
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
    fn set_clipboard(&self, text: String) -> Future<()> {
        XConnection::with_window_inner(self.0, move |inner| {
            inner.copy_and_paste.owned.replace(text.clone());
            inner.update_selection_owner();
            Ok(())
        })
    }
}
