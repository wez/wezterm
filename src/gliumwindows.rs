//! Generic system dependent windows via glium+glutin
#![allow(dead_code)]

use clipboard::{Clipboard, ClipboardImpl, Paste};
use failure::Error;
use font::FontConfiguration;
use glium::{self, glutin};
use glium::glutin::{ElementState, MouseCursor};
use opengl::render::Renderer;
use opengl::textureatlas::OutOfTextureSpace;
use pty::MasterPty;
use std::io;
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus};
use std::rc::Rc;
use term::{self, Terminal};
use term::{MouseButton, MouseEventKind};
use term::KeyCode;
use term::KeyModifiers;
use term::hyperlink::Hyperlink;
use wakeup::Wakeup;

#[derive(Debug, Fail)]
pub enum SessionTerminated {
    #[fail(display = "Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[fail(display = "Error: {:?}", err)]
    Error { err: Error },
    #[fail(display = "Window Closed")]
    WindowClosed,
}

struct Host {
    display: glium::Display,
    pty: MasterPty,
    clipboard: Clipboard,
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
        self.display.gl_window().set_title(title);
    }
}

pub struct TerminalWindow {
    host: Host,
    renderer: Renderer,
    width: u16,
    height: u16,
    cell_height: usize,
    cell_width: usize,
    terminal: Terminal,
    process: Child,
    last_mouse_coords: (f64, f64),
    last_modifiers: KeyModifiers,
    window_position: Option<(i32, i32)>,
    /// is is_some, holds position to be restored after exiting
    /// fullscreen mode.
    is_fullscreen: Option<(i32, i32)>,
}

impl TerminalWindow {
    pub fn new(
        event_loop: &glutin::EventsLoop,
        wakeup: Wakeup,
        width: u16,
        height: u16,
        terminal: Terminal,
        pty: MasterPty,
        process: Child,
        fonts: FontConfiguration,
        palette: term::color::ColorPalette,
    ) -> Result<TerminalWindow, Error> {
        let (cell_height, cell_width) = {
            // Urgh, this is a bit repeaty, but we need to satisfy the borrow checker
            let font = fonts.default_font()?;
            let metrics = font.borrow_mut().get_fallback(0)?.metrics();
            (metrics.cell_height, metrics.cell_width)
        };

        let window = glutin::WindowBuilder::new()
            .with_dimensions(width.into(), height.into())
            .with_title("wezterm");
        let context = glutin::ContextBuilder::new()
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGlEs, (2, 0)))
            .with_vsync(true)
            .with_pixel_format(24, 8)
            .with_srgb(true);
        let display =
            glium::Display::new(window, context, &event_loop).map_err(|e| format_err!("{:?}", e))?;
        let window_id = display.gl_window().id();

        let host = Host {
            display,
            pty,
            clipboard: Clipboard::new(wakeup, window_id)?,
        };

        host.display.gl_window().set_cursor(MouseCursor::Text);
        let window_position = host.display.gl_window().get_position();

        let renderer = Renderer::new(&host.display, width, height, fonts, palette)?;
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        Ok(TerminalWindow {
            host,
            renderer,
            width,
            height,
            cell_height,
            cell_width,
            terminal,
            process,
            last_mouse_coords: (0.0, 0.0),
            last_modifiers: Default::default(),
            window_position,
            is_fullscreen: None,
        })
    }

    pub fn window_id(&self) -> glutin::WindowId {
        self.host.display.gl_window().id()
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.host.display.draw();
        let res = self.renderer.paint(&mut target, &mut self.terminal);
        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target.finish().unwrap();

        // The only error we want to catch is texture space related;
        // when that happens we need to blow our glyph cache and
        // allocate a newer bigger texture.
        match res {
            Err(err) => {
                match err.downcast_ref::<OutOfTextureSpace>() {
                    Some(&OutOfTextureSpace { size }) => {
                        eprintln!("out of texture space, allocating {}", size);
                        self.renderer.recreate_atlas(&self.host.display, size)?;
                        self.terminal.make_all_lines_dirty();
                        // Recursively initiate a new paint
                        return self.paint();
                    }
                    _ => (),
                };
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    pub fn try_read_pty(&mut self) -> Result<(), Error> {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        match self.host.pty.read(&mut buf) {
            Ok(size) => self.terminal.advance_bytes(&buf[0..size], &mut self.host),
            Err(err) => {
                if err.kind() != io::ErrorKind::WouldBlock {
                    eprintln!("error reading from pty: {:?}", err)
                }
            }
        }
        Ok(())
    }

    fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        // Simple approach to hidpi displays; if the display factor is
        // two (retina) then we want things to be twice as large so we
        // simply divide by the scaling factor to have them take up
        // twice as much space.  OpenGL is going to scale things automatically.
        let scale = self.host.display.gl_window().hidpi_factor();
        let width = (width as f32 / scale) as u16;
        let height = (height as f32 / scale) as u16;

        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);

            self.width = width;
            self.height = height;
            self.renderer.resize(&self.host.display, width, height)?;

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
        x: f64,
        y: f64,
        modifiers: glium::glutin::ModifiersState,
    ) -> Result<(), Error> {
        self.last_mouse_coords = (x, y);
        self.terminal.mouse_event(
            term::MouseEvent {
                kind: MouseEventKind::Move,
                button: MouseButton::None,
                x: (x as usize / self.cell_width) as usize,
                y: (y as usize / self.cell_height) as i64,
                modifiers: Self::decode_modifiers(modifiers),
            },
            &mut self.host,
        )?;
        // Deliberately not forcing a paint on mouse move as it
        // makes selection feel sluggish
        // self.paint_if_needed()?;

        // When hovering over a hyperlink, show an appropriate
        // mouse cursor to give the cue that it is clickable
        let cursor = if self.terminal.current_highlight().is_some() {
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
        self.terminal.mouse_event(
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
                x: (self.last_mouse_coords.0 as usize / self.cell_width) as usize,
                y: (self.last_mouse_coords.1 as usize / self.cell_height) as i64,
                modifiers: Self::decode_modifiers(modifiers),
            },
            &mut self.host,
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
            glutin::MouseScrollDelta::PixelDelta(_, pixels) => {
                let lines = pixels / self.cell_height as f32;
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
        for _ in 0..times {
            self.terminal.mouse_event(
                term::MouseEvent {
                    kind: MouseEventKind::Press,
                    button,
                    x: (self.last_mouse_coords.0 as usize / self.cell_width) as usize,
                    y: (self.last_mouse_coords.1 as usize / self.cell_height) as i64,
                    modifiers: Self::decode_modifiers(modifiers),
                },
                &mut self.host,
            )?;
        }
        self.paint_if_needed()?;

        Ok(())
    }

    fn key_event(&mut self, event: glium::glutin::KeyboardInput) -> Result<(), Error> {
        let mods = Self::decode_modifiers(event.modifiers);
        self.last_modifiers = mods;
        if let Some(code) = event.virtual_keycode {
            use glium::glutin::VirtualKeyCode as V;
            let key = match code {
                V::Key1
                | V::Key2
                | V::Key3
                | V::Key4
                | V::Key5
                | V::Key6
                | V::Key7
                | V::Key8
                | V::Key9
                | V::Key0
                | V::A
                | V::B
                | V::C
                | V::D
                | V::E
                | V::F
                | V::G
                | V::H
                | V::I
                | V::J
                | V::K
                | V::L
                | V::M
                | V::N
                | V::O
                | V::P
                | V::Q
                | V::R
                | V::S
                | V::T
                | V::U
                | V::V
                | V::W
                | V::X
                | V::Y
                | V::Z
                | V::Return
                | V::Back
                | V::Escape
                | V::Delete
                | V::Colon
                | V::Space
                | V::Equals
                | V::Add
                | V::Apostrophe
                | V::Backslash
                | V::Grave
                | V::LBracket
                | V::Minus
                | V::Period
                | V::RBracket
                | V::Semicolon
                | V::Slash
                | V::Comma
                | V::Subtract
                | V::At
                | V::Tab => {
                    // These are all handled by ReceivedCharacter
                    return Ok(());
                }
                V::F1 => KeyCode::F(1),
                V::F2 => KeyCode::F(2),
                V::F3 => KeyCode::F(3),
                V::F4 => KeyCode::F(4),
                V::F5 => KeyCode::F(5),
                V::F6 => KeyCode::F(6),
                V::F7 => KeyCode::F(7),
                V::F8 => KeyCode::F(8),
                V::F9 => KeyCode::F(9),
                V::F10 => KeyCode::F(10),
                V::F11 => KeyCode::F(11),
                V::F12 => KeyCode::F(12),
                V::F13 => KeyCode::F(13),
                V::F14 => KeyCode::F(14),
                V::F15 => KeyCode::F(15),
                V::Insert => KeyCode::Insert,
                V::Home => KeyCode::Home,
                V::End => KeyCode::End,
                V::PageDown => KeyCode::PageDown,
                V::PageUp => KeyCode::PageUp,
                V::Left => KeyCode::Left,
                V::Up => KeyCode::Up,
                V::Right => KeyCode::Right,
                V::Down => KeyCode::Down,
                V::LAlt | V::RAlt => KeyCode::Alt,
                V::LControl | V::RControl => KeyCode::Control,
                V::LMenu | V::RMenu => KeyCode::Super,
                V::LShift | V::RShift => KeyCode::Shift,
                V::LWin | V::RWin => KeyCode::Super,
                _ => {
                    eprintln!("unhandled key: {:?}", event);
                    return Ok(());
                }
            };

            match event.state {
                ElementState::Pressed => self.terminal.key_down(key, mods, &mut self.host)?,
                ElementState::Released => self.terminal.key_up(key, mods, &mut self.host)?,
            }
        }
        self.paint_if_needed()?;
        Ok(())
    }

    pub fn paint_if_needed(&mut self) -> Result<(), Error> {
        if self.terminal.has_dirty_lines() {
            self.paint()?;
        }
        Ok(())
    }

    pub fn dispatch_event(&mut self, event: glutin::Event) -> Result<(), Error> {
        use glium::glutin::{Event, WindowEvent};
        match event {
            Event::WindowEvent {
                event: WindowEvent::Closed,
                ..
            } => {
                return Err(SessionTerminated::WindowClosed.into());
            }
            Event::WindowEvent {
                event: WindowEvent::HiDPIFactorChanged(_),
                ..
            } => {
                // Assuming that this is dragging a window between hidpi and
                // normal dpi displays.  Treat this as a resize event of sorts
                let size = self.host.display.gl_window().get_inner_size();
                if let Some((width, height)) = size {
                    self.resize_surfaces(width as u16, height as u16)?;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(width, height),
                ..
            } => {
                self.resize_surfaces(width as u16, height as u16)?;
            }
            Event::WindowEvent {
                event: WindowEvent::Moved(x, y),
                ..
            } => {
                self.window_position = Some((x, y));
            }
            Event::WindowEvent {
                event: WindowEvent::ReceivedCharacter(c),
                ..
            } => {
                // Primitive Alt-Enter fullscreen shortcut
                if c == '\r' && self.last_modifiers.contains(KeyModifiers::ALT) {
                    let window = self.host.display.gl_window();
                    let pos = self.is_fullscreen.take();
                    if let Some((x, y)) = pos {
                        window.set_fullscreen(None);
                        window.set_position(x, y);
                    } else {
                        // We use our own idea of the position because get_position()
                        // appears to only return the initial position of the window
                        // on Linux.
                        self.is_fullscreen = self.window_position.take();
                        window.set_fullscreen(Some(window.get_current_monitor()));
                    }
                } else {
                    self.terminal
                        .key_down(KeyCode::Char(c), self.last_modifiers, &mut self.host)?;
                }
                self.paint_if_needed()?;
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
                        position: (x, y),
                        modifiers,
                        ..
                    },
                ..
            } => {
                self.mouse_move(x, y, modifiers)?;
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

    pub fn need_paint(&self) -> bool {
        self.terminal.has_dirty_lines()
    }

    pub fn test_for_child_exit(&mut self) -> Result<(), SessionTerminated> {
        match self.process.try_wait() {
            Ok(Some(status)) => Err(SessionTerminated::ProcessStatus { status }),
            Ok(None) => Ok(()),
            Err(e) => Err(SessionTerminated::Error { err: e.into() }),
        }
    }
}
