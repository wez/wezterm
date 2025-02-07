// Don't create a new standard console window when launched from the windows GUI.
#![cfg_attr(not(test), windows_subsystem = "windows")]

use crate::customglyph::BlockKey;
use crate::glyphcache::GlyphCache;
use crate::utilsprites::RenderMetrics;
use ::window::*;
use anyhow::{anyhow, Context};
use clap::builder::ValueParser;
use clap::{Parser, ValueHint};
use config::keyassignment::{SpawnCommand, SpawnTabDomain};
use config::{ConfigHandle, SerialDomain, SshDomain, SshMultiplexing};
use mux::activity::Activity;
use mux::domain::{Domain, LocalDomain};
use mux::Mux;
use mux_lua::MuxDomain;
use portable_pty::cmdbuilder::CommandBuilder;
use promise::spawn::block_on;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env::current_dir;
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::cell::{CellAttributes, UnicodeVersion};
use termwiz::surface::{Line, SEQ_ZERO};
use unicode_normalization::UnicodeNormalization;
use wezterm_bidi::Direction;
use wezterm_client::domain::ClientDomain;
use wezterm_font::shaper::PresentationWidth;
use wezterm_font::FontConfiguration;
use wezterm_gui_subcommands::*;
use wezterm_mux_server_impl::update_mux_domains;
use wezterm_toast_notification::*;

mod colorease;
mod commands;
mod customglyph;
mod download;
mod frontend;
mod glyphcache;
mod inputmap;
mod overlay;
mod quad;
mod renderstate;
mod resize_increment_calculator;
mod scripting;
mod scrollbar;
mod selection;
mod shapecache;
mod spawn;
mod stats;
mod tabbar;
mod termwindow;
mod unicode_names;
mod uniforms;
mod update;
mod utilsprites;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

pub use selection::SelectionMode;
pub use termwindow::{set_window_class, set_window_position, TermWindow, ICON_DATA};

#[derive(Debug, Parser)]
#[command(
    about = "Wez's Terminal Emulator\nhttp://github.com/wezterm/wezterm",
    version = config::wezterm_version()
)]
struct Opt {
    /// Skip loading wezterm.lua
    #[arg(long, short = 'n')]
    skip_config: bool,

    /// Specify the configuration file to use, overrides the normal
    /// configuration file resolution
    #[arg(
        long = "config-file",
        value_parser,
        conflicts_with = "skip_config",
        value_hint=ValueHint::FilePath,
    )]
    config_file: Option<OsString>,

    /// Override specific configuration values
    #[arg(
        long = "config",
        name = "name=value",
        value_parser=ValueParser::new(name_equals_value),
        number_of_values = 1)]
    config_override: Vec<(String, String)>,

    /// On Windows, whether to attempt to attach to the parent
    /// process console to display logging output
    #[arg(long = "attach-parent-console")]
    #[allow(dead_code)]
    attach_parent_console: bool,

    #[command(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Debug, Parser, Clone)]
enum SubCommand {
    #[command(
        name = "start",
        about = "Start the GUI, optionally running an alternative program [aliases: -e]"
    )]
    Start(StartCommand),

    /// Start the GUI in blocking mode. You shouldn't see this, but you
    /// may see it in shell completions because of this open clap issue:
    /// <https://github.com/clap-rs/clap/issues/1335>
    #[command(short_flag_alias = 'e', hide = true)]
    BlockingStart(StartCommand),

    #[command(name = "ssh", about = "Establish an ssh session")]
    Ssh(SshCommand),

    #[command(name = "serial", about = "Open a serial port")]
    Serial(SerialCommand),

    #[command(name = "connect", about = "Connect to wezterm multiplexer")]
    Connect(ConnectCommand),

    #[command(name = "ls-fonts", about = "Display information about fonts")]
    LsFonts(LsFontsCommand),

    #[command(name = "show-keys", about = "Show key assignments")]
    ShowKeys(ShowKeysCommand),
}

async fn async_run_ssh(opts: SshCommand) -> anyhow::Result<()> {
    let mut ssh_option = HashMap::new();
    if opts.verbose {
        ssh_option.insert("wezterm_ssh_verbose".to_string(), "true".to_string());
    }
    for (k, v) in opts.config_override {
        ssh_option.insert(k.to_lowercase().to_string(), v);
    }

    let dom = SshDomain {
        name: format!("SSH to {}", opts.user_at_host_and_port),
        remote_address: opts.user_at_host_and_port.host_and_port.clone(),
        username: opts.user_at_host_and_port.username.clone(),
        multiplexing: SshMultiplexing::None,
        ssh_option,
        ..Default::default()
    };

    let start_command = StartCommand {
        always_new_process: true,
        class: opts.class,
        cwd: None,
        no_auto_connect: true,
        position: opts.position,
        workspace: None,
        prog: opts.prog.clone(),
        ..Default::default()
    };

    let cmd = if !opts.prog.is_empty() {
        let builder = CommandBuilder::from_argv(opts.prog);
        Some(builder)
    } else {
        None
    };

    let domain: Arc<dyn Domain> = Arc::new(mux::ssh::RemoteSshDomain::with_ssh_domain(&dom)?);
    let mux = Mux::get();
    mux.add_domain(&domain);
    mux.set_default_domain(&domain);

    let should_publish = false;
    async_run_terminal_gui(cmd, start_command, should_publish).await
}

fn run_ssh(opts: SshCommand) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }
    if let Some(pos) = opts.position.as_ref() {
        set_window_position(pos.clone());
    }

    build_initial_mux(&config::configuration(), None, None)?;

    let gui = crate::frontend::try_new()?;

    promise::spawn::spawn(async {
        if let Err(err) = async_run_ssh(opts).await {
            terminate_with_error(err);
        }
    })
    .detach();

    maybe_show_configuration_error_window();
    gui.run_forever()
}

async fn async_run_serial(opts: SerialCommand) -> anyhow::Result<()> {
    let serial_domain = SerialDomain {
        name: format!("Serial Port {}", opts.port),
        port: Some(opts.port.clone()),
        baud: opts.baud,
    };

    let start_command = StartCommand {
        always_new_process: true,
        class: opts.class,
        cwd: None,
        no_auto_connect: true,
        position: opts.position,
        workspace: None,
        domain: Some(serial_domain.name.clone()),
        ..Default::default()
    };

    let cmd = None;

    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new_serial_domain(serial_domain)?);
    let mux = Mux::get();
    mux.add_domain(&domain);

    let should_publish = false;
    async_run_terminal_gui(cmd, start_command, should_publish).await
}

fn run_serial(config: config::ConfigHandle, opts: SerialCommand) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }
    if let Some(pos) = opts.position.as_ref() {
        set_window_position(pos.clone());
    }

    build_initial_mux(&config, None, None)?;

    let gui = crate::frontend::try_new()?;

    promise::spawn::spawn(async {
        if let Err(err) = async_run_serial(opts).await {
            terminate_with_error(err);
        }
    })
    .detach();

    maybe_show_configuration_error_window();
    gui.run_forever()
}

fn have_panes_in_domain_and_ws(domain: &Arc<dyn Domain>, workspace: &Option<String>) -> bool {
    let mux = Mux::get();
    let have_panes_in_domain = mux
        .iter_panes()
        .iter()
        .any(|p| p.domain_id() == domain.domain_id());

    if !have_panes_in_domain {
        return false;
    }

    if let Some(ws) = &workspace {
        for window_id in mux.iter_windows_in_workspace(ws) {
            if let Some(win) = mux.get_window(window_id) {
                for t in win.iter() {
                    for p in t.iter_panes_ignoring_zoom() {
                        if p.pane.domain_id() == domain.domain_id() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    } else {
        true
    }
}

async fn spawn_tab_in_domain_if_mux_is_empty(
    cmd: Option<CommandBuilder>,
    is_connecting: bool,
    domain: Option<Arc<dyn Domain>>,
    workspace: Option<String>,
) -> anyhow::Result<()> {
    let mux = Mux::get();

    let domain = domain.unwrap_or_else(|| mux.default_domain());

    if !is_connecting {
        if have_panes_in_domain_and_ws(&domain, &workspace) {
            return Ok(());
        }
    }

    let window_id = {
        // Force the builder to notify the frontend early,
        // so that the attach await below doesn't block it.
        // This has the consequence of creating the window
        // at the initial size instead of populating it
        // from the size specified in the remote mux.
        // We use the TabAddedToWindow mux notification
        // to detect and adjust the size later on.
        let position = None;
        let builder = mux.new_empty_window(workspace.clone(), position);
        *builder
    };

    let config = config::configuration();
    config.update_ulimit()?;

    domain.attach(Some(window_id)).await?;

    if have_panes_in_domain_and_ws(&domain, &workspace) {
        trigger_and_log_gui_attached(MuxDomain(domain.domain_id())).await;
        return Ok(());
    }

    let _config_subscription = config::subscribe_to_config_reload(move || {
        promise::spawn::spawn_into_main_thread(async move {
            if let Err(err) = update_mux_domains(&config::configuration()) {
                log::error!("Error updating mux domains: {:#}", err);
            }
        })
        .detach();
        true
    });

    let dpi = config.dpi.unwrap_or_else(|| ::window::default_dpi());
    let _tab = domain
        .spawn(
            config.initial_size(dpi as u32, Some(cell_pixel_dims(&config, dpi)?)),
            cmd,
            None,
            window_id,
        )
        .await?;
    trigger_and_log_gui_attached(MuxDomain(domain.domain_id())).await;
    Ok(())
}

async fn connect_to_auto_connect_domains() -> anyhow::Result<()> {
    let mux = Mux::get();
    let domains = mux.iter_domains();
    for dom in domains {
        if let Some(dom) = dom.downcast_ref::<ClientDomain>() {
            if dom.connect_automatically() {
                dom.attach(None).await?;
            }
        }
    }
    Ok(())
}

async fn trigger_gui_startup(
    lua: Option<Rc<mlua::Lua>>,
    spawn: Option<SpawnCommand>,
) -> anyhow::Result<()> {
    if let Some(lua) = lua {
        let args = lua.pack_multi(spawn)?;
        config::lua::emit_event(&lua, ("gui-startup".to_string(), args)).await?;
    }
    Ok(())
}

async fn trigger_and_log_gui_startup(spawn_command: Option<SpawnCommand>) {
    if let Err(err) =
        config::with_lua_config_on_main_thread(move |lua| trigger_gui_startup(lua, spawn_command))
            .await
    {
        let message = format!("while processing gui-startup event: {:#}", err);
        log::error!("{}", message);
        persistent_toast_notification("Error", &message);
    }
}

async fn trigger_gui_attached(lua: Option<Rc<mlua::Lua>>, domain: MuxDomain) -> anyhow::Result<()> {
    if let Some(lua) = lua {
        let args = lua.pack_multi(domain)?;
        config::lua::emit_event(&lua, ("gui-attached".to_string(), args)).await?;
    }
    Ok(())
}

async fn trigger_and_log_gui_attached(domain: MuxDomain) {
    if let Err(err) =
        config::with_lua_config_on_main_thread(move |lua| trigger_gui_attached(lua, domain)).await
    {
        let message = format!("while processing gui-attached event: {:#}", err);
        log::error!("{}", message);
        persistent_toast_notification("Error", &message);
    }
}

fn cell_pixel_dims(config: &ConfigHandle, dpi: f64) -> anyhow::Result<(usize, usize)> {
    let fontconfig = Rc::new(FontConfiguration::new(Some(config.clone()), dpi as usize)?);
    let render_metrics = RenderMetrics::new(&fontconfig)?;
    Ok((
        render_metrics.cell_size.width as usize,
        render_metrics.cell_size.height as usize,
    ))
}

async fn async_run_terminal_gui(
    cmd: Option<CommandBuilder>,
    opts: StartCommand,
    should_publish: bool,
) -> anyhow::Result<()> {
    let unix_socket_path =
        config::RUNTIME_DIR.join(format!("gui-sock-{}", unsafe { libc::getpid() }));
    std::env::set_var("WEZTERM_UNIX_SOCKET", unix_socket_path.clone());
    wezterm_blob_leases::register_storage(Arc::new(
        wezterm_blob_leases::simple_tempdir::SimpleTempDir::new_in(&*config::CACHE_DIR)?,
    ))?;
    if let Err(err) = spawn_mux_server(unix_socket_path, should_publish) {
        log::warn!("{:#}", err);
    }

    if !opts.no_auto_connect {
        connect_to_auto_connect_domains().await?;
    }

    let spawn_command = match &cmd {
        Some(cmd) => Some(SpawnCommand::from_command_builder(cmd)?),
        None => None,
    };

    // Apply the domain to the command
    let spawn_command = match (spawn_command, &opts.domain) {
        (Some(spawn), Some(name)) => Some(SpawnCommand {
            domain: SpawnTabDomain::DomainName(name.to_string()),
            ..spawn
        }),
        (None, Some(name)) => Some(SpawnCommand {
            domain: SpawnTabDomain::DomainName(name.to_string()),
            ..SpawnCommand::default()
        }),
        (spawn, None) => spawn,
    };
    let mux = Mux::get();

    let domain = if let Some(name) = &opts.domain {
        let domain = mux
            .get_domain_by_name(name)
            .ok_or_else(|| anyhow!("invalid domain {name}"))?;
        Some(domain)
    } else {
        None
    };

    if !opts.attach {
        trigger_and_log_gui_startup(spawn_command).await;
    }

    let is_connecting = opts.attach;

    if let Some(domain) = &domain {
        if !opts.attach {
            let window_id = {
                // Force the builder to notify the frontend early,
                // so that the attach await below doesn't block it.
                let workspace = None;
                let position = None;
                let builder = mux.new_empty_window(workspace, position);
                *builder
            };

            domain.attach(Some(window_id)).await?;
            let config = config::configuration();
            let dpi = config.dpi.unwrap_or_else(|| ::window::default_dpi());
            let tab = domain
                .spawn(
                    config.initial_size(dpi as u32, Some(cell_pixel_dims(&config, dpi)?)),
                    cmd.clone(),
                    None,
                    window_id,
                )
                .await?;
            let mut window = mux
                .get_window_mut(window_id)
                .ok_or_else(|| anyhow!("failed to get mux window id {window_id}"))?;
            if let Some(tab_idx) = window.idx_by_id(tab.tab_id()) {
                window.set_active_without_saving(tab_idx);
            }
            trigger_and_log_gui_attached(MuxDomain(domain.domain_id())).await;
        }
    }
    spawn_tab_in_domain_if_mux_is_empty(cmd, is_connecting, domain, opts.workspace).await
}

#[derive(Debug)]
enum Publish {
    TryPathOrPublish(PathBuf),
    NoConnectNoPublish,
    NoConnectButPublish,
}

impl Publish {
    pub fn resolve(mux: &Arc<Mux>, config: &ConfigHandle, always_new_process: bool) -> Self {
        if mux.default_domain().domain_name() != config.default_domain.as_deref().unwrap_or("local")
        {
            return Self::NoConnectNoPublish;
        }

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

    pub fn try_spawn(
        &mut self,
        cmd: Option<CommandBuilder>,
        config: &ConfigHandle,
        workspace: Option<&str>,
        domain: SpawnTabDomain,
        new_tab: bool,
    ) -> anyhow::Result<bool> {
        if let Publish::TryPathOrPublish(gui_sock) = &self {
            let dom = config::UnixDomain {
                socket_path: Some(gui_sock.clone()),
                no_serve_automatically: true,
                ..Default::default()
            };
            let mut ui = mux::connui::ConnectionUI::new_headless();
            match wezterm_client::client::Client::new_unix_domain(None, &dom, false, &mut ui, true)
            {
                Ok(client) => {
                    let executor = promise::spawn::ScopedExecutor::new();
                    let command = cmd.clone();
                    let res = block_on(executor.run(async move {
                        let vers = client.verify_version_compat(&mut ui).await?;

                        if vers.executable_path != std::env::current_exe().context("resolve executable path")? {
                            *self = Publish::NoConnectNoPublish;
                            anyhow::bail!(
                                "Running GUI is a different executable from us, will start a new one");
                        }
                        if vers.config_file_path
                            != std::env::var_os("WEZTERM_CONFIG_FILE").map(Into::into)
                        {
                            *self = Publish::NoConnectNoPublish;
                            anyhow::bail!(
                                "Running GUI has different config from us, will start a new one"
                            );
                        }

                        let window_id = if new_tab || config.prefer_to_spawn_tabs {
                            if let Ok(pane_id) = client.resolve_pane_id(None).await {
                                let panes = client.list_panes().await?;

                                let mut window_id = None;
                                'outer: for tabroot in panes.tabs {
                                    let mut cursor = tabroot.into_tree().cursor();

                                    loop {
                                        if let Some(entry) = cursor.leaf_mut() {
                                            if entry.pane_id == pane_id {
                                                window_id.replace(entry.window_id);
                                                break 'outer;
                                            }
                                        }
                                        match cursor.preorder_next() {
                                            Ok(c) => cursor = c,
                                            Err(_) => break,
                                        }
                                    }
                                }
                                window_id

                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        client
                            .spawn_v2(codec::SpawnV2 {
                                domain,
                                window_id,
                                command,
                                command_dir: None,
                                size: config.initial_size(0, None),
                                workspace: workspace.unwrap_or(
                                    config
                                        .default_workspace
                                        .as_deref()
                                        .unwrap_or(mux::DEFAULT_WORKSPACE)
                                ).to_string(),
                            })
                            .await
                    }));

                    match res {
                        Ok(res) => {
                            log::info!(
                                "Spawned your command via the existing GUI instance. \
                             Use wezterm start --always-new-process if you do not want this behavior. \
                             Result={:?}",
                                res
                            );
                            Ok(true)
                        }
                        Err(err) => {
                            log::trace!(
                                "while attempting to ask existing instance to spawn: {:#}",
                                err
                            );
                            Ok(false)
                        }
                    }
                }
                Err(err) => {
                    // Couldn't connect: it's probably a stale symlink.
                    // That's fine: we can continue with starting a fresh gui below.
                    log::trace!("{:#}", err);
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }
}

fn spawn_mux_server(unix_socket_path: PathBuf, should_publish: bool) -> anyhow::Result<()> {
    let mut listener =
        wezterm_mux_server_impl::local::LocalListener::with_domain(&config::UnixDomain {
            socket_path: Some(unix_socket_path.clone()),
            ..Default::default()
        })?;
    std::thread::spawn(move || {
        let name_holder;
        if should_publish {
            name_holder = wezterm_client::discovery::publish_gui_sock_path(
                &unix_socket_path,
                &crate::termwindow::get_window_class(),
            );
            if let Err(err) = &name_holder {
                log::warn!("{:#}", err);
            }
        }

        listener.run();
        std::fs::remove_file(unix_socket_path).ok();
    });

    Ok(())
}

fn setup_mux(
    local_domain: Arc<dyn Domain>,
    config: &ConfigHandle,
    default_domain_name: Option<&str>,
    default_workspace_name: Option<&str>,
) -> anyhow::Result<Arc<Mux>> {
    let mux = Arc::new(mux::Mux::new(Some(local_domain.clone())));
    Mux::set_mux(&mux);
    let client_id = Arc::new(mux::client::ClientId::new());
    mux.register_client(client_id.clone());
    mux.replace_identity(Some(client_id));
    let default_workspace_name = default_workspace_name.unwrap_or(
        config
            .default_workspace
            .as_deref()
            .unwrap_or(mux::DEFAULT_WORKSPACE),
    );
    mux.set_active_workspace(&default_workspace_name);
    crate::update::load_last_release_info_and_set_banner();
    update_mux_domains(config)?;

    let default_name =
        default_domain_name.unwrap_or(config.default_domain.as_deref().unwrap_or("local"));

    let domain = mux.get_domain_by_name(default_name).ok_or_else(|| {
        anyhow::anyhow!(
            "desired default domain '{}' was not found in mux!?",
            default_name
        )
    })?;
    mux.set_default_domain(&domain);

    Ok(mux)
}

fn build_initial_mux(
    config: &ConfigHandle,
    default_domain_name: Option<&str>,
    default_workspace_name: Option<&str>,
) -> anyhow::Result<Arc<Mux>> {
    let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new("local")?);
    setup_mux(domain, config, default_domain_name, default_workspace_name)
}

fn run_terminal_gui(opts: StartCommand, default_domain_name: Option<String>) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }
    if let Some(pos) = opts.position.as_ref() {
        set_window_position(pos.clone());
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
        if let Some(cwd) = &opts.cwd {
            builder.cwd(if cwd.is_relative() {
                current_dir()?.join(cwd).into_os_string().into()
            } else {
                Cow::Borrowed(cwd.as_ref())
            });
        }
        Some(builder)
    } else {
        None
    };

    let mux = build_initial_mux(
        &config,
        default_domain_name.as_deref(),
        opts.workspace.as_deref(),
    )?;

    // First, let's see if we can ask an already running wezterm to do this.
    // We must do this before we start the gui frontend as the scheduler
    // requirements are different.
    let mut publish = Publish::resolve(
        &mux,
        &config,
        opts.always_new_process || opts.position.is_some(),
    );
    log::trace!("{:?}", publish);
    if publish.try_spawn(
        cmd.clone(),
        &config,
        opts.workspace.as_deref(),
        match &opts.domain {
            Some(name) => SpawnTabDomain::DomainName(name.to_string()),
            None => SpawnTabDomain::DefaultDomain,
        },
        opts.new_tab,
    )? {
        return Ok(());
    }

    let gui = crate::frontend::try_new()?;
    let activity = Activity::new();

    promise::spawn::spawn(async move {
        if let Err(err) = async_run_terminal_gui(cmd, opts, publish.should_publish()).await {
            terminate_with_error(err);
        }
        drop(activity);
    })
    .detach();

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
    let mut err_text = format!("{err:#}");

    let warnings = config::configuration_warnings_and_errors();
    if !warnings.is_empty() {
        let err = warnings.join("\n");
        err_text = format!("{err_text}\nConfiguration Error: {err}");
    }

    terminate_with_error_message(&err_text)
}

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

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
    let warnings = config::configuration_warnings_and_errors();
    if !warnings.is_empty() {
        let err = warnings.join("\n");
        mux::connui::show_configuration_error_message(&err);
    }
}

fn run_show_keys(config: config::ConfigHandle, cmd: &ShowKeysCommand) -> anyhow::Result<()> {
    let map = crate::inputmap::InputMap::new(&config);
    if cmd.lua {
        map.dump_config(cmd.key_table.as_deref());
    } else {
        map.show_keys();
    }
    Ok(())
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

    let font_config = Rc::new(wezterm_font::FontConfiguration::new(
        Some(config.clone()),
        config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
    )?);

    let render_metrics = crate::utilsprites::RenderMetrics::new(&font_config)?;

    let bidi_hint = if config.bidi_enabled {
        Some(config.bidi_direction)
    } else {
        None
    };

    let unicode_version = UnicodeVersion {
        version: config.unicode_version,
        ambiguous_are_wide: config.treat_east_asian_ambiguous_width_as_wide,
    };

    let text = match (&cmd.text, &cmd.codepoints) {
        (Some(text), _) => Some(text.to_string()),
        (_, Some(codepoints)) => {
            let mut s = String::new();
            for cp in codepoints.split(",") {
                let cp = u32::from_str_radix(cp, 16)
                    .with_context(|| format!("{cp} is not a hex number"))?;
                let c = char::from_u32(cp)
                    .ok_or_else(|| anyhow!("{cp} is not a valid unicode codepoint value"))?;
                s.push(c);
            }
            Some(s)
        }
        _ => None,
    };

    if let Some(text) = &text {
        // Emulate the effect of output normalization
        let text = if config.normalize_output_to_unicode_nfc {
            text.nfc().collect()
        } else {
            text.to_string()
        };

        let line = Line::from_text(
            &text,
            &CellAttributes::default(),
            SEQ_ZERO,
            Some(unicode_version),
        );
        let cell_clusters = line.cluster(bidi_hint);
        let ft_lib = wezterm_font::ftwrap::Library::new()?;

        let mut glyph_cache = GlyphCache::new_in_memory(&font_config, 256)?;

        for cluster in cell_clusters {
            let style = font_config.match_style(&config, &cluster.attrs);
            let font = font_config.resolve_font(style)?;
            let presentation_width = PresentationWidth::with_cluster(&cluster);
            let infos = font
                .blocking_shape(
                    &cluster.text,
                    Some(cluster.presentation),
                    cluster.direction,
                    None,
                    Some(&presentation_width),
                )
                .unwrap();

            // We must grab the handles after shaping, so that we get the
            // revised list that includes system fallbacks!
            let handles = font.clone_handles();
            let faces: Vec<_> = handles
                .iter()
                .map(|p| ft_lib.face_from_locator(&p.handle).ok())
                .collect();

            let mut iter = infos.iter().peekable();

            let mut byte_lens = vec![];
            for c in cluster.text.chars() {
                let len = c.len_utf8();
                for _ in 0..len {
                    byte_lens.push(len);
                }
            }
            println!("{:?}", cluster.direction);

            while let Some(info) = iter.next() {
                let idx = cluster.byte_to_cell_idx(info.cluster as usize);
                let followed_by_space = match line.get_cell(idx + 1) {
                    Some(cell) => cell.str() == " ",
                    None => false,
                };

                let text = if cluster.direction == Direction::LeftToRight {
                    if let Some(next) = iter.peek() {
                        line.columns_as_str(idx..cluster.byte_to_cell_idx(next.cluster as usize))
                    } else {
                        let last_idx = cluster.byte_to_cell_idx(cluster.text.len() - 1);
                        line.columns_as_str(idx..last_idx + 1)
                    }
                } else {
                    let info_len = byte_lens[info.cluster as usize];
                    let last_idx = cluster.byte_to_cell_idx(info.cluster as usize + info_len - 1);
                    line.columns_as_str(idx..last_idx + 1)
                };

                let parsed = &handles[info.font_idx];
                let escaped = format!("{}", text.escape_unicode());
                let mut is_custom = false;

                let cached_glyph = glyph_cache.cached_glyph(
                    &info,
                    &style,
                    followed_by_space,
                    &font,
                    &render_metrics,
                    info.num_cells,
                )?;

                let mut texture = cached_glyph.texture.clone();

                if config.custom_block_glyphs {
                    if let Some(block) = info.only_char.and_then(BlockKey::from_char) {
                        texture.replace(glyph_cache.cached_block(block, &render_metrics)?);
                        println!(
                            "{:2} {:4} {:12} drawn by wezterm because custom_block_glyphs=true: {:?}",
                            info.cluster, text, escaped, block
                        );
                        is_custom = true;
                    }
                }

                if !is_custom {
                    let glyph_name = faces[info.font_idx]
                        .as_ref()
                        .and_then(|face| {
                            face.get_glyph_name(info.glyph_pos)
                                .map(|name| format!("{},", name))
                        })
                        .unwrap_or_else(String::new);

                    println!(
                        "{:2} {:4} {:12} x_adv={:<2} cells={:<2} glyph={}{:<4} {}\n{:38}{}",
                        info.cluster,
                        text,
                        escaped,
                        cached_glyph.x_advance.get(),
                        info.num_cells,
                        glyph_name,
                        info.glyph_pos,
                        parsed.lua_name(),
                        "",
                        parsed.handle.diagnostic_string()
                    );
                }

                if cmd.rasterize_ascii {
                    let mut glyph = String::new();

                    if let Some(texture) = &cached_glyph.texture {
                        use ::window::bitmaps::ImageTexture;
                        if let Some(tex) = texture.texture.downcast_ref::<ImageTexture>() {
                            for y in texture.coords.min_y()..texture.coords.max_y() {
                                for &px in tex.image.borrow().horizontal_pixel_range(
                                    texture.coords.min_x() as usize,
                                    texture.coords.max_x() as usize,
                                    y as usize,
                                ) {
                                    let px = u32::from_be(px);
                                    let (b, g, r, a) = (
                                        (px >> 8) as u8,
                                        (px >> 16) as u8,
                                        (px >> 24) as u8,
                                        (px & 0xff) as u8,
                                    );
                                    // Use regular RGB for other terminals, but then
                                    // set RGBA for wezterm
                                    glyph.push_str(&format!(
                                "\x1b[38:2::{r}:{g}:{b}m\x1b[38:6::{r}:{g}:{b}:{a}m\u{2588}\x1b[0m"
                            ));
                                }
                                glyph.push('\n');
                            }
                        }
                    }

                    if !is_custom {
                        println!(
                            "bearing: x={} y={}, offset: x={} y={}",
                            cached_glyph.bearing_x.get(),
                            cached_glyph.bearing_y.get(),
                            cached_glyph.x_offset.get(),
                            cached_glyph.y_offset.get(),
                        );
                    }
                    println!("{glyph}");
                }
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
            let pixel_sizes = if font.pixel_sizes.is_empty() {
                "".to_string()
            } else {
                format!(" pixel_sizes={:?}", font.pixel_sizes)
            };
            println!(
                "{} -- {}{}{}",
                font.lua_name(),
                font.aka(),
                font.handle.diagnostic_string(),
                pixel_sizes
            );
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
                        "{} -- {}{}{}",
                        font.lua_name(),
                        font.aka(),
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

fn run() -> anyhow::Result<()> {
    // Inform the system of our AppUserModelID.
    // Without this, our toast notifications won't be correctly
    // attributed to our application.
    #[cfg(windows)]
    {
        unsafe {
            ::windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(
                ::windows::core::PCWSTR(wide_string("org.wezfurlong.wezterm").as_ptr()),
            )
            .unwrap();
        }
    }

    let opts = Opt::parse();

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
    // window_funcs is not set up by env_bootstrap as window_funcs is
    // GUI environment specific and env_bootstrap is used to setup the
    // headless mux server.
    config::lua::add_context_setup_func(window_funcs::register);
    config::lua::add_context_setup_func(crate::scripting::register);
    config::lua::add_context_setup_func(crate::stats::register);

    stats::Stats::init()?;
    let _saver = umask::UmaskSaver::new();

    config::common_init(
        opts.config_file.as_ref(),
        &opts.config_override,
        opts.skip_config,
    )?;
    let config = config::configuration();
    if let Some(value) = &config.default_ssh_auth_sock {
        std::env::set_var("SSH_AUTH_SOCK", value);
    }

    let sub = match opts.cmd.as_ref().cloned() {
        Some(SubCommand::BlockingStart(start)) => {
            // Act as if the normal start subcommand was used,
            // except that we always start a new instance.
            // This is needed for compatibility, because many tools assume
            // that "$TERMINAL -e $COMMAND" blocks until the command finished.
            SubCommand::Start(StartCommand {
                always_new_process: true,
                ..start
            })
        }
        Some(sub) => sub,
        None => {
            // Need to fake an argv0
            let mut argv = vec!["wezterm-gui".to_string()];
            for a in &config.default_gui_startup_args {
                argv.push(a.clone());
            }
            SubCommand::try_parse_from(&argv).with_context(|| {
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
            let res = run_terminal_gui(start, None);
            wezterm_blob_leases::clear_storage();
            res
        }
        SubCommand::BlockingStart(_) => unreachable!(),
        SubCommand::Ssh(ssh) => run_ssh(ssh),
        SubCommand::Serial(serial) => run_serial(config, serial),
        SubCommand::Connect(connect) => run_terminal_gui(
            StartCommand {
                domain: Some(connect.domain_name.clone()),
                class: connect.class,
                workspace: connect.workspace,
                position: connect.position,
                prog: connect.prog,
                new_tab: connect.new_tab,
                always_new_process: true,
                attach: true,
                _cmd: false,
                no_auto_connect: false,
                cwd: None,
            },
            Some(connect.domain_name),
        ),
        SubCommand::LsFonts(cmd) => run_ls_fonts(config, &cmd),
        SubCommand::ShowKeys(cmd) => run_show_keys(config, &cmd),
    }
}
