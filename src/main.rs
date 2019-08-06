// Don't create a new standard console window when launched from the windows GUI.
#![windows_subsystem = "windows"]

use failure::{err_msg, format_err, Error, Fallible};
use std::ffi::OsString;
use std::fs::DirBuilder;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;
use structopt::StructOpt;
use tabout::{tabulate_output, Alignment, Column};

use std::rc::Rc;
use std::sync::Arc;

mod config;
mod frontend;
mod mux;
mod opengl;
mod ratelim;
mod server;
mod ssh;

use crate::frontend::FrontEndSelection;
use crate::mux::domain::{Domain, LocalDomain};
use crate::mux::Mux;
use crate::server::client::{unix_connect_with_retry, Client};
use crate::server::domain::{ClientDomain, ClientDomainConfig};
use portable_pty::cmdbuilder::CommandBuilder;
use portable_pty::PtySize;

mod font;
use crate::font::{FontConfiguration, FontSystemSelection};

//    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
//    terminal.advance_bytes(message);
// !=

#[derive(Debug, StructOpt)]
#[structopt(about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm")]
#[structopt(raw(
    global_setting = "structopt::clap::AppSettings::ColoredHelp",
    version = r#"env!("VERGEN_SEMVER_LIGHTWEIGHT")"#,
))]
struct Opt {
    /// Skip loading ~/.wezterm.toml
    #[structopt(short = "n")]
    skip_config: bool,

    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, StructOpt, Default, Clone)]
struct StartCommand {
    #[structopt(
        long = "front-end",
        raw(
            possible_values = "&FrontEndSelection::variants()",
            case_insensitive = "true"
        )
    )]
    front_end: Option<FrontEndSelection>,

    #[structopt(
        long = "font-system",
        raw(
            possible_values = "&FontSystemSelection::variants()",
            case_insensitive = "true"
        )
    )]
    font_system: Option<FontSystemSelection>,

    /// If true, do not connect to domains marked as connect_automatically
    /// in your wezterm.toml configuration file.
    #[structopt(long = "no-auto-connect")]
    no_auto_connect: bool,

    /// Detach from the foreground and become a background process
    #[structopt(long = "daemonize")]
    daemonize: bool,

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

    #[structopt(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[structopt(name = "cli", about = "Interact with experimental mux server")]
    Cli(CliCommand),
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
    #[structopt(name = "list", about = "list windows and tabs")]
    List,

    #[structopt(name = "proxy", about = "start rpc proxy pipe")]
    Proxy,
}

#[derive(Debug, StructOpt, Clone)]
struct SshCommand {
    #[structopt(
        long = "front-end",
        raw(
            possible_values = "&FrontEndSelection::variants()",
            case_insensitive = "true"
        )
    )]
    front_end: Option<FrontEndSelection>,

    #[structopt(
        long = "font-system",
        raw(
            possible_values = "&FontSystemSelection::variants()",
            case_insensitive = "true"
        )
    )]
    font_system: Option<FontSystemSelection>,

    /// Specifies the remote system using the form:
    /// `[username@]host[:port]`.
    /// If `username@` is omitted, then your local $USER is used
    /// instead.
    /// If `:port` is omitted, then the standard ssh port (22) is
    /// used instead.
    user_at_host_and_port: String,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm ssh user@host -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
}

#[derive(Debug, StructOpt, Clone)]
struct ConnectCommand {
    #[structopt(
        long = "front-end",
        raw(
            possible_values = "&FrontEndSelection::variants()",
            case_insensitive = "true"
        )
    )]
    front_end: Option<FrontEndSelection>,

    #[structopt(
        long = "font-system",
        raw(
            possible_values = "&FontSystemSelection::variants()",
            case_insensitive = "true"
        )
    )]
    font_system: Option<FontSystemSelection>,

    /// Name of the multiplexer domain section from the configuration
    /// to which you'd like to connect
    domain_name: String,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm start -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[structopt(parse(from_os_str))]
    prog: Vec<OsString>,
}

pub fn create_user_owned_dirs(p: &Path) -> Fallible<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    {
        builder.mode(0o700);
    }

    builder.create(p)?;
    Ok(())
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

struct SshParameters {
    username: String,
    host_and_port: String,
}

fn username_from_env() -> Fallible<String> {
    #[cfg(unix)]
    const USER: &str = "USER";
    #[cfg(windows)]
    const USER: &str = "USERNAME";

    std::env::var(USER).map_err(|e| format_err!("while resolving {} env var: {}", USER, e))
}

impl SshParameters {
    fn parse(host: &str) -> Fallible<Self> {
        let parts: Vec<&str> = host.split('@').collect();

        if parts.len() == 2 {
            Ok(Self {
                username: parts[0].to_string(),
                host_and_port: parts[1].to_string(),
            })
        } else if parts.len() == 1 {
            Ok(Self {
                username: username_from_env()?,
                host_and_port: parts[0].to_string(),
            })
        } else {
            failure::bail!("failed to parse ssh parameters from `{}`", host);
        }
    }
}

fn run_ssh(config: Arc<config::Config>, opts: &SshCommand) -> Fallible<()> {
    let font_system = opts.font_system.unwrap_or(config.font_system);
    font_system.set_default();

    let fontconfig = Rc::new(FontConfiguration::new(Arc::clone(&config), font_system));
    let cmd = if !opts.prog.is_empty() {
        let argv: Vec<&std::ffi::OsStr> = opts.prog.iter().map(|x| x.as_os_str()).collect();
        let mut builder = CommandBuilder::new(&argv[0]);
        builder.args(&argv[1..]);
        Some(builder)
    } else {
        None
    };

    let params = SshParameters::parse(&opts.user_at_host_and_port)?;

    let sess = ssh::ssh_connect(&params.host_and_port, &params.username)?;
    let pty_system = Box::new(portable_pty::ssh::SshSession::new(sess, &config.term));
    let domain: Arc<dyn Domain> = Arc::new(ssh::RemoteSshDomain::with_pty_system(
        &opts.user_at_host_and_port,
        &config,
        pty_system,
    ));

    let mux = Rc::new(mux::Mux::new(&config, &domain));
    Mux::set_mux(&mux);

    let front_end = opts.front_end.unwrap_or(config.front_end);
    let gui = front_end.try_new(&mux)?;
    domain.attach()?;

    let window_id = mux.new_empty_window();
    let tab = domain.spawn(PtySize::default(), cmd, window_id)?;
    gui.spawn_new_window(mux.config(), &fontconfig, &tab, window_id)?;

    gui.run_forever()
}

fn client_domains(config: &Arc<config::Config>) -> Vec<ClientDomainConfig> {
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

fn run_mux_client(config: Arc<config::Config>, opts: &ConnectCommand) -> Fallible<()> {
    let client_config = client_domains(&config)
        .into_iter()
        .find(|c| c.name() == opts.domain_name)
        .ok_or_else(|| {
            format_err!(
                "no multiplexer domain with name `{}` was found in the configuration",
                opts.domain_name
            )
        })?;

    let font_system = opts.font_system.unwrap_or(config.font_system);
    font_system.set_default();

    let fontconfig = Rc::new(FontConfiguration::new(Arc::clone(&config), font_system));

    let domain: Arc<dyn Domain> = Arc::new(ClientDomain::new(client_config));
    let mux = Rc::new(mux::Mux::new(&config, &domain));
    Mux::set_mux(&mux);

    let front_end = opts.front_end.unwrap_or(config.front_end);
    let gui = front_end.try_new(&mux)?;
    domain.attach()?;

    if mux.is_empty() {
        let cmd = if !opts.prog.is_empty() {
            let argv: Vec<&std::ffi::OsStr> = opts.prog.iter().map(|x| x.as_os_str()).collect();
            let mut builder = CommandBuilder::new(&argv[0]);
            builder.args(&argv[1..]);
            Some(builder)
        } else {
            None
        };
        let window_id = mux.new_empty_window();
        let tab = mux
            .default_domain()
            .spawn(PtySize::default(), cmd, window_id)?;
        gui.spawn_new_window(mux.config(), &fontconfig, &tab, window_id)?;
    }

    for dom in mux.iter_domains() {
        log::error!("domain {} state {:?}", dom.domain_id(), dom.state());
    }

    gui.run_forever()
}

fn run_terminal_gui(config: Arc<config::Config>, opts: &StartCommand) -> Fallible<()> {
    #[cfg(unix)]
    {
        if opts.daemonize {
            let stdout = config.daemon_options.open_stdout()?;
            let stderr = config.daemon_options.open_stderr()?;
            let mut daemonize = daemonize::Daemonize::new()
                .stdout(stdout)
                .stderr(stderr)
                .working_directory(dirs::home_dir().ok_or_else(|| err_msg("can't find home dir"))?);

            if !running_under_wsl() {
                // pid file locking is only partly function when running under
                // WSL 1; it is possible for the pid file to exist after a reboot
                // and for attempts to open and lock it to fail when there are no
                // other processes that might possibly hold a lock on it.
                // So, we only use a pid file when not under WSL.
                daemonize = daemonize.pid_file(config.daemon_options.pid_file());
            }
            daemonize.start()?;
        }
    }

    let font_system = opts.font_system.unwrap_or(config.font_system);
    font_system.set_default();

    let fontconfig = Rc::new(FontConfiguration::new(Arc::clone(&config), font_system));

    let cmd = if !opts.prog.is_empty() {
        let argv: Vec<&std::ffi::OsStr> = opts.prog.iter().map(|x| x.as_os_str()).collect();
        let mut builder = CommandBuilder::new(&argv[0]);
        builder.args(&argv[1..]);
        Some(builder)
    } else {
        None
    };

    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local", &config)?);
    let mux = Rc::new(mux::Mux::new(&config, &domain));
    Mux::set_mux(&mux);

    let front_end = opts.front_end.unwrap_or(config.front_end);
    let gui = front_end.try_new(&mux)?;
    domain.attach()?;

    fn record_domain(mux: &Rc<Mux>, client: ClientDomain) -> Fallible<Arc<dyn Domain>> {
        let domain: Arc<dyn Domain> = Arc::new(client);
        mux.add_domain(&domain);
        Ok(domain)
    }

    if front_end != FrontEndSelection::MuxServer && !opts.no_auto_connect {
        for client_config in client_domains(&config) {
            let connect_automatically = client_config.connect_automatically();
            let dom = record_domain(&mux, ClientDomain::new(client_config))?;
            if connect_automatically {
                dom.attach()?;
            }
        }
    }

    if mux.is_empty() {
        let window_id = mux.new_empty_window();
        let tab = mux
            .default_domain()
            .spawn(PtySize::default(), cmd, window_id)?;
        gui.spawn_new_window(mux.config(), &fontconfig, &tab, window_id)?;
    }

    for dom in mux.iter_domains() {
        log::error!("domain {} state {:?}", dom.domain_id(), dom.state());
    }

    gui.run_forever()
}

fn main() -> Result<(), Error> {
    let result = run();
    if let Err(e) = &result {
        let message = e.to_string();

        tinyfiledialogs::message_box_ok(
            "wezterm",
            &message,
            tinyfiledialogs::MessageBoxIcon::Error,
        );
        log::error!("{}", message);
    }
    result
}

fn run() -> Result<(), Error> {
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

            std::env::set_current_dir(
                dirs::home_dir().ok_or_else(|| err_msg("can't find home dir"))?,
            )?;
        }
    };
    pretty_env_logger::init();

    let opts = Opt::from_args();
    let config = Arc::new(if opts.skip_config {
        config::Config::default_config()
    } else {
        config::Config::load()?
    });

    match opts
        .cmd
        .as_ref()
        .cloned()
        .unwrap_or_else(|| SubCommand::Start(StartCommand::default()))
    {
        SubCommand::Start(start) => {
            log::info!("Using configuration: {:#?}\nopts: {:#?}", config, opts);
            run_terminal_gui(config, &start)
        }
        SubCommand::Ssh(ssh) => run_ssh(config, &ssh),
        SubCommand::Connect(connect) => run_mux_client(config, &connect),
        SubCommand::Cli(cli) => {
            let initial = true;
            let client = Client::new_default_unix_domain(&config, initial)?;
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
                            name: "SIZE".to_string(),
                            alignment: Alignment::Left,
                        },
                        Column {
                            name: "TITLE".to_string(),
                            alignment: Alignment::Left,
                        },
                    ];
                    let mut data = vec![];
                    let tabs = client.list_tabs().wait()?;
                    for entry in tabs.tabs.iter() {
                        data.push(vec![
                            entry.window_id.to_string(),
                            entry.tab_id.to_string(),
                            format!("{}x{}", entry.size.cols, entry.size.rows),
                            entry.title.clone(),
                        ]);
                    }
                    tabulate_output(&cols, &data, &mut std::io::stdout().lock())?;
                }
                CliSubCommand::Proxy => {
                    // The client object we created above will have spawned
                    // the server if needed, so now all we need to do is turn
                    // ourselves into basically netcat.
                    drop(client);

                    let unix_dom = config.unix_domains.first().unwrap();
                    let sock_path = unix_dom.socket_path();
                    let stream = unix_connect_with_retry(&sock_path)?;

                    // Spawn a thread to pull data from the socket and write
                    // it to stdout
                    let duped = stream.try_clone()?;
                    std::thread::spawn(move || {
                        let stdout = std::io::stdout();
                        consume_stream(duped, stdout.lock()).ok();
                    });

                    // and pull data from stdin and write it to the socket
                    let stdin = std::io::stdin();
                    consume_stream(stdin.lock(), stream)?;
                }
            }
            Ok(())
        }
    }
}

fn consume_stream<F: Read, T: Write>(mut from_stream: F, mut to_stream: T) -> Fallible<()> {
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
