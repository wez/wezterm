use crate::Window;
use failure::Fallible;
use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use xcb_util::ffi::keysyms::{xcb_key_symbols_alloc, xcb_key_symbols_free, xcb_key_symbols_t};

pub struct Connection {
    pub display: *mut x11::xlib::Display,
    conn: xcb::Connection,
    screen_num: i32,
    pub atom_protocols: xcb::Atom,
    pub atom_delete: xcb::Atom,
    pub atom_utf8_string: xcb::Atom,
    pub atom_xsel_data: xcb::Atom,
    pub atom_targets: xcb::Atom,
    pub atom_clipboard: xcb::Atom,
    keysyms: *mut xcb_key_symbols_t,
    pub(crate) windows: RefCell<HashMap<xcb::xproto::Window, Window>>,
    should_terminate: RefCell<bool>,
}

impl std::ops::Deref for Connection {
    type Target = xcb::Connection;

    fn deref(&self) -> &xcb::Connection {
        &self.conn
    }
}

impl Evented for Connection {
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

#[link(name = "X11-xcb")]
extern "C" {
    fn XGetXCBConnection(display: *mut x11::xlib::Display) -> *mut xcb::ffi::xcb_connection_t;
    fn XSetEventQueueOwner(display: *mut x11::xlib::Display, owner: i32);
}

thread_local! {
    static CONN: RefCell<Option<Arc<Connection>>> = RefCell::new(None);
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
        _ => None,
    }
}

impl Connection {
    pub fn get() -> Option<Arc<Self>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Arc::clone(mux));
            }
        });
        res
    }

    pub fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    pub fn run_message_loop(&self) -> Fallible<()> {
        self.conn.flush();
        while let Some(event) = self.conn.wait_for_event() {
            self.process_xcb_event(&event)?;
            self.conn.flush();
            if *self.should_terminate.borrow() {
                break;
            }
        }

        Ok(())
    }

    fn process_xcb_event(&self, event: &xcb::GenericEvent) -> Fallible<()> {
        if let Some(window_id) = window_id_from_event(event) {
            self.process_window_event(window_id, event)?;
        } else {
            /*
            let r = event.response_type() & 0x7f;
            if r == self.conn.kbd_ev {
                // key press/release are not processed here.
                // xkbcommon depends on those events in order to:
                //    - update modifiers state
                //    - update keymap/state on keyboard changes
                self.conn.keyboard.process_xkb_event(&self.conn, event)?;
            }
            */
        }
        Ok(())
    }

    fn window_by_id(&self, window_id: xcb::xproto::Window) -> Option<Window> {
        self.windows.borrow().get(&window_id).cloned()
    }

    fn process_window_event(
        &self,
        window_id: xcb::xproto::Window,
        event: &xcb::GenericEvent,
    ) -> Fallible<()> {
        if let Some(window) = self.window_by_id(window_id) {
            window.dispatch_event(event)?;
        }
        Ok(())
    }

    pub fn init() -> Fallible<Arc<Connection>> {
        let display = unsafe { x11::xlib::XOpenDisplay(std::ptr::null()) };
        if display.is_null() {
            failure::bail!("failed to open display");
        }
        let screen_num = unsafe { x11::xlib::XDefaultScreen(display) };
        let conn = unsafe { xcb::Connection::from_raw_conn(XGetXCBConnection(display)) };
        unsafe { XSetEventQueueOwner(display, 1) };

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

        let keysyms = unsafe { xcb_key_symbols_alloc(conn.get_raw_conn()) };

        let conn = Arc::new(Connection {
            display,
            conn,
            screen_num,
            atom_protocols,
            atom_clipboard,
            atom_delete,
            keysyms,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            windows: RefCell::new(HashMap::new()),
            should_terminate: RefCell::new(false),
        });

        CONN.with(|m| *m.borrow_mut() = Some(Arc::clone(&conn)));
        Ok(conn)
    }

    pub fn conn(&self) -> &xcb::Connection {
        &self.conn
    }

    pub fn screen_num(&self) -> i32 {
        self.screen_num
    }

    pub fn atom_delete(&self) -> xcb::Atom {
        self.atom_delete
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            xcb_key_symbols_free(self.keysyms);
        }
    }
}
