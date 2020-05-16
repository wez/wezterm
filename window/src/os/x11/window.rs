use super::*;
use crate::bitmaps::*;
use crate::connection::ConnectionOps;
use crate::os::xkeysyms;
use crate::os::{Connection, Window};
use crate::{
    Color, Dimensions, KeyEvent, MouseButtons, MouseCursor, MouseEvent, MouseEventKind, MousePress,
    Operator, PaintContext, Point, Rect, ScreenPoint, Size, WindowCallbacks, WindowOps,
    WindowOpsMut,
};
use anyhow::anyhow;
use promise::{Future, Promise};
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use xcb::ffi::xcb_cursor_t;

struct XcbCursor {
    id: xcb_cursor_t,
    conn: Rc<XConnection>,
}

impl Drop for XcbCursor {
    fn drop(&mut self) {
        xcb::free_cursor(&self.conn, self.id);
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
    conn: Rc<XConnection>,
    callbacks: Box<dyn WindowCallbacks>,
    window_context: Context,
    width: u16,
    height: u16,
    expose: VecDeque<Rect>,
    paint_all: bool,
    buffer_image: BufferImage,
    cursor: Option<MouseCursor>,
    cursors: HashMap<Option<MouseCursor>, XcbCursor>,
    copy_and_paste: CopyAndPaste,
    #[cfg(feature = "opengl")]
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
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
}

struct X11GraphicsContext<'a> {
    buffer: &'a mut dyn BitmapImage,
}

impl<'a> PaintContext for X11GraphicsContext<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        self.buffer.clear_rect(rect, color)
    }

    fn clear(&mut self, color: Color) {
        self.buffer.clear(color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.buffer.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: 96,
        }
    }

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        self.buffer
            .draw_image(dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        self.buffer.draw_line(start, end, color, operator);
    }
}

impl XWindowInner {
    pub fn paint(&mut self) -> anyhow::Result<()> {
        let window_dimensions =
            Rect::from_size(Size::new(self.width as isize, self.height as isize));

        if self.paint_all {
            self.paint_all = false;
            self.expose.clear();
            self.expose.push_back(window_dimensions);
        } else if self.expose.is_empty() {
            return Ok(());
        }

        #[cfg(feature = "opengl")]
        {
            if let Some(gl_context) = self.gl_state.as_ref() {
                self.expose.clear();

                let mut frame = glium::Frame::new(
                    Rc::clone(&gl_context),
                    (u32::from(self.width), u32::from(self.height)),
                );

                self.callbacks.paint_opengl(&mut frame);
                frame.finish()?;
                return Ok(());
            }
        }

        let (buf_width, buf_height) = self.buffer_image.image_dimensions();
        if buf_width != self.width.into() || buf_height != self.height.into() {
            // Window was resized, so we need to update our buffer
            self.buffer_image = BufferImage::new(
                &self.conn,
                self.window_id,
                self.width as usize,
                self.height as usize,
            );
        }

        for rect in self.expose.drain(..) {
            // Clip the rectangle to the current window size.
            // It can be larger than the window size in the case where we are working
            // through a series of resize exposures during a live resize, and we're
            // now sized smaller then when we queued the exposure.
            let rect = Rect::new(
                rect.origin,
                Size::new(
                    rect.size.width.min(self.width as isize),
                    rect.size.height.min(self.height as isize),
                ),
            );

            let mut context = X11GraphicsContext {
                buffer: &mut self.buffer_image,
            };

            self.callbacks.paint(&mut context);

            match &self.buffer_image {
                BufferImage::Shared(ref im) => {
                    self.window_context.copy_area(
                        im,
                        rect.origin.x as i16,
                        rect.origin.y as i16,
                        &self.window_id,
                        rect.origin.x as i16,
                        rect.origin.y as i16,
                        rect.size.width as u16,
                        rect.size.height as u16,
                    );
                }
                BufferImage::Image(ref buffer) => {
                    if rect == window_dimensions {
                        self.window_context.put_image(0, 0, buffer);
                    } else {
                        let mut im =
                            Image::new(rect.size.width as usize, rect.size.height as usize);

                        im.draw_image(Point::new(0, 0), Some(rect), buffer, Operator::Source);

                        self.window_context.put_image(
                            rect.origin.x as i16,
                            rect.origin.y as i16,
                            &im,
                        );
                    }
                }
            }
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

        let cursor_id = match self.cursors.get(&cursor) {
            Some(cursor) => cursor.id,
            None => {
                let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
                    // `/usr/include/X11/cursorfont.h`
                    MouseCursor::Arrow => 132,
                    MouseCursor::Hand => 58,
                    MouseCursor::Text => 152,
                };

                let cursor_id: xcb::ffi::xcb_cursor_t = self.conn.generate_id();
                xcb::create_glyph_cursor(
                    &self.conn,
                    cursor_id,
                    self.conn.cursor_font_id,
                    self.conn.cursor_font_id,
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
                        conn: Rc::clone(&self.conn),
                    },
                );

                cursor_id
            }
        };

        xcb::change_window_attributes(
            &self.conn,
            self.window_id,
            &[(xcb::ffi::XCB_CW_CURSOR, cursor_id)],
        );

        self.cursor = cursor;

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: &xcb::GenericEvent) -> anyhow::Result<()> {
        let r = event.response_type() & 0x7f;
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
                    dpi: 96,
                })
            }
            xcb::KEY_PRESS | xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                self.copy_and_paste.time = key_press.time();
                if let Some((code, mods)) = self.conn.keyboard.process_key_event(key_press) {
                    let key = KeyEvent {
                        key: code,
                        raw_key: None,
                        modifiers: mods,
                        repeat_count: 1,
                        key_is_down: r == xcb::KEY_PRESS,
                    };
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
                if msg.data().data32()[0] == self.conn.atom_delete() && self.callbacks.can_close() {
                    xcb::destroy_window(self.conn.conn(), self.window_id);
                }
            }
            xcb::DESTROY_NOTIFY => {
                self.callbacks.destroy();
                self.conn.windows.borrow_mut().remove(&self.window_id);
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
                    self.conn.atom_xsel_data
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
        for &selection in &[xcb::ATOM_PRIMARY, self.conn.atom_clipboard] {
            let current_owner = xcb::get_selection_owner(&self.conn, selection)
                .get_reply()
                .unwrap()
                .owner();
            if self.copy_and_paste.owned.is_none() && current_owner == self.window_id {
                // We don't have a selection but X thinks we do; disown it!
                xcb::set_selection_owner(
                    &self.conn,
                    xcb::NONE,
                    selection,
                    self.copy_and_paste.time,
                );
            } else if self.copy_and_paste.owned.is_some() && current_owner != self.window_id {
                // We have the selection but X doesn't think we do; assert it!
                xcb::set_selection_owner(
                    &self.conn,
                    self.window_id,
                    selection,
                    self.copy_and_paste.time,
                );
            }
        }
        self.conn.flush();
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
            self.conn.atom_xsel_data,
            self.conn.atom_utf8_string,
            xcb::ATOM_PRIMARY,
            self.conn.atom_clipboard,
        );

        let selprop = if request.target() == self.conn.atom_targets {
            // They want to know which targets we support
            let atoms: [u32; 1] = [self.conn.atom_utf8_string];
            xcb::xproto::change_property(
                &self.conn,
                xcb::xproto::PROP_MODE_REPLACE as u8,
                request.requestor(),
                request.property(),
                xcb::xproto::ATOM_ATOM,
                32, /* 32-bit atom value */
                &atoms,
            );

            // let the requestor know that we set their property
            request.property()
        } else if request.target() == self.conn.atom_utf8_string
            || request.target() == xcb::xproto::ATOM_STRING
        {
            // We'll accept requests for UTF-8 or STRING data.
            // We don't and won't do any conversion from UTF-8 to
            // whatever STRING represents; let's just assume that
            // the other end is going to handle it correctly.
            if let Some(text) = self.copy_and_paste.owned.as_ref() {
                xcb::xproto::change_property(
                    &self.conn,
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
            &self.conn,
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
        log::trace!(
            "SELECTION_NOTIFY received selection={} (prim={} clip={}) target={} property={}",
            selection.selection(),
            xcb::ATOM_PRIMARY,
            self.conn.atom_clipboard,
            selection.target(),
            selection.property()
        );

        if (selection.selection() == xcb::ATOM_PRIMARY
            || selection.selection() == self.conn.atom_clipboard)
            && selection.property() != xcb::NONE
        {
            match xcb_util::icccm::get_text_property(
                &self.conn,
                selection.requestor(),
                selection.property(),
            )
            .get_reply()
            {
                Ok(prop) => {
                    if let Some(mut promise) = self.copy_and_paste.request.take() {
                        promise.ok(prop.name().to_owned());
                    }
                    xcb::delete_property(&self.conn, self.window_id, self.conn.atom_xsel_data);
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

    #[allow(dead_code, clippy::identity_op)]
    fn disable_decorations(&mut self) -> anyhow::Result<()> {
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
            decorations: 0, // off
            input_mode: 0,
            status: 0,
        };

        let hints_slice =
            unsafe { std::slice::from_raw_parts(&hints as *const _ as *const u32, 5) };

        let atom = xcb::intern_atom(self.conn.conn(), false, "_MOTIF_WM_HINTS")
            .get_reply()?
            .atom();
        xcb::change_property(
            self.conn.conn(),
            xcb::PROP_MODE_REPLACE as u8,
            self.window_id,
            atom,
            atom,
            32,
            hints_slice,
        );
        Ok(())
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

            xcb::create_window_checked(
                conn.conn(),
                xcb::COPY_FROM_PARENT as u8,
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
                &[(
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
                )],
            )
            .request_check()?;

            let window_context = Context::new(&conn, &window_id);

            let buffer_image = BufferImage::new(&conn, window_id, width, height);

            Arc::new(Mutex::new(XWindowInner {
                window_id,
                conn: Rc::clone(&conn),
                callbacks,
                window_context,
                width: width.try_into()?,
                height: height.try_into()?,
                expose: VecDeque::new(),
                paint_all: true,
                copy_and_paste: CopyAndPaste::default(),
                buffer_image,
                cursor: None,
                cursors: HashMap::new(),
                #[cfg(feature = "opengl")]
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

        window.lock().unwrap().callbacks.created(&window_handle);

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
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
    fn hide(&mut self) {}
    fn show(&mut self) {
        xcb::map_window(self.conn.conn(), self.window_id);
    }
    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        XWindowInner::set_cursor(self, cursor).unwrap();
    }
    fn invalidate(&mut self) {
        self.paint_all = true;
    }

    fn set_inner_size(&mut self, width: usize, height: usize) {
        xcb::configure_window(
            self.conn.conn(),
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
        xcb_util::ewmh::request_move_resize_window(
            self.conn.ewmh_conn(),
            self.conn.screen_num,
            self.window_id,
            xcb::xproto::GRAVITY_STATIC,
            1, // normal program
            xcb_util::ewmh::MOVE_RESIZE_MOVE
                | xcb_util::ewmh::MOVE_RESIZE_WINDOW_X
                | xcb_util::ewmh::MOVE_RESIZE_WINDOW_Y,
            coords.x as u32,
            coords.y as u32,
            // these dimensions are ignored because we're not
            // passing the relevant MOVE_RESIZE_XX flags above,
            // but are preserved here for clarity on what these
            // parameters do
            self.width as u32,
            self.height as u32,
        );

        /*
        xcb::configure_window(
            self.conn.conn(),
            self.window_id,
            &[
                (xcb::CONFIG_WINDOW_X as u16, coords.x as u32),
                (xcb::CONFIG_WINDOW_Y as u16, coords.y as u32),
            ],
        );
        */
    }

    /// Change the title for the window manager
    fn set_title(&mut self, title: &str) {
        xcb_util::icccm::set_wm_name(self.conn.conn(), self.window_id, title);
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
            self.conn.ewmh_conn(),
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

    #[cfg(feature = "opengl")]
    fn enable_opengl<
        R,
        F: Send
            + 'static
            + Fn(
                &mut dyn Any,
                &dyn WindowOps,
                anyhow::Result<std::rc::Rc<glium::backend::Context>>,
            ) -> anyhow::Result<R>,
    >(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        XConnection::with_window_inner(self.0, move |inner| {
            let window = XWindow(inner.window_id);

            let gl_state = crate::egl::GlState::create(
                Some(inner.conn.display as *const _),
                inner.window_id as *mut _,
            )
            .map(Rc::new)
            .and_then(|state| unsafe {
                Ok(glium::backend::Context::new(
                    Rc::clone(&state),
                    true,
                    if cfg!(debug_assertions) {
                        glium::debug::DebugCallbackBehavior::DebugMessageOnError
                    } else {
                        glium::debug::DebugCallbackBehavior::Ignore
                    },
                )?)
            });

            inner.gl_state = gl_state.as_ref().map(Rc::clone).ok();

            func(inner.callbacks.as_any(), &window, gl_state)
        })
    }

    /// Initiate textual transfer from the clipboard
    fn get_clipboard(&self) -> Future<String> {
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
                // Find the owner and ask them to send us the buffer
                xcb::convert_selection(
                    &inner.conn,
                    inner.window_id,
                    // we used to request the clipboard rather than the
                    // primary selection because, under xwayland, access to the
                    // primary selection is forbidden by default citing a security
                    // concern.
                    // These days we have much better native wayland support, so we
                    // default to PRIMARY and allow setting this env var as an
                    // escape hatch.
                    if std::env::var_os("WEZTERM_X11_PREFER_CLIPBOARD_OVER_PRIMARY").is_some() {
                        inner.conn.atom_clipboard
                    } else {
                        xcb::ATOM_PRIMARY
                    },
                    inner.conn.atom_utf8_string,
                    inner.conn.atom_xsel_data,
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
