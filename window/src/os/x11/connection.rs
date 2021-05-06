use super::keyboard::Keyboard;
use crate::connection::ConnectionOps;
use crate::os::x11::window::XWindowInner;
use crate::os::Connection;
use crate::spawn::*;
use crate::timerlist::{TimerEntry, TimerList};
use anyhow::{anyhow, bail, Context as _};
use mio::unix::EventedFd;
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use xcb_util::ffi::keysyms::{xcb_key_symbols_alloc, xcb_key_symbols_free, xcb_key_symbols_t};

pub struct XConnection {
    pub conn: xcb_util::ewmh::Connection,
    default_dpi: RefCell<f64>,
    pub screen_num: i32,
    pub root: xcb::xproto::Window,
    pub keyboard: Keyboard,
    pub kbd_ev: u8,
    pub atom_protocols: xcb::Atom,
    pub cursor_font_id: xcb::ffi::xcb_font_t,
    pub atom_delete: xcb::Atom,
    pub atom_utf8_string: xcb::Atom,
    pub atom_xsel_data: xcb::Atom,
    pub atom_targets: xcb::Atom,
    pub atom_clipboard: xcb::Atom,
    pub atom_gtk_edge_constraints: xcb::Atom,
    keysyms: *mut xcb_key_symbols_t,
    pub(crate) xrm: RefCell<HashMap<String, String>>,
    pub(crate) windows: RefCell<HashMap<xcb::xproto::Window, Arc<Mutex<XWindowInner>>>>,
    should_terminate: RefCell<bool>,
    timers: RefCell<TimerList>,
    pub(crate) visual: xcb::xproto::Visualtype,
    pub(crate) depth: u8,
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
}

impl std::ops::Deref for XConnection {
    type Target = xcb::Connection;

    fn deref(&self) -> &xcb::Connection {
        &self.conn
    }
}

impl Evented for XConnection {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.conn.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.conn.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.conn.as_raw_fd()).deregister(poll)
    }
}

fn window_id_from_event(event: &xcb::GenericEvent) -> Option<xcb::xproto::Window> {
    match event.response_type() & 0x7f {
        xcb::EXPOSE => {
            let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
            Some(expose.window())
        }
        xcb::CONFIGURE_NOTIFY => {
            let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
            Some(cfg.window())
        }
        xcb::KEY_PRESS | xcb::KEY_RELEASE => {
            let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
            Some(key_press.event())
        }
        xcb::MOTION_NOTIFY => {
            let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };
            Some(motion.event())
        }
        xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
            let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };
            Some(button_press.event())
        }
        xcb::CLIENT_MESSAGE => {
            let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
            Some(msg.window())
        }
        xcb::DESTROY_NOTIFY => {
            let msg: &xcb::DestroyNotifyEvent = unsafe { xcb::cast_event(event) };
            Some(msg.window())
        }
        xcb::SELECTION_CLEAR => {
            let msg: &xcb::SelectionClearEvent = unsafe { xcb::cast_event(event) };
            Some(msg.owner())
        }
        xcb::PROPERTY_NOTIFY => {
            let msg: &xcb::PropertyNotifyEvent = unsafe { xcb::cast_event(event) };
            Some(msg.window())
        }
        xcb::SELECTION_NOTIFY => {
            let msg: &xcb::SelectionNotifyEvent = unsafe { xcb::cast_event(event) };
            Some(msg.requestor())
        }
        xcb::SELECTION_REQUEST => {
            let msg: &xcb::SelectionRequestEvent = unsafe { xcb::cast_event(event) };
            Some(msg.owner())
        }
        xcb::FOCUS_IN => {
            let msg: &xcb::FocusInEvent = unsafe { xcb::cast_event(event) };
            Some(msg.event())
        }
        xcb::FOCUS_OUT => {
            let msg: &xcb::FocusOutEvent = unsafe { xcb::cast_event(event) };
            Some(msg.event())
        }
        _ => None,
    }
}

fn connect_with_xlib_display() -> anyhow::Result<(xcb::Connection, i32)> {
    let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
    anyhow::ensure!(!display.is_null(), "failed to open X11 display");
    let default_screen = unsafe { x11::xlib::XDefaultScreen(display) };

    // Note: we don't use xcb::Connection::connect_with_xlib_display because it
    // asserts rather than reports an error if it cannot connect to the server!
    let conn = unsafe { xcb::Connection::new_from_xlib_display(display) };
    conn.set_event_queue_owner(xcb::EventQueueOwner::Xcb);
    Ok((conn, default_screen))
}

impl ConnectionOps for XConnection {
    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn default_dpi(&self) -> f64 {
        *self.default_dpi.borrow()
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        self.conn.flush();

        const TOK_XCB: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_xcb = Token(TOK_XCB);
        let tok_spawn = Token(TOK_SPAWN);

        let poll = Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.register(self, tok_xcb, Ready::readable(), PollOpt::level())?;
        poll.register(
            &*SPAWN_QUEUE,
            tok_spawn,
            Ready::readable(),
            PollOpt::level(),
        )?;

        let paint_interval = Duration::from_millis(25);
        let mut last_interval = Instant::now();

        while !*self.should_terminate.borrow() {
            self.timers.borrow_mut().run_ready();

            let now = Instant::now();
            let diff = now - last_interval;
            let period = if diff >= paint_interval {
                self.do_paint();
                last_interval = now;
                paint_interval
            } else {
                paint_interval - diff
            };

            // Process any events that might have accumulated in the local
            // buffer (eg: due to a flush) before we potentially go to sleep.
            // The locally queued events won't mark the fd as ready, so we
            // could potentially sleep when there is work to be done if we
            // relied solely on that.
            self.process_queued_xcb()?;

            // Check the spawn queue before we try to sleep; there may
            // be work pending and we don't guarantee that there is a
            // 1:1 wakeup to queued function, so we need to be assertive
            // in order to avoid missing wakeups
            let period = if SPAWN_QUEUE.run() {
                // if we processed one, we don't want to sleep because
                // there may be others to deal with
                Duration::new(0, 0)
            } else {
                self.timers
                    .borrow()
                    .time_until_due(Instant::now())
                    .map(|duration| duration.min(period))
                    .unwrap_or(period)
            };

            match poll.poll(&mut events, Some(period)) {
                Ok(_) => {
                    // We process both event sources unconditionally
                    // in the loop above anyway; we're just using
                    // this to get woken up.
                }

                Err(err) => {
                    bail!("polling for events: {:?}", err);
                }
            }
        }

        Ok(())
    }

    fn schedule_timer<F: FnMut() + 'static>(&self, interval: std::time::Duration, callback: F) {
        self.timers.borrow_mut().insert(TimerEntry {
            callback: Box::new(callback),
            due: Instant::now(),
            interval,
        });
    }
}

impl XConnection {
    pub(crate) fn update_xrm(&self) {
        let xrm = crate::x11::xrm::parse_root_resource_manager(&self.conn, self.root)
            .unwrap_or(HashMap::new());
        let dpi = xrm
            .get("Xft.dpi")
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("96")
            .parse::<f64>()
            .unwrap_or(crate::DEFAULT_DPI);

        *self.xrm.borrow_mut() = xrm;
        *self.default_dpi.borrow_mut() = dpi;
    }

    fn process_queued_xcb(&self) -> anyhow::Result<()> {
        match self.conn.poll_for_event() {
            None => match self.conn.has_error() {
                Ok(_) => (),
                Err(err) => {
                    bail!("X11 connection is broken: {:?} {}", err, err.to_string());
                }
            },
            Some(event) => {
                if let Err(err) = self.process_xcb_event(&event) {
                    return Err(err);
                }
            }
        }
        self.conn.flush();

        loop {
            match self.conn.poll_for_queued_event() {
                None => return Ok(()),
                Some(event) => self.process_xcb_event(&event)?,
            }
            self.conn.flush();
        }
    }

    fn process_xcb_event(&self, event: &xcb::GenericEvent) -> anyhow::Result<()> {
        if let Some(window_id) = window_id_from_event(event) {
            self.process_window_event(window_id, event)?;
        } else {
            let r = event.response_type() & 0x7f;
            if r == self.kbd_ev {
                // key press/release are not processed here.
                // xkbcommon depends on those events in order to:
                //    - update modifiers state
                //    - update keymap/state on keyboard changes
                self.keyboard.process_xkb_event(&self.conn, event)?;
            }
        }
        Ok(())
    }

    pub(crate) fn window_by_id(
        &self,
        window_id: xcb::xproto::Window,
    ) -> Option<Arc<Mutex<XWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Arc::clone)
    }

    fn process_window_event(
        &self,
        window_id: xcb::xproto::Window,
        event: &xcb::GenericEvent,
    ) -> anyhow::Result<()> {
        if let Some(window) = self.window_by_id(window_id) {
            let mut inner = window.lock().unwrap();
            inner.dispatch_event(event)?;
        }
        Ok(())
    }

    pub(crate) fn create_new() -> anyhow::Result<XConnection> {
        let (conn, screen_num) = connect_with_xlib_display()?;
        let conn = xcb_util::ewmh::Connection::connect(conn)
            .map_err(|_| anyhow!("failed to init ewmh"))?;

        let atom_protocols = xcb::intern_atom(&conn, false, "WM_PROTOCOLS")
            .get_reply()?
            .atom();
        let atom_delete = xcb::intern_atom(&conn, false, "WM_DELETE_WINDOW")
            .get_reply()?
            .atom();
        let atom_utf8_string = xcb::intern_atom(&conn, false, "UTF8_STRING")
            .get_reply()?
            .atom();
        let atom_xsel_data = xcb::intern_atom(&conn, false, "XSEL_DATA")
            .get_reply()?
            .atom();
        let atom_targets = xcb::intern_atom(&conn, false, "TARGETS")
            .get_reply()?
            .atom();
        let atom_clipboard = xcb::intern_atom(&conn, false, "CLIPBOARD")
            .get_reply()?
            .atom();
        let atom_gtk_edge_constraints = xcb::intern_atom(&conn, false, "_GTK_EDGE_CONSTRAINTS")
            .get_reply()?
            .atom();

        let keysyms = unsafe { xcb_key_symbols_alloc((*conn).get_raw_conn()) };

        let screen = conn
            .get_setup()
            .roots()
            .nth(screen_num as usize)
            .ok_or_else(|| anyhow!("no screen?"))?;

        let mut visuals = vec![];
        for depth in screen.allowed_depths() {
            let depth_bpp = depth.depth();
            if depth_bpp == 24 || depth_bpp == 32 {
                for vis in depth.visuals() {
                    if vis.class() == xcb::xproto::VISUAL_CLASS_TRUE_COLOR as u8
                        && vis.bits_per_rgb_value() == 8
                    {
                        visuals.push((depth_bpp, vis));
                    }
                }
            }
        }
        if visuals.is_empty() {
            bail!("no suitable visuals of depth 24 or 32 are available");
        }
        visuals.sort_by(|(a_depth, _), (b_depth, _)| b_depth.cmp(&a_depth));
        let (depth, visual) = visuals[0];

        log::trace!(
            "picked depth {} visual id:0x{:x}, class:{}, bits_per_rgb_value:{}, \
                    colormap entries:{}, masks: r=0x{:x},g=0x{:x},b=0x{:x}",
            depth,
            visual.visual_id(),
            visual.class(),
            visual.bits_per_rgb_value(),
            visual.colormap_entries(),
            visual.red_mask(),
            visual.green_mask(),
            visual.blue_mask()
        );
        let (keyboard, kbd_ev) = Keyboard::new(&conn)?;

        let cursor_font_id = conn.generate_id();
        let cursor_font_name = "cursor";
        xcb::open_font_checked(&conn, cursor_font_id, cursor_font_name)
            .request_check()
            .context("xcb::open_font_checked")?;

        let root = screen.root();

        let xrm =
            crate::x11::xrm::parse_root_resource_manager(&conn, root).unwrap_or(HashMap::new());
        let default_dpi = RefCell::new(
            xrm.get("Xft.dpi")
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("96")
                .parse::<f64>()
                .unwrap_or(crate::DEFAULT_DPI),
        );

        let conn = XConnection {
            conn,
            default_dpi,
            cursor_font_id,
            screen_num,
            root,
            xrm: RefCell::new(xrm),
            atom_protocols,
            atom_clipboard,
            atom_gtk_edge_constraints,
            atom_delete,
            keysyms,
            keyboard,
            kbd_ev,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            windows: RefCell::new(HashMap::new()),
            should_terminate: RefCell::new(false),
            timers: RefCell::new(TimerList::new()),
            depth,
            visual,
            gl_connection: RefCell::new(None),
        };

        Ok(conn)
    }

    pub fn ewmh_conn(&self) -> &xcb_util::ewmh::Connection {
        &self.conn
    }

    pub fn conn(&self) -> &xcb::Connection {
        &*self.conn
    }

    pub fn screen_num(&self) -> i32 {
        self.screen_num
    }

    pub fn atom_delete(&self) -> xcb::Atom {
        self.atom_delete
    }

    /// Run through all of the windows and cause them to paint if they need it.
    fn do_paint(&self) {
        for window in self.windows.borrow().values() {
            window.lock().unwrap().paint().unwrap();
        }
        self.conn.flush();
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnOnce(&mut XWindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window: xcb::xproto::Window,
        f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();

        promise::spawn::spawn_into_main_thread(async move {
            if let Some(handle) = Connection::get().unwrap().x11().window_by_id(window) {
                let mut inner = handle.lock().unwrap();
                prom.result(f(&mut inner));
            }
        })
        .detach();

        future
    }
}

impl Drop for XConnection {
    fn drop(&mut self) {
        unsafe {
            xcb_key_symbols_free(self.keysyms);
        }
    }
}
