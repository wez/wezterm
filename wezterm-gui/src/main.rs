// Don't create a new standard console window when launched from the windows GUI.
#![cfg_attr(not(test), windows_subsystem = "windows")]

use crate::frontend::front_end;
use ::window::*;
use anyhow::{anyhow, Context};
use config::{ConfigHandle, SshBackend};
use mux::activity::Activity;
use mux::domain::{Domain, LocalDomain};
use mux::Mux;
use portable_pty::cmdbuilder::CommandBuilder;
use promise::spawn::block_on;
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use structopt::StructOpt;
use termwiz::cell::CellAttributes;
use termwiz::surface::{Line, SEQ_ZERO};
use wezterm_client::domain::{ClientDomain, ClientDomainConfig};
use wezterm_gui_subcommands::*;
use wezterm_ssh::*;
use wezterm_toast_notification::*;

mod cache;
mod customglyph;
mod download;
mod frontend;
mod glyphcache;
mod markdown;
mod overlay;
mod quad;
mod renderstate;
mod scripting;
mod scrollbar;
mod selection;
mod shapecache;
mod stats;
mod tabbar;
mod termwindow;
mod update;
mod utilsprites;

pub use selection::SelectionMode;
pub use termwindow::set_window_class;
pub use termwindow::TermWindow;
pub use termwindow::ICON_DATA;

#[derive(Debug, StructOpt)]
#[structopt(
    about = "Wez's Terminal Emulator\nhttp://github.com/wez/wezterm",
    global_setting = structopt::clap::AppSettings::ColoredHelp,
    version = config::wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[structopt(name = "skip-config", short = "n")]
    skip_config: bool,

    /// Specify the configuration file to use, overrides the normal
    /// configuration file resolution
    #[structopt(
        long = "config-file",
        parse(from_os_str),
        conflicts_with = "skip-config"
    )]
    config_file: Option<OsString>,

    /// Override specific configuration values
    #[structopt(
        long = "config",
        name = "name=value",
        parse(try_from_str = name_equals_value),
        number_of_values = 1)]
    config_override: Vec<(String, String)>,

    /// On Windows, whether to attempt to attach to the parent
    /// process console to display logging output
    #[structopt(long = "attach-parent-console")]
    #[allow(dead_code)]
    attach_parent_console: bool,

    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, StructOpt, Clone)]
enum SubCommand {
    #[structopt(
        name = "start",
        about = "Start the GUI, optionally running an alternative program"
    )]
    Start(StartCommand),

    #[structopt(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[structopt(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[structopt(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[structopt(name = "ls-fonts", about = "Display information about fonts")]
    LsFonts(LsFontsCommand),
}

async fn async_run_ssh(opts: SshCommand) -> anyhow::Result<()> {
    let config = config::configuration();

    let mut ssh_config = Config::new();
    ssh_config.add_default_config_files();

    let mut fields = opts.user_at_host_and_port.host_and_port.split(':');
    let host = fields
        .next()
        .ok_or_else(|| anyhow::anyhow!("no host component somehow"))?;
    let port = fields.next();

    let mut ssh_config = ssh_config.for_host(host);
    ssh_config.insert(
        "wezterm_ssh_backend".to_string(),
        match config.ssh_backend {
            SshBackend::Ssh2 => "ssh2",
            SshBackend::LibSsh => "libssh",
        }
        .to_string(),
    );
    if opts.verbose {
        ssh_config.insert("wezterm_ssh_verbose".to_string(), "true".to_string());
    }

    if let Some(username) = &opts.user_at_host_and_port.username {
        ssh_config.insert("user".to_string(), username.to_string());
    }
    if let Some(port) = port {
        ssh_config.insert("port".to_string(), port.to_string());
    }
    for (k, v) in opts.config_override {
        ssh_config.insert(k.to_lowercase().to_string(), v);
    }

    let _gui = front_end().unwrap();

    let cmd = if !opts.prog.is_empty() {
        let builder = CommandBuilder::from_argv(opts.prog);
        Some(builder)
    } else {
        None
    };

    let domain: Arc<dyn Domain> = Arc::new(mux::ssh::RemoteSshDomain::with_ssh_config(
        &opts.user_at_host_and_port.to_string(),
        ssh_config,
    )?);

    let mux = Mux::get().unwrap();
    mux.add_domain(&domain);
    mux.set_default_domain(&domain);
    domain.attach().await?;

    // Allow spawning local commands into new tabs/panes
    let local_domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local")?);
    mux.add_domain(&local_domain);

    let window_id = mux.new_empty_window();
    let _tab = domain
        .spawn(config.initial_size(), cmd, None, *window_id)
        .await?;

    Ok(())
}

fn run_ssh(opts: SshCommand) -> anyhow::Result<()> {
    // Set up the mux with no default domain; there's a good chance that
    // we'll need to show authentication UI and we don't want its domain
    // to become the default domain.
    let mux = Rc::new(mux::Mux::new(None));
    Mux::set_mux(&mux);
    crate::update::load_last_release_info_and_set_banner();

    let gui = crate::frontend::try_new()?;

    // Initiate an ssh connection; since that is a blocking process with
    // callbacks, we have to run it in another thread
    promise::spawn::spawn(async {
        if let Err(err) = async_run_ssh(opts).await {
            terminate_with_error(err);
        }
    })
    .detach();

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn run_serial(config: config::ConfigHandle, opts: &SerialCommand) -> anyhow::Result<()> {
    let mut serial = portable_pty::serial::SerialTty::new(&opts.port);
    if let Some(baud) = opts.baud {
        serial.set_baud_rate(serial::BaudRate::from_speed(baud));
    }

    let pty_system = Box::new(serial);
    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::with_pty_system("local", pty_system));
    let mux = Rc::new(mux::Mux::new(Some(domain.clone())));
    Mux::set_mux(&mux);
    crate::update::load_last_release_info_and_set_banner();

    let gui = crate::frontend::try_new()?;
    block_on(domain.attach())?; // FIXME: blocking

    {
        let window_id = mux.new_empty_window();
        // FIXME: blocking
        let _tab = block_on(domain.spawn(config.initial_size(), None, None, *window_id))?;
    }

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn client_domains(config: &config::ConfigHandle) -> Vec<ClientDomainConfig> {
    let mut domains = vec![];
    for unix_dom in &config.unix_domains {
        domains.push(ClientDomainConfig::Unix(unix_dom.clone()));
    }

    for ssh_dom in &config.ssh_domains {
        if ssh_dom.use_multiplexer {
            domains.push(ClientDomainConfig::Ssh(ssh_dom.clone()));
        }
    }

    for tls_client in &config.tls_clients {
        domains.push(ClientDomainConfig::Tls(tls_client.clone()));
    }
    domains
}

fn run_mux_client(config: config::ConfigHandle, opts: &ConnectCommand) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }

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
    crate::update::load_last_release_info_and_set_banner();
    // Allow spawning local commands into new tabs/panes
    let local_domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local")?);
    mux.add_domain(&local_domain);

    let gui = crate::frontend::try_new()?;
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
    })
    .detach();

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

    let have_panes_in_domain = mux
        .iter_panes()
        .iter()
        .any(|p| p.domain_id() == domain.domain_id());

    if have_panes_in_domain {
        return Ok(());
    }

    let config = config::configuration();
    let window_id = mux.new_empty_window();
    let _tab = domain
        .spawn(config.initial_size(), cmd, None, *window_id)
        .await?;
    Ok(())
}

async fn update_mux_domains(config: &ConfigHandle, do_auto_connect: bool) -> anyhow::Result<()> {
    let mux = Mux::get().unwrap();

    fn record_domain(mux: &Rc<Mux>, client: ClientDomain) -> anyhow::Result<Arc<dyn Domain>> {
        if let Some(domain) = mux.get_domain_by_name(client.domain_name()) {
            Ok(domain)
        } else {
            let domain: Arc<dyn Domain> = Arc::new(client);
            mux.add_domain(&domain);
            Ok(domain)
        }
    }

    for client_config in client_domains(&config) {
        let connect_automatically = client_config.connect_automatically();
        let dom = record_domain(&mux, ClientDomain::new(client_config))?;
        if do_auto_connect && connect_automatically {
            dom.attach().await?;
        }
    }

    for wsl_dom in &config.wsl_domains {
        if mux.get_domain_by_name(&wsl_dom.name).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new_wsl(wsl_dom.clone())?);
        mux.add_domain(&domain);
    }

    if let Some(name) = &config.default_domain {
        if let Some(dom) = mux.get_domain_by_name(name) {
            mux.set_default_domain(&dom);
        }
    }

    Ok(())
}

async fn async_run_terminal_gui(
    cmd: Option<CommandBuilder>,
    do_auto_connect: bool,
) -> anyhow::Result<()> {
    update_mux_domains(&config::configuration(), do_auto_connect).await?;
    spawn_tab_in_default_domain_if_mux_is_empty(cmd).await
}

#[derive(Debug)]
enum Publish {
    TryPathOrPublish(PathBuf),
    NoConnectNoPublish,
    NoConnectButPublish,
}

impl Publish {
    pub fn resolve(always_new_process: bool) -> Self {
        if always_new_process {
            return Self::NoConnectNoPublish;
        }

        if config::is_config_overridden() {
            // They're using a specific config file: assume that it is
            // different from the running gui
            log::trace!("skip existing gui: config is different");
            return Self::NoConnectNoPublish;
        }

        match wezterm_client::discovery::resolve_gui_sock_path(
            &crate::termwindow::get_window_class(),
        ) {
            Ok(path) => Self::TryPathOrPublish(path),
            Err(_) => Self::NoConnectButPublish,
        }
    }

    pub fn should_publish(&self) -> bool {
        match self {
            Self::TryPathOrPublish(_) | Self::NoConnectButPublish => true,
            Self::NoConnectNoPublish => false,
        }
    }
}

fn run_terminal_gui(opts: StartCommand) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }

    let config = config::configuration();
    let need_builder = !opts.prog.is_empty() || opts.cwd.is_some();

    let cmd = if need_builder {
        let prog = opts.prog.iter().map(|s| s.as_os_str()).collect::<Vec<_>>();
        let mut builder = config.build_prog(
            if prog.is_empty() { None } else { Some(prog) },
            config.default_prog.as_ref(),
            config.default_cwd.as_ref(),
        )?;
        if let Some(cwd) = opts.cwd {
            builder.cwd(cwd);
        }
        Some(builder)
    } else {
        None
    };

    // First, let's see if we can ask an already running wezterm to do this
    let publish = Publish::resolve(opts.always_new_process);
    log::trace!("{:?}", publish);
    if let Publish::TryPathOrPublish(gui_sock) = &publish {
        let dom = config::UnixDomain {
            socket_path: Some(gui_sock.clone()),
            no_serve_automatically: true,
            ..Default::default()
        };
        let mut ui = mux::connui::ConnectionUI::new_headless();
        match wezterm_client::client::Client::new_unix_domain(None, &dom, false, &mut ui, true) {
            Ok(client) => {
                let executor = promise::spawn::ScopedExecutor::new();
                let command = cmd.clone();
                let res = block_on(executor.run(async move {
                    client.verify_version_compat(&mut ui).await?;
                    client
                        .spawn_v2(codec::SpawnV2 {
                            domain: config::keyassignment::SpawnTabDomain::DefaultDomain,
                            window_id: None,
                            command,
                            command_dir: None,
                            size: config.initial_size(),
                        })
                        .await
                }));

                match res {
                    Ok(res) => {
                        log::info!(
                            "Spawned your command via the existing GUI instance. \
                             Use --always-new-process if you do not want this behavior. \
                             Result={:?}",
                            res
                        );
                        return Ok(());
                    }
                    Err(err) => {
                        log::warn!(
                            "while attempting to ask existing instance to spawn: {:#}",
                            err
                        );
                    }
                }
            }
            Err(err) => {
                // Couldn't connect: it's probably a stale symlink.
                // That's fine: we can continue with starting a fresh gui below.
                log::trace!("{:#}", err);
            }
        }
    }

    let unix_socket_path =
        config::RUNTIME_DIR.join(format!("gui-sock-{}", unsafe { libc::getpid() }));
    std::env::set_var("WEZTERM_UNIX_SOCKET", unix_socket_path.clone());

    if let Ok(mut listener) =
        wezterm_mux_server_impl::local::LocalListener::with_domain(&config::UnixDomain {
            socket_path: Some(unix_socket_path.clone()),
            ..Default::default()
        })
    {
        let unix_socket_path = unix_socket_path.clone();
        std::thread::spawn(move || {
            let name_holder;
            if publish.should_publish() {
                name_holder = wezterm_client::discovery::publish_gui_sock_path(
                    &unix_socket_path,
                    &crate::termwindow::get_window_class(),
                );
                if let Err(err) = &name_holder {
                    log::warn!("{:#}", err);
                }
            }

            listener.run();
            std::fs::remove_file(unix_socket_path).ok(); // FIXME: move into listener itself
        });
    }

    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local")?);
    let mux = Rc::new(mux::Mux::new(Some(domain.clone())));
    Mux::set_mux(&mux);
    crate::update::load_last_release_info_and_set_banner();

    let gui = crate::frontend::try_new()?;
    let activity = Activity::new();
    let do_auto_connect = !opts.no_auto_connect;

    promise::spawn::spawn(async move {
        if let Err(err) = async_run_terminal_gui(cmd, do_auto_connect).await {
            terminate_with_error(err);
        }
        drop(activity);
    })
    .detach();

    // Arrange to update the list of domains in the mux when the config is reloaded
    fn do_update_mux_domains() {
        promise::spawn::spawn(async move {
            if let Err(err) = update_mux_domains(&config::configuration(), false).await {
                log::error!("Error updating mux domains: {:#}", err);
            }
        })
        .detach();
    }

    let _config_subscription = config::subscribe_to_config_reload(move || {
        promise::spawn::spawn_into_main_thread(async move {
            // We can't inline the call to update_mux_domains here because
            // the compiler would then want its body to be Send
            do_update_mux_domains();
        })
        .detach();
        true
    });

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn fatal_toast_notification(title: &str, message: &str) {
    persistent_toast_notification(title, message);
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
    config::designate_this_as_the_main_thread();
    config::assign_error_callback(mux::connui::show_configuration_error_message);
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
        mux::connui::show_configuration_error_message(&err);
    }
}

pub fn run_ls_fonts(config: config::ConfigHandle, cmd: &LsFontsCommand) -> anyhow::Result<()> {
    use wezterm_font::parser::ParsedFont;

    if let Err(err) = config::configuration_result() {
        log::error!("{}", err);
        return Ok(());
    }

    // Disable the normal config error UI window, as we don't have
    // a fully baked GUI environment running
    config::assign_error_callback(|err| eprintln!("{}", err));

    let font_config = wezterm_font::FontConfiguration::new(
        Some(config.clone()),
        config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
    )?;

    if let Some(text) = &cmd.text {
        let line = Line::from_text(text, &CellAttributes::default(), SEQ_ZERO);
        let cell_clusters = line.cluster(None);
        for cluster in cell_clusters {
            let style = font_config.match_style(&config, &cluster.attrs);
            let font = font_config.resolve_font(style)?;
            let infos = font
                .blocking_shape(&cluster.text, Some(cluster.presentation))
                .unwrap();

            // We must grab the handles after shaping, so that we get the
            // revised list that includes system fallbacks!
            let handles = font.clone_handles();

            let mut iter = infos.iter().peekable();
            while let Some(info) = iter.next() {
                let idx = cluster.byte_to_cell_idx(info.cluster as usize);
                let text = if let Some(ahead) = iter.peek() {
                    line.columns_as_str(idx..cluster.byte_to_cell_idx(ahead.cluster as usize))
                } else {
                    line.columns_as_str(idx..line.cells().len())
                };

                let parsed = &handles[info.font_idx];
                let escaped = format!("{}", text.escape_unicode());
                if config.custom_block_glyphs {
                    if let Some(block) = customglyph::BlockKey::from_str(&text) {
                        println!(
                            "{:4} {:12} drawn by wezterm: {:?}",
                            cluster.text, escaped, block
                        );
                        continue;
                    }
                }

                println!(
                    "{:4} {:12} x_adv={:<2} glyph={:<4} {}\n{:38}{}",
                    text,
                    escaped,
                    info.x_advance.get(),
                    info.glyph_pos,
                    parsed.lua_name(),
                    "",
                    parsed.handle.diagnostic_string()
                );
            }
        }
        return Ok(());
    }

    println!("Primary font:");
    let default_font = font_config.default_font()?;
    println!(
        "{}",
        ParsedFont::lua_fallback(&default_font.clone_handles())
    );
    println!();

    for rule in &config.font_rules {
        println!();

        let mut condition = "When".to_string();
        if let Some(intensity) = &rule.intensity {
            condition.push_str(&format!(" Intensity={:?}", intensity));
        }
        if let Some(underline) = &rule.underline {
            condition.push_str(&format!(" Underline={:?}", underline));
        }
        if let Some(italic) = &rule.italic {
            condition.push_str(&format!(" Italic={:?}", italic));
        }
        if let Some(blink) = &rule.blink {
            condition.push_str(&format!(" Blink={:?}", blink));
        }
        if let Some(rev) = &rule.reverse {
            condition.push_str(&format!(" Reverse={:?}", rev));
        }
        if let Some(strikethrough) = &rule.strikethrough {
            condition.push_str(&format!(" Strikethrough={:?}", strikethrough));
        }
        if let Some(invisible) = &rule.invisible {
            condition.push_str(&format!(" Invisible={:?}", invisible));
        }

        println!("{}:", condition);
        let font = font_config.resolve_font(&rule.font)?;
        println!("{}", ParsedFont::lua_fallback(&font.clone_handles()));
        println!();
    }

    println!("Title font:");
    let title_font = font_config.title_font()?;
    println!("{}", ParsedFont::lua_fallback(&title_font.clone_handles()));
    println!();

    if cmd.list_system {
        let font_dirs = font_config.list_fonts_in_font_dirs();
        println!(
            "{} fonts found in your font_dirs + built-in fonts:",
            font_dirs.len()
        );
        for font in font_dirs {
            println!("{} -- {}", font.lua_name(), font.handle.diagnostic_string());
        }

        match font_config.list_system_fonts() {
            Ok(sys_fonts) => {
                println!(
                    "{} system fonts found using {:?}:",
                    sys_fonts.len(),
                    config.font_locator
                );
                for font in sys_fonts {
                    let pixel_sizes = if font.pixel_sizes.is_empty() {
                        "".to_string()
                    } else {
                        format!(" pixel_sizes={:?}", font.pixel_sizes)
                    };
                    println!(
                        "{} -- {}{}",
                        font.lua_name(),
                        font.handle.diagnostic_string(),
                        pixel_sizes
                    );
                }
            }
            Err(err) => log::error!("Unable to list system fonts: {}", err),
        }
    }

    Ok(())
}

#[cfg(windows)]
mod win_bindings {
    ::windows::include_bindings!();
    pub use self::Windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
}

fn run() -> anyhow::Result<()> {
    // Inform the system of our AppUserModelID.
    // Without this, our toast notifications won't be correctly
    // attributed to our application.
    #[cfg(windows)]
    {
        unsafe {
            win_bindings::SetCurrentProcessExplicitAppUserModelID("org.wezfurlong.wezterm").is_ok();
        }
    }

    let opts = Opt::from_args();

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
        if opts.attach_parent_console {
            winapi::um::wincon::AttachConsole(winapi::um::wincon::ATTACH_PARENT_PROCESS);
        }
    };

    env_bootstrap::bootstrap();

    stats::Stats::init()?;
    let _saver = umask::UmaskSaver::new();

    config::common_init(
        opts.config_file.as_ref(),
        &opts.config_override,
        opts.skip_config,
    );
    let config = config::configuration();

    let sub = match opts.cmd.as_ref().cloned() {
        Some(sub) => sub,
        None => {
            // Need to fake an argv0
            let mut argv = vec!["wezterm-gui".to_string()];
            for a in &config.default_gui_startup_args {
                argv.push(a.clone());
            }
            SubCommand::from_iter_safe(&argv).with_context(|| {
                format!(
                    "parsing the default_gui_startup_args config: {:?}",
                    config.default_gui_startup_args
                )
            })?
        }
    };

    match sub {
        SubCommand::Start(start) => {
            log::trace!("Using configuration: {:#?}\nopts: {:#?}", config, opts);
            run_terminal_gui(start)
        }
        SubCommand::Ssh(ssh) => run_ssh(ssh),
        SubCommand::Serial(serial) => run_serial(config, &serial),
        SubCommand::Connect(connect) => run_mux_client(config, &connect),
        SubCommand::LsFonts(cmd) => run_ls_fonts(config, &cmd),
    }
}
