#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate serde_derive;
#[macro_use]
pub mod log;

use clap::{App, Arg};
use failure::Error;

#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
mod xwindows;

use std::rc::Rc;

mod config;

mod futurecore;
mod opengl;

#[cfg(any(windows, feature = "force-glutin", target_os = "macos"))]
mod gliumwindows;
mod guiloop;

use crate::guiloop::{GuiEventLoop, TerminalWindow};

mod font;
use crate::font::FontConfiguration;

mod pty;
pub use crate::pty::{openpty, Child, Command, ExitStatus, MasterPty, SlavePty};
#[cfg(unix)]
mod sigchld;

use std::env;

/// Determine which shell to run.
/// We take the contents of the $SHELL env var first, then
/// fall back to looking it up from the password database.
#[cfg(unix)]
fn get_shell() -> Result<String, Error> {
    env::var("SHELL").or_else(|_| {
        let ent = unsafe { libc::getpwuid(libc::getuid()) };

        if ent.is_null() {
            Ok("/bin/sh".into())
        } else {
            use std::ffi::CStr;
            use std::str;
            let shell = unsafe { CStr::from_ptr((*ent).pw_shell) };
            shell
                .to_str()
                .map(str::to_owned)
                .map_err(|e| format_err!("failed to resolve shell: {:?}", e))
        }
    })
}

#[cfg(windows)]
fn get_shell() -> Result<String, Error> {
    Ok(env::var("ComSpec").unwrap_or("cmd.exe".into()))
}

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
//    terminal.advance_bytes(message);
// !=

fn main() -> Result<(), Error> {
    let args = App::new("wezterm")
        .version("0.1")
        .author("Wez Furlong <wez@wezfurlong.org>")
        .about(
            "Wez's Terminal Emulator\n\
             http://github.com/wez/wezterm",
        )
        .arg(
            Arg::with_name("SKIP_CONFIG")
                .short("n")
                .help("Skip loading ~/.wezterm.toml"),
        )
        .arg(Arg::with_name("PROG").multiple(true).help(
            "Instead of executing your shell, run PROG. \
             For example: `wezterm -- bash -l` will spawn bash \
             as if it were a login shell.",
        ))
        .get_matches();
    let config = Rc::new(if args.is_present("SKIP_CONFIG") {
        config::Config::default_config()
    } else {
        config::Config::load()?
    });
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
    // This convoluted run() signature is present because of this issue:
    // https://github.com/tomaka/winit/issues/413
    GuiEventLoop::run(&event_loop, &config, &fontconfig)?;
    Ok(())
}

fn spawn_window(
    event_loop: &Rc<GuiEventLoop>,
    cmd: Option<Vec<&std::ffi::OsStr>>,
    config: &Rc<config::Config>,
    fontconfig: &Rc<FontConfiguration>,
) -> Result<(), Error> {
    let mut cmd = match cmd {
        Some(args) => {
            let mut args = args.iter();
            let mut cmd = Command::new(args.next().expect("executable name"));
            cmd.args(args);
            cmd
        }
        None => Command::new(get_shell()?),
    };

    cmd.env("TERM", &config.term);

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

    let (master, slave) = openpty(
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
