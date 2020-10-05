use config::configuration;
use mux::activity::Activity;
use mux::domain::{Domain, LocalDomain};
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use std::ffi::OsString;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use structopt::*;

mod daemonize;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

#[derive(Debug, StructOpt)]
#[structopt(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    global_setting = structopt::clap::AppSettings::ColoredHelp,
    version = config::wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[structopt(short = "n")]
    skip_config: bool,

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

fn main() {
    pretty_env_logger::init_timed();
    if let Err(err) = run() {
        eprintln!("boo {}", err);
        log::error!("{}", err);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    //stats::Stats::init()?;
    let _saver = umask::UmaskSaver::new();

    let opts = Opt::from_args();
    if !opts.skip_config {
        config::reload();
    }
    let config = config::configuration();

    #[cfg(unix)]
    {
        if opts.daemonize {
            daemonize::daemonize(&config)?;
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

    let executor = promise::spawn::SimpleExecutor::new();

    spawn_listener().map_err(|e| {
        log::error!("problem spawning listeners: {:?}", e);
        e
    })?;

    let activity = Activity::new();

    promise::spawn::spawn(async move {
        if let Err(err) = async_run(cmd).await {
            terminate_with_error(err);
        }
        drop(activity);
    })
    .detach();

    loop {
        executor.tick()?;

        if Mux::get().unwrap().is_empty() && mux::activity::Activity::count() == 0 {
            log::error!("No more tabs; all done!");
            return Ok(());
        }
    }
}

async fn async_run(cmd: Option<CommandBuilder>) -> anyhow::Result<()> {
    let mux = Mux::get().unwrap();

    let domain = mux.default_domain();
    domain.attach().await?;

    let config = config::configuration();
    let window_id = mux.new_empty_window();
    let _tab = mux
        .default_domain()
        .spawn(config.initial_size(), cmd, None, *window_id)
        .await?;
    Ok(())
}

fn terminate_with_error(err: anyhow::Error) -> ! {
    log::error!("{:#}; terminating", err);
    std::process::exit(1);
}

mod dispatch;
mod local;
mod ossl;
mod pki;
mod sessionhandler;

lazy_static::lazy_static! {
    static ref PKI: pki::Pki = pki::Pki::init().expect("failed to initialize PKI");
}

pub fn spawn_listener() -> anyhow::Result<()> {
    let config = configuration();
    for unix_dom in &config.unix_domains {
        let mut listener = local::LocalListener::with_domain(unix_dom)?;
        thread::spawn(move || {
            listener.run();
        });
    }

    for tls_server in &config.tls_servers {
        ossl::spawn_tls_listener(tls_server)?;
    }

    Ok(())
}
