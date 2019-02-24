//! Generic system dependent windows via glium+glutin

use crate::config::Config;
use crate::failure::Error;
use crate::font::FontConfiguration;
use crate::guicommon::tabs::{Tab, TabId, Tabs};
use crate::guicommon::window::{Dimensions, TerminalWindow};
use crate::guiloop::glutinloop::GuiEventLoop;
use crate::guiloop::SessionTerminated;
use crate::opengl::render::Renderer;
use crate::{spawn_window_impl, Child, MasterPty};
use clipboard::{ClipboardContext, ClipboardProvider};
use glium;
use glium::glutin::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use glium::glutin::{self, ElementState, MouseCursor};
use std::cell::RefMut;
use std::io::Write;
use std::rc::Rc;
use term::KeyCode;
use term::KeyModifiers;
use term::{self, Terminal};
use term::{MouseButton, MouseEventKind};
use termwiz::hyperlink::Hyperlink;
#[cfg(target_os = "macos")]
use winit::os::macos::WindowExt;

/// Implements `TerminalHost` for a Tab.
/// `TabHost` instances are short lived and borrow references to
/// other state.
struct TabHost<'a> {
    pty: &'a mut MasterPty,
    host: &'a mut Host,
}

struct Host {
    event_loop: Rc<GuiEventLoop>,
    display: glium::Display,
    clipboard: Clipboard,
    window_position: Option<LogicalPosition>,
    /// if is_some, holds position to be restored after exiting
    /// fullscreen mode.
    is_fullscreen: Option<LogicalPosition>,
    config: Rc<Config>,
    fonts: Rc<FontConfiguration>,
}

/// macOS gets unhappy if we set up the clipboard too early,
/// so we use this to defer it until we use it
#[derive(Default)]
struct Clipboard {
    clipboard: Option<ClipboardContext>,
}

impl Clipboard {
    fn clipboard(&mut self) -> Result<&mut ClipboardContext, Error> {
        if self.clipboard.is_none() {
            self.clipboard = Some(ClipboardContext::new().map_err(|e| format_err!("{}", e))?);
        }
        Ok(self.clipboard.as_mut().unwrap())
    }

    pub fn get_clipboard(&mut self) -> Result<String, Error> {
        self.clipboard()?
            .get_contents()
            .map_err(|e| format_err!("{}", e))
    }

    pub fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.clipboard()?
            .set_contents(clip.unwrap_or_else(|| "".into()))
            .map_err(|e| format_err!("{}", e))?;
        // Request the clipboard contents we just set; on some systems
        // if we copy and paste in wezterm, the clipboard isn't visible
        // to us again until the second call to get_clipboard.
        self.get_clipboard().map(|_| ())
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
        self.host.clipboard.get_clipboard()
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.host.clipboard.set_clipboard(clip)
    }

    fn set_title(&mut self, _title: &str) {
        // activate_tab_relative calls Terminal::update_title()
        // in the appropriate context
        self.activate_tab_relative(0);
    }

    fn toggle_full_screen(&mut self) {
        let window = self.host.display.gl_window();
        if let Some(pos) = self.host.is_fullscreen.take() {
            // Use simple fullscreen mode on macos, as wez personally
            // prefers the faster transition to/from this mode than
            // the Lion+ slow transition to a new Space.  This could
            // be made into a config option if someone really wanted
            // that behavior.
            #[cfg(target_os = "macos")]
            window.set_simple_fullscreen(false);
            #[cfg(not(target_os = "macos"))]
            window.set_fullscreen(None);
            window.set_position(pos);
        } else {
            // We use our own idea of the position because get_position()
            // appears to only return the initial position of the window
            // on Linux.
            self.host.is_fullscreen = self.host.window_position.take();

            #[cfg(target_os = "macos")]
            window.set_simple_fullscreen(true);
            #[cfg(not(target_os = "macos"))]
            window.set_fullscreen(Some(window.get_current_monitor()));
        }
    }

    fn new_window(&mut self) {
        let event_loop = Rc::clone(&self.host.event_loop);
        let config = Rc::clone(&self.host.config);
        let fonts = Rc::clone(&self.host.fonts);
        self.host.event_loop.spawn_fn(move || {
            let (terminal, master, child, fonts) = spawn_window_impl(None, &config, &fonts)?;
            let window =
                GliumTerminalWindow::new(&event_loop, terminal, master, child, &fonts, &config)?;

            event_loop.add_window(window)
        });
    }

    fn new_tab(&mut self) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            |win| win.spawn_tab().map(|_| ()),
        );
    }

    fn activate_tab(&mut self, tab: usize) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            move |win| win.activate_tab(tab),
        );
    }

    fn activate_tab_relative(&mut self, tab: isize) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            move |win| win.activate_tab_relative(tab),
        );
    }

    fn increase_font_size(&mut self) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            |win| {
                let scale = win.fonts.get_font_scale();
                win.scaling_changed(Some(scale * 1.1), None, win.width, win.height)
            },
        );
    }

    fn decrease_font_size(&mut self) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            |win| {
                let scale = win.fonts.get_font_scale();
                win.scaling_changed(Some(scale * 0.9), None, win.width, win.height)
            },
        );
    }

    fn reset_font_size(&mut self) {
        GuiEventLoop::with_window(
            &self.host.event_loop,
            self.host.display.gl_window().id(),
            |win| win.scaling_changed(Some(1.0), None, win.width, win.height),
        );
    }
}

pub struct GliumTerminalWindow {
    host: Host,
    event_loop: Rc<GuiEventLoop>,
    config: Rc<Config>,
    fonts: Rc<FontConfiguration>,
    renderer: Renderer,
    width: u16,
    height: u16,
    cell_height: usize,
    cell_width: usize,
    last_mouse_coords: PhysicalPosition,
    last_modifiers: KeyModifiers,
    allow_received_character: bool,
    tabs: Tabs,
}

impl TerminalWindow for GliumTerminalWindow {
    fn get_tabs(&self) -> &Tabs {
        &self.tabs
    }
    fn get_tabs_mut(&mut self) -> &mut Tabs {
        &mut self.tabs
    }
    fn config(&self) -> &Rc<Config> {
        &self.config
    }
    fn fonts(&self) -> &Rc<FontConfiguration> {
        &self.fonts
    }

    fn set_window_title(&mut self, title: &str) -> Result<(), Error> {
        self.host.display.gl_window().set_title(title);
        Ok(())
    }

    fn frame(&self) -> glium::Frame {
        self.host.display.draw()
    }

    fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error> {
        self.renderer.recreate_atlas(&self.host.display, size)
    }
    fn renderer_and_terminal(&mut self) -> (&mut Renderer, RefMut<term::Terminal>) {
        (
            &mut self.renderer,
            self.tabs.get_active().unwrap().terminal(),
        )
    }
    fn tab_was_created(&mut self, tab_id: TabId) -> Result<(), Error> {
        match self.tabs.get_by_id(tab_id) {
            Ok(tab) => {
                self.event_loop
                    .schedule_read_pty(tab.pty().try_clone()?, self.window_id(), tab_id)
            }
            _ => Ok(()),
        }
    }
    fn deregister_tab(&mut self, _tab_id: TabId) -> Result<(), Error> {
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
        self.renderer.scaling_changed(&mut self.host.display)
    }
    fn advise_renderer_of_resize(&mut self, width: u16, height: u16) -> Result<(), Error> {
        self.width = width;
        self.height = height;
        self.renderer.resize(&self.host.display, width, height)
    }
    fn resize_if_not_full_screen(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if self.host.is_fullscreen.is_none() {
            {
                let size = PhysicalSize::new(width.into(), height.into());
                let window = self.host.display.gl_window();
                let dpi = window.get_hidpi_factor();
                window.set_inner_size(size.to_logical(dpi));
            }
            self.resize_surfaces(width, height, true)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl GliumTerminalWindow {
    pub fn new(
        event_loop: &Rc<GuiEventLoop>,
        terminal: Terminal,
        pty: MasterPty,
        process: Child,
        fonts: &Rc<FontConfiguration>,
        config: &Rc<Config>,
    ) -> Result<GliumTerminalWindow, Error> {
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
        let logical_size = LogicalSize::new(width.into(), height.into());
        eprintln!("make window with {}x{}", width, height);

        let display = {
            let pref_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .with_pixel_format(24, 8);
            let window = glutin::WindowBuilder::new()
                .with_dimensions(logical_size)
                .with_title("wezterm");

            let mut_loop = event_loop.event_loop.borrow_mut();

            glium::Display::new(window, pref_context, &*mut_loop)
                .map_err(|e| format_err!("{:?}", e))?
        };
        let window_position = display.gl_window().get_position();

        let host = Host {
            event_loop: Rc::clone(event_loop),
            display,
            clipboard: Clipboard::default(),
            window_position,
            is_fullscreen: None,
            config: Rc::clone(config),
            fonts: Rc::clone(fonts),
        };

        host.display.gl_window().set_cursor(MouseCursor::Text);

        let renderer = Renderer::new(&host.display, width, height, fonts, palette)?;
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        let tab = Tab::new(terminal, process, pty);

        Ok(GliumTerminalWindow {
            host,
            event_loop: Rc::clone(event_loop),
            config: Rc::clone(config),
            fonts: Rc::clone(fonts),
            renderer,
            width,
            height,
            cell_height,
            cell_width,
            last_mouse_coords: PhysicalPosition::new(0.0, 0.0),
            last_modifiers: Default::default(),
            allow_received_character: false,
            tabs: Tabs::new(tab),
        })
    }

    pub fn get_tab_id_by_idx(&self, tab_idx: usize) -> usize {
        self.tabs
            .get_by_idx(tab_idx)
            .expect("invalid tab_idx")
            .tab_id()
    }

    pub fn window_id(&self) -> glutin::WindowId {
        self.host.display.gl_window().id()
    }

    pub fn clone_current_pty(&self) -> Result<MasterPty, Error> {
        self.tabs
            .get_active()
            .ok_or_else(|| format_err!("no active tab"))?
            .pty()
            .try_clone()
    }

    pub fn process_data_read_from_pty(&mut self, data: &[u8], tab_id: usize) -> Result<(), Error> {
        let tab = self.tabs.get_by_id(tab_id)?;

        tab.terminal().advance_bytes(
            data,
            &mut TabHost {
                pty: &mut *tab.pty(),
                host: &mut self.host,
            },
        );

        Ok(())
    }

    fn resize_surfaces_logical(&mut self, size: LogicalSize) -> Result<bool, Error> {
        let dpi_scale = self.host.display.gl_window().get_hidpi_factor();
        let (width, height): (u32, u32) = size.to_physical(dpi_scale).into();
        let width = width as u16;
        let height = height as u16;
        self.resize_surfaces(width, height, false)
    }

    fn decode_modifiers(state: glium::glutin::ModifiersState) -> term::KeyModifiers {
        let mut mods = Default::default();
        if state.shift {
            mods |= term::KeyModifiers::SHIFT;
        }
        if state.ctrl {
            mods |= term::KeyModifiers::CTRL;
        }
        if state.alt {
            mods |= term::KeyModifiers::ALT;
        }
        if state.logo {
            mods |= term::KeyModifiers::SUPER;
        }
        mods
    }

    fn mouse_move(
        &mut self,
        position: PhysicalPosition,
        modifiers: glium::glutin::ModifiersState,
    ) -> Result<(), Error> {
        let tab = match self.tabs.get_active() {
            Some(tab) => tab,
            None => return Ok(()),
        };

        self.last_mouse_coords = position;
        let (x, y): (i32, i32) = position.into();
        tab.terminal().mouse_event(
            term::MouseEvent {
                kind: MouseEventKind::Move,
                button: MouseButton::None,
                x: (x as usize / self.cell_width) as usize,
                y: (y as usize / self.cell_height) as i64,
                modifiers: Self::decode_modifiers(modifiers),
            },
            &mut TabHost {
                pty: &mut *tab.pty(),
                host: &mut self.host,
            },
        )?;
        // Deliberately not forcing a paint on mouse move as it
        // makes selection feel sluggish
        // self.paint_if_needed()?;

        // When hovering over a hyperlink, show an appropriate
        // mouse cursor to give the cue that it is clickable

        let cursor = if tab.terminal().current_highlight().is_some() {
            MouseCursor::Hand
        } else {
            MouseCursor::Text
        };
        self.host.display.gl_window().set_cursor(cursor);

        Ok(())
    }

    fn mouse_click(
        &mut self,
        state: ElementState,
        button: glutin::MouseButton,
        modifiers: glium::glutin::ModifiersState,
    ) -> Result<(), Error> {
        let tab = match self.tabs.get_active() {
            Some(tab) => tab,
            None => return Ok(()),
        };

        tab.terminal().mouse_event(
            term::MouseEvent {
                kind: match state {
                    ElementState::Pressed => MouseEventKind::Press,
                    ElementState::Released => MouseEventKind::Release,
                },
                button: match button {
                    glutin::MouseButton::Left => MouseButton::Left,
                    glutin::MouseButton::Right => MouseButton::Right,
                    glutin::MouseButton::Middle => MouseButton::Middle,
                    glutin::MouseButton::Other(_) => return Ok(()),
                },
                x: (self.last_mouse_coords.x as usize / self.cell_width) as usize,
                y: (self.last_mouse_coords.y as usize / self.cell_height) as i64,
                modifiers: Self::decode_modifiers(modifiers),
            },
            &mut TabHost {
                pty: &mut *tab.pty(),
                host: &mut self.host,
            },
        )?;
        self.paint_if_needed()?;

        Ok(())
    }

    /// Handle a scroll wheel or touchpad scroll gesture.
    /// The delta can provide either a LineDelta or a PixelData
    /// depending on the source of the input.
    /// On Linux with a touch pad I'm seeing fractional LineDelta
    /// values depending on the velocity of my scroll swipe.
    /// We need to translate this to a series of wheel events to
    /// pass to the underlying terminal model.
    fn mouse_wheel(
        &mut self,
        delta: glutin::MouseScrollDelta,
        modifiers: glium::glutin::ModifiersState,
    ) -> Result<(), Error> {
        // Figure out which wheel button and how many times we want
        // to trigger it based on the magnitude of the wheel event.
        // We currently only care about vertical scrolling so the code
        // below will return early if all we have is horizontal scroll
        // components.
        let (button, times) = match delta {
            glutin::MouseScrollDelta::LineDelta(_, lines) if lines > 0.0 => {
                (MouseButton::WheelUp, lines.abs().ceil() as usize)
            }
            glutin::MouseScrollDelta::LineDelta(_, lines) if lines < 0.0 => {
                (MouseButton::WheelDown, lines.abs().ceil() as usize)
            }
            glutin::MouseScrollDelta::PixelDelta(position) => {
                let lines = position.y / self.cell_height as f64;
                if lines > 0.0 {
                    (MouseButton::WheelUp, lines.abs().ceil() as usize)
                } else if lines < 0.0 {
                    (MouseButton::WheelDown, lines.abs().ceil() as usize)
                } else {
                    return Ok(());
                }
            }
            _ => return Ok(()),
        };

        let tab = match self.tabs.get_active() {
            Some(tab) => tab,
            None => return Ok(()),
        };
        for _ in 0..times {
            tab.terminal().mouse_event(
                term::MouseEvent {
                    kind: MouseEventKind::Press,
                    button,
                    x: (self.last_mouse_coords.x as usize / self.cell_width) as usize,
                    y: (self.last_mouse_coords.y as usize / self.cell_height) as i64,
                    modifiers: Self::decode_modifiers(modifiers),
                },
                &mut TabHost {
                    pty: &mut *tab.pty(),
                    host: &mut self.host,
                },
            )?;
        }
        self.paint_if_needed()?;

        Ok(())
    }

    /// Winit, which is the underlying windowing library, doesn't have a very consistent
    /// story around how it constructs KeyboardInput instances.  For example when running
    /// against X11 inside WSL, the VirtualKeyCode is set to Grave when backtick is pressed,
    /// but is None when `~` is pressed (shift+Grave).
    /// In this situation we don't know whether ReceivedCharacter will follow with the
    /// `~` translated.
    /// Because we cannot trust the input data, this function is present to compute
    /// a VirtualKeyCode from the scan_code.
    /// This isn't great because correctly interpreting the scan_code requires more
    /// system dependent context than we have available.
    /// For now we have to put up with it; this may result in us effectively being
    /// hardcoded to a US English keyboard layout until we come up with something
    /// better.
    fn scancode_to_virtual(scan_code: u32) -> Option<glium::glutin::VirtualKeyCode> {
        use glium::glutin::VirtualKeyCode as V;
        let code = match scan_code {
            0x29 => V::Grave,
            0x02 => V::Key1,
            0x03 => V::Key2,
            0x04 => V::Key3,
            0x05 => V::Key4,
            0x06 => V::Key5,
            0x07 => V::Key6,
            0x08 => V::Key7,
            0x09 => V::Key8,
            0x0a => V::Key9,
            0x0b => V::Key0,
            0x0c => V::Minus,
            0x0d => V::Equals,
            0x0e => V::Back,
            0x0f => V::Tab,
            0x10 => V::Q,
            0x11 => V::W,
            0x12 => V::E,
            0x13 => V::R,
            0x14 => V::T,
            0x15 => V::Y,
            0x16 => V::U,
            0x17 => V::I,
            0x18 => V::O,
            0x19 => V::P,
            0x1a => V::LBracket,
            0x1b => V::RBracket,
            0x2b => V::Backslash,
            0x1e => V::A,
            0x1f => V::S,
            0x20 => V::D,
            0x21 => V::F,
            0x22 => V::G,
            0x23 => V::H,
            0x24 => V::J,
            0x25 => V::K,
            0x26 => V::L,
            0x27 => V::Semicolon,
            0x28 => V::Apostrophe,
            0x1c => V::Return,
            0x2a => V::LShift,
            0x2c => V::Z,
            0x2d => V::X,
            0x2e => V::C,
            0x2f => V::V,
            0x30 => V::B,
            0x31 => V::N,
            0x32 => V::M,
            0x33 => V::Comma,
            0x34 => V::Period,
            0x35 => V::Slash,
            0x36 => V::RShift,
            0x1d => V::LControl,
            0x38 => V::RControl,
            0x39 => V::Space,
            0x01 => V::Escape,
            _ => return None,
        };
        Some(code)
    }

    fn normalize_keycode(code: glium::glutin::VirtualKeyCode, shifted: bool) -> Option<KeyCode> {
        use glium::glutin::VirtualKeyCode as V;
        macro_rules! shifted {
            ($lower:expr, $upper:expr) => {
                if shifted {
                    KeyCode::Char($upper)
                } else {
                    KeyCode::Char($lower)
                }
            };
            ($lower:expr) => {
                if shifted {
                    KeyCode::Char($lower.to_ascii_uppercase())
                } else {
                    KeyCode::Char($lower)
                }
            };
        }
        let key = match code {
            V::Key1 => shifted!('1', '!'),
            V::Key2 => shifted!('2', '@'),
            V::Key3 => shifted!('3', '#'),
            V::Key4 => shifted!('4', '$'),
            V::Key5 => shifted!('5', '%'),
            V::Key6 => shifted!('6', '^'),
            V::Key7 => shifted!('7', '&'),
            V::Key8 => shifted!('8', '*'),
            V::Key9 => shifted!('9', '('),
            V::Key0 => shifted!('0', ')'),
            V::A => shifted!('a'),
            V::B => shifted!('b'),
            V::C => shifted!('c'),
            V::D => shifted!('d'),
            V::E => shifted!('e'),
            V::F => shifted!('f'),
            V::G => shifted!('g'),
            V::H => shifted!('h'),
            V::I => shifted!('i'),
            V::J => shifted!('j'),
            V::K => shifted!('k'),
            V::L => shifted!('l'),
            V::M => shifted!('m'),
            V::N => shifted!('n'),
            V::O => shifted!('o'),
            V::P => shifted!('p'),
            V::Q => shifted!('q'),
            V::R => shifted!('r'),
            V::S => shifted!('s'),
            V::T => shifted!('t'),
            V::U => shifted!('u'),
            V::V => shifted!('v'),
            V::W => shifted!('w'),
            V::X => shifted!('x'),
            V::Y => shifted!('y'),
            V::Z => shifted!('z'),
            V::Return | V::NumpadEnter => KeyCode::Enter,
            V::Back => KeyCode::Backspace,
            V::Escape => KeyCode::Escape,
            V::Delete => KeyCode::Delete,
            V::Colon => KeyCode::Char(':'),
            V::Space => KeyCode::Char(' '),
            V::Equals => shifted!('=', '+'),
            V::Add => KeyCode::Char('+'),
            V::Apostrophe => shifted!('\'', '"'),
            V::Backslash => shifted!('\\', '|'),
            V::Grave => shifted!('`', '~'),
            V::LBracket => shifted!('[', '{'),
            V::Minus => shifted!('-', '_'),
            V::Period => shifted!('.', '>'),
            V::RBracket => shifted!(']', '}'),
            V::Semicolon => shifted!(';', ':'),
            V::Slash => shifted!('/', '?'),
            V::Comma => shifted!(',', '<'),
            V::Subtract => shifted!('-', '_'),
            V::At => KeyCode::Char('@'),
            V::Tab => KeyCode::Char('\t'),
            V::F1 => KeyCode::Function(1),
            V::F2 => KeyCode::Function(2),
            V::F3 => KeyCode::Function(3),
            V::F4 => KeyCode::Function(4),
            V::F5 => KeyCode::Function(5),
            V::F6 => KeyCode::Function(6),
            V::F7 => KeyCode::Function(7),
            V::F8 => KeyCode::Function(8),
            V::F9 => KeyCode::Function(9),
            V::F10 => KeyCode::Function(10),
            V::F11 => KeyCode::Function(11),
            V::F12 => KeyCode::Function(12),
            V::F13 => KeyCode::Function(13),
            V::F14 => KeyCode::Function(14),
            V::F15 => KeyCode::Function(15),
            V::Insert => KeyCode::Insert,
            V::Home => KeyCode::Home,
            V::End => KeyCode::End,
            V::PageDown => KeyCode::PageDown,
            V::PageUp => KeyCode::PageUp,
            V::Left => KeyCode::LeftArrow,
            V::Up => KeyCode::UpArrow,
            V::Right => KeyCode::RightArrow,
            V::Down => KeyCode::DownArrow,
            V::LAlt | V::RAlt => KeyCode::Alt,
            V::LControl | V::RControl => KeyCode::Control,
            V::LShift | V::RShift => KeyCode::Shift,
            V::LWin | V::RWin => KeyCode::Super,
            _ => return None,
        };
        Some(key)
    }

    fn keycode_from_input(event: &glium::glutin::KeyboardInput) -> Option<KeyCode> {
        if let Some(code) = event.virtual_keycode {
            Self::normalize_keycode(code, event.modifiers.shift)
        } else if let Some(code) = Self::scancode_to_virtual(event.scancode) {
            Self::normalize_keycode(code, event.modifiers.shift)
        } else {
            None
        }
    }

    fn key_event(&mut self, event: glium::glutin::KeyboardInput) -> Result<(), Error> {
        let tab = match self.tabs.get_active() {
            Some(tab) => tab,
            None => return Ok(()),
        };
        let mods = Self::decode_modifiers(event.modifiers);
        self.last_modifiers = mods;
        self.allow_received_character = false;
        if let Some(key) = Self::keycode_from_input(&event) {
            // debug!("event {:?} -> {:?}", event, key);
            match event.state {
                ElementState::Pressed => tab.terminal().key_down(
                    key,
                    mods,
                    &mut TabHost {
                        pty: &mut *tab.pty(),
                        host: &mut self.host,
                    },
                )?,

                ElementState::Released => tab.terminal().key_up(
                    key,
                    mods,
                    &mut TabHost {
                        pty: &mut *tab.pty(),
                        host: &mut self.host,
                    },
                )?,
            }
        } else {
            eprintln!("event {:?} with no mapping", event);
        }
        self.paint_if_needed()?;
        Ok(())
    }

    pub fn dispatch_event(&mut self, event: &glutin::Event) -> Result<(), Error> {
        use glium::glutin::{Event, WindowEvent};
        match *event {
            Event::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            } => {
                return Err(SessionTerminated::WindowClosed.into());
            }
            Event::WindowEvent {
                event: WindowEvent::HiDpiFactorChanged(factor),
                ..
            } => {
                // Assuming that this is dragging a window between hidpi and
                // normal dpi displays.  Treat this as a resize event of sorts
                eprintln!("HiDpiFactorChanged {}", factor);
                self.scaling_changed(None, Some(factor), self.width, self.height)?;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // On Linux we may get here when HiDpiFactorChanged occurs, but
                // without ever receiving a HiDpiFactorChanged event.
                // We need to synthesize that event here by checking what we
                // think we know, otherwise we will use the wrong font size.
                let old_dpi_scale = self.fonts.get_dpi_scale();
                let dpi_scale = self.host.display.gl_window().get_hidpi_factor();
                if old_dpi_scale != dpi_scale {
                    let (width, height): (u32, u32) = size.to_physical(dpi_scale).into();
                    eprintln!(
                        "Synthesize HiDpiFactorChanged {} -> {}",
                        old_dpi_scale, dpi_scale
                    );
                    self.scaling_changed(None, Some(dpi_scale), width as u16, height as u16)?;
                } else {
                    self.resize_surfaces_logical(size)?;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Moved(position),
                ..
            } => {
                self.host.window_position = Some(position);
            }
            Event::WindowEvent {
                event: WindowEvent::ReceivedCharacter(c),
                ..
            } => {
                // Coupled with logic in key_event which gates whether
                // we allow processing unicode chars here
                // eprintln!("ReceivedCharacter {} {:?}", c as u32, c);
                if self.allow_received_character {
                    self.allow_received_character = false;
                    let tab = match self.tabs.get_active() {
                        Some(tab) => tab,
                        None => return Ok(()),
                    };
                    tab.terminal().key_down(
                        KeyCode::Char(c),
                        self.last_modifiers,
                        &mut TabHost {
                            pty: &mut *tab.pty(),
                            host: &mut self.host,
                        },
                    )?;
                    self.paint_if_needed()?;
                }
                return Ok(());
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                self.key_event(input)?;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::CursorMoved {
                        position,
                        modifiers,
                        ..
                    },
                ..
            } => {
                let dpi_scale = self.host.display.gl_window().get_hidpi_factor();
                self.mouse_move(position.to_physical(dpi_scale), modifiers)?;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state,
                        button,
                        modifiers,
                        ..
                    },
                ..
            } => {
                self.mouse_click(state, button, modifiers)?;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta, modifiers, ..
                    },
                ..
            } => {
                self.mouse_wheel(delta, modifiers)?;
            }
            Event::WindowEvent {
                event: WindowEvent::Refresh,
                ..
            } => {
                self.paint()?;
            }
            _ => {}
        }
        Ok(())
    }
}
