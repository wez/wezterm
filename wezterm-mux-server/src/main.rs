use anyhow::{anyhow, bail, Context, Error};
use config::{configuration, TlsDomainServer};
use crossbeam::channel::unbounded as channel;
use log::error;
use mux::activity::Activity;
use mux::domain::{Domain, LocalDomain};
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use promise::spawn::spawn_into_main_thread;
use promise::*;
use std::ffi::OsString;
use std::net::TcpListener;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use structopt::*;

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

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_timed();
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
            let stdout = config.daemon_options.open_stdout()?;
            let stderr = config.daemon_options.open_stderr()?;
            let mut daemonize = daemonize::Daemonize::new()
                .stdout(stdout)
                .stderr(stderr)
                .working_directory(config::HOME_DIR.clone());

            if !config::running_under_wsl() {
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

    let (tx, rx) = channel();

    let tx_main = tx.clone();
    let tx_low = tx.clone();
    let queue_func = move |f: SpawnFunc| {
        tx_main.send(f).ok();
    };
    let queue_func_low = move |f: SpawnFunc| {
        tx_low.send(f).ok();
    };
    promise::spawn::set_schedulers(
        Box::new(move |task| queue_func(Box::new(move || task.run()))),
        Box::new(move |task| queue_func_low(Box::new(move || task.run()))),
    );

    spawn_listener()?;

    let activity = Activity::new();

    promise::spawn::spawn(async move {
        if let Err(err) = async_run(cmd).await {
            terminate_with_error(err);
        }
        drop(activity);
    });

    loop {
        match rx.recv() {
            Ok(func) => func(),
            Err(err) => bail!("while waiting for events: {:?}", err),
        }

        if Mux::get().unwrap().is_empty() && mux::activity::Activity::count() == 0 {
            log::info!("No more tabs; all done!");
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

mod clientsession;
mod local;
mod ossl;
mod pki;
mod pollable;
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
