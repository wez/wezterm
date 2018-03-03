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

use glium::glutin::WindowId;
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use mio_extras::channel::{channel as mio_channel, Receiver as MioReceiver, Sender as MioSender};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::CStr;
use std::os::unix::io::RawFd;
use std::process::Command;
use std::str;
use std::sync::mpsc::TryRecvError;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

mod config;

mod futurecore;
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

enum PtyMsg {
    AddPty(WindowId, RawFd),
    #[allow(dead_code)]
    DelPty(WindowId, RawFd),
    Terminate,
}

fn run_pty_thread(mut wakeup: Wakeup, receiver: MioReceiver<PtyMsg>) -> Result<(), Error> {
    let poll = Poll::new()?;

    let mut events = Events::with_capacity(8);
    let mut last_paint = Instant::now();
    let refresh = Duration::from_millis(50);

    let mut window_id_to_token = HashMap::new();
    let mut token_to_window_id = HashMap::new();
    let mut next_token_id = 10;

    poll.register(&receiver, Token(0), Ready::readable(), PollOpt::level())?;

    loop {
        let now = Instant::now();
        let diff = now - last_paint;
        let period = if diff >= refresh {
            // Tick and wakeup the gui thread to ask it to render
            // if needed.  Without this we'd only repaint when
            // the window system decides that we were damaged.
            // We don't want to paint after every state change
            // as that would be too frequent.
            wakeup.send(WakeupMsg::Paint)?;
            last_paint = now;
            refresh
        } else {
            refresh - diff
        };

        match poll.poll(&mut events, Some(period)) {
            Ok(_) => for event in &events {
                if event.token() == Token(0) {
                    match receiver.try_recv() {
                        Err(TryRecvError::Empty) => {}
                        Err(err) => bail!("error receiving PtyMsg {:?}", err),
                        Ok(PtyMsg::Terminate) => {
                            eprintln!("pty thread: Terminate");
                            return Ok(());
                        }
                        Ok(PtyMsg::AddPty(window_id, fd)) => {
                            let token = Token(next_token_id);
                            poll.register(
                                &EventedFd(&fd),
                                token,
                                Ready::readable(),
                                PollOpt::edge(),
                            )?;
                            window_id_to_token.insert(window_id, token);
                            token_to_window_id.insert(token, window_id);
                            next_token_id += 1;
                        }
                        Ok(PtyMsg::DelPty(_window_id, fd)) => {
                            eprintln!("DelPty {:?} {}", _window_id, fd);
                            poll.deregister(&EventedFd(&fd))?;
                        }
                    }
                } else {
                    match token_to_window_id.get(&event.token()) {
                        Some(window_id) => {
                            wakeup.send(WakeupMsg::PtyReadable(*window_id))?;
                        }
                        None => (),
                    }
                }
            },
            _ => {}
        }
    }
}

fn start_pty_thread(wakeup: Wakeup, receiver: MioReceiver<PtyMsg>) -> JoinHandle<()> {
    thread::spawn(move || match run_pty_thread(wakeup, receiver) {
        Ok(_) => {}
        Err(err) => eprintln!("pty thread returned error {:?}", err),
    })
}

struct GuiEventLoop {
    event_loop: RefCell<glium::glutin::EventsLoop>,
    windows_by_id: RefCell<HashMap<WindowId, gliumwindows::TerminalWindow>>,
    wakeup_receiver: std::sync::mpsc::Receiver<WakeupMsg>,
    wakeup: Wakeup,
    pty_sender: MioSender<PtyMsg>,
    pty_thread: Option<JoinHandle<()>>,
    terminate: RefCell<bool>,
}

impl GuiEventLoop {
    fn new() -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();
        let (wakeup_receiver, wakeup) = Wakeup::new(event_loop.create_proxy());
        sigchld::activate(wakeup.clone())?;

        let (pty_sender, pty_receiver) = mio_channel();
        let pty_thread = start_pty_thread(wakeup.clone(), pty_receiver);

        Ok(Self {
            event_loop: RefCell::new(event_loop),
            wakeup_receiver,
            wakeup,
            windows_by_id: RefCell::new(HashMap::new()),
            pty_sender,
            pty_thread: Some(pty_thread),
            terminate: RefCell::new(false),
        })
    }

    fn add_window(&self, window: gliumwindows::TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let fd = window.pty_fd();
        self.windows_by_id.borrow_mut().insert(window_id, window);
        self.pty_sender.send(PtyMsg::AddPty(window_id, fd))?;
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

    fn process_wakeups(&self, dead_windows: &mut HashSet<WindowId>) -> Result<(), Error> {
        loop {
            match self.wakeup_receiver.try_recv() {
                Ok(WakeupMsg::PtyReadable(window_id)) => {
                    self.windows_by_id
                        .borrow_mut()
                        .get_mut(&window_id)
                        .map(|w| w.try_read_pty());
                }
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
                Ok(WakeupMsg::Paint) => {
                    for (_, window) in self.windows_by_id.borrow_mut().iter_mut() {
                        window.paint_if_needed().unwrap();
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
            self.run_event_loop(&mut dead_windows)?;

            for window_id in dead_windows {
                // TODO: DelPty.  Doing this here spews errors
                /*
                self.pty_sender
                    .send(PtyMsg::DelPty(window_id, master_fd))
                    .unwrap();
                    */
                self.windows_by_id.borrow_mut().remove(&window_id);
            }

            if self.windows_by_id.borrow().len() == 0 {
                // If we have no more windows left to manage, we're done
                *self.terminate.borrow_mut() = true;
            }
        }
        self.pty_sender.send(PtyMsg::Terminate)?;
        Ok(())
    }
}

impl Drop for GuiEventLoop {
    fn drop(&mut self) {
        match self.pty_thread.take() {
            Some(t) => {
                t.join().unwrap();
            }
            None => {}
        }
    }
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
