#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate serde_derive;
#[macro_use]
pub mod log;
use failure::Error;
use std::ffi::OsString;
use structopt::StructOpt;

#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
mod xwindows;

use std::rc::Rc;

mod config;
mod futurecore;
mod gliumwindows;
mod guiloop;
mod opengl;
use crate::guiloop::GuiSelection;
use crate::guiloop::GuiSystem;

mod font;
use crate::font::{FontConfiguration, FontSystemSelection};

mod pty;
pub use crate::pty::{openpty, Child, Command, ExitStatus, MasterPty, SlavePty};

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

#[derive(Debug, StructOpt)]
#[structopt(
    name = "wezterm",
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm"
)]
#[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct Opt {
    /// Skip loading ~/.wezterm.toml
    #[structopt(short = "n")]
    skip_config: bool,

    #[structopt(
        long = "gui-system",
        raw(
            possible_values = "&GuiSelection::variants()",
            case_insensitive = "true"
        )
    )]
    gui_system: Option<GuiSelection>,

    #[structopt(
        long = "font-system",
        raw(
            possible_values = "&FontSystemSelection::variants()",
            case_insensitive = "true"
        )
    )]
    font_system: Option<FontSystemSelection>,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
}

fn main() -> Result<(), Error> {
    let opts = Opt::from_args();
    let config = Rc::new(if opts.skip_config {
        config::Config::default_config()
    } else {
        config::Config::load()?
    });
    println!("Using configuration: {:#?}\nopts: {:#?}", config, opts);

    let font_system = opts.font_system.unwrap_or(config.font_system);
    let fontconfig = Rc::new(FontConfiguration::new(Rc::clone(&config), font_system));

    let cmd = if opts.prog.len() > 0 {
        Some(opts.prog.iter().map(|x| x.as_os_str()).collect())
    } else {
        None
    };

    let gui_system = opts.gui_system.unwrap_or(config.gui_system);
    let gui = gui_system.new()?;

    spawn_window(&*gui, cmd, &config, &fontconfig)?;
    gui.run_forever(&config, &fontconfig)
}

fn spawn_window(
    gui: &GuiSystem,
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

    gui.spawn_new_window(terminal, master, child, config, fontconfig)
}
