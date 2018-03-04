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

use std::env;
use std::ffi::CStr;
use std::process::Command;
use std::rc::Rc;
use std::str;

mod config;

mod futurecore;
mod remotemio;
mod opengl;

mod clipboard;
mod glutinloop;
use glutinloop::GuiEventLoop;
mod gliumwindows;
use gliumwindows::TerminalWindow;

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
    let config = Rc::new(config::Config::load()?);
    println!("Using configuration: {:#?}", config);

    let fontconfig = Rc::new(FontConfiguration::new(Rc::clone(&config)));

    let cmd = if args.is_present("PROG") {
        Some(
            args.values_of_os("PROG")
                .expect("PROG wasn't present after all!?")
                .collect(),
        )
    } else {
        None
    };

    let event_loop = Rc::new(GuiEventLoop::new()?);

    spawn_window(&event_loop, cmd, &config, &fontconfig)?;

    event_loop.run()?;
    Ok(())
}

fn spawn_window(
    event_loop: &Rc<GuiEventLoop>,
    cmd: Option<Vec<&std::ffi::OsStr>>,
    config: &Rc<config::Config>,
    fontconfig: &Rc<FontConfiguration>,
) -> Result<(), Error> {
    let cmd = match cmd {
        Some(args) => {
            let mut args = args.iter();
            let mut cmd = Command::new(args.next().expect("executable name"));
            cmd.args(args);
            cmd
        }
        None => Command::new(get_shell()?),
    };

    // First step is to figure out the font metrics so that we know how
    // big things are going to be.
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

    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    let terminal = term::Terminal::new(
        initial_rows as usize,
        initial_cols as usize,
        config.scrollback_lines.unwrap_or(3500),
        config.hyperlink_rules.clone(),
    );

    let window = TerminalWindow::new(event_loop, terminal, master, child, fontconfig, config)?;

    event_loop.add_window(window)
}

fn main() {
    run().unwrap();
}
