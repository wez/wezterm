#[cfg(target_os = "macos")]
extern crate core_text;
extern crate euclid;
#[macro_use]
extern crate failure;
#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(any(target_os = "android", all(unix, not(target_os = "macos"))))]
extern crate freetype;
extern crate gl;
#[macro_use]
extern crate glium;
extern crate harfbuzz;
extern crate libc;
extern crate mio;
extern crate palette;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate term;
extern crate toml;
extern crate unicode_width;
#[cfg(all(unix, not(target_os = "macos")))]
extern crate x11;
#[macro_use]
pub mod log;

use failure::Error;

#[cfg(all(unix, not(target_os = "macos")))]
extern crate xcb;
#[cfg(all(unix, not(target_os = "macos")))]
extern crate xcb_util;

use mio::{Events, Poll, PollOpt, Ready, Token};
use std::env;
use std::ffi::CStr;
use std::process::Command;
use std::str;
use std::time::Duration;

mod config;

mod opengl;

mod clipboard;
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
    let poll = Poll::new()?;
    poll.register(&master, Token(0), Ready::readable(), PollOpt::edge())?;

    let waiter = sigchld::ChildWaiter::new()?;
    poll.register(&waiter, Token(2), Ready::readable(), PollOpt::edge())?;

    let mut events_loop = glium::glutin::EventsLoop::new();

    let mut window = gliumwindows::TerminalWindow::new(
        &events_loop,
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

    let mut events = Events::with_capacity(8);
    let mut done = false;

    loop {
        if done {
            break;
        }

        if poll.poll(&mut events, Some(Duration::new(0, 2000)))? != 0 {
            for event in &events {
                if event.token() == Token(0) && event.readiness().is_readable() {
                    window.handle_pty_readable_event()?;
                }
                if event.token() == Token(2) {
                    println!("sigchld ready");
                    let pid = waiter.read_one()?;
                    println!("got sigchld from pid {}", pid);
                    window.test_for_child_exit()?;
                }
            }
        } else if window.need_paint() {
            window.paint()?;
        }
        events_loop.poll_events(|event| match window.dispatch_event(event) {
            Ok(_) => {}
            Err(err) => {
                eprintln!("{:?}", err);
                done = true;
            }
        });
    }
    Ok(())
}

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
//    terminal.advance_bytes(message);
// !=

fn run() -> Result<(), Error> {
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

    let cmd = Command::new(get_shell()?);
    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    let terminal = term::Terminal::new(
        initial_rows as usize,
        initial_cols as usize,
        config.scrollback_lines.unwrap_or(3500),
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
