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

use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use std::env;
use std::ffi::CStr;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::str;
use std::thread;
use std::time::{Duration, Instant};

mod config;

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

fn run_glium(
    master: pty::MasterPty,
    child: std::process::Child,
    config: config::Config,
    fontconfig: FontConfiguration,
    terminal: term::Terminal,
    initial_pixel_width: u16,
    initial_pixel_height: u16,
) -> Result<(), Error> {
    let mut events_loop = glium::glutin::EventsLoop::new();

    let (wakeup_receiver, wakeup) = Wakeup::new(events_loop.create_proxy());
    sigchld::activate(wakeup.clone())?;

    let master_fd = master.as_raw_fd();

    let mut window = gliumwindows::TerminalWindow::new(
        &events_loop,
        wakeup.clone(),
        wakeup_receiver,
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

    {
        let mut wakeup = wakeup.clone();
        thread::spawn(move || {
            let poll = Poll::new().expect("mio Poll failed to init");
            poll.register(
                &EventedFd(&master_fd),
                Token(0),
                Ready::readable(),
                PollOpt::edge(),
            ).expect("failed to register pty");
            let mut events = Events::with_capacity(8);
            let mut last_paint = Instant::now();
            let refresh = Duration::from_millis(50);

            loop {
                let now = Instant::now();
                let diff = now - last_paint;
                let period = if diff >= refresh {
                    // Tick and wakeup the gui thread to ask it to render
                    // if needed.  Without this we'd only repaint when
                    // the window system decides that we were damaged.
                    // We don't want to paint after every state change
                    // as that would be too frequent.
                    wakeup
                        .send(WakeupMsg::Paint)
                        .expect("failed to wakeup gui thread");
                    last_paint = now;
                    refresh
                } else {
                    refresh - diff
                };

                match poll.poll(&mut events, Some(period)) {
                    Ok(_) => for event in &events {
                        if event.token() == Token(0) && event.readiness().is_readable() {
                            wakeup
                                .send(WakeupMsg::PtyReadable)
                                .expect("failed to wakeup gui thread");
                        }
                    },
                    _ => {}
                }
            }
        });
    }

    events_loop.run_forever(|event| match window.dispatch_event(event) {
        Ok(_) => glium::glutin::ControlFlow::Continue,
        Err(err) => {
            eprintln!("{:?}", err);
            glium::glutin::ControlFlow::Break
        }
    });

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
