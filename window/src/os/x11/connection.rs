use super::keyboard::{Keyboard, KeyboardWithFallback};
use crate::connection::ConnectionOps;
use crate::os::x11::window::XWindowInner;
use crate::os::x11::xsettings::*;
use crate::os::Connection;
use crate::screen::{ScreenInfo, Screens};
use crate::spawn::*;
use crate::{Appearance, DeadKeyStatus, ScreenRect};
use anyhow::{anyhow, bail, Context as _};
use mio::event::Source;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Registry, Token};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::os::unix::io::AsRawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use x11::xlib;
use xcb::x::Atom;
use xcb::{dri2, Raw, Xid};

enum ScreenResources {
    Current(xcb::randr::GetScreenResourcesCurrentReply),
    All(xcb::randr::GetScreenResourcesReply),
}

impl ScreenResources {
    fn outputs(&self) -> &[xcb::randr::Output] {
        match self {
            Self::Current(cur) => cur.outputs(),
            Self::All(all) => all.outputs(),
        }
    }

    fn config_timestamp(&self) -> xcb::x::Timestamp {
        match self {
            Self::Current(cur) => cur.config_timestamp(),
            Self::All(all) => all.config_timestamp(),
        }
    }

    pub fn modes(&self) -> &[xcb::randr::ModeInfo] {
        match self {
            Self::Current(cur) => cur.modes(),
            Self::All(all) => all.modes(),
        }
    }
}

pub struct XConnection {
    pub conn: xcb::Connection,
    default_dpi: RefCell<f64>,
    pub(crate) xsettings: RefCell<XSettingsMap>,
    pub screen_num: i32,
    pub root: xcb::x::Window,
    pub keyboard: KeyboardWithFallback,
    pub kbd_ev: u8,
    pub atom_protocols: Atom,
    pub cursor_font_id: xcb::x::Font,
    pub atom_delete: Atom,
    pub atom_utf8_string: Atom,
    pub atom_xsel_data: Atom,
    pub atom_targets: Atom,
    pub atom_clipboard: Atom,
    pub atom_texturilist: Atom,
    pub atom_xmozurl: Atom,
    pub atom_xdndaware: Atom,
    pub atom_xdndtypelist: Atom,
    pub atom_xdndselection: Atom,
    pub atom_xdndenter: Atom,
    pub atom_xdndposition: Atom,
    pub atom_xdndstatus: Atom,
    pub atom_xdndleave: Atom,
    pub atom_xdnddrop: Atom,
    pub atom_xdndfinished: Atom,
    pub atom_xdndactioncopy: Atom,
    pub atom_xdndactionmove: Atom,
    pub atom_xdndactionlink: Atom,
    pub atom_xdndactionask: Atom,
    pub atom_xdndactionprivate: Atom,
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
    pub atom_net_wm_moveresize: Atom,
    pub atom_net_supported: Atom,
    pub atom_net_supporting_wm_check: Atom,
    pub atom_net_active_window: Atom,
    pub(crate) xrm: RefCell<HashMap<String, String>>,
    pub(crate) windows: RefCell<HashMap<xcb::x::Window, Arc<Mutex<XWindowInner>>>>,
    pub(crate) child_to_parent_id: RefCell<HashMap<xcb::x::Window, xcb::x::Window>>,
    should_terminate: RefCell<bool>,
    pub(crate) visual: xcb::x::Visualtype,
    pub(crate) depth: u8,
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
    pub(crate) ime: RefCell<std::pin::Pin<Box<xcb_imdkit::ImeClient>>>,
    pub(crate) ime_process_event_result: RefCell<anyhow::Result<()>>,
    pub(crate) has_randr: bool,
    pub(crate) atom_names: RefCell<HashMap<Atom, String>>,
    pub(crate) supported: RefCell<HashSet<Atom>>,
    pub(crate) screens: RefCell<Option<Screens>>,
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
        xcb::Event::Present(xcb::present::Event::ConfigureNotify(e)) => Some(e.window()),
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

/// Returns the name of the window manager
fn get_wm_name(
    conn: &xcb::Connection,
    root: xcb::x::Window,
    atom_net_supporting_wm_check: Atom,
    atom_net_wm_name: Atom,
    atom_utf8_string: Atom,
) -> anyhow::Result<String> {
    let reply = conn
        .wait_for_reply(conn.send_request(&xcb::x::GetProperty {
            delete: false,
            window: root,
            property: atom_net_supporting_wm_check,
            r#type: xcb::x::ATOM_WINDOW,
            long_offset: 0,
            long_length: 4,
        }))
        .context("GetProperty _NET_SUPPORTING_WM_CHECK")?;

    let wm_window = match reply.value::<xcb::x::Window>().get(0) {
        Some(w) => *w,
        None => anyhow::bail!("empty list of windows"),
    };

    let reply = conn
        .wait_for_reply(conn.send_request(&xcb::x::GetProperty {
            delete: false,
            window: wm_window,
            property: atom_net_wm_name,
            r#type: atom_utf8_string,
            long_offset: 0,
            long_length: 1024,
        }))
        .context("GetProperty _NET_WM_NAME from window manager")?;
    Ok(String::from_utf8_lossy(reply.value::<u8>()).to_string())
}

impl ConnectionOps for XConnection {
    fn name(&self) -> String {
        match get_wm_name(
            &self.conn,
            self.root,
            self.atom_net_supporting_wm_check,
            self.atom_net_wm_name,
            self.atom_utf8_string,
        ) {
            Ok(name) => format!("X11 {name}"),
            Err(err) => {
                log::error!("error fetching window manager name: {err:#}");

                "X11".to_string()
            }
        }
    }

    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn default_dpi(&self) -> f64 {
        *self.default_dpi.borrow()
    }

    fn get_appearance(&self) -> Appearance {
        match promise::spawn::block_on(crate::os::xdg_desktop_portal::get_appearance()) {
            Ok(Some(appearance)) => return appearance,
            Ok(None) => {}
            Err(err) => {
                log::warn!("Unable to resolve appearance using xdg-desktop-portal: {err:#}");
            }
        }
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

    fn screens(&self) -> anyhow::Result<Screens> {
        if !self.has_randr {
            anyhow::bail!("XRANDR is not available, cannot query screen geometry");
        }

        let config = config::configuration();

        // NOTE: GetScreenResourcesCurrent is fast, but may sometimes return nothing. In this case,
        // fallback to slow GetScreenResources.
        //
        // references:
        // - https://github.com/qt/qtbase/blob/c234700c836777d08db6229fdc997cc7c99e45fb/src/plugins/platforms/xcb/qxcbscreen.cpp#L963
        // - https://github.com/qt/qtbase/blob/c234700c836777d08db6229fdc997cc7c99e45fb/src/plugins/platforms/xcb/qxcbconnection_screens.cpp#L390
        //
        // related issue: https://github.com/wezterm/wezterm/issues/5802
        let res = match self
            .send_and_wait_request(&xcb::randr::GetScreenResourcesCurrent { window: self.root })
            .context("get_screen_resources_current")
        {
            Ok(cur) if cur.outputs().len() > 0 => ScreenResources::Current(cur),
            _ => ScreenResources::All(
                self.send_and_wait_request(&xcb::randr::GetScreenResources { window: self.root })
                    .context("get_screen_resources")?,
            ),
        };

        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        let mut by_name = HashMap::new();

        for &o in res.outputs() {
            let info = self
                .send_and_wait_request(&xcb::randr::GetOutputInfo {
                    output: o,
                    config_timestamp: res.config_timestamp(),
                })
                .context("get_output_info")?;
            let name = String::from_utf8_lossy(info.name()).to_string();
            let c = info.crtc();
            if let Ok(cinfo) = self.send_and_wait_request(&xcb::randr::GetCrtcInfo {
                crtc: c,
                config_timestamp: res.config_timestamp(),
            }) {
                let mode = cinfo.mode();
                let max_fps = res
                    .modes()
                    .iter()
                    .find(|m| m.id == mode.resource_id())
                    .and_then(|m| {
                        use xcb::randr::ModeFlag;
                        let mut vtotal = m.vtotal;
                        if m.mode_flags.contains(ModeFlag::DOUBLE_SCAN) {
                            // Doublescan doubles the number of lines
                            vtotal *= 2;
                        }
                        if m.mode_flags.contains(ModeFlag::INTERLACE) {
                            // Interlace splits the frame into two fields.
                            // The field rate is what is typically reported
                            // by monitors.
                            vtotal /= 2;
                        }
                        if m.htotal > 0 && vtotal > 0 {
                            Some(
                                (m.dot_clock as f32 / (m.htotal as f32 * vtotal as f32)).ceil()
                                    as usize,
                            )
                        } else {
                            None
                        }
                    });
                let bounds = euclid::rect(
                    cinfo.x() as isize,
                    cinfo.y() as isize,
                    cinfo.width() as isize,
                    cinfo.height() as isize,
                );
                virtual_rect = virtual_rect.union(&bounds);

                let mut effective_dpi = Some(self.default_dpi());
                if let Some(dpi) = config.dpi_by_screen.get(&name).copied() {
                    effective_dpi.replace(dpi);
                } else if let Some(dpi) = config.dpi {
                    effective_dpi.replace(dpi);
                }

                let info = ScreenInfo {
                    name: name.clone(),
                    rect: bounds,
                    scale: 1.0,
                    max_fps,
                    effective_dpi,
                };
                by_name.insert(name, info);
            }
        }

        // The main screen is the one either at the origin of
        // the virtual area, or if that doesn't exist for some weird
        // reason, the screen closest to the origin.
        let main = by_name
            .values()
            .min_by_key(|screen| {
                screen
                    .rect
                    .origin
                    .to_f32()
                    .distance_to(euclid::Point2D::origin())
                    .abs() as isize
            })
            .ok_or_else(|| anyhow::anyhow!("no screens were found"))?
            .clone();

        let active = self
            .screen_from_focused_window(&by_name)
            .unwrap_or_else(|_| main.clone());

        Ok(Screens {
            main,
            active,
            by_name,
            virtual_rect,
        })
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
            self.process_queued_xcb().context("process_queued_xcb")?;

            // Check the spawn queue before we try to sleep; there may
            // be work pending and we don't guarantee that there is a
            // 1:1 wakeup to queued function, so we need to be assertive
            // in order to avoid missing wakeups
            if SPAWN_QUEUE.run() {
                // if we processed one, we don't want to sleep because
                // there may be others to deal with
                continue;
            }

            self.dispatch_pending_events()
                .context("dispatch_pending_events")?;
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
        self.update_net_supported();
    }

    fn update_net_supported(&self) {
        if let Ok(reply) = self.send_and_wait_request(&xcb::x::GetProperty {
            delete: false,
            window: self.root,
            property: self.atom_net_supported,
            r#type: xcb::x::ATOM_ATOM,
            long_offset: 0,
            long_length: 1024,
        }) {
            let supported: HashSet<Atom> = reply.value::<Atom>().iter().copied().collect();
            *self.supported.borrow_mut() = supported;
        }
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {
        for win in self.windows.borrow().values() {
            win.lock().unwrap().appearance_changed(appearance);
        }
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
        self.conn.flush().context("flushing pending requests")?;

        loop {
            match self
                .conn
                .poll_for_queued_event()
                .context("poll_for_queued_event")?
            {
                None => {
                    self.conn.flush().context("flushing pending requests")?;
                    return Ok(());
                }
                Some(event) => self
                    .process_xcb_event_ime(&event)
                    .context("process_xcb_event_ime")?,
            }
            self.conn.flush().context("flushing pending requests")?;
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

    unsafe fn rewire_event(&self, raw_ev: *mut xcb::ffi::xcb_generic_event_t) {
        let ev_type = ((*raw_ev).response_type & 0x7f) as i32;

        if let Some(func) = xlib::XESetWireToEvent(self.conn.get_raw_dpy(), ev_type, None) {
            xlib::XESetWireToEvent(self.conn.get_raw_dpy(), ev_type, Some(func));
            (*raw_ev).sequence = xlib::XLastKnownRequestProcessed(self.conn.get_raw_dpy()) as u16;
            let mut dummy: xlib::XEvent = std::mem::zeroed();
            func(
                self.conn.get_raw_dpy(),
                &mut dummy as *mut xlib::XEvent,
                raw_ev as *mut xlib::xEvent,
            );
        }
    }

    pub(crate) fn get_cached_screens(&self) -> anyhow::Result<Screens> {
        {
            let screens = self.screens.borrow();
            if let Some(cached) = screens.as_ref() {
                return Ok(cached.clone());
            }
        }

        let screens = self.screens()?;

        self.screens.borrow_mut().replace(screens.clone());

        Ok(screens)
    }

    fn process_xcb_event(&self, event: &xcb::Event) -> anyhow::Result<()> {
        match event {
            // Following stuff is not obvious at all.
            // This was necessary in the past to handle GL when XCB owns the event queue.
            // It may not be necessary anymore, but it is included here
            // because <https://github.com/wezterm/wezterm/issues/1992> is a resize related
            // issue and it might possibly be related to these dri2 related issues:
            // <https://bugs.freedesktop.org/show_bug.cgi?id=35945#c4>
            // and mailing thread starting here:
            // <http://lists.freedesktop.org/archives/xcb/2015-November/010556.html>
            xcb::Event::Dri2(dri2::Event::BufferSwapComplete(ev)) => unsafe {
                self.rewire_event(ev.as_raw())
            },
            xcb::Event::Dri2(dri2::Event::InvalidateBuffers(ev)) => unsafe {
                self.rewire_event(ev.as_raw())
            },
            xcb::Event::RandR(randr) => {
                log::trace!("{randr:?}");
                // Clear our cache
                self.screens.borrow_mut().take();
            }
            _ => {}
        }

        if let Some(window_id) = window_id_from_event(event) {
            self.process_window_event(window_id, event)?;
        } else if matches!(event, xcb::Event::Xkb(_)) {
            // key press/release are not processed here.
            // xkbcommon depends on those events in order to:
            //    - update modifiers state
            //    - update keymap/state on keyboard changes
            if let Some((mods, leds)) = self.keyboard.process_xkb_event(&self.conn, event)? {
                // route changed state to the window with focus
                for window in self.windows.borrow().values() {
                    let mut window = window.lock().unwrap();
                    if window.has_focus == Some(true) {
                        window
                            .events
                            .dispatch(crate::WindowEvent::AdviseModifiersLedStatus(mods, leds));
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn window_by_id(
        &self,
        window_id: xcb::x::Window,
    ) -> Option<Arc<Mutex<XWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Arc::clone)
    }

    fn parent_id_by_child_id(&self, child_id: xcb::x::Window) -> Option<xcb::x::Window> {
        self.child_to_parent_id.borrow().get(&child_id).copied()
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
        } else if let Some(parent_id) = self.parent_id_by_child_id(window_id) {
            if let Some(window) = self.window_by_id(parent_id) {
                let mut inner = window.lock().unwrap();
                inner.dispatch_event(event)?;
            }
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
                xcb::Extension::Present,
                xcb::Extension::RandR,
                xcb::Extension::Render,
                xcb::Extension::Dri2,
            ],
        )?;
        conn.set_event_queue_owner(xcb::EventQueueOwner::Xcb);

        let atom_protocols = Self::intern_atom(&conn, "WM_PROTOCOLS")?;
        let atom_delete = Self::intern_atom(&conn, "WM_DELETE_WINDOW")?;
        let atom_utf8_string = Self::intern_atom(&conn, "UTF8_STRING")?;
        let atom_xsel_data = Self::intern_atom(&conn, "XSEL_DATA")?;
        let atom_targets = Self::intern_atom(&conn, "TARGETS")?;
        let atom_clipboard = Self::intern_atom(&conn, "CLIPBOARD")?;
        let atom_texturilist = Self::intern_atom(&conn, "text/uri-list")?;
        let atom_xmozurl = Self::intern_atom(&conn, "text/x-moz-url")?;
        let atom_xdndaware = Self::intern_atom(&conn, "XdndAware")?;
        let atom_xdndtypelist = Self::intern_atom(&conn, "XdndTypeList")?;
        let atom_xdndselection = Self::intern_atom(&conn, "XdndSelection")?;
        let atom_xdndenter = Self::intern_atom(&conn, "XdndEnter")?;
        let atom_xdndposition = Self::intern_atom(&conn, "XdndPosition")?;
        let atom_xdndstatus = Self::intern_atom(&conn, "XdndStatus")?;
        let atom_xdndleave = Self::intern_atom(&conn, "XdndLeave")?;
        let atom_xdnddrop = Self::intern_atom(&conn, "XdndDrop")?;
        let atom_xdndfinished = Self::intern_atom(&conn, "XdndFinished")?;
        let atom_xdndactioncopy = Self::intern_atom(&conn, "XdndActionCopy")?;
        let atom_xdndactionmove = Self::intern_atom(&conn, "XdndActionMove")?;
        let atom_xdndactionlink = Self::intern_atom(&conn, "XdndActionLink")?;
        let atom_xdndactionask = Self::intern_atom(&conn, "XdndActionAsk")?;
        let atom_xdndactionprivate = Self::intern_atom(&conn, "XdndActionPrivate")?;
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
        let atom_net_wm_moveresize = Self::intern_atom(&conn, "_NET_WM_MOVERESIZE")?;
        let atom_net_supported = Self::intern_atom(&conn, "_NET_SUPPORTED")?;
        let atom_net_supporting_wm_check = Self::intern_atom(&conn, "_NET_SUPPORTING_WM_CHECK")?;
        let atom_net_active_window = Self::intern_atom(&conn, "_NET_ACTIVE_WINDOW")?;

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
        let keyboard = KeyboardWithFallback::new(keyboard)?;

        let cursor_font_id = conn.generate_id();
        let cursor_font_name = "cursor";
        conn.check_request(conn.send_request_checked(&xcb::x::OpenFont {
            fid: cursor_font_id,
            name: cursor_font_name.as_bytes(),
        }))
        .context("OpenFont")?;

        let root = screen.root();

        if has_randr {
            conn.check_request(conn.send_request_checked(&xcb::randr::SelectInput {
                window: root,
                enable: xcb::randr::NotifyMask::SCREEN_CHANGE
                    | xcb::randr::NotifyMask::PROVIDER_CHANGE
                    | xcb::randr::NotifyMask::RESOURCE_CHANGE,
            }))
            .context("XRANDR::SelectInput")?;
        }

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
            atom_texturilist,
            atom_xmozurl,
            atom_xdndaware,
            atom_xdndtypelist,
            atom_xdndselection,
            atom_xdndenter,
            atom_xdndposition,
            atom_xdndstatus,
            atom_xdndleave,
            atom_xdnddrop,
            atom_xdndfinished,
            atom_xdndactioncopy,
            atom_xdndactionmove,
            atom_xdndactionlink,
            atom_xdndactionask,
            atom_xdndactionprivate,
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
            atom_net_wm_moveresize,
            atom_net_supported,
            atom_net_supporting_wm_check,
            atom_net_active_window,
            atom_net_wm_icon,
            keyboard,
            kbd_ev,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            windows: RefCell::new(HashMap::new()),
            child_to_parent_id: RefCell::new(HashMap::new()),
            should_terminate: RefCell::new(false),
            depth,
            visual,
            gl_connection: RefCell::new(None),
            ime: RefCell::new(ime),
            ime_process_event_result: RefCell::new(Ok(())),
            has_randr,
            atom_names: RefCell::new(HashMap::new()),
            supported: RefCell::new(HashSet::new()),
            screens: RefCell::new(None),
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

        conn.update_net_supported();

        Ok(conn)
    }

    pub(crate) fn send_and_wait_request<R>(
        &self,
        req: &R,
    ) -> anyhow::Result<<<R as xcb::Request>::Cookie as xcb::CookieWithReplyChecked>::Reply>
    where
        R: xcb::Request + std::fmt::Debug,
        R::Cookie: xcb::CookieWithReplyChecked,
    {
        let cookie = self.conn.send_request(req);
        self.conn
            .wait_for_reply(cookie)
            .with_context(|| format!("{req:#?}"))
    }

    pub(crate) fn send_request_no_reply<R>(&self, req: &R) -> anyhow::Result<()>
    where
        R: xcb::RequestWithoutReply + std::fmt::Debug,
    {
        self.conn
            .send_and_check_request(req)
            .with_context(|| format!("{req:#?}"))
    }

    pub(crate) fn send_request_no_reply_log<R>(&self, req: &R)
    where
        R: xcb::RequestWithoutReply + std::fmt::Debug,
    {
        if let Err(err) = self.send_request_no_reply(req) {
            log::error!("{err:#}");
        }
    }

    pub fn atom_name(&self, atom: Atom) -> String {
        if let Some(name) = self.atom_names.borrow().get(&atom) {
            return name.to_string();
        }
        let cookie = self.conn.send_request(&xcb::x::GetAtomName { atom });
        let name = if let Ok(reply) = self.conn.wait_for_reply(cookie) {
            reply.name().to_string()
        } else {
            format!("{:?}", atom)
        };

        self.atom_names.borrow_mut().insert(atom, name.to_string());
        name
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
                if inner.window_id != window {
                    prom.result(Err(anyhow!("window {window:?} has been destroyed")));
                } else {
                    prom.result(f(&mut inner));
                }
            }
        })
        .detach();

        future
    }

    fn screen_from_focused_window(
        &self,
        by_name: &HashMap<String, ScreenInfo>,
    ) -> anyhow::Result<ScreenInfo> {
        let focused = self
            .send_and_wait_request(&xcb::x::GetInputFocus {})
            .context("querying focused window")?;
        let geom = self
            .send_and_wait_request(&xcb::x::GetGeometry {
                drawable: xcb::x::Drawable::Window(focused.focus()),
            })
            .context("querying geometry")?;
        let trans_geom = self
            .send_and_wait_request(&xcb::x::TranslateCoordinates {
                src_window: focused.focus(),
                dst_window: self.root,
                src_x: 0,
                src_y: 0,
            })
            .context("querying root coordinates")?;
        let window_rect: ScreenRect = euclid::rect(
            trans_geom.dst_x().into(),
            trans_geom.dst_y().into(),
            geom.width() as isize,
            geom.height() as isize,
        );
        Ok(by_name
            .values()
            .filter_map(|screen| {
                screen
                    .rect
                    .intersection(&window_rect)
                    .map(|r| (screen, r.area()))
            })
            .max_by_key(|s| s.1)
            .ok_or_else(|| anyhow::anyhow!("active window is not in any screen"))?
            .0
            .clone())
    }
}
