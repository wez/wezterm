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

use futures::Future;
use glium::glutin::WindowId;
use mio::{PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::process::Command;
use std::str;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

mod config;

mod futurecore;
mod remotemio;
mod opengl;

mod clipboard;
mod wakeup;
use wakeup::{Wakeup, WakeupMsg};
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

struct GuiEventLoop {
    event_loop: RefCell<glium::glutin::EventsLoop>,
    windows_by_id: RefCell<HashMap<WindowId, gliumwindows::TerminalWindow>>,
    windows_by_fd: RefCell<HashMap<RawFd, WindowId>>,
    wakeup_receiver: std::sync::mpsc::Receiver<WakeupMsg>,
    wakeup: Wakeup,
    terminate: RefCell<bool>,
    core: futurecore::Core,
    poll: remotemio::IOMgr,
    poll_rx: Receiver<remotemio::Notification>,
}

impl GuiEventLoop {
    fn new() -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();
        let core = futurecore::Core::new();
        let (wakeup_receiver, wakeup) = Wakeup::new(event_loop.create_proxy());

        let (wake_tx, poll_rx) = wakeup::channel(event_loop.create_proxy());

        let poll = remotemio::IOMgr::new(Duration::from_millis(50), wake_tx);
        sigchld::activate(wakeup.clone())?;

        Ok(Self {
            core,
            poll,
            poll_rx,
            event_loop: RefCell::new(event_loop),
            wakeup_receiver,
            wakeup,
            windows_by_id: RefCell::new(HashMap::new()),
            windows_by_fd: RefCell::new(HashMap::new()),
            terminate: RefCell::new(false),
        })
    }

    fn add_window(&self, window: gliumwindows::TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let fd = window.pty_fd();
        self.windows_by_id.borrow_mut().insert(window_id, window);
        self.windows_by_fd.borrow_mut().insert(fd, window_id);
        self.poll
            .register(fd, Token(fd as usize), Ready::readable(), PollOpt::edge())?
            .wait()??;
        Ok(())
    }

    fn process_gui_event(
        &self,
        event: glium::glutin::Event,
        dead_windows: &mut HashSet<WindowId>,
    ) -> Result<glium::glutin::ControlFlow, Error> {
        use glium::glutin::ControlFlow::{Break, Continue};
        use glium::glutin::Event;
        let result = match event {
            Event::WindowEvent { window_id, .. } => {
                match self.windows_by_id.borrow_mut().get_mut(&window_id) {
                    Some(window) => match window.dispatch_event(event) {
                        Ok(_) => Continue,
                        Err(err) => match err.downcast_ref::<gliumwindows::SessionTerminated>() {
                            Some(_) => {
                                dead_windows.insert(window_id.clone());
                                Continue
                            }
                            _ => {
                                eprintln!("{:?}", err);
                                Break
                            }
                        },
                    },
                    None => {
                        // This happens surprisingly often!
                        // eprintln!("window event for unknown {:?}", window_id);
                        Continue
                    }
                }
            }
            Event::Awakened => Break,
            _ => Continue,
        };
        Ok(result)
    }

    fn process_pty_event(&self, event: mio::Event) -> Result<(), Error> {
        // The token is the fd
        let fd = event.token().0 as RawFd;

        let mut by_fd = self.windows_by_fd.borrow_mut();
        let result = {
            let window_id = by_fd.get(&fd).ok_or_else(|| {
                format_err!("fd {} has no associated window in windows_by_fd map", fd)
            })?;

            let mut by_id = self.windows_by_id.borrow_mut();
            let window = by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!(
                    "fd {} -> window_id {:?} but no associated window is in the windows_by_id map",
                    fd,
                    window_id
                )
            })?;
            window.try_read_pty()
        };

        match result {
            Ok(_) => Ok(()),
            Err(err) => match err.downcast_ref::<gliumwindows::SessionTerminated>() {
                Some(_) => {
                    eprintln!("shutting down pty: {:?}", err);
                    self.poll.deregister(fd)?.wait()??;
                    by_fd.remove(&fd);
                    Ok(())
                }
                _ => {
                    bail!("{:?}", err);
                }
            },
        }
    }

    fn do_paint(&self) {
        for (_, window) in self.windows_by_id.borrow_mut().iter_mut() {
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

    fn process_wakeups(&self, dead_windows: &mut HashSet<WindowId>) -> Result<(), Error> {
        loop {
            match self.wakeup_receiver.try_recv() {
                Ok(WakeupMsg::SigChld) => {
                    for (window_id, window) in self.windows_by_id.borrow_mut().iter_mut() {
                        match window.test_for_child_exit() {
                            Ok(_) => {}
                            Err(err) => {
                                eprintln!("pty finished: {:?}", err);
                                dead_windows.insert(window_id.clone());
                            }
                        }
                    }
                }
                Ok(WakeupMsg::Paste(window_id)) => {
                    self.windows_by_id
                        .borrow_mut()
                        .get_mut(&window_id)
                        .map(|w| w.process_clipboard());
                }
                Err(_) => return Ok(()),
            }
        }
    }

    fn run_event_loop(&self, mut dead_windows: &mut HashSet<WindowId>) -> Result<(), Error> {
        let mut event_loop = self.event_loop.borrow_mut();
        event_loop.run_forever(|event| {
            use glium::glutin::ControlFlow::{Break, Continue};

            let result = self.process_gui_event(event, &mut dead_windows);

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

    fn run(&self) -> Result<(), Error> {
        while !*self.terminate.borrow() {
            let mut dead_windows = HashSet::new();
            self.process_wakeups(&mut dead_windows)?;
            self.process_wakeups_new()?;
            self.run_event_loop(&mut dead_windows)?;

            for window_id in dead_windows {
                self.windows_by_id.borrow_mut().remove(&window_id);
            }

            if self.windows_by_id.borrow().len() == 0 {
                // If we have no more windows left to manage, we're done
                *self.terminate.borrow_mut() = true;
            }
        }
        Ok(())
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
        event_loop.wakeup.clone(),
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
