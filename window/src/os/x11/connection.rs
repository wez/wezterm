use super::keyboard::Keyboard;
use crate::connection::ConnectionOps;
use crate::os::x11::window::XWindowInner;
use crate::os::x11::xsettings::*;
use crate::os::Connection;
use crate::spawn::*;
use crate::Appearance;
use anyhow::{anyhow, bail, Context as _};
use mio::unix::EventedFd;
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use xcb_util::ffi::keysyms::{xcb_key_symbols_alloc, xcb_key_symbols_free, xcb_key_symbols_t};

pub struct XConnection {
    pub conn: xcb_util::ewmh::Connection,
    default_dpi: RefCell<f64>,
    pub(crate) xsettings: RefCell<XSettingsMap>,
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
    pub atom_xsettings_selection: xcb::Atom,
    pub atom_xsettings_settings: xcb::Atom,
    pub atom_manager: xcb::Atom,
    pub atom_state_maximized_vert: xcb::Atom,
    pub atom_state_maximized_horz: xcb::Atom,
    pub atom_state_hidden: xcb::Atom,
    pub atom_state_fullscreen: xcb::Atom,
    pub atom_net_wm_state: xcb::Atom,
    keysyms: *mut xcb_key_symbols_t,
    pub(crate) xrm: RefCell<HashMap<String, String>>,
    pub(crate) windows: RefCell<HashMap<xcb::xproto::Window, Arc<Mutex<XWindowInner>>>>,
    should_terminate: RefCell<bool>,
    pub(crate) visual: xcb::xproto::Visualtype,
    pub(crate) depth: u8,
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
    pub(crate) ime: RefCell<std::pin::Pin<Box<xcb_imdkit::ImeClient>>>,
    pub(crate) ime_process_event_result: RefCell<anyhow::Result<()>>,
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

    fn get_appearance(&self) -> Appearance {
        if let Some(XSetting::String(name)) = self.xsettings.borrow().get("Net/ThemeName") {
            let lower = name.to_ascii_lowercase();
            match lower.as_str() {
                "highcontrast" => Appearance::LightHighContrast,
                "highcontrastinverse" => Appearance::DarkHighContrast,
                "adwaita" => Appearance::Light,
                "adwaita-dark" => Appearance::Dark,
                lower => {
                    if lower.contains("dark") {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    }
                }
            }
        } else {
            Appearance::Dark
        }
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

        while !*self.should_terminate.borrow() {
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
            if SPAWN_QUEUE.run() {
                // if we processed one, we don't want to sleep because
                // there may be others to deal with
                continue;
            }

            if let Err(err) = poll.poll(&mut events, None) {
                bail!("polling for events: {:?}", err);
            }
        }

        Ok(())
    }
}

fn compute_default_dpi(xrm: &HashMap<String, String>, xsettings: &XSettingsMap) -> f64 {
    if let Some(XSetting::Integer(dpi)) = xsettings.get("Xft/DPI") {
        *dpi as f64 / 1024.0
    } else {
        xrm.get("Xft.dpi")
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("96")
            .parse::<f64>()
            .unwrap_or(crate::DEFAULT_DPI)
    }
}

impl XConnection {
    pub(crate) fn update_xrm(&self) {
        match read_xsettings(
            &self.conn,
            self.atom_xsettings_selection,
            self.atom_xsettings_settings,
        ) {
            Ok(settings) => {
                if *self.xsettings.borrow() != settings {
                    log::trace!("xsettings changed to {:?}", settings);
                    *self.xsettings.borrow_mut() = settings;
                }
            }
            Err(err) => {
                log::trace!("error reading xsettings: {:#}", err);
            }
        }

        let xrm = crate::x11::xrm::parse_root_resource_manager(&self.conn, self.root)
            .unwrap_or(HashMap::new());
        *self.xrm.borrow_mut() = xrm;

        let dpi = compute_default_dpi(&self.xrm.borrow(), &self.xsettings.borrow());
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
                if let Err(err) = self.process_xcb_event_ime(&event) {
                    return Err(err);
                }
            }
        }
        self.conn.flush();

        loop {
            match self.conn.poll_for_queued_event() {
                None => return Ok(()),
                Some(event) => self.process_xcb_event_ime(&event)?,
            }
            self.conn.flush();
        }
    }

    fn process_xcb_event_ime(&self, event: &xcb::GenericEvent) -> anyhow::Result<()> {
        // check for previous errors produced by the IME forward_event callback
        self.ime_process_event_result.replace(Ok(()))?;

        if config::configuration().use_ime && self.ime.borrow_mut().process_event(event) {
            self.ime_process_event_result.replace(Ok(()))
        } else {
            self.process_xcb_event(event)
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

    pub(crate) fn create_new() -> anyhow::Result<Rc<XConnection>> {
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
        let atom_xsettings_selection =
            xcb::intern_atom(&conn, false, &format!("_XSETTINGS_S{}", screen_num))
                .get_reply()?
                .atom();
        let atom_xsettings_settings = xcb::intern_atom(&conn, false, "_XSETTINGS_SETTINGS")
            .get_reply()?
            .atom();
        let atom_manager = xcb::intern_atom(&conn, false, "MANAGER")
            .get_reply()?
            .atom();
        let atom_state_maximized_vert =
            xcb::intern_atom(&conn, false, "_NET_WM_STATE_MAXIMIZED_VERT")
                .get_reply()?
                .atom();
        let atom_state_maximized_horz =
            xcb::intern_atom(&conn, false, "_NET_WM_STATE_MAXIMIZED_HORZ")
                .get_reply()?
                .atom();
        let atom_state_hidden = xcb::intern_atom(&conn, false, "_NET_WM_STATE_HIDDEN")
            .get_reply()?
            .atom();
        let atom_state_fullscreen = xcb::intern_atom(&conn, false, "_NET_WM_STATE_FULLSCREEN")
            .get_reply()?
            .atom();
        let atom_net_wm_state = xcb::intern_atom(&conn, false, "_NET_WM_STATE")
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
                        && vis.bits_per_rgb_value() >= 8
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

        let xsettings = read_xsettings(&conn, atom_xsettings_selection, atom_xsettings_settings)
            .unwrap_or_else(|err| {
                log::trace!("Failed to read xsettings: {:#}", err);
                Default::default()
            });
        log::trace!("xsettings are {:?}", xsettings);

        let default_dpi = RefCell::new(compute_default_dpi(&xrm, &xsettings));

        xcb_imdkit::ImeClient::set_logger(|msg| log::debug!("Ime: {}", msg));
        let ime = unsafe {
            xcb_imdkit::ImeClient::unsafe_new(
                &conn,
                screen_num,
                xcb_imdkit::InputStyle::DEFAULT,
                None,
            )
        };

        let conn = Rc::new(XConnection {
            conn,
            default_dpi,
            xsettings: RefCell::new(xsettings),
            cursor_font_id,
            screen_num,
            root,
            xrm: RefCell::new(xrm),
            atom_protocols,
            atom_clipboard,
            atom_gtk_edge_constraints,
            atom_xsettings_selection,
            atom_xsettings_settings,
            atom_manager,
            atom_delete,
            atom_state_maximized_vert,
            atom_state_maximized_horz,
            atom_state_hidden,
            atom_state_fullscreen,
            atom_net_wm_state,
            keysyms,
            keyboard,
            kbd_ev,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            windows: RefCell::new(HashMap::new()),
            should_terminate: RefCell::new(false),
            depth,
            visual,
            gl_connection: RefCell::new(None),
            ime: RefCell::new(ime),
            ime_process_event_result: RefCell::new(Ok(())),
        });

        {
            let conn = conn.clone();
            conn.clone()
                .ime
                .borrow_mut()
                .set_commit_string_cb(move |window_id, input| {
                    if let Some(window) = conn.window_by_id(window_id) {
                        let mut inner = window.lock().unwrap();
                        inner.dispatch_ime_text(input);
                    }
                });
        }
        {
            let conn = conn.clone();
            conn.clone()
                .ime
                .borrow_mut()
                .set_forward_event_cb(move |_win, e| {
                    if let err @ Err(_) = conn.process_xcb_event(unsafe { std::mem::transmute(e) })
                    {
                        if let Err(err) = conn.ime_process_event_result.replace(err) {
                            log::warn!("IME process event error dropped: {}", err);
                        }
                    }
                });
        }

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
