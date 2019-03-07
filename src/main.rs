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
use std::sync::Arc;

mod config;
mod gliumwindows;
mod guicommon;
mod guiloop;
mod mux;
mod opengl;
mod server;
use crate::guicommon::tabs::{LocalTab, Tab};
use crate::guiloop::GuiSelection;
use crate::guiloop::GuiSystem;
use crate::mux::Mux;

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

    /// Rather than running the terminal, run as the
    /// multiplexer server.
    #[structopt(long = "start-mux-server")]
    start_mux_server: bool,
}

fn run_terminal_gui(config: Arc<config::Config>, opts: Opt) -> Result<(), Error> {
    let font_system = opts.font_system.unwrap_or(config.font_system);
    font_system.set_default();

    let fontconfig = Rc::new(FontConfiguration::new(Arc::clone(&config), font_system));

    let cmd = if !opts.prog.is_empty() {
        Some(opts.prog.iter().map(|x| x.as_os_str()).collect())
    } else {
        None
    };

    let mux = Rc::new(mux::Mux::default());
    Mux::set_mux(&mux);

    let gui_system = opts.gui_system.unwrap_or(config.gui_system);
    let gui = gui_system.try_new(&mux)?;

    spawn_window(&mux, &*gui, cmd, &config, &fontconfig)?;
    gui.run_forever()
}

fn main() -> Result<(), Error> {
    let opts = Opt::from_args();
    let config = Arc::new(if opts.skip_config {
        config::Config::default_config()
    } else {
        config::Config::load()?
    });
    println!("Using configuration: {:#?}\nopts: {:#?}", config, opts);

    if opts.start_mux_server {
        server::listener::run_mux_server(config)
    } else {
        run_terminal_gui(config, opts)
    }
}

fn spawn_tab(
    config: &Arc<config::Config>,
    cmd: Option<Vec<&std::ffi::OsStr>>,
) -> Result<Rc<Tab>, Error> {
    let cmd = config.build_prog(cmd)?;

    let initial_cols = 80u16;
    let initial_rows = 24u16;
    let initial_pixel_width = 0;
    let initial_pixel_height = 0;

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

    Ok(Rc::new(LocalTab::new(terminal, child, master)))
}

fn spawn_window(
    mux: &Rc<Mux>,
    gui: &GuiSystem,
    cmd: Option<Vec<&std::ffi::OsStr>>,
    config: &Arc<config::Config>,
    fontconfig: &Rc<FontConfiguration>,
) -> Result<(), Error> {
    let tab = spawn_tab(config, cmd)?;
    mux.add_tab(gui.gui_executor(), &tab)?;

    gui.spawn_new_window(config, &fontconfig, &tab)
}
