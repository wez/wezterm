use super::keyboard::Keyboard;
use crate::connection::ConnectionOps;
use crate::os::x11::window::XWindowInner;
use crate::os::x11::xsettings::*;
use crate::os::Connection;
use crate::spawn::*;
use crate::{Appearance, DeadKeyStatus};
use anyhow::{anyhow, bail, Context as _};
use mio::event::Source;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Registry, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use xcb::x::Atom;

pub struct XConnection {
    pub conn: xcb::Connection,
    default_dpi: RefCell<f64>,
    pub(crate) xsettings: RefCell<XSettingsMap>,
    pub screen_num: i32,
    pub root: xcb::x::Window,
    pub keyboard: Keyboard,
    pub kbd_ev: u8,
    pub atom_protocols: Atom,
    pub cursor_font_id: xcb::x::Font,
    pub atom_delete: Atom,
    pub atom_utf8_string: Atom,
    pub atom_xsel_data: Atom,
    pub atom_targets: Atom,
    pub atom_clipboard: Atom,
    pub atom_gtk_edge_constraints: Atom,
    pub atom_xsettings_selection: Atom,
    pub atom_xsettings_settings: Atom,
    pub atom_manager: Atom,
    pub atom_state_maximized_vert: Atom,
    pub atom_state_maximized_horz: Atom,
    pub atom_state_hidden: Atom,
    pub atom_state_fullscreen: Atom,
    pub atom_net_wm_state: Atom,
    pub atom_motif_wm_hints: Atom,
    pub atom_net_wm_pid: Atom,
    pub atom_net_wm_name: Atom,
    pub atom_net_wm_icon: Atom,
    pub atom_net_move_resize_window: Atom,
    pub(crate) xrm: RefCell<HashMap<String, String>>,
    pub(crate) windows: RefCell<HashMap<xcb::x::Window, Arc<Mutex<XWindowInner>>>>,
    should_terminate: RefCell<bool>,
    pub(crate) visual: xcb::x::Visualtype,
    pub(crate) depth: u8,
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
    pub(crate) ime: RefCell<std::pin::Pin<Box<xcb_imdkit::ImeClient>>>,
    pub(crate) ime_process_event_result: RefCell<anyhow::Result<()>>,
    pub(crate) has_randr: bool,
}

impl std::ops::Deref for XConnection {
    type Target = xcb::Connection;

    fn deref(&self) -> &xcb::Connection {
        &self.conn
    }
}

impl Source for XConnection {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.conn.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.conn.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> std::io::Result<()> {
        SourceFd(&self.conn.as_raw_fd()).deregister(registry)
    }
}

fn window_id_from_event(event: &xcb::Event) -> Option<xcb::x::Window> {
    match event {
        xcb::Event::X(xcb::x::Event::Expose(e)) => Some(e.window()),
        xcb::Event::X(xcb::x::Event::ConfigureNotify(e)) => Some(e.window()),
        xcb::Event::X(xcb::x::Event::KeyPress(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::KeyRelease(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::MotionNotify(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::ButtonPress(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::ButtonRelease(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::ClientMessage(e)) => Some(e.window()),
        xcb::Event::X(xcb::x::Event::DestroyNotify(e)) => Some(e.window()),
        xcb::Event::X(xcb::x::Event::SelectionClear(e)) => Some(e.owner()),
        xcb::Event::X(xcb::x::Event::SelectionNotify(e)) => Some(e.requestor()),
        xcb::Event::X(xcb::x::Event::SelectionRequest(e)) => Some(e.owner()),
        xcb::Event::X(xcb::x::Event::PropertyNotify(e)) => Some(e.window()),
        xcb::Event::X(xcb::x::Event::FocusIn(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::FocusOut(e)) => Some(e.event()),
        xcb::Event::X(xcb::x::Event::LeaveNotify(e)) => Some(e.event()),
        _ => None,
    }
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
        self.conn.flush()?;

        const TOK_XCB: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_xcb = Token(TOK_XCB);
        let tok_spawn = Token(TOK_SPAWN);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.registry().register(
            &mut SourceFd(&self.conn.as_raw_fd()),
            tok_xcb,
            Interest::READABLE,
        )?;
        poll.registry().register(
            &mut SourceFd(&SPAWN_QUEUE.raw_fd()),
            tok_spawn,
            Interest::READABLE,
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

            self.dispatch_pending_events()?;
            if let Err(err) = poll.poll(&mut events, None) {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                bail!("polling for events: {:?}", err);
            }
        }

        Ok(())
    }

    fn beep(&self) {
        self.conn.send_request(&xcb::x::Bell { percent: 0 });
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
        if let Some(event) = self
            .conn
            .poll_for_event()
            .context("X11 connection is broken")?
        {
            if let Err(err) = self.process_xcb_event_ime(&event) {
                return Err(err);
            }
        }
        self.conn.flush()?;

        loop {
            match self.conn.poll_for_queued_event()? {
                None => {
                    self.conn.flush()?;
                    return Ok(());
                }
                Some(event) => self.process_xcb_event_ime(&event)?,
            }
            self.conn.flush()?;
        }
    }

    fn process_xcb_event_ime(&self, event: &xcb::Event) -> anyhow::Result<()> {
        // check for previous errors produced by the IME forward_event callback
        self.ime_process_event_result.replace(Ok(()))?;

        if config::configuration().use_ime && self.ime.borrow_mut().process_event(event) {
            self.ime_process_event_result.replace(Ok(()))
        } else {
            self.process_xcb_event(event)
        }
    }

    fn process_xcb_event(&self, event: &xcb::Event) -> anyhow::Result<()> {
        if let Some(window_id) = window_id_from_event(event) {
            self.process_window_event(window_id, event)?;
        } else if matches!(event, xcb::Event::Xkb(_)) {
            // key press/release are not processed here.
            // xkbcommon depends on those events in order to:
            //    - update modifiers state
            //    - update keymap/state on keyboard changes
            self.keyboard.process_xkb_event(&self.conn, event)?;
        }
        Ok(())
    }

    pub(crate) fn window_by_id(
        &self,
        window_id: xcb::x::Window,
    ) -> Option<Arc<Mutex<XWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Arc::clone)
    }

    fn dispatch_pending_events(&self) -> anyhow::Result<()> {
        for window in self.windows.borrow().values() {
            let mut inner = window.lock().unwrap();
            inner.dispatch_pending_events()?;
        }

        Ok(())
    }

    fn process_window_event(
        &self,
        window_id: xcb::x::Window,
        event: &xcb::Event,
    ) -> anyhow::Result<()> {
        if let Some(window) = self.window_by_id(window_id) {
            let mut inner = window.lock().unwrap();
            inner.dispatch_event(event)?;
        }
        Ok(())
    }

    fn intern_atom(conn: &xcb::Connection, name: &str) -> anyhow::Result<Atom> {
        let cookie = conn.send_request(&xcb::x::InternAtom {
            only_if_exists: false,
            name: name.as_bytes(),
        });
        let reply = conn.wait_for_reply(cookie)?;
        Ok(reply.atom())
    }

    pub(crate) fn create_new() -> anyhow::Result<Rc<XConnection>> {
        let (conn, screen_num) = xcb::Connection::connect_with_xlib_display_and_extensions(
            &[xcb::Extension::Xkb],
            &[
                xcb::Extension::RandR,
                xcb::Extension::Render,
                xcb::Extension::Xkb,
            ],
        )?;

        let atom_protocols = Self::intern_atom(&conn, "WM_PROTOCOLS")?;
        let atom_delete = Self::intern_atom(&conn, "WM_DELETE_WINDOW")?;
        let atom_utf8_string = Self::intern_atom(&conn, "UTF8_STRING")?;
        let atom_xsel_data = Self::intern_atom(&conn, "XSEL_DATA")?;
        let atom_targets = Self::intern_atom(&conn, "TARGETS")?;
        let atom_clipboard = Self::intern_atom(&conn, "CLIPBOARD")?;
        let atom_gtk_edge_constraints = Self::intern_atom(&conn, "_GTK_EDGE_CONSTRAINTS")?;
        let atom_xsettings_selection =
            Self::intern_atom(&conn, &format!("_XSETTINGS_S{}", screen_num))?;
        let atom_xsettings_settings = Self::intern_atom(&conn, "_XSETTINGS_SETTINGS")?;
        let atom_manager = Self::intern_atom(&conn, "MANAGER")?;
        let atom_state_maximized_vert = Self::intern_atom(&conn, "_NET_WM_STATE_MAXIMIZED_VERT")?;
        let atom_state_maximized_horz = Self::intern_atom(&conn, "_NET_WM_STATE_MAXIMIZED_HORZ")?;
        let atom_state_hidden = Self::intern_atom(&conn, "_NET_WM_STATE_HIDDEN")?;
        let atom_state_fullscreen = Self::intern_atom(&conn, "_NET_WM_STATE_FULLSCREEN")?;
        let atom_net_wm_state = Self::intern_atom(&conn, "_NET_WM_STATE")?;
        let atom_motif_wm_hints = Self::intern_atom(&conn, "_MOTIF_WM_HINTS")?;
        let atom_net_wm_pid = Self::intern_atom(&conn, "_NET_WM_PID")?;
        let atom_net_wm_name = Self::intern_atom(&conn, "_NET_WM_NAME")?;
        let atom_net_wm_icon = Self::intern_atom(&conn, "_NET_WM_ICON")?;
        let atom_net_move_resize_window = Self::intern_atom(&conn, "_NET_MOVERESIZE_WINDOW")?;

        let has_randr = conn.active_extensions().any(|e| e == xcb::Extension::RandR);

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
                    if vis.class() == xcb::x::VisualClass::TrueColor
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
        let visual = *visual;

        log::trace!(
            "picked depth {} visual id:0x{:x}, class:{:?}, bits_per_rgb_value:{}, \
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
        conn.check_request(conn.send_request_checked(&xcb::x::OpenFont {
            fid: cursor_font_id,
            name: cursor_font_name.as_bytes(),
        }))
        .context("OpenFont")?;

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
        log::trace!("computed initial dpi: {:?}", default_dpi);

        let input_style = match config::configuration().ime_preedit_rendering {
            config::ImePreeditRendering::Builtin => xcb_imdkit::InputStyle::PREEDIT_CALLBACKS,
            config::ImePreeditRendering::System => xcb_imdkit::InputStyle::DEFAULT,
        };

        xcb_imdkit::ImeClient::set_logger(|msg| log::debug!("Ime: {}", msg));
        let ime = unsafe {
            xcb_imdkit::ImeClient::unsafe_new(
                &conn,
                screen_num,
                input_style,
                config::configuration().xim_im_name.as_deref(),
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
            atom_motif_wm_hints,
            atom_net_wm_pid,
            atom_net_wm_name,
            atom_net_move_resize_window,
            atom_net_wm_icon,
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
            has_randr,
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
        if config::configuration().ime_preedit_rendering == config::ImePreeditRendering::Builtin {
            let conn = conn.clone();
            conn.clone()
                .ime
                .borrow_mut()
                .set_preedit_draw_cb(move |window_id, info| {
                    if let Some(window) = conn.window_by_id(window_id) {
                        let mut inner = window.lock().unwrap();

                        let text = info.text();
                        let status = DeadKeyStatus::Composing(text);
                        inner.dispatch_ime_compose_status(status);
                    }
                });
        }
        if config::configuration().ime_preedit_rendering == config::ImePreeditRendering::Builtin {
            let conn = conn.clone();
            conn.clone()
                .ime
                .borrow_mut()
                .set_preedit_done_cb(move |window_id| {
                    if let Some(window) = conn.window_by_id(window_id) {
                        let mut inner = window.lock().unwrap();
                        inner.dispatch_ime_compose_status(DeadKeyStatus::None);
                    }
                });
        }
        {
            let conn = conn.clone();
            conn.clone()
                .ime
                .borrow_mut()
                .set_forward_event_cb(move |_win, e| {
                    if let err @ Err(_) = conn.process_xcb_event(e) {
                        if let Err(err) = conn.ime_process_event_result.replace(err) {
                            log::warn!("IME process event error dropped: {}", err);
                        }
                    }
                });
        }

        Ok(conn)
    }

    pub fn conn(&self) -> &xcb::Connection {
        &self.conn
    }

    pub fn screen_num(&self) -> i32 {
        self.screen_num
    }

    pub fn atom_delete(&self) -> Atom {
        self.atom_delete
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnOnce(&mut XWindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window: xcb::x::Window,
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
