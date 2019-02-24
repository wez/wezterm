use super::super::opengl::render::Renderer;
use super::super::Child;
use super::xkeysyms;
use super::{Connection, Window};
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::guicommon::host::{HostHelper, HostImpl};
use crate::guicommon::tabs::{Tab, TabId, Tabs};
use crate::guicommon::window::{Dimensions, TerminalWindow};
use crate::guiloop::x11::{GuiEventLoop, WindowId};
use crate::guiloop::SessionTerminated;
use crate::MasterPty;
use failure::Error;
use futures;
use std::cell::RefMut;
use std::io::{self, Read, Write};
use std::rc::Rc;
use term::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use termwiz::hyperlink::Hyperlink;
use xcb;

/// Implements `TerminalHost` for a Tab.
/// `TabHost` instances are short lived and borrow references to
/// other state.
struct TabHost<'a> {
    pty: &'a mut MasterPty,
    host: &'a mut HostImpl<Host>,
}

/// Holds most of the information we need to implement `TerminalHost`
struct Host {
    window: Window,
    event_loop: Rc<GuiEventLoop>,
    fonts: Rc<FontConfiguration>,
    config: Rc<Config>,
}

impl HostHelper for Host {}

pub struct X11TerminalWindow {
    host: HostImpl<Host>,
    conn: Rc<Connection>,
    fonts: Rc<FontConfiguration>,
    renderer: Renderer,
    width: u16,
    height: u16,
    cell_height: usize,
    cell_width: usize,
    tabs: Tabs,
}

impl<'a> TabHost<'a> {
    fn with_window<F: 'static + Fn(&mut X11TerminalWindow) -> Result<(), Error>>(&self, func: F) {
        let events = Rc::clone(&self.host.event_loop);
        let window_id = self.host.window.window.window_id;

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .with_window(window_id, &func)
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
    }
}

impl<'a> term::TerminalHost for TabHost<'a> {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        match open::that(link.uri()) {
            Ok(_) => {}
            Err(err) => eprintln!("failed to open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        self.host.get_clipboard()
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.host.set_clipboard(clip)
    }

    fn set_title(&mut self, _title: &str) {
        self.with_window(move |win| {
            win.update_title();
            Ok(())
        })
    }

    fn new_window(&mut self) {
        let event_loop = Rc::clone(&self.host.event_loop);
        let events = Rc::clone(&self.host.event_loop);
        let config = Rc::clone(&self.host.config);
        let fonts = Rc::clone(&self.host.fonts);

        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .spawn_window(&event_loop, &config, &fonts)
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
        self.with_window(move |win| win.activate_tab(tab))
    }

    fn activate_tab_relative(&mut self, tab: isize) {
        self.with_window(move |win| win.activate_tab_relative(tab))
    }

    fn increase_font_size(&mut self) {
        self.with_window(move |win| {
            let scale = win.fonts.get_font_scale();
            win.scaling_changed(Some(scale * 1.1), None, win.width, win.height)
        })
    }

    fn decrease_font_size(&mut self) {
        self.with_window(move |win| {
            let scale = win.fonts.get_font_scale();
            win.scaling_changed(Some(scale * 0.9), None, win.width, win.height)
        })
    }

    fn reset_font_size(&mut self) {
        self.with_window(move |win| win.scaling_changed(Some(1.0), None, win.width, win.height))
    }
}

impl TerminalWindow for X11TerminalWindow {
    fn get_tabs(&self) -> &Tabs {
        &self.tabs
    }
    fn get_tabs_mut(&mut self) -> &mut Tabs {
        &mut self.tabs
    }
    fn config(&self) -> &Rc<Config> {
        &self.host.config
    }
    fn fonts(&self) -> &Rc<FontConfiguration> {
        &self.host.fonts
    }

    fn set_window_title(&mut self, title: &str) -> Result<(), Error> {
        self.host.window.set_title(title);
        Ok(())
    }
    fn frame(&self) -> glium::Frame {
        self.host.window.draw()
    }

    fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error> {
        self.renderer.recreate_atlas(&self.host.window, size)
    }
    fn renderer_and_terminal(&mut self) -> (&mut Renderer, RefMut<term::Terminal>) {
        (
            &mut self.renderer,
            self.tabs.get_active().unwrap().terminal(),
        )
    }
    fn tab_was_created(&mut self, _tab_id: TabId) -> Result<(), Error> {
        Ok(())
    }
    fn deregister_tab(&mut self, tab_id: TabId) -> Result<(), Error> {
        let events = Rc::clone(&self.host.event_loop);
        self.host
            .event_loop
            .core
            .spawn(futures::future::poll_fn(move || {
                events
                    .deregister_tab(tab_id)
                    .map(futures::Async::Ready)
                    .map_err(|_| ())
            }));
        Ok(())
    }
    fn get_dimensions(&self) -> Dimensions {
        Dimensions {
            width: self.width,
            height: self.height,
            cell_height: self.cell_height,
            cell_width: self.cell_width,
        }
    }
    fn advise_renderer_that_scaling_has_changed(
        &mut self,
        cell_width: usize,
        cell_height: usize,
    ) -> Result<(), Error> {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.renderer.scaling_changed(&self.host.window)
    }
    fn advise_renderer_of_resize(&mut self, width: u16, height: u16) -> Result<(), Error> {
        self.width = width;
        self.height = height;
        self.renderer.resize(&self.host.window, width, height)
    }
    fn resize_if_not_full_screen(&mut self, _width: u16, _height: u16) -> Result<bool, Error> {
        // FIXME: it would be nice to implement this!
        // It requires some plumbing to allow sending xcb_configure_window with
        // XCB_CONFIG_WINDOW_WIDTH and XCB_CONFIG_WINDOW_HEIGHT set.
        Ok(false)
    }
}

impl X11TerminalWindow {
    pub fn new(
        event_loop: &Rc<GuiEventLoop>,
        terminal: term::Terminal,
        pty: MasterPty,
        process: Child,
        fonts: &Rc<FontConfiguration>,
        config: &Rc<Config>,
    ) -> Result<X11TerminalWindow, Error> {
        let palette = config
            .colors
            .as_ref()
            .map(|p| p.clone().into())
            .unwrap_or_else(term::color::ColorPalette::default);

        let metrics = fonts.default_font_metrics()?;
        let (cell_height, cell_width) = (metrics.cell_height, metrics.cell_width);

        let size = pty.get_size()?;
        let width = size.ws_xpixel;
        let height = size.ws_ypixel;

        let window = Window::new(&event_loop.conn, width, height)?;
        window.set_title("wezterm");

        let host = HostImpl::new(Host {
            window,
            event_loop: Rc::clone(event_loop),
            config: Rc::clone(config),
            fonts: Rc::clone(fonts),
        });

        let renderer = Renderer::new(&host.window, width, height, fonts, palette)?;
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        host.window.show();

        let tab = Tab::new(terminal, process, pty);

        Ok(X11TerminalWindow {
            host,
            renderer,
            conn: Rc::clone(&event_loop.conn),
            fonts: Rc::clone(&fonts),
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

    pub fn expose(&mut self, _x: u16, _y: u16, _width: u16, _height: u16) -> Result<(), Error> {
        self.paint()
    }

    pub fn try_read_pty(&mut self, tab_id: TabId) -> Result<(), Error> {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        let tab = self.tabs.get_by_id(tab_id)?;

        let result = tab.pty().read(&mut buf);
        match result {
            Ok(size) => {
                tab.terminal().advance_bytes(
                    &buf[0..size],
                    &mut TabHost {
                        pty: &mut *tab.pty(),
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
        self.conn.xkb_lookup_keysym(event)
    }

    fn mouse_event(&mut self, event: MouseEvent) -> Result<(), Error> {
        let tab = match self.tabs.get_active() {
            Some(tab) => tab,
            None => return Ok(()),
        };
        tab.terminal().mouse_event(
            event,
            &mut TabHost {
                pty: &mut *tab.pty(),
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
                self.resize_surfaces(cfg.width(), cfg.height(), false)?;
            }
            xcb::KEY_PRESS => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                let tab = match self.tabs.get_active() {
                    Some(tab) => tab,
                    None => return Ok(()),
                };
                if let Some((code, mods)) = self.decode_key(key_press) {
                    tab.terminal().key_down(
                        code,
                        mods,
                        &mut TabHost {
                            pty: &mut *tab.pty(),
                            host: &mut self.host,
                        },
                    )?;
                }
            }
            xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                if let Some((code, mods)) = self.decode_key(key_press) {
                    let tab = match self.tabs.get_active() {
                        Some(tab) => tab,
                        None => return Ok(()),
                    };
                    tab.terminal().key_up(
                        code,
                        mods,
                        &mut TabHost {
                            pty: &mut *tab.pty(),
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
}
