use super::{Connection, Window};
use super::super::opengl::render::Renderer;
use super::xkeysyms;
use clipboard::{Clipboard, ClipboardImpl, Paste};
use config::Config;
use failure::Error;
use font::FontConfiguration;
use guiloop::{GuiEventLoop, SessionTerminated, WindowId};
use opengl::textureatlas::OutOfTextureSpace;
use pty::MasterPty;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::process::Child;
use std::process::Command;
use std::rc::Rc;
use term::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use term::hyperlink::Hyperlink;
use xcb;

/// Holds the information we need to implement `TerminalHost`
struct Host {
    window: Window,
    pty: MasterPty,
    timestamp: xcb::xproto::Timestamp,
    clipboard: Clipboard,
}

pub struct TerminalWindow {
    host: Host,
    conn: Rc<Connection>,
    renderer: Renderer,
    width: u16,
    height: u16,
    cell_height: usize,
    cell_width: usize,
    terminal: term::Terminal,
    process: Child,
}

impl term::TerminalHost for Host {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        // TODO: make this configurable
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&link.url);
        match cmd.spawn() {
            Ok(_) => {}
            Err(err) => eprintln!("failed to spawn xdg-open {}: {:?}", link.url, err),
        }
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        self.clipboard.get_clipboard()
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.clipboard.set_clipboard(clip)
    }

    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
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
        let window_id = window.window_id;

        let host = Host {
            window,
            pty,
            timestamp: 0,
            clipboard: Clipboard::new(event_loop.paster.clone(), window_id)?,
        };

        let renderer = Renderer::new(&host.window, width, height, fonts, palette)?;
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        host.window.show();

        Ok(TerminalWindow {
            host,
            renderer,
            conn: Rc::clone(&event_loop.conn),
            width,
            height,
            cell_height,
            cell_width,
            terminal,
            process,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.host.window.window_id
    }

    pub fn pty_fd(&self) -> RawFd {
        self.host.pty.as_raw_fd()
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
            self.host.pty.resize(rows, cols, width, height)?;
            self.terminal.resize(rows as usize, cols as usize);

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
        let res = self.renderer.paint(&mut target, &mut self.terminal);
        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target.finish().unwrap();

        // The only error we want to catch is texture space related;
        // when that happens we need to blow our glyph cache and
        // allocate a newer bigger texture.
        match res {
            Err(err) => {
                if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                    eprintln!("out of texture space, allocating {}", size);
                    self.renderer.recreate_atlas(&self.host.window, size)?;
                    self.terminal.make_all_lines_dirty();
                    // Recursively initiate a new paint
                    return self.paint();
                }
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    pub fn paint_if_needed(&mut self) -> Result<(), Error> {
        if self.terminal.has_dirty_lines() {
            self.paint()?;
        }
        Ok(())
    }

    pub fn process_clipboard(&mut self) -> Result<(), Error> {
        match self.host.clipboard.try_get_paste() {
            Ok(Some(Paste::Cleared)) => {
                self.terminal.clear_selection();
            }
            Ok(_) => {}
            Err(err) => bail!("clipboard thread died? {:?}", err),
        }
        self.paint_if_needed()?;
        Ok(())
    }

    pub fn test_for_child_exit(&mut self) -> Result<(), SessionTerminated> {
        match self.process.try_wait() {
            Ok(Some(status)) => Err(SessionTerminated::ProcessStatus { status }),
            Ok(None) => Ok(()),
            Err(e) => Err(SessionTerminated::Error { err: e.into() }),
        }
    }

    pub fn try_read_pty(&mut self) -> Result<(), Error> {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        match self.host.pty.read(&mut buf) {
            Ok(size) => self.terminal.advance_bytes(&buf[0..size], &mut self.host),
            Err(err) => {
                if err.kind() != io::ErrorKind::WouldBlock {
                    return Err(SessionTerminated::Error { err: err.into() }.into());
                }
            }
        }
        Ok(())
    }

    fn decode_key(&self, event: &xcb::KeyPressEvent) -> (KeyCode, KeyModifiers) {
        let mods = xkeysyms::modifiers(event);
        let sym = self.conn
            .lookup_keysym(event, mods.contains(KeyModifiers::SHIFT));
        (xkeysyms::xcb_keysym_to_keycode(sym), mods)
    }

    fn mouse_event(&mut self, event: MouseEvent) -> Result<(), Error> {
        self.terminal.mouse_event(event, &mut self.host)?;
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
                self.host.timestamp = key_press.time();
                let (code, mods) = self.decode_key(key_press);
                self.terminal.key_down(code, mods, &mut self.host)?;
            }
            xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                self.host.timestamp = key_press.time();
                let (code, mods) = self.decode_key(key_press);
                self.terminal.key_up(code, mods, &mut self.host)?;
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
                self.host.timestamp = button_press.time();

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
}
