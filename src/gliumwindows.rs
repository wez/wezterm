//! Generic system dependent windows via glium+glutin
#![allow(dead_code)]

use failure::Error;
use font::FontConfiguration;
use glium::{self, glutin};
use opengl::render::Renderer;
use pty::MasterPty;
use std::io::{Read, Write};
use std::process::Child;
use std::rc::Rc;
use term::{self, Terminal};
use term::hyperlink::Hyperlink;

struct Host {
    display: glium::Display,
    pty: MasterPty,
    clipboard: Option<String>,
}

impl term::TerminalHost for Host {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }
    fn click_link(&mut self, _link: &Rc<Hyperlink>) {}
    fn get_clipboard(&mut self) -> Result<String, Error> {
        bail!("no clipboard");
    }
    fn set_clipboard(&mut self, _clip: Option<String>) -> Result<(), Error> {
        bail!("no clipboard");
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
}

impl TerminalWindow {
    pub fn new(
        event_loop: &glutin::EventsLoop,
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

        let host = Host {
            display,
            pty,
            clipboard: None,
        };

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
        })
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.host.display.draw();
        let res = self.renderer.paint(&mut target, &mut self.terminal);
        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target.finish().unwrap();
        res?;
        Ok(())
    }

    pub fn handle_pty_readable_event(&mut self) {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        match self.host.pty.read(&mut buf) {
            Ok(size) => self.terminal.advance_bytes(&buf[0..size], &mut self.host),
            Err(err) => eprintln!("error reading from pty: {:?}", err),
        }
    }
}
