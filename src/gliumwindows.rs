//! Generic system dependent windows via glium+glutin

use crate::config::Config;
use crate::failure::Error;
use crate::font::FontConfiguration;
use crate::guicommon::host::{HostHelper, HostImpl, TabHost};
use crate::guicommon::tabs::{Tab, TabId, Tabs};
use crate::guicommon::window::{Dimensions, TerminalWindow};
use crate::guiloop::glutinloop::GuiEventLoop;
use crate::guiloop::SessionTerminated;
use crate::mux::renderable::Renderable;
use crate::opengl::render::Renderer;
use glium;
use glium::glutin::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use glium::glutin::{self, ElementState, MouseCursor};
use std::rc::Rc;
use term;
use term::KeyCode;
use term::KeyModifiers;
use term::{MouseButton, MouseEventKind};
#[cfg(target_os = "macos")]
use winit::os::macos::WindowExt;

struct Host {
    event_loop: Rc<GuiEventLoop>,
    display: glium::Display,
    window_position: Option<LogicalPosition>,
    /// if is_some, holds position to be restored after exiting
    /// fullscreen mode.
    is_fullscreen: Option<LogicalPosition>,
    config: Rc<Config>,
    fonts: Rc<FontConfiguration>,
}

impl HostHelper for Host {
    fn with_window<F: 'static + Fn(&mut TerminalWindow) -> Result<(), Error>>(&self, func: F) {
        GuiEventLoop::with_window(&self.event_loop, self.display.gl_window().id(), func);
    }

    fn toggle_full_screen(&mut self) {
        if let Some(pos) = self.is_fullscreen.take() {
            let window = self.display.gl_window();
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
            self.is_fullscreen = self.window_position.take();

            let window = self.display.gl_window();
            #[cfg(target_os = "macos")]
            window.set_simple_fullscreen(true);
            #[cfg(not(target_os = "macos"))]
            window.set_fullscreen(Some(window.get_current_monitor()));
        }
    }
}

pub struct GliumTerminalWindow {
    host: HostImpl<Host>,
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
    have_pending_resize_check: bool,
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
    fn renderer_and_tab(&mut self) -> (&mut Renderer, &Tab) {
        (&mut self.renderer, self.tabs.get_active().unwrap())
    }

    fn tab_was_created(&mut self, tab: &Rc<Tab>) -> Result<(), Error> {
        self.event_loop.register_tab(tab)
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
        self.renderer.scaling_changed(&self.host.display)
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
    fn check_for_resize(&mut self) -> Result<(), Error> {
        self.have_pending_resize_check = false;
        let old_dpi_scale = self.fonts.get_dpi_scale();
        let size = self
            .host
            .display
            .gl_window()
            .get_inner_size()
            .ok_or_else(|| format_err!("failed to get inner window size"))?;
        let dpi_scale = self.host.display.gl_window().get_hidpi_factor();
        let (width, height): (u32, u32) = size.to_physical(dpi_scale).into();
        eprintln!(
            "resize {}x{}@{} -> {}x{}@{}",
            self.width, self.height, old_dpi_scale, width, height, dpi_scale
        );
        if (old_dpi_scale - dpi_scale).abs() >= std::f64::EPSILON {
            self.scaling_changed(None, Some(dpi_scale), width as u16, height as u16)?;
        } else {
            self.resize_surfaces(width as u16, height as u16, false)?;
        }
        Ok(())
    }
}

impl GliumTerminalWindow {
    pub fn new(
        event_loop: &Rc<GuiEventLoop>,
        fonts: &Rc<FontConfiguration>,
        config: &Rc<Config>,
        tab: &Rc<Tab>,
    ) -> Result<GliumTerminalWindow, Error> {
        let palette = config
            .colors
            .as_ref()
            .map(|p| p.clone().into())
            .unwrap_or_else(term::color::ColorPalette::default);

        let (physical_rows, physical_cols) = tab.renderer().physical_dimensions();

        let metrics = fonts.default_font_metrics()?;
        let (cell_height, cell_width) = (
            metrics.cell_height.ceil() as usize,
            metrics.cell_width.ceil() as usize,
        );

        let width = cell_width * physical_cols;
        let height = cell_height * physical_rows;

        let logical_size = LogicalSize::new(width as f64, height as f64);
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

        let host = HostImpl::new(Host {
            event_loop: Rc::clone(event_loop),
            display,
            window_position,
            is_fullscreen: None,
            config: Rc::clone(config),
            fonts: Rc::clone(fonts),
        });

        host.display.gl_window().set_cursor(MouseCursor::Text);

        let width = width as u16;
        let height = height as u16;
        let renderer = Renderer::new(&host.display, width, height, fonts, palette)?;

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
            have_pending_resize_check: false,
        })
    }

    pub fn window_id(&self) -> glutin::WindowId {
        self.host.display.gl_window().id()
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
        tab.mouse_event(
            term::MouseEvent {
                kind: MouseEventKind::Move,
                button: MouseButton::None,
                x: (x as usize / self.cell_width) as usize,
                y: (y as usize / self.cell_height) as i64,
                modifiers: Self::decode_modifiers(modifiers),
            },
            &mut TabHost::new(&mut *tab.writer(), &mut self.host),
        )?;
        // Deliberately not forcing a paint on mouse move as it
        // makes selection feel sluggish
        // self.paint_if_needed()?;

        // When hovering over a hyperlink, show an appropriate
        // mouse cursor to give the cue that it is clickable

        let cursor = if tab.renderer().current_highlight().is_some() {
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

        tab.mouse_event(
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
            &mut TabHost::new(&mut *tab.writer(), &mut self.host),
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
            tab.mouse_event(
                term::MouseEvent {
                    kind: MouseEventKind::Press,
                    button,
                    x: (self.last_mouse_coords.x as usize / self.cell_width) as usize,
                    y: (self.last_mouse_coords.y as usize / self.cell_height) as i64,
                    modifiers: Self::decode_modifiers(modifiers),
                },
                &mut TabHost::new(&mut *tab.writer(), &mut self.host),
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

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cyclomatic_complexity))]
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
                ElementState::Pressed => {
                    if mods == KeyModifiers::SUPER && key == KeyCode::Char('n') {
                        GuiEventLoop::schedule_spawn_new_window(
                            &self.event_loop,
                            &self.host.config,
                            &self.host.fonts,
                        );
                        return Ok(());
                    }

                    if self.host.process_gui_shortcuts(tab, mods, key)? {
                        return Ok(());
                    }

                    tab.key_down(key, mods)?;
                }
                ElementState::Released => {}
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
                event: WindowEvent::HiDpiFactorChanged(_),
                ..
            }
            | Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                if !self.have_pending_resize_check {
                    self.have_pending_resize_check = true;
                    self.host.with_window(|win| win.check_for_resize());
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
                    tab.key_down(KeyCode::Char(c), self.last_modifiers)?;
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
