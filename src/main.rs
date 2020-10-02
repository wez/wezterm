// Don't create a new standard console window when launched from the windows GUI.
#![windows_subsystem = "windows"]

use crate::server::listener::umask;
use anyhow::{anyhow, bail};
use config::{wezterm_version, SshParameters};
use promise::spawn::block_on;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use structopt::StructOpt;
use tabout::{tabulate_output, Alignment, Column};

// This module defines a macro, so it must be referenced before any other mods
mod scripting;

mod connui;
mod frontend;
use config::keyassignment;
mod localtab;
mod markdown;
mod mux;
mod ratelim;
mod server;
mod ssh;
mod stats;
mod termwiztermtab;
mod update;

use crate::frontend::activity::Activity;
use crate::frontend::{front_end, FrontEndSelection};
use crate::mux::domain::{Domain, LocalDomain};
use crate::mux::pane::PaneId;
use crate::mux::tab::SplitDirection;
use crate::mux::Mux;
use crate::server::client::{unix_connect_with_retry, Client};
use crate::server::domain::{ClientDomain, ClientDomainConfig};
use portable_pty::cmdbuilder::CommandBuilder;

mod font;
use crate::font::locator::FontLocatorSelection;
use crate::font::rasterizer::FontRasterizerSelection;
use crate::font::shaper::FontShaperSelection;
use crate::font::FontConfiguration;

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

#[derive(Debug, StructOpt, Default, Clone)]
struct StartCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    front_end: Option<FrontEndSelection>,

    #[structopt(
        long = "font-locator",
        possible_values = &FontLocatorSelection::variants(),
        case_insensitive = true
    )]
    font_locator: Option<FontLocatorSelection>,

    #[structopt(
        long = "font-rasterizer",
        possible_values = &FontRasterizerSelection::variants(),
        case_insensitive = true
    )]
    font_rasterizer: Option<FontRasterizerSelection>,

    #[structopt(
        long = "font-shaper",
        possible_values = &FontShaperSelection::variants(),
        case_insensitive = true
    )]
    font_shaper: Option<FontShaperSelection>,

    /// If true, do not connect to domains marked as connect_automatically
    /// in your wezterm.toml configuration file.
    #[structopt(long = "no-auto-connect")]
    no_auto_connect: bool,

    /// Detach from the foreground and become a background process
    #[structopt(long = "daemonize")]
    daemonize: bool,

    /// Specify the current working directory for the initially
    /// spawned program
    #[structopt(long = "cwd", parse(from_os_str))]
    cwd: Option<OsString>,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm start -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
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

#[derive(Debug, StructOpt, Clone)]
struct SshCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    front_end: Option<FrontEndSelection>,

    /// Specifies the remote system using the form:
    /// `[username@]host[:port]`.
    /// If `username@` is omitted, then your local $USER is used
    /// instead.
    /// If `:port` is omitted, then the standard ssh port (22) is
    /// used instead.
    user_at_host_and_port: SshParameters,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm ssh user@host -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
}

#[derive(Debug, StructOpt, Clone)]
struct SerialCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    front_end: Option<FrontEndSelection>,

    /// Set the baud rate.  The default is 9600 baud.
    #[structopt(long = "baud")]
    baud: Option<usize>,

    /// Specifies the serial device name.
    /// On Windows systems this can be a name like `COM0`.
    /// On posix systems this will be something like `/dev/ttyUSB0`
    #[structopt(parse(from_os_str))]
    port: OsString,
}

#[derive(Debug, StructOpt, Clone)]
struct ConnectCommand {
    #[structopt(
        long = "front-end",
        possible_values = &FrontEndSelection::variants(),
        case_insensitive = true
    )]
    front_end: Option<FrontEndSelection>,

    /// Name of the multiplexer domain section from the configuration
    /// to which you'd like to connect
    domain_name: String,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm start -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
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
    /// The name of the image file to be displayed
    #[structopt(parse(from_os_str))]
    file_name: OsString,
}

impl ImgCatCommand {
    fn run(&self) -> anyhow::Result<()> {
        let mut f = std::fs::File::open(&self.file_name)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;

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

pub fn running_under_wsl() -> bool {
    #[cfg(unix)]
    unsafe {
        let mut name: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut name) == 0 {
            let version = std::ffi::CStr::from_ptr(name.version.as_ptr())
                .to_string_lossy()
                .into_owned();
            return version.contains("Microsoft");
        }
    };

    false
}

async fn async_run_ssh(opts: SshCommand) -> anyhow::Result<()> {
    // Establish the connection; it may show UI for authentication
    let params = &opts.user_at_host_and_port;
    let sess = ssh::async_ssh_connect(&params.host_and_port, &params.username).await?;
    // Now we have a connected session, set up the ssh domain and make it
    // the default domain
    let gui = front_end().unwrap();

    let cmd = if !opts.prog.is_empty() {
        let builder = CommandBuilder::from_argv(opts.prog);
        Some(builder)
    } else {
        None
    };

    let config = config::configuration();
    let pty_system = Box::new(portable_pty::ssh::SshSession::new(sess, &config.term));
    let domain: Arc<dyn Domain> = Arc::new(ssh::RemoteSshDomain::with_pty_system(
        &opts.user_at_host_and_port.to_string(),
        pty_system,
    ));

    let mux = Mux::get().unwrap();
    mux.add_domain(&domain);
    mux.set_default_domain(&domain);
    domain.attach().await?;

    let window_id = mux.new_empty_window();
    let tab = domain
        .spawn(config.initial_size(), cmd, None, window_id)
        .await?;
    let fontconfig = Rc::new(FontConfiguration::new());
    gui.spawn_new_window(&fontconfig, &tab, window_id)?;

    Ok(())
}

fn run_ssh(config: config::ConfigHandle, opts: SshCommand) -> anyhow::Result<()> {
    let front_end_selection = opts.front_end.unwrap_or(config.front_end);
    let gui = crate::frontend::try_new(front_end_selection)?;

    // Set up the mux with no default domain; there's a good chance that
    // we'll need to show authentication UI and we don't want its domain
    // to become the default domain.
    let mux = Rc::new(mux::Mux::new(None));
    Mux::set_mux(&mux);

    // Keep the frontend alive until we've run through the ssh authentication
    // phase.  This is passed into the thread and dropped when it is done.
    let activity = Activity::new();

    // Initiate an ssh connection; since that is a blocking process with
    // callbacks, we have to run it in another thread
    promise::spawn::spawn(async {
        if let Err(err) = async_run_ssh(opts).await {
            terminate_with_error(err);
        }
        // This captures the activity ownership into this future, but also
        // ensures that we drop it either when we error out, or if not,
        // only once we reach this point in the processing flow.
        drop(activity);
    });

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn run_serial(config: config::ConfigHandle, opts: &SerialCommand) -> anyhow::Result<()> {
    let fontconfig = Rc::new(FontConfiguration::new());

    let mut serial = portable_pty::serial::SerialTty::new(&opts.port);
    if let Some(baud) = opts.baud {
        serial.set_baud_rate(serial::BaudRate::from_speed(baud));
    }

    let pty_system = Box::new(serial);
    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::with_pty_system("local", pty_system));
    let mux = Rc::new(mux::Mux::new(Some(domain.clone())));
    Mux::set_mux(&mux);

    let front_end = opts.front_end.unwrap_or(config.front_end);
    let gui = crate::frontend::try_new(front_end)?;
    block_on(domain.attach())?; // FIXME: blocking

    let window_id = mux.new_empty_window();
    let tab = block_on(domain.spawn(config.initial_size(), None, None, window_id))?; // FIXME: blocking
    gui.spawn_new_window(&fontconfig, &tab, window_id)?;

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn client_domains(config: &config::ConfigHandle) -> Vec<ClientDomainConfig> {
    let mut domains = vec![];
    for unix_dom in &config.unix_domains {
        domains.push(ClientDomainConfig::Unix(unix_dom.clone()));
    }

    for ssh_dom in &config.ssh_domains {
        domains.push(ClientDomainConfig::Ssh(ssh_dom.clone()));
    }

    for tls_client in &config.tls_clients {
        domains.push(ClientDomainConfig::Tls(tls_client.clone()));
    }
    domains
}

fn run_mux_client(config: config::ConfigHandle, opts: &ConnectCommand) -> anyhow::Result<()> {
    let client_config = client_domains(&config)
        .into_iter()
        .find(|c| c.name() == opts.domain_name)
        .ok_or_else(|| {
            anyhow!(
                "no multiplexer domain with name `{}` was found in the configuration",
                opts.domain_name
            )
        })?;

    let domain: Arc<dyn Domain> = Arc::new(ClientDomain::new(client_config));
    let mux = Rc::new(mux::Mux::new(Some(domain.clone())));
    Mux::set_mux(&mux);

    let front_end_selection = opts.front_end.unwrap_or(config.front_end);
    let gui = crate::frontend::try_new(front_end_selection)?;
    let opts = opts.clone();

    let cmd = if !opts.prog.is_empty() {
        let builder = CommandBuilder::from_argv(opts.prog);
        Some(builder)
    } else {
        None
    };

    let activity = Activity::new();
    promise::spawn::spawn(async {
        if let Err(err) = spawn_tab_in_default_domain_if_mux_is_empty(cmd).await {
            terminate_with_error(err);
        }
        drop(activity);
    });

    gui.run_forever()
}

async fn spawn_tab_in_default_domain_if_mux_is_empty(
    cmd: Option<CommandBuilder>,
) -> anyhow::Result<()> {
    let mux = Mux::get().unwrap();

    if !mux.is_empty() {
        return Ok(());
    }
    let domain = mux.default_domain();
    domain.attach().await?;

    if !mux.is_empty() {
        return Ok(());
    }

    let config = config::configuration();
    let window_id = mux.new_empty_window();
    let tab = mux
        .default_domain()
        .spawn(config.initial_size(), cmd, None, window_id)
        .await?;
    let fontconfig = Rc::new(FontConfiguration::new());
    front_end()
        .unwrap()
        .spawn_new_window(&fontconfig, &tab, window_id)?;
    Ok(())
}

async fn async_run_terminal_gui(
    cmd: Option<CommandBuilder>,
    do_auto_connect: bool,
) -> anyhow::Result<()> {
    let mux = Mux::get().unwrap();

    fn record_domain(mux: &Rc<Mux>, client: ClientDomain) -> anyhow::Result<Arc<dyn Domain>> {
        let domain: Arc<dyn Domain> = Arc::new(client);
        mux.add_domain(&domain);
        Ok(domain)
    }

    if do_auto_connect {
        let config = config::configuration();
        for client_config in client_domains(&config) {
            let connect_automatically = client_config.connect_automatically();
            let dom = record_domain(&mux, ClientDomain::new(client_config))?;
            if connect_automatically {
                dom.attach().await?;
            }
        }
    }

    spawn_tab_in_default_domain_if_mux_is_empty(cmd).await
}

fn run_terminal_gui(config: config::ConfigHandle, opts: StartCommand) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        if opts.daemonize {
            let stdout = config.daemon_options.open_stdout()?;
            let stderr = config.daemon_options.open_stderr()?;
            let mut daemonize = daemonize::Daemonize::new()
                .stdout(stdout)
                .stderr(stderr)
                .working_directory(config::HOME_DIR.clone());

            if !running_under_wsl() {
                // pid file locking is only partly function when running under
                // WSL 1; it is possible for the pid file to exist after a reboot
                // and for attempts to open and lock it to fail when there are no
                // other processes that might possibly hold a lock on it.
                // So, we only use a pid file when not under WSL.
                daemonize = daemonize.pid_file(config.daemon_options.pid_file());
            }
            if let Err(err) = daemonize.start() {
                use daemonize::DaemonizeError;
                match err {
                    DaemonizeError::OpenPidfile
                    | DaemonizeError::LockPidfile(_)
                    | DaemonizeError::ChownPidfile(_)
                    | DaemonizeError::WritePid => {
                        bail!("{} {}", err, config.daemon_options.pid_file().display());
                    }
                    DaemonizeError::ChangeDirectory => {
                        bail!("{} {}", err, config::HOME_DIR.display());
                    }
                    _ => return Err(err.into()),
                }
            }

            // Remove some environment variables that aren't super helpful or
            // that are potentially misleading when we're starting up the
            // server.
            // We may potentially want to look into starting/registering
            // a session of some kind here as well in the future.
            for name in &[
                "OLDPWD",
                "PWD",
                "SHLVL",
                "SSH_AUTH_SOCK",
                "SSH_CLIENT",
                "SSH_CONNECTION",
                "_",
            ] {
                std::env::remove_var(name);
            }
        }
    }

    opts.font_locator
        .unwrap_or(config.font_locator)
        .set_default();
    opts.font_shaper.unwrap_or(config.font_shaper).set_default();
    opts.font_rasterizer
        .unwrap_or(config.font_rasterizer)
        .set_default();

    let need_builder = !opts.prog.is_empty() || opts.cwd.is_some();

    let cmd = if need_builder {
        let mut builder = if opts.prog.is_empty() {
            CommandBuilder::new_default_prog()
        } else {
            CommandBuilder::from_argv(opts.prog)
        };
        if let Some(cwd) = opts.cwd {
            builder.cwd(cwd);
        }
        Some(builder)
    } else {
        None
    };

    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local")?);
    let mux = Rc::new(mux::Mux::new(Some(domain.clone())));
    Mux::set_mux(&mux);

    let front_end_selection = opts.front_end.unwrap_or(config.front_end);
    let gui = crate::frontend::try_new(front_end_selection)?;
    let activity = Activity::new();
    let do_auto_connect =
        front_end_selection != FrontEndSelection::MuxServer && !opts.no_auto_connect;

    promise::spawn::spawn(async move {
        if let Err(err) = async_run_terminal_gui(cmd, do_auto_connect).await {
            terminate_with_error(err);
        }
        drop(activity);
    });

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn toast_notification(title: &str, message: &str) {
    #[cfg(not(windows))]
    {
        notify_rust::Notification::new()
            .summary(title)
            .body(message)
            // Stay on the screen until dismissed
            .hint(notify_rust::NotificationHint::Resident(true))
            // timeout isn't respected on macos
            .timeout(0)
            .show()
            .ok();
    }

    #[cfg(windows)]
    {
        let title = title.to_owned();
        let message = message.to_owned();

        // We need to be in a different thread from the caller
        // in case we get called in the guts of a windows message
        // loop dispatch and are unable to pump messages
        std::thread::spawn(move || {
            use winrt_notification::Toast;

            Toast::new(Toast::POWERSHELL_APP_ID)
                .title(&title)
                .text1(&message)
                .duration(winrt_notification::Duration::Long)
                .show()
                .ok();
        });
    }
}

fn fatal_toast_notification(title: &str, message: &str) {
    toast_notification(title, message);
    // We need a short delay otherwise the notification
    // will not show
    #[cfg(windows)]
    std::thread::sleep(std::time::Duration::new(2, 0));
}

fn notify_on_panic() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(s) = info.payload().downcast_ref::<&str>() {
            fatal_toast_notification("Wezterm panic", s);
        }
        default_hook(info);
    }));
}

fn terminate_with_error_message(err: &str) -> ! {
    log::error!("{}; terminating", err);
    fatal_toast_notification("Wezterm Error", &err);
    std::process::exit(1);
}

fn terminate_with_error(err: anyhow::Error) -> ! {
    terminate_with_error_message(&format!("{:#}", err));
}

fn main() {
    config::assign_lua_factory(scripting::make_lua_context);
    config::assign_error_callback(crate::connui::show_configuration_error_message);
    notify_on_panic();
    if let Err(e) = run() {
        terminate_with_error(e);
    }
    Mux::shutdown();
    frontend::shutdown();
}

fn maybe_show_configuration_error_window() {
    if let Err(err) = config::configuration_result() {
        let err = format!("{:#}", err);
        connui::show_configuration_error_message(&err);
    }
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

    // This is a bit gross.
    // In order to not to automatically open a standard windows console when
    // we run, we use the windows_subsystem attribute at the top of this
    // source file.  That comes at the cost of causing the help output
    // to disappear if we are actually invoked from a console.
    // This AttachConsole call will attach us to the console of the parent
    // in that situation, but since we were launched as a windows subsystem
    // application we will be running asynchronously from the shell in
    // the command window, which means that it will appear to the user
    // that we hung at the end, when in reality the shell is waiting for
    // input but didn't know to re-draw the prompt.
    #[cfg(windows)]
    unsafe {
        if winapi::um::wincon::AttachConsole(winapi::um::wincon::ATTACH_PARENT_PROCESS) == 0 {
            /*
            // If we failed to attach the console then we're running in
            // a gui only context.  To aid in troubleshooting, let's redirect
            // the stdio streams to a log file
            let stdout = config.daemon_options.open_stdout()?;
            let stderr = config.daemon_options.open_stderr()?;
            use filedescriptor::IntoRawFileDescriptor;
            use winapi::um::processenv::SetStdHandle;
            use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
            SetStdHandle(STD_OUTPUT_HANDLE, stdout.into_raw_file_descriptor());
            SetStdHandle(STD_ERROR_HANDLE, stderr.into_raw_file_descriptor());
            */

            std::env::set_current_dir(config::HOME_DIR.as_path())?;
        }
    };
    pretty_env_logger::init_timed();
    stats::Stats::init()?;
    let _saver = umask::UmaskSaver::new();

    let opts = Opt::from_args();
    if !opts.skip_config {
        config::reload();
    }
    let config = config::configuration();

    #[cfg(target_os = "macos")]
    {
        window::os::macos::use_ime(config.use_ime);
    }

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(start) => {
            log::info!("Using configuration: {:#?}\nopts: {:#?}", config, opts);
            run_terminal_gui(config, start)
        }
        SubCommand::Ssh(ssh) => run_ssh(config, ssh),
        SubCommand::Serial(serial) => run_serial(config, &serial),
        SubCommand::Connect(connect) => run_mux_client(config, &connect),
        SubCommand::ImageCat(cmd) => cmd.run(),
        SubCommand::Cli(cli) => {
            // Start a front end so that the futures executor is running
            let front_end = crate::frontend::try_new(FrontEndSelection::Null)?;

            let initial = true;
            let mut ui = crate::connui::ConnectionUI::new_headless();
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
                    let panes = block_on(client.list_panes())?; // FIXME: blocking

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

                    let spawned = block_on(client.split_pane(crate::server::codec::SplitPane {
                        pane_id,
                        direction: if horizontal {
                            SplitDirection::Horizontal
                        } else {
                            SplitDirection::Vertical
                        },
                        domain: keyassignment::SpawnTabDomain::CurrentPaneDomain,
                        command: if prog.is_empty() {
                            None
                        } else {
                            let builder = CommandBuilder::from_argv(prog);
                            Some(builder)
                        },
                        command_dir: cwd.and_then(|c| c.to_str().map(|s| s.to_string())),
                    }))?;

                    log::debug!("{:?}", spawned);
                    println!("{}", spawned.pane_id);
                }
                CliSubCommand::Proxy => {
                    // The client object we created above will have spawned
                    // the server if needed, so now all we need to do is turn
                    // ourselves into basically netcat.
                    drop(client);

                    crate::stats::disable_stats_printing();

                    let mux = Rc::new(mux::Mux::new(None));
                    Mux::set_mux(&mux);
                    let unix_dom = config.unix_domains.first().unwrap();
                    let sock_path = unix_dom.socket_path();
                    let stream = unix_connect_with_retry(&sock_path)?;

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
                    front_end.run_forever()?;
                }
                CliSubCommand::TlsCreds => {
                    let creds = block_on(client.get_tls_creds())?;
                    crate::server::codec::Pdu::GetTlsCredsResponse(creds)
                        .encode(std::io::stdout().lock(), 0)?;
                }
            }
            Ok(())
        }
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
