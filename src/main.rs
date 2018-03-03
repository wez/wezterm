extern crate clap;
#[cfg(target_os = "macos")]
extern crate core_text;
extern crate euclid;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
extern crate freetype;
extern crate futures;
extern crate gl;
#[macro_use]
extern crate glium;
extern crate harfbuzz_sys;
extern crate libc;
extern crate mio;
extern crate mio_extras;
extern crate palette;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate term;
extern crate toml;
extern crate unicode_width;
#[macro_use]
pub mod log;

use clap::{App, Arg};
use failure::Error;

#[cfg(all(unix, not(target_os = "macos")))]
extern crate xcb;
#[cfg(all(unix, not(target_os = "macos")))]
extern crate xcb_util;

use futures::{future, Future};
use glium::glutin::WindowId;
use mio::{PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::process::Command;
use std::rc::Rc;
use std::str;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

mod config;

mod futurecore;
mod remotemio;
mod opengl;

mod clipboard;
mod wakeup;
use wakeup::GuiSender;
mod gliumwindows;

mod font;
use font::FontConfiguration;

mod pty;
mod sigchld;

/// Determine which shell to run.
/// We take the contents of the $SHELL env var first, then
/// fall back to looking it up from the password database.
fn get_shell() -> Result<String, Error> {
    env::var("SHELL").or_else(|_| {
        let ent = unsafe { libc::getpwuid(libc::getuid()) };

        if ent.is_null() {
            Ok("/bin/sh".into())
        } else {
            let shell = unsafe { CStr::from_ptr((*ent).pw_shell) };
            shell
                .to_str()
                .map(str::to_owned)
                .map_err(|e| format_err!("failed to resolve shell: {:?}", e))
        }
    })
}

#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, gliumwindows::TerminalWindow>,
    by_fd: HashMap<RawFd, WindowId>,
}

struct GuiEventLoop {
    event_loop: RefCell<glium::glutin::EventsLoop>,
    windows: Rc<RefCell<Windows>>,
    core: futurecore::Core,
    poll: remotemio::IOMgr,
    poll_rx: Receiver<remotemio::Notification>,
    paster: GuiSender<WindowId>,
    paster_rx: Receiver<WindowId>,
    sigchld_rx: Receiver<()>,
}

impl GuiEventLoop {
    fn new() -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();
        let core = futurecore::Core::new(event_loop.create_proxy());

        let (wake_tx, poll_rx) = wakeup::channel(event_loop.create_proxy());
        let (paster, paster_rx) = wakeup::channel(event_loop.create_proxy());
        let (sigchld_tx, sigchld_rx) = wakeup::channel(event_loop.create_proxy());

        let poll = remotemio::IOMgr::new(Duration::from_millis(50), wake_tx);
        sigchld::activate(sigchld_tx)?;

        Ok(Self {
            core,
            poll,
            poll_rx,
            paster,
            paster_rx,
            sigchld_rx,
            event_loop: RefCell::new(event_loop),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    fn add_window(&self, window: gliumwindows::TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let fd = window.pty_fd();
        let mut windows = self.windows.borrow_mut();
        windows.by_id.insert(window_id, window);
        windows.by_fd.insert(fd, window_id);
        self.poll
            .register(fd, Token(fd as usize), Ready::readable(), PollOpt::edge())?
            .wait()??;
        Ok(())
    }

    fn process_gui_event(
        &self,
        event: &glium::glutin::Event,
    ) -> Result<glium::glutin::ControlFlow, Error> {
        use glium::glutin::ControlFlow::{Break, Continue};
        use glium::glutin::Event;
        let result = match *event {
            Event::WindowEvent { window_id, .. } => {
                let dead = match self.windows.borrow_mut().by_id.get_mut(&window_id) {
                    Some(window) => match window.dispatch_event(event) {
                        Ok(_) => None,
                        Err(err) => match err.downcast_ref::<gliumwindows::SessionTerminated>() {
                            Some(_) => Some(window_id),
                            _ => return Err(err),
                        },
                    },
                    None => None,
                };

                if let Some(window_id) = dead {
                    self.schedule_window_close(window_id)?;
                }
                Continue
            }
            Event::Awakened => Break,
            _ => Continue,
        };
        Ok(result)
    }

    fn schedule_window_close(&self, window_id: WindowId) -> Result<(), Error> {
        let fd = {
            let mut windows = self.windows.borrow_mut();

            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;
            window.pty_fd()
        };

        let windows = Rc::clone(&self.windows);

        self.core.spawn(self.poll.deregister(fd)?.then(move |_| {
            println!("done dereg");
            let mut windows = windows.borrow_mut();
            windows.by_id.remove(&window_id);
            windows.by_fd.remove(&fd);
            future::ok(())
        }));

        Ok(())
    }

    fn process_pty_event(&self, event: mio::Event) -> Result<(), Error> {
        // The token is the fd
        let fd = event.token().0 as RawFd;

        let (window_id, result) = {
            let mut windows = self.windows.borrow_mut();

            let window_id = windows
                .by_fd
                .get(&fd)
                .ok_or_else(|| {
                    format_err!("fd {} has no associated window in windows_by_fd map", fd)
                })
                .map(|w| *w)?;

            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!(
                    "fd {} -> window_id {:?} but no associated window is in the windows_by_id map",
                    fd,
                    window_id
                )
            })?;
            (window_id, window.try_read_pty())
        };

        match result {
            Ok(_) => Ok(()),
            Err(err) => match err.downcast_ref::<gliumwindows::SessionTerminated>() {
                Some(_) => {
                    self.schedule_window_close(window_id)?;
                    Ok(())
                }
                _ => {
                    bail!("{:?}", err);
                }
            },
        }
    }

    fn do_paint(&self) {
        for window in &mut self.windows.borrow_mut().by_id.values_mut() {
            window.paint_if_needed().unwrap();
        }
    }

    fn process_wakeups_new(&self) -> Result<(), Error> {
        loop {
            match self.poll_rx.try_recv() {
                Ok(remotemio::Notification::EventReady(event)) => {
                    match self.process_pty_event(event) {
                        Ok(_) => {}
                        Err(err) => eprintln!("process_pty_event: {:?}", err),
                    }
                }
                Ok(remotemio::Notification::IntervalDone) => self.do_paint(),
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("poll_rx disconnected {:?}", err),
            }
        }
    }

    fn process_paste_wakeups(&self) -> Result<(), Error> {
        loop {
            match self.paster_rx.try_recv() {
                Ok(window_id) => {
                    self.windows
                        .borrow_mut()
                        .by_id
                        .get_mut(&window_id)
                        .map(|w| w.process_clipboard());
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("paster_rx disconnected {:?}", err),
            }
        }
    }

    fn process_sigchld_wakeups(&self) -> Result<(), Error> {
        loop {
            match self.sigchld_rx.try_recv() {
                Ok(_) => {
                    let window_ids: Vec<WindowId> = self.windows
                        .borrow_mut()
                        .by_id
                        .iter_mut()
                        .filter_map(|(window_id, window)| match window.test_for_child_exit() {
                            Ok(_) => None,
                            Err(_) => Some(*window_id),
                        })
                        .collect();

                    for window_id in window_ids {
                        self.schedule_window_close(window_id)?;
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("paster_rx disconnected {:?}", err),
            }
        }
    }

    fn run_event_loop(&self) -> Result<(), Error> {
        let mut event_loop = self.event_loop.borrow_mut();
        event_loop.run_forever(|event| {
            use glium::glutin::ControlFlow::{Break, Continue};

            let result = self.process_gui_event(&event);

            match result {
                Ok(Continue) => Continue,
                Ok(Break) => Break,
                Err(err) => {
                    eprintln!("Error in event loop: {:?}", err);
                    Break
                }
            }
        });
        Ok(())
    }

    fn process_futures(&self) {
        loop {
            if !self.core.turn() {
                break;
            }
            println!("did one future");
        }
    }

    fn run(&self) -> Result<(), Error> {
        loop {
            self.process_futures();

            {
                let windows = self.windows.borrow();
                if windows.by_id.is_empty() && windows.by_fd.is_empty() {
                    eprintln!("No more windows; done!");
                    return Ok(());
                }
            }

            self.run_event_loop()?;
            self.process_wakeups_new()?;
            self.process_paste_wakeups()?;
            self.process_sigchld_wakeups()?;
        }
    }
}

impl Drop for GuiEventLoop {
    fn drop(&mut self) {}
}

fn run_glium(
    master: pty::MasterPty,
    child: std::process::Child,
    config: config::Config,
    fontconfig: FontConfiguration,
    terminal: term::Terminal,
    initial_pixel_width: u16,
    initial_pixel_height: u16,
) -> Result<(), Error> {
    let event_loop = GuiEventLoop::new()?;

    let window = gliumwindows::TerminalWindow::new(
        &*event_loop.event_loop.borrow_mut(),
        event_loop.paster.clone(),
        initial_pixel_width,
        initial_pixel_height,
        terminal,
        master,
        child,
        fontconfig,
        config
            .colors
            .map(|p| p.into())
            .unwrap_or_else(term::color::ColorPalette::default),
    )?;

    event_loop.add_window(window)?;

    event_loop.run()?;

    Ok(())
}

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
//    terminal.advance_bytes(message);
// !=

fn run() -> Result<(), Error> {
    let args = App::new("wezterm")
        .version("0.1")
        .author("Wez Furlong <wez@wezfurlong.org>")
        .about(
            "Wez's Terminal Emulator\n\
             http://github.com/wez/wezterm",
        )
        .arg(Arg::with_name("PROG").multiple(true).help(
            "Instead of executing your shell, run PROG. \
             For example: `wezterm -- bash -l` will spawn bash \
             as if it were a login shell.",
        ))
        .get_matches();
    let config = config::Config::load()?;
    println!("Using configuration: {:#?}", config);

    // First step is to figure out the font metrics so that we know how
    // big things are going to be.

    let fontconfig = FontConfiguration::new(config.clone());
    let font = fontconfig.default_font()?;

    // we always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let metrics = font.borrow_mut().get_fallback(0)?.metrics();

    let initial_cols = 80u16;
    let initial_rows = 24u16;
    let initial_pixel_width = initial_cols * metrics.cell_width.ceil() as u16;
    let initial_pixel_height = initial_rows * metrics.cell_height.ceil() as u16;

    let (master, slave) = pty::openpty(
        initial_rows,
        initial_cols,
        initial_pixel_width,
        initial_pixel_height,
    )?;

    let cmd = if args.is_present("PROG") {
        let mut args = args.values_of_os("PROG")
            .expect("PROG wasn't present after all!?");
        let mut cmd = Command::new(args.next().expect("executable name"));
        cmd.args(args);
        cmd
    } else {
        Command::new(get_shell()?)
    };
    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    let terminal = term::Terminal::new(
        initial_rows as usize,
        initial_cols as usize,
        config.scrollback_lines.unwrap_or(3500),
        config.hyperlink_rules.clone(),
    );

    run_glium(
        master,
        child,
        config,
        fontconfig,
        terminal,
        initial_pixel_width,
        initial_pixel_height,
    )
}

fn main() {
    run().unwrap();
}
