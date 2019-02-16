use super::super::opengl::render::Renderer;
use super::super::{get_shell, spawn_window};
use super::xkeysyms;
use super::{Connection, Window};
use clipboard::{Clipboard, ClipboardImpl, Paste};
use config::Config;
use failure::Error;
use font::FontConfiguration;
use futures;
use guiloop::{GuiEventLoop, SessionTerminated, WindowId};
use opengl::textureatlas::OutOfTextureSpace;
use pty;
use pty::MasterPty;
use std::cell::RefCell;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::process::Child;
use std::process::Command;
use std::rc::Rc;
use term::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use termwiz::hyperlink::Hyperlink;
use xcb;

/// Holds the terminal state for a tab owned by this window
struct Tab {
    terminal: RefCell<term::Terminal>,
    process: RefCell<Child>,
    pty: RefCell<MasterPty>,
}

impl Drop for Tab {
    fn drop(&mut self) {
        // Avoid lingering zombies
        self.process.borrow_mut().kill().ok();
        self.process.borrow_mut().wait().ok();
    }
}

struct Tabs {
    tabs: Vec<Tab>,
    active: usize,
}

impl Tabs {
    fn new(tab: Tab) -> Self {
        Self {
            tabs: vec![tab],
            active: 0,
        }
    }

    fn get_active(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn set_active(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.active = idx;
        self.tabs[idx].terminal.borrow_mut().make_all_lines_dirty();
    }

    fn get_for_fd(&self, fd: RawFd) -> Option<&Tab> {
        for t in &self.tabs {
            if t.pty.borrow().as_raw_fd() == fd {
                return Some(t);
            }
        }
        None
    }

    fn index_for_fd(&self, fd: RawFd) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if self.tabs[i].pty.borrow().as_raw_fd() == fd {
                return Some(i);
            }
        }
        None
    }

    fn remove_tab_for_fd(&mut self, fd: RawFd) {
        if let Some(idx) = self.index_for_fd(fd) {
            self.tabs.remove(idx);
            let len = self.tabs.len();
            if self.active == idx && idx >= len {
                self.set_active(len - 1);
            }
        }
    }
}

/// Implements `TerminalHost` for a Tab.
/// `TabHost` instances are short lived and borrow references to
/// other state.
struct TabHost<'a> {
    pty: &'a mut MasterPty,
    host: &'a mut Host,
}

/// Holds most of the information we need to implement `TerminalHost`
struct Host {
    window: Window,
    clipboard: Clipboard,
    event_loop: Rc<GuiEventLoop>,
    fonts: Rc<FontConfiguration>,
    config: Rc<Config>,
}

pub struct TerminalWindow {
    host: Host,
    conn: Rc<Connection>,
    renderer: Renderer,
    width: u16,
    height: u16,
    cell_height: usize,
    cell_width: usize,
    tabs: Tabs,
}

impl<'a> term::TerminalHost for TabHost<'a> {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        // TODO: make this configurable
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&link.uri());
        match cmd.spawn() {
            Ok(_) => {}
            Err(err) => eprintln!("failed to spawn xdg-open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        self.host.clipboard.get_clipboard()
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.host.clipboard.set_clipboard(clip)
    }

    fn set_title(&mut self, _title: &str) {
        let events = Rc::clone(&self.host.event_loop);
        let window_id = self.host.window.window.window_id;

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .with_window(window_id, |win| {
                        win.update_title();
                        Ok(())
                    })
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }

    fn new_window(&mut self) {
        let event_loop = Rc::clone(&self.host.event_loop);
        let config = Rc::clone(&self.host.config);
        let fonts = Rc::clone(&self.host.fonts);
        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                spawn_window(&event_loop, None, &config, &fonts)
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }

    fn new_tab(&mut self) {
        let events = Rc::clone(&self.host.event_loop);
        let window_id = self.host.window.window.window_id;

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .spawn_tab(window_id)
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }

    fn activate_tab(&mut self, tab: usize) {
        let events = Rc::clone(&self.host.event_loop);
        let window_id = self.host.window.window.window_id;

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .with_window(window_id, |win| win.activate_tab(tab))
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }

    fn activate_tab_relative(&mut self, tab: isize) {
        let events = Rc::clone(&self.host.event_loop);
        let window_id = self.host.window.window.window_id;

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .with_window(window_id, |win| win.activate_tab_relative(tab))
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }
}

impl TerminalWindow {
    pub fn new(
        event_loop: &Rc<GuiEventLoop>,
        terminal: term::Terminal,
        pty: MasterPty,
        process: Child,
        fonts: &Rc<FontConfiguration>,
        config: &Rc<Config>,
    ) -> Result<TerminalWindow, Error> {
        let palette = config
            .colors
            .as_ref()
            .map(|p| p.clone().into())
            .unwrap_or_else(term::color::ColorPalette::default);

        let (cell_height, cell_width) = {
            // Urgh, this is a bit repeaty, but we need to satisfy the borrow checker
            let font = fonts.default_font()?;
            let metrics = font.borrow_mut().get_fallback(0)?.metrics();
            (metrics.cell_height, metrics.cell_width)
        };

        let size = pty.get_size()?;
        let width = size.ws_xpixel;
        let height = size.ws_ypixel;

        let window = Window::new(&event_loop.conn, width, height)?;
        window.set_title("wezterm");
        let window_id = window.window.window_id;

        let host = Host {
            window,
            clipboard: Clipboard::new(event_loop.paster.clone(), window_id)?,
            event_loop: Rc::clone(event_loop),
            config: Rc::clone(config),
            fonts: Rc::clone(fonts),
        };

        let renderer = Renderer::new(&host.window, width, height, fonts, palette)?;
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        host.window.show();

        let tab = Tab {
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
        };

        Ok(TerminalWindow {
            host,
            renderer,
            conn: Rc::clone(&event_loop.conn),
            width,
            height,
            cell_height,
            cell_width,
            tabs: Tabs::new(tab),
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.host.window.window.window_id
    }

    pub fn pty_fds(&self) -> Vec<RawFd> {
        self.tabs
            .tabs
            .iter()
            .map(|tab| tab.pty.borrow().as_raw_fd())
            .collect()
    }

    pub fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);

            self.width = width;
            self.height = height;
            self.renderer.resize(&self.host.window, width, height)?;

            // The +1 in here is to handle an irritating case.
            // When we get N rows with a gap of cell_height - 1 left at
            // the bottom, we can usually squeeze that extra row in there,
            // so optimistically pretend that we have that extra pixel!
            let rows = ((height as usize + 1) / self.cell_height) as u16;
            let cols = ((width as usize + 1) / self.cell_width) as u16;

            for mut tab in &mut self.tabs.tabs {
                tab.pty.borrow_mut().resize(rows, cols, width, height)?;
                tab.terminal
                    .borrow_mut()
                    .resize(rows as usize, cols as usize);
            }

            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    pub fn expose(&mut self, _x: u16, _y: u16, _width: u16, _height: u16) -> Result<(), Error> {
        self.paint()
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.host.window.draw();
        let res = self.renderer.paint(
            &mut target,
            &mut self.tabs.get_active().terminal.borrow_mut(),
        );
        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target
            .finish()
            .expect("target.finish failed and we don't know how to recover");

        // The only error we want to catch is texture space related;
        // when that happens we need to blow our glyph cache and
        // allocate a newer bigger texture.
        match res {
            Err(err) => {
                if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                    eprintln!("out of texture space, allocating {}", size);
                    self.renderer.recreate_atlas(&self.host.window, size)?;
                    self.tabs
                        .get_active()
                        .terminal
                        .borrow_mut()
                        .make_all_lines_dirty();
                    // Recursively initiate a new paint
                    return self.paint();
                }
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    pub fn paint_if_needed(&mut self) -> Result<(), Error> {
        let dirty = self.tabs.get_active().terminal.borrow().has_dirty_lines();
        if dirty {
            self.paint()?;
        }
        Ok(())
    }

    pub fn process_clipboard(&mut self) -> Result<(), Error> {
        match self.host.clipboard.try_get_paste() {
            Ok(Some(Paste::Cleared)) => {
                self.tabs
                    .get_active()
                    .terminal
                    .borrow_mut()
                    .clear_selection();
            }
            Ok(_) => {}
            Err(err) => bail!("clipboard thread died? {:?}", err),
        }
        self.paint_if_needed()?;
        Ok(())
    }

    pub fn test_for_child_exit(&mut self) -> Result<(), SessionTerminated> {
        match self.tabs.get_active().process.borrow_mut().try_wait() {
            Ok(Some(status)) => Err(SessionTerminated::ProcessStatus { status }),
            Ok(None) => Ok(()),
            Err(e) => Err(SessionTerminated::Error { err: e.into() }),
        }
    }

    pub fn try_read_pty(&mut self, fd: RawFd) -> Result<(), Error> {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        let tab = self
            .tabs
            .get_for_fd(fd)
            .ok_or_else(|| format_err!("no tab for fd {}", fd))?;

        let result = tab.pty.borrow_mut().read(&mut buf);
        match result {
            Ok(size) => {
                tab.terminal.borrow_mut().advance_bytes(
                    &buf[0..size],
                    &mut TabHost {
                        pty: &mut *tab.pty.borrow_mut(),
                        host: &mut self.host,
                    },
                );
            }
            Err(err) => {
                if err.kind() != io::ErrorKind::WouldBlock {
                    return Err(SessionTerminated::Error { err: err.into() }.into());
                }
            }
        }

        Ok(())
    }

    fn decode_key(&self, event: &xcb::KeyPressEvent) -> Option<(KeyCode, KeyModifiers)> {
        let mods = xkeysyms::modifiers(event);
        let sym = self
            .conn
            .lookup_keysym(event, mods.contains(KeyModifiers::SHIFT));
        xkeysyms::xcb_keysym_to_keycode(sym).map(|code| (code, mods))
    }

    fn mouse_event(&mut self, event: MouseEvent) -> Result<(), Error> {
        self.tabs.get_active().terminal.borrow_mut().mouse_event(
            event,
            &mut TabHost {
                pty: &mut *self.tabs.get_active().pty.borrow_mut(),
                host: &mut self.host,
            },
        )?;
        Ok(())
    }

    pub fn dispatch_event(&mut self, event: &xcb::GenericEvent) -> Result<(), Error> {
        let r = event.response_type() & 0x7f;
        match r {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
                self.expose(expose.x(), expose.y(), expose.width(), expose.height())?;
            }
            xcb::CONFIGURE_NOTIFY => {
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                self.resize_surfaces(cfg.width(), cfg.height())?;
            }
            xcb::KEY_PRESS => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                if let Some((code, mods)) = self.decode_key(key_press) {
                    self.tabs.get_active().terminal.borrow_mut().key_down(
                        code,
                        mods,
                        &mut TabHost {
                            pty: &mut *self.tabs.get_active().pty.borrow_mut(),
                            host: &mut self.host,
                        },
                    )?;
                }
            }
            xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                if let Some((code, mods)) = self.decode_key(key_press) {
                    self.tabs.get_active().terminal.borrow_mut().key_up(
                        code,
                        mods,
                        &mut TabHost {
                            pty: &mut *self.tabs.get_active().pty.borrow_mut(),
                            host: &mut self.host,
                        },
                    )?;
                }
            }
            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };

                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    button: MouseButton::None,
                    x: (motion.event_x() as usize / self.cell_width) as usize,
                    y: (motion.event_y() as usize / self.cell_height) as i64,
                    modifiers: xkeysyms::modifiers_from_state(motion.state()),
                };
                self.mouse_event(event)?;
            }
            xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };

                let event = MouseEvent {
                    kind: match r {
                        xcb::BUTTON_PRESS => MouseEventKind::Press,
                        xcb::BUTTON_RELEASE => MouseEventKind::Release,
                        _ => unreachable!("button event mismatch"),
                    },
                    x: (button_press.event_x() as usize / self.cell_width) as usize,
                    y: (button_press.event_y() as usize / self.cell_height) as i64,
                    button: match button_press.detail() {
                        1 => MouseButton::Left,
                        2 => MouseButton::Middle,
                        3 => MouseButton::Right,
                        4 => MouseButton::WheelUp,
                        5 => MouseButton::WheelDown,
                        _ => {
                            eprintln!("button {} is not implemented", button_press.detail());
                            return Ok(());
                        }
                    },
                    modifiers: xkeysyms::modifiers_from_state(button_press.state()),
                };

                self.mouse_event(event)?;
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
                println!("CLIENT_MESSAGE {:?}", msg.data().data32());
                if msg.data().data32()[0] == self.conn.atom_delete() {
                    return Err(SessionTerminated::WindowClosed.into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn spawn_tab(&mut self) -> Result<RawFd, Error> {
        let rows = (self.height as usize + 1) / self.cell_height;
        let cols = (self.width as usize + 1) / self.cell_width;

        let (pty, slave) = pty::openpty(rows as u16, cols as u16, self.width, self.height)?;
        let mut cmd = Command::new(get_shell()?);
        cmd.env("TERM", &self.host.config.term);

        let process = RefCell::new(slave.spawn_command(cmd)?);
        eprintln!("spawned: {:?}", process);

        let terminal = RefCell::new(term::Terminal::new(
            rows,
            cols,
            self.host.config.scrollback_lines.unwrap_or(3500),
            self.host.config.hyperlink_rules.clone(),
        ));

        let fd = pty.as_raw_fd();

        let tab = Tab {
            terminal,
            process,
            pty: RefCell::new(pty),
        };

        self.tabs.tabs.push(tab);
        let len = self.tabs.tabs.len();
        self.activate_tab(len - 1)?;

        Ok(fd)
    }

    pub fn close_tab_for_fd(&mut self, fd: RawFd) -> Result<(), Error> {
        self.tabs.remove_tab_for_fd(fd);
        self.update_title();
        Ok(())
    }

    fn update_title(&mut self) {
        let num_tabs = self.tabs.tabs.len();
        let tab_no = self.tabs.active;

        let terminal = self.tabs.get_active().terminal.borrow();

        if num_tabs == 1 {
            self.host.window.set_title(terminal.get_title());
        } else {
            self.host.window.set_title(&format!(
                "[{}/{}] {}",
                tab_no + 1,
                num_tabs,
                terminal.get_title()
            ));
        }
    }

    pub fn activate_tab(&mut self, tab: usize) -> Result<(), Error> {
        let max = self.tabs.tabs.len();
        if tab < max {
            self.tabs.set_active(tab);
            self.update_title();
        }
        Ok(())
    }

    pub fn activate_tab_relative(&mut self, delta: isize) -> Result<(), Error> {
        let max = self.tabs.tabs.len();
        let active = self.tabs.active as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        self.activate_tab(tab as usize % max)
    }
}
