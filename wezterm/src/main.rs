use anyhow::anyhow;
use config::wezterm_version;
use mux::activity::Activity;
use mux::pane::PaneId;
use mux::tab::SplitDirection;
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use structopt::StructOpt;
use tabout::{tabulate_output, Alignment, Column};
use wezterm_client::client::{unix_connect_with_retry, Client};
use wezterm_gui_subcommands::*;

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
//    terminal.advance_bytes(message);
// !=

#[derive(Debug, StructOpt)]
#[structopt(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    global_setting = structopt::clap::AppSettings::ColoredHelp,
    version = wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[structopt(short = "n")]
    skip_config: bool,

    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, StructOpt, Clone)]
enum SubCommand {
    #[structopt(name = "start", about = "Start a front-end")]
    Start(StartCommand),

    #[structopt(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[structopt(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[structopt(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[structopt(name = "cli", about = "Interact with experimental mux server")]
    Cli(CliCommand),

    #[structopt(name = "imgcat", about = "Output an image to the terminal")]
    ImageCat(ImgCatCommand),
}

#[derive(Debug, StructOpt, Clone)]
struct CliCommand {
    /// Don't automatically start the server
    #[structopt(long = "no-auto-start")]
    no_auto_start: bool,

    #[structopt(subcommand)]
    sub: CliSubCommand,
}

#[derive(Debug, StructOpt, Clone)]
enum CliSubCommand {
    #[structopt(name = "list", about = "list windows, tabs and panes")]
    List,

    #[structopt(name = "proxy", about = "start rpc proxy pipe")]
    Proxy,

    #[structopt(name = "tlscreds", about = "obtain tls credentials")]
    TlsCreds,

    #[structopt(
        name = "split-pane",
        about = "split the current pane.
Outputs the pane-id for the newly created pane on success"
    )]
    SplitPane {
        /// Specify the pane that should be split.
        /// The default is to use the current pane based on the
        /// environment variable WEZTERM_PANE.
        #[structopt(long = "pane-id")]
        pane_id: Option<PaneId>,

        /// Split horizontally rather than vertically
        #[structopt(long = "horizontal")]
        horizontal: bool,

        /// Specify the current working directory for the initially
        /// spawned program
        #[structopt(long = "cwd", parse(from_os_str))]
        cwd: Option<OsString>,

        /// Instead of executing your shell, run PROG.
        /// For example: `wezterm start -- bash -l` will spawn bash
        /// as if it were a login shell.
        #[structopt(parse(from_os_str))]
        prog: Vec<OsString>,
    },
}

use termwiz::escape::osc::{
    ITermDimension, ITermFileData, ITermProprietary, OperatingSystemCommand,
};

#[derive(Debug, StructOpt, Clone)]
struct ImgCatCommand {
    /// Specify the display width; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal width.
    #[structopt(long = "width")]
    width: Option<ITermDimension>,
    /// Specify the display height; defaults to "auto" which automatically selects
    /// an appropriate size.  You may also use an integer value `N` to specify the
    /// number of cells, or `Npx` to specify the number of pixels, or `N%` to
    /// size relative to the terminal height.
    #[structopt(long = "height")]
    height: Option<ITermDimension>,
    /// Do not respect the aspect ratio.  The default is to respect the aspect
    /// ratio
    #[structopt(long = "preserve-aspect-ratio")]
    no_preserve_aspect_ratio: bool,
    /// The name of the image file to be displayed.
    /// If omitted, will attempt to read it from stdin.
    #[structopt(parse(from_os_str))]
    file_name: Option<OsString>,
}

impl ImgCatCommand {
    fn run(&self) -> anyhow::Result<()> {
        let mut data = Vec::new();
        if let Some(file_name) = self.file_name.as_ref() {
            let mut f = std::fs::File::open(file_name)?;
            f.read_to_end(&mut data)?;
        } else {
            let mut stdin = std::io::stdin();
            stdin.read_to_end(&mut data)?;
        }

        let osc = OperatingSystemCommand::ITermProprietary(ITermProprietary::File(Box::new(
            ITermFileData {
                name: None,
                size: Some(data.len()),
                width: self.width.unwrap_or_else(Default::default),
                height: self.height.unwrap_or_else(Default::default),
                preserve_aspect_ratio: !self.no_preserve_aspect_ratio,
                inline: true,
                data,
            },
        )));
        println!("{}", osc);

        Ok(())
    }
}

fn terminate_with_error_message(err: &str) -> ! {
    log::error!("{}; terminating", err);
    std::process::exit(1);
}

fn terminate_with_error(err: anyhow::Error) -> ! {
    terminate_with_error_message(&format!("{:#}", err));
}

fn main() {
    config::designate_this_as_the_main_thread();
    config::assign_error_callback(mux::connui::show_configuration_error_message);
    if let Err(e) = run() {
        terminate_with_error(e);
    }
    Mux::shutdown();
}

/// If LANG isn't set in the environment, make an attempt at setting
/// it to a UTF-8 version of the current locale known to NSLocale.
#[cfg(target_os = "macos")]
fn set_lang_from_locale() {
    use cocoa::base::id;
    use cocoa::foundation::NSString;
    use objc::runtime::Object;
    use objc::*;

    if std::env::var_os("LANG").is_none() {
        unsafe fn nsstring_to_str<'a>(ns: *mut Object) -> &'a str {
            let data = NSString::UTF8String(ns as id) as *const u8;
            let len = NSString::len(ns as id);
            let bytes = std::slice::from_raw_parts(data, len);
            std::str::from_utf8_unchecked(bytes)
        }

        unsafe {
            let locale: *mut Object = msg_send![class!(NSLocale), autoupdatingCurrentLocale];
            let lang_code_obj: *mut Object = msg_send![locale, languageCode];
            let country_code_obj: *mut Object = msg_send![locale, countryCode];

            {
                let lang_code = nsstring_to_str(lang_code_obj);
                let country_code = nsstring_to_str(country_code_obj);

                let candidate = format!("{}_{}.UTF-8", lang_code, country_code);
                let candidate_cstr = std::ffi::CString::new(candidate.as_bytes().clone())
                    .expect("make cstr from str");

                // If this looks like a working locale then export it to
                // the environment so that our child processes inherit it.
                let old = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
                if !libc::setlocale(libc::LC_CTYPE, candidate_cstr.as_ptr()).is_null() {
                    std::env::set_var("LANG", &candidate);
                }
                libc::setlocale(libc::LC_CTYPE, old);
            }

            let _: () = msg_send![lang_code_obj, release];
            let _: () = msg_send![country_code_obj, release];
            let _: () = msg_send![locale, release];
        }
    }
}

fn run() -> anyhow::Result<()> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            std::env::set_var("WEZTERM_EXECUTABLE_DIR", dir);
        }
        std::env::set_var("WEZTERM_EXECUTABLE", exe);
    }

    #[cfg(target_os = "macos")]
    set_lang_from_locale();

    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage = std::path::PathBuf::from(appimage);

        // We were started via an AppImage, presumably ourselves.
        // AppImage exports ARGV0 into the environment and that causes
        // everything that was indirectly spawned by us to appear to
        // be the AppImage.  eg: if you `vim foo` it shows as
        // `WezTerm.AppImage foo`, which is super confusing for everyone!
        // Let's just unset that from the environment!
        std::env::remove_var("ARGV0");

        // This AppImage feature allows redirecting HOME and XDG_CONFIG_HOME
        // to live alongside the executable for portable use:
        // https://github.com/AppImage/AppImageKit/issues/368
        // When we spawn children, we don't want them to inherit this,
        // but we do want to respect them for config loading.
        // Let's force resolution and cleanup our environment now.

        /// Given "/some/path.AppImage" produce "/some/path.AppImageSUFFIX".
        /// We only support this for "path.AppImage" that can be converted
        /// to UTF-8.  Otherwise, we return "/some/path.AppImage" unmodified.
        fn append_extra_file_name_suffix(p: &Path, suffix: &str) -> PathBuf {
            if let Some(name) = p.file_name().and_then(|o| o.to_str()) {
                p.with_file_name(format!("{}{}", name, suffix))
            } else {
                p.to_path_buf()
            }
        }

        /// Our config stuff exports these env vars to help portable apps locate
        /// the correct environment when it is launched via wezterm.
        /// However, if we are using the system wezterm to spawn a portable
        /// AppImage then we want these to not take effect.
        fn clean_wezterm_config_env() {
            std::env::remove_var("WEZTERM_CONFIG_FILE");
            std::env::remove_var("WEZTERM_CONFIG_DIR");
        }

        if config::HOME_DIR.starts_with(append_extra_file_name_suffix(&appimage, ".home")) {
            // Fixup HOME to point to the user's actual home dir
            std::env::remove_var("HOME");
            std::env::set_var("HOME", dirs::home_dir().expect("can't resolve HOME dir"));
            clean_wezterm_config_env();
        }

        if config::CONFIG_DIR.starts_with(append_extra_file_name_suffix(&appimage, ".config")) {
            std::env::remove_var("XDG_CONFIG_HOME");
            clean_wezterm_config_env();
        }
    }

    pretty_env_logger::init_timed();
    let _saver = umask::UmaskSaver::new();

    let opts = Opt::from_args();
    if !opts.skip_config {
        config::reload();
    }
    let config = config::configuration();

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(_)
        | SubCommand::Ssh(_)
        | SubCommand::Serial(_)
        | SubCommand::Connect(_) => delegate_to_gui(),
        SubCommand::ImageCat(cmd) => cmd.run(),
        SubCommand::Cli(cli) => run_cli(config, cli),
    }
}

fn delegate_to_gui() -> anyhow::Result<()> {
    use std::process::Command;

    let exe_name = if cfg!(windows) {
        "wezterm-gui.exe"
    } else {
        "wezterm-gui"
    };

    let exe = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow!("exe has no parent dir!?"))?
        .join(exe_name);

    let mut cmd = Command::new(exe);
    cmd.args(std::env::args_os().skip(1));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        return Err(anyhow::anyhow!("failed to exec: {:?}", cmd.exec()));
    }

    #[cfg(windows)]
    {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}

async fn run_cli_async(config: config::ConfigHandle, cli: CliCommand) -> anyhow::Result<()> {
    let initial = true;
    let mut ui = mux::connui::ConnectionUI::new_headless();
    let client = Client::new_default_unix_domain(initial, &mut ui)?;
    match cli.sub {
        CliSubCommand::List => {
            let cols = vec![
                Column {
                    name: "WINID".to_string(),
                    alignment: Alignment::Right,
                },
                Column {
                    name: "TABID".to_string(),
                    alignment: Alignment::Right,
                },
                Column {
                    name: "PANEID".to_string(),
                    alignment: Alignment::Right,
                },
                Column {
                    name: "SIZE".to_string(),
                    alignment: Alignment::Left,
                },
                Column {
                    name: "TITLE".to_string(),
                    alignment: Alignment::Left,
                },
                Column {
                    name: "CWD".to_string(),
                    alignment: Alignment::Left,
                },
            ];
            let mut data = vec![];
            let panes = client.list_panes().await?;

            for tabroot in panes.tabs {
                let mut cursor = tabroot.into_tree().cursor();

                loop {
                    if let Some(entry) = cursor.leaf_mut() {
                        data.push(vec![
                            entry.window_id.to_string(),
                            entry.tab_id.to_string(),
                            entry.pane_id.to_string(),
                            format!("{}x{}", entry.size.cols, entry.size.rows),
                            entry.title.clone(),
                            entry
                                .working_dir
                                .as_ref()
                                .map(|url| url.url.as_str())
                                .unwrap_or("")
                                .to_string(),
                        ]);
                    }
                    match cursor.preorder_next() {
                        Ok(c) => cursor = c,
                        Err(_) => break,
                    }
                }
            }

            tabulate_output(&cols, &data, &mut std::io::stdout().lock())?;
        }
        CliSubCommand::SplitPane {
            pane_id,
            cwd,
            prog,
            horizontal,
        } => {
            let pane_id: PaneId = match pane_id {
                Some(p) => p,
                None => std::env::var("WEZTERM_PANE")
                    .map_err(|_| {
                        anyhow!(
                            "--pane-id was not specified and $WEZTERM_PANE
                                    is not set in the environment"
                        )
                    })?
                    .parse()?,
            };

            let spawned = client
                .split_pane(codec::SplitPane {
                    pane_id,
                    direction: if horizontal {
                        SplitDirection::Horizontal
                    } else {
                        SplitDirection::Vertical
                    },
                    domain: config::keyassignment::SpawnTabDomain::CurrentPaneDomain,
                    command: if prog.is_empty() {
                        None
                    } else {
                        let builder = CommandBuilder::from_argv(prog);
                        Some(builder)
                    },
                    command_dir: cwd.and_then(|c| c.to_str().map(|s| s.to_string())),
                })
                .await?;

            log::debug!("{:?}", spawned);
            println!("{}", spawned.pane_id);
        }
        CliSubCommand::Proxy => {
            // The client object we created above will have spawned
            // the server if needed, so now all we need to do is turn
            // ourselves into basically netcat.
            drop(client);

            let mux = Rc::new(mux::Mux::new(None));
            Mux::set_mux(&mux);
            let unix_dom = config.unix_domains.first().unwrap();
            let sock_path = unix_dom.socket_path();
            let stream = unix_connect_with_retry(&sock_path, false)?;

            // Keep the threads below alive forever; they'll
            // exit the process when they're done.
            let _activity = Activity::new();

            // Spawn a thread to pull data from the socket and write
            // it to stdout
            let duped = stream.try_clone()?;
            std::thread::spawn(move || {
                let stdout = std::io::stdout();
                consume_stream_then_exit_process(duped, stdout.lock());
            });

            // and pull data from stdin and write it to the socket
            std::thread::spawn(move || {
                let stdin = std::io::stdin();
                consume_stream_then_exit_process(stdin.lock(), stream);
            });
        }
        CliSubCommand::TlsCreds => {
            let creds = client.get_tls_creds().await?;
            codec::Pdu::GetTlsCredsResponse(creds).encode(std::io::stdout().lock(), 0)?;
        }
    }
    Ok(())
}

fn run_cli(config: config::ConfigHandle, cli: CliCommand) -> anyhow::Result<()> {
    let executor = promise::spawn::SimpleExecutor::new();
    promise::spawn::spawn(async move {
        match run_cli_async(config, cli).await {
            Ok(_) => std::process::exit(0),
            Err(err) => {
                terminate_with_error(err);
            }
        }
    })
    .detach();
    loop {
        executor.tick()?;
    }
}

fn consume_stream<F: Read, T: Write>(mut from_stream: F, mut to_stream: T) -> anyhow::Result<()> {
    let mut buf = [0u8; 8192];

    loop {
        let size = from_stream.read(&mut buf)?;
        if size == 0 {
            break;
        }
        to_stream.write_all(&buf[0..size])?;
        to_stream.flush()?;
    }
    Ok(())
}

fn consume_stream_then_exit_process<F: Read, T: Write>(from_stream: F, to_stream: T) -> ! {
    consume_stream(from_stream, to_stream).ok();
    std::thread::sleep(std::time::Duration::new(2, 0));
    std::process::exit(0);
}
