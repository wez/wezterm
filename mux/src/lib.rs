use crate::client::{ClientId, ClientInfo};
use crate::pane::{Pane, PaneId};
use crate::tab::{SplitRequest, Tab, TabId};
use crate::window::{Window, WindowId};
use anyhow::{anyhow, Context, Error};
use config::keyassignment::SpawnTabDomain;
use config::{configuration, ExitBehavior};
use domain::{Domain, DomainId, DomainState, SplitSource};
use filedescriptor::{poll, pollfd, socketpair, AsRawSocketDescriptor, FileDescriptor, POLLIN};
#[cfg(unix)]
use libc::{SOL_SOCKET, SO_RCVBUF, SO_SNDBUF};
use log::error;
use metrics::histogram;
use percent_encoding::percent_decode_str;
use portable_pty::{CommandBuilder, ExitStatus, PtySize};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use termwiz::escape::csi::{DecPrivateMode, DecPrivateModeCode, Device, Mode};
use termwiz::escape::{Action, CSI};
use thiserror::*;
use wezterm_term::{Clipboard, ClipboardSelection, DownloadHandler, TerminalSize};
#[cfg(windows)]
use winapi::um::winsock2::{SOL_SOCKET, SO_RCVBUF, SO_SNDBUF};

pub mod activity;
pub mod client;
pub mod connui;
pub mod domain;
pub mod localpane;
pub mod pane;
pub mod renderable;
pub mod ssh;
pub mod tab;
pub mod termwiztermtab;
pub mod tmux;
pub mod tmux_commands;
mod tmux_pty;
pub mod window;

use crate::activity::Activity;

pub const DEFAULT_WORKSPACE: &str = "default";

#[derive(Clone, Debug)]
pub enum MuxNotification {
    PaneOutput(PaneId),
    PaneAdded(PaneId),
    PaneRemoved(PaneId),
    WindowCreated(WindowId),
    WindowRemoved(WindowId),
    WindowInvalidated(WindowId),
    WindowWorkspaceChanged(WindowId),
    ActiveWorkspaceChanged(Arc<ClientId>),
    Alert {
        pane_id: PaneId,
        alert: wezterm_term::Alert,
    },
    Empty,
    AssignClipboard {
        pane_id: PaneId,
        selection: ClipboardSelection,
        clipboard: Option<String>,
    },
    SaveToDownloads {
        name: Option<String>,
        data: Arc<Vec<u8>>,
    },
    TabAddedToWindow {
        tab_id: TabId,
        window_id: WindowId,
    },
}

static SUB_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<Tab>>>,
    panes: RefCell<HashMap<PaneId, Rc<dyn Pane>>>,
    windows: RefCell<HashMap<WindowId, Window>>,
    default_domain: RefCell<Option<Arc<dyn Domain>>>,
    domains: RefCell<HashMap<DomainId, Arc<dyn Domain>>>,
    domains_by_name: RefCell<HashMap<String, Arc<dyn Domain>>>,
    subscribers: RefCell<HashMap<usize, Box<dyn Fn(MuxNotification) -> bool>>>,
    banner: RefCell<Option<String>>,
    clients: RefCell<HashMap<ClientId, ClientInfo>>,
    identity: RefCell<Option<Arc<ClientId>>>,
    num_panes_by_workspace: RefCell<HashMap<String, usize>>,
}

const BUFSIZE: usize = 1024 * 1024;

/// This function bounces parsed actions over to the main thread to feed to
/// the pty in the mux.
/// It blocks until the mux has finished consuming the data, which provides
/// some back-pressure so that eg: ctrl-c can remain responsive.
fn send_actions_to_mux(pane_id: PaneId, dead: &Arc<AtomicBool>, actions: Vec<Action>) {
    let start = Instant::now();
    promise::spawn::block_on(promise::spawn::spawn_into_main_thread({
        let dead = Arc::clone(&dead);
        async move {
            let mux = Mux::get().unwrap();
            if let Some(pane) = mux.get_pane(pane_id) {
                let start = Instant::now();
                pane.perform_actions(actions);
                histogram!(
                    "send_actions_to_mux.perform_actions.latency",
                    start.elapsed()
                );
                mux.notify(MuxNotification::PaneOutput(pane_id));
            } else {
                // Something else removed the pane from
                // the mux, so signal that we should stop
                // trying to process it in read_from_pane_pty.
                dead.store(true, Ordering::Relaxed);
            }
        }
    }));
    histogram!("send_actions_to_mux.latency", start.elapsed());
    histogram!("send_actions_to_mux.rate", 1.);
}

fn parse_buffered_data(pane_id: PaneId, dead: &Arc<AtomicBool>, mut rx: FileDescriptor) {
    let mut buf = vec![0; configuration().mux_output_parser_buffer_size];
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut actions = vec![];
    let mut hold = false;
    let mut action_size = 0;
    let mut delay_ms = configuration().mux_output_parser_coalesce_delay_ms;

    loop {
        match rx.read(&mut buf) {
            Ok(size) if size == 0 => {
                dead.store(true, Ordering::Relaxed);
                break;
            }
            Err(_) => {
                dead.store(true, Ordering::Relaxed);
                break;
            }
            Ok(size) => {
                parser.parse(&buf[0..size], |action| {
                    let mut flush = false;
                    match &action {
                        Action::CSI(CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                            DecPrivateModeCode::SynchronizedOutput,
                        )))) => {
                            hold = true;

                            // Flush prior actions
                            if !actions.is_empty() {
                                send_actions_to_mux(pane_id, dead, std::mem::take(&mut actions));
                                action_size = 0;
                            }
                        }
                        Action::CSI(CSI::Mode(Mode::ResetDecPrivateMode(
                            DecPrivateMode::Code(DecPrivateModeCode::SynchronizedOutput),
                        ))) => {
                            hold = false;
                            flush = true;
                        }
                        Action::CSI(CSI::Device(dev)) if matches!(**dev, Device::SoftReset) => {
                            hold = false;
                            flush = true;
                        }
                        _ => {}
                    };
                    action.append_to(&mut actions);

                    if flush && !actions.is_empty() {
                        send_actions_to_mux(pane_id, dead, std::mem::take(&mut actions));
                        action_size = 0;
                    }
                });
                action_size += size;
                if !actions.is_empty() && !hold {
                    // If we haven't accumulated too much data,
                    // pause for a short while to increase the chances
                    // that we coalesce a full "frame" from an unoptimized
                    // TUI program
                    if action_size < buf.len() {
                        if delay_ms > 0 {
                            let mut pfd = [pollfd {
                                fd: rx.as_socket_descriptor(),
                                events: POLLIN,
                                revents: 0,
                            }];
                            if let Ok(1) = poll(&mut pfd, Some(Duration::from_millis(delay_ms))) {
                                // We can read now without blocking, so accumulate
                                // more data into actions
                                continue;
                            }

                            // Not readable in time: let the data we have flow into
                            // the terminal model
                        }
                    }

                    send_actions_to_mux(pane_id, dead, std::mem::take(&mut actions));
                    action_size = 0;
                }

                let config = configuration();
                buf.resize(config.mux_output_parser_buffer_size, 0);
                delay_ms = config.mux_output_parser_coalesce_delay_ms;
            }
        }
    }
}

fn set_socket_buffer(fd: &mut FileDescriptor, option: i32, size: usize) -> anyhow::Result<()> {
    let socklen = std::mem::size_of_val(&size);
    unsafe {
        let res = libc::setsockopt(
            fd.as_socket_descriptor(),
            SOL_SOCKET,
            option,
            &size as *const usize as *const _,
            socklen as _,
        );
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error()).context("setsockopt")
        }
    }
}

fn allocate_socketpair() -> anyhow::Result<(FileDescriptor, FileDescriptor)> {
    let (mut tx, mut rx) = socketpair().context("socketpair")?;
    set_socket_buffer(&mut tx, SO_SNDBUF, BUFSIZE).context("SO_SNDBUF")?;
    set_socket_buffer(&mut rx, SO_RCVBUF, BUFSIZE).context("SO_RCVBUF")?;
    Ok((tx, rx))
}

/// This function is run in a separate thread; its purpose is to perform
/// blocking reads from the pty (non-blocking reads are not portable to
/// all platforms and pty/tty types), parse the escape sequences and
/// relay the actions to the mux thread to apply them to the pane.
fn read_from_pane_pty(pane_id: PaneId, banner: Option<String>, mut reader: Box<dyn std::io::Read>) {
    let mut buf = vec![0; BUFSIZE];

    // This is used to signal that an error occurred either in this thread,
    // or in the main mux thread.  If `true`, this thread will terminate.
    let dead = Arc::new(AtomicBool::new(false));

    let (mut tx, rx) = match allocate_socketpair() {
        Ok(pair) => pair,
        Err(err) => {
            log::error!("read_from_pane_pty: Unable to allocate a socketpair: {err:#}");
            localpane::emit_output_for_pane(
                pane_id,
                &format!(
                    "⚠️  wezterm: read_from_pane_pty: \
                    Unable to allocate a socketpair: {err:#}"
                ),
            );
            return;
        }
    };

    std::thread::spawn({
        let dead = Arc::clone(&dead);
        move || parse_buffered_data(pane_id, &dead, rx)
    });

    if let Some(banner) = banner {
        tx.write_all(banner.as_bytes()).ok();
    }

    while !dead.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                log::trace!("read_pty EOF: pane_id {}", pane_id);
                break;
            }
            Err(err) => {
                error!("read_pty failed: pane {} {:?}", pane_id, err);
                break;
            }
            Ok(size) => {
                histogram!("read_from_pane_pty.bytes.rate", size as f64);
                if let Err(err) = tx.write_all(&buf[..size]) {
                    error!(
                        "read_pty failed to write to parser: pane {} {:?}",
                        pane_id, err
                    );
                    break;
                }
            }
        }
    }

    match configuration().exit_behavior {
        ExitBehavior::Hold | ExitBehavior::CloseOnCleanExit => {
            // We don't know if we can unilaterally close
            // this pane right now, so don't!
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                log::trace!("checking for dead windows after EOF on pane {}", pane_id);
                mux.prune_dead_windows();
            })
            .detach();
        }
        ExitBehavior::Close => {
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                mux.remove_pane(pane_id);
            })
            .detach();
        }
    }

    dead.store(true, Ordering::Relaxed);
}

thread_local! {
    static MUX: RefCell<Option<Rc<Mux>>> = RefCell::new(None);
}

pub struct MuxWindowBuilder {
    window_id: WindowId,
    activity: Option<Activity>,
    notified: bool,
}

impl MuxWindowBuilder {
    fn notify(&mut self) {
        if self.notified {
            return;
        }
        self.notified = true;
        let activity = self.activity.take().unwrap();
        let window_id = self.window_id;
        if let Some(mux) = Mux::get() {
            // If we're already on the mux thread, just send the notification
            // immediately.
            // This is super important for Wayland; if we push it to the
            // spawn queue below then the extra milliseconds of delay
            // causes it to get confused and shutdown the connection!?
            mux.notify(MuxNotification::WindowCreated(window_id));
        } else {
            promise::spawn::spawn_into_main_thread(async move {
                if let Some(mux) = Mux::get() {
                    mux.notify(MuxNotification::WindowCreated(window_id));
                    drop(activity);
                }
            })
            .detach();
        }
    }
}

impl Drop for MuxWindowBuilder {
    fn drop(&mut self) {
        self.notify();
    }
}

impl std::ops::Deref for MuxWindowBuilder {
    type Target = WindowId;

    fn deref(&self) -> &WindowId {
        &self.window_id
    }
}

impl Mux {
    pub fn new(default_domain: Option<Arc<dyn Domain>>) -> Self {
        let mut domains = HashMap::new();
        let mut domains_by_name = HashMap::new();
        if let Some(default_domain) = default_domain.as_ref() {
            domains.insert(default_domain.domain_id(), Arc::clone(default_domain));

            domains_by_name.insert(
                default_domain.domain_name().to_string(),
                Arc::clone(default_domain),
            );
        }

        Self {
            tabs: RefCell::new(HashMap::new()),
            panes: RefCell::new(HashMap::new()),
            windows: RefCell::new(HashMap::new()),
            default_domain: RefCell::new(default_domain),
            domains_by_name: RefCell::new(domains_by_name),
            domains: RefCell::new(domains),
            subscribers: RefCell::new(HashMap::new()),
            banner: RefCell::new(None),
            clients: RefCell::new(HashMap::new()),
            identity: RefCell::new(None),
            num_panes_by_workspace: RefCell::new(HashMap::new()),
        }
    }

    fn recompute_pane_count(&self) {
        let mut count = HashMap::new();
        for window in self.windows.borrow().values() {
            let workspace = window.get_workspace();
            for tab in window.iter() {
                *count.entry(workspace.to_string()).or_insert(0) += tab.count_panes();
            }
        }
        *self.num_panes_by_workspace.borrow_mut() = count;
    }

    pub fn client_had_input(&self, client_id: &ClientId) {
        if let Some(info) = self.clients.borrow_mut().get_mut(client_id) {
            info.update_last_input();
        }
    }

    pub fn record_input_for_current_identity(&self) {
        if let Some(ident) = self.identity.borrow().as_ref() {
            self.client_had_input(ident);
        }
    }

    pub fn record_focus_for_current_identity(&self, pane_id: PaneId) {
        if let Some(ident) = self.identity.borrow().as_ref() {
            self.record_focus_for_client(ident, pane_id);
        }
    }

    pub fn record_focus_for_client(&self, client_id: &ClientId, pane_id: PaneId) {
        let mut prior = None;
        if let Some(info) = self.clients.borrow_mut().get_mut(client_id) {
            prior = info.focused_pane_id;
            info.update_focused_pane(pane_id);
        }

        if prior == Some(pane_id) {
            return;
        }
        // Synthesize focus events
        if let Some(prior_id) = prior {
            if let Some(pane) = self.get_pane(prior_id) {
                pane.focus_changed(false);
            }
        }
        if let Some(pane) = self.get_pane(pane_id) {
            pane.focus_changed(true);
        }
    }

    pub fn register_client(&self, client_id: Arc<ClientId>) {
        self.clients
            .borrow_mut()
            .insert((*client_id).clone(), ClientInfo::new(client_id));
    }

    pub fn iter_clients(&self) -> Vec<ClientInfo> {
        self.clients
            .borrow()
            .values()
            .map(|info| info.clone())
            .collect()
    }

    /// Returns a list of the unique workspace names known to the mux.
    /// This is taken from all known windows.
    pub fn iter_workspaces(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .windows
            .borrow()
            .values()
            .map(|w| w.get_workspace().to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Generate a new unique workspace name
    pub fn generate_workspace_name(&self) -> String {
        let used = self.iter_workspaces();
        for candidate in names::Generator::default() {
            if !used.contains(&candidate) {
                return candidate;
            }
        }
        unreachable!();
    }

    /// Returns the effective active workspace name
    pub fn active_workspace(&self) -> String {
        self.identity
            .borrow()
            .as_ref()
            .and_then(|ident| {
                self.clients
                    .borrow()
                    .get(&ident)
                    .and_then(|info| info.active_workspace.clone())
            })
            .unwrap_or_else(|| DEFAULT_WORKSPACE.to_string())
    }

    /// Returns the effective active workspace name for a given client
    pub fn active_workspace_for_client(&self, ident: &Arc<ClientId>) -> String {
        self.clients
            .borrow()
            .get(&ident)
            .and_then(|info| info.active_workspace.clone())
            .unwrap_or_else(|| DEFAULT_WORKSPACE.to_string())
    }

    pub fn set_active_workspace_for_client(&self, ident: &Arc<ClientId>, workspace: &str) {
        let mut clients = self.clients.borrow_mut();
        if let Some(info) = clients.get_mut(&ident) {
            info.active_workspace.replace(workspace.to_string());
            self.notify(MuxNotification::ActiveWorkspaceChanged(ident.clone()));
        }
    }

    /// Assigns the active workspace name for the current identity
    pub fn set_active_workspace(&self, workspace: &str) {
        if let Some(ident) = self.identity.borrow().clone() {
            self.set_active_workspace_for_client(&ident, workspace);
        }
    }

    /// Overrides the current client identity.
    /// Returns `IdentityHolder` which will restore the prior identity
    /// when it is dropped.
    /// This can be used to change the identity for the duration of a block.
    pub fn with_identity(&self, id: Option<Arc<ClientId>>) -> IdentityHolder {
        let prior = self.replace_identity(id);
        IdentityHolder { prior }
    }

    /// Replace the identity, returning the prior identity
    pub fn replace_identity(&self, id: Option<Arc<ClientId>>) -> Option<Arc<ClientId>> {
        std::mem::replace(&mut *self.identity.borrow_mut(), id)
    }

    /// Returns the active identity
    pub fn active_identity(&self) -> Option<Arc<ClientId>> {
        self.identity.borrow().clone()
    }

    pub fn unregister_client(&self, client_id: &ClientId) {
        self.clients.borrow_mut().remove(client_id);
    }

    pub fn subscribe<F>(&self, subscriber: F)
    where
        F: Fn(MuxNotification) -> bool + 'static,
    {
        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
        self.subscribers
            .borrow_mut()
            .insert(sub_id, Box::new(subscriber));
    }

    pub fn notify(&self, notification: MuxNotification) {
        let mut subscribers = self.subscribers.borrow_mut();
        subscribers.retain(|_, notify| notify(notification.clone()));
    }

    pub fn default_domain(&self) -> Arc<dyn Domain> {
        self.default_domain
            .borrow()
            .as_ref()
            .map(Arc::clone)
            .unwrap()
    }

    pub fn set_default_domain(&self, domain: &Arc<dyn Domain>) {
        *self.default_domain.borrow_mut() = Some(Arc::clone(domain));
    }

    pub fn get_domain(&self, id: DomainId) -> Option<Arc<dyn Domain>> {
        self.domains.borrow().get(&id).cloned()
    }

    pub fn get_domain_by_name(&self, name: &str) -> Option<Arc<dyn Domain>> {
        self.domains_by_name.borrow().get(name).cloned()
    }

    pub fn add_domain(&self, domain: &Arc<dyn Domain>) {
        if self.default_domain.borrow().is_none() {
            *self.default_domain.borrow_mut() = Some(Arc::clone(domain));
        }
        self.domains
            .borrow_mut()
            .insert(domain.domain_id(), Arc::clone(domain));
        self.domains_by_name
            .borrow_mut()
            .insert(domain.domain_name().to_string(), Arc::clone(domain));
    }

    pub fn set_mux(mux: &Rc<Mux>) {
        MUX.with(|m| {
            *m.borrow_mut() = Some(Rc::clone(mux));
        });
    }

    pub fn shutdown() {
        MUX.with(|m| drop(m.borrow_mut().take()));
    }

    pub fn get() -> Option<Rc<Mux>> {
        let mut res = None;
        MUX.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Rc::clone(mux));
            }
        });
        res
    }

    pub fn get_pane(&self, pane_id: PaneId) -> Option<Rc<dyn Pane>> {
        self.panes.borrow().get(&pane_id).map(Rc::clone)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        self.tabs.borrow().get(&tab_id).map(Rc::clone)
    }

    pub fn add_pane(&self, pane: &Rc<dyn Pane>) -> Result<(), Error> {
        if self.panes.borrow().contains_key(&pane.pane_id()) {
            return Ok(());
        }

        let clipboard: Arc<dyn Clipboard> = Arc::new(MuxClipboard {
            pane_id: pane.pane_id(),
        });
        pane.set_clipboard(&clipboard);

        let downloader: Arc<dyn DownloadHandler> = Arc::new(MuxDownloader {});
        pane.set_download_handler(&downloader);

        self.panes
            .borrow_mut()
            .insert(pane.pane_id(), Rc::clone(pane));
        let pane_id = pane.pane_id();
        if let Some(reader) = pane.reader()? {
            let banner = self.banner.borrow().clone();
            thread::spawn(move || read_from_pane_pty(pane_id, banner, reader));
        }
        self.recompute_pane_count();
        self.notify(MuxNotification::PaneAdded(pane_id));
        Ok(())
    }

    pub fn add_tab_no_panes(&self, tab: &Rc<Tab>) {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));
        self.recompute_pane_count();
    }

    pub fn add_tab_and_active_pane(&self, tab: &Rc<Tab>) -> Result<(), Error> {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));
        let pane = tab
            .get_active_pane()
            .ok_or_else(|| anyhow!("tab MUST have an active pane"))?;
        self.add_pane(&pane)
    }

    fn remove_pane_internal(&self, pane_id: PaneId) {
        log::debug!("removing pane {}", pane_id);
        if let Some(pane) = self.panes.borrow_mut().remove(&pane_id).clone() {
            log::debug!("killing pane {}", pane_id);
            pane.kill();
            self.recompute_pane_count();
            self.notify(MuxNotification::PaneRemoved(pane_id));
        }
    }

    fn remove_tab_internal(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        log::debug!("remove_tab_internal tab {}", tab_id);

        let tab = self.tabs.borrow_mut().remove(&tab_id)?;

        if let Ok(mut windows) = self.windows.try_borrow_mut() {
            for w in windows.values_mut() {
                w.remove_by_id(tab_id);
            }
        }

        let mut pane_ids = vec![];
        for pos in tab.iter_panes_ignoring_zoom() {
            pane_ids.push(pos.pane.pane_id());
        }
        log::debug!("panes to remove: {pane_ids:?}");
        for pane_id in pane_ids {
            self.remove_pane_internal(pane_id);
        }
        self.recompute_pane_count();

        Some(tab)
    }

    fn remove_window_internal(&self, window_id: WindowId) {
        log::debug!("remove_window_internal {}", window_id);
        let domains: Vec<Arc<dyn Domain>> = self.domains.borrow().values().cloned().collect();
        for dom in domains {
            dom.local_window_is_closing(window_id);
        }
        let window = self.windows.borrow_mut().remove(&window_id);
        if let Some(window) = window {
            for tab in window.iter() {
                self.remove_tab_internal(tab.tab_id());
            }
            self.notify(MuxNotification::WindowRemoved(window_id));
        }
        self.recompute_pane_count();
    }

    pub fn remove_pane(&self, pane_id: PaneId) {
        self.remove_pane_internal(pane_id);
        self.prune_dead_windows();
    }

    pub fn remove_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        let tab = self.remove_tab_internal(tab_id);
        self.prune_dead_windows();
        tab
    }

    pub fn prune_dead_windows(&self) {
        if Activity::count() > 0 {
            log::trace!("prune_dead_windows: Activity::count={}", Activity::count());
            return;
        }
        let live_tab_ids: Vec<TabId> = self.tabs.borrow().keys().cloned().collect();
        let mut dead_windows = vec![];
        let dead_tab_ids: Vec<TabId>;

        {
            let mut windows = match self.windows.try_borrow_mut() {
                Ok(w) => w,
                Err(_) => {
                    // It's ok if our caller already locked it; we can prune later.
                    log::trace!("prune_dead_windows: self.windows already borrowed");
                    return;
                }
            };
            for (window_id, win) in windows.iter_mut() {
                win.prune_dead_tabs(&live_tab_ids);
                if win.is_empty() {
                    log::trace!("prune_dead_windows: window is now empty");
                    dead_windows.push(*window_id);
                }
            }

            dead_tab_ids = self
                .tabs
                .borrow()
                .iter()
                .filter_map(|(&id, tab)| if tab.is_dead() { Some(id) } else { None })
                .collect();
        }

        for tab_id in dead_tab_ids {
            log::trace!("tab {} is dead", tab_id);
            self.remove_tab_internal(tab_id);
        }

        for window_id in dead_windows {
            log::trace!("window {} is dead", window_id);
            self.remove_window_internal(window_id);
        }

        if self.is_empty() {
            log::trace!("prune_dead_windows: is_empty, send MuxNotification::Empty");
            self.notify(MuxNotification::Empty);
        } else {
            log::trace!("prune_dead_windows: not empty");
        }
    }

    pub fn kill_window(&self, window_id: WindowId) {
        self.remove_window_internal(window_id);
        self.prune_dead_windows();
    }

    pub fn get_window(&self, window_id: WindowId) -> Option<Ref<Window>> {
        if !self.windows.borrow().contains_key(&window_id) {
            return None;
        }
        Some(Ref::map(self.windows.borrow(), |windows| {
            windows.get(&window_id).unwrap()
        }))
    }

    pub fn get_window_mut(&self, window_id: WindowId) -> Option<RefMut<Window>> {
        if !self.windows.borrow().contains_key(&window_id) {
            return None;
        }
        Some(RefMut::map(self.windows.borrow_mut(), |windows| {
            windows.get_mut(&window_id).unwrap()
        }))
    }

    pub fn get_active_tab_for_window(&self, window_id: WindowId) -> Option<Rc<Tab>> {
        let window = self.get_window(window_id)?;
        window.get_active().map(Rc::clone)
    }

    pub fn new_empty_window(&self, workspace: Option<String>) -> MuxWindowBuilder {
        let window = Window::new(workspace);
        let window_id = window.window_id();
        self.windows.borrow_mut().insert(window_id, window);
        MuxWindowBuilder {
            window_id,
            activity: Some(Activity::new()),
            notified: false,
        }
    }

    pub fn add_tab_to_window(&self, tab: &Rc<Tab>, window_id: WindowId) -> anyhow::Result<()> {
        let tab_id = tab.tab_id();
        {
            let mut window = self
                .get_window_mut(window_id)
                .ok_or_else(|| anyhow!("add_tab_to_window: no such window_id {}", window_id))?;
            window.push(tab);
        }
        self.recompute_pane_count();
        self.notify(MuxNotification::TabAddedToWindow { tab_id, window_id });
        Ok(())
    }

    pub fn window_containing_tab(&self, tab_id: TabId) -> Option<WindowId> {
        for w in self.windows.borrow().values() {
            for t in w.iter() {
                if t.tab_id() == tab_id {
                    return Some(w.window_id());
                }
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.panes.borrow().is_empty()
    }

    pub fn is_workspace_empty(&self, workspace: &str) -> bool {
        *self
            .num_panes_by_workspace
            .borrow()
            .get(workspace)
            .unwrap_or(&0)
            == 0
    }

    pub fn is_active_workspace_empty(&self) -> bool {
        let workspace = self.active_workspace();
        self.is_workspace_empty(&workspace)
    }

    pub fn iter_panes(&self) -> Vec<Rc<dyn Pane>> {
        self.panes
            .borrow()
            .iter()
            .map(|(_, v)| Rc::clone(v))
            .collect()
    }

    pub fn iter_windows_in_workspace(&self, workspace: &str) -> Vec<WindowId> {
        let mut windows: Vec<WindowId> = self
            .windows
            .borrow()
            .iter()
            .filter_map(|(k, w)| {
                if w.get_workspace() == workspace {
                    Some(k)
                } else {
                    None
                }
            })
            .cloned()
            .collect();
        windows.sort();
        windows
    }

    pub fn iter_windows(&self) -> Vec<WindowId> {
        self.windows.borrow().keys().cloned().collect()
    }

    pub fn iter_domains(&self) -> Vec<Arc<dyn Domain>> {
        self.domains.borrow().values().cloned().collect()
    }

    pub fn resolve_pane_id(&self, pane_id: PaneId) -> Option<(DomainId, WindowId, TabId)> {
        let mut ids = None;
        for tab in self.tabs.borrow().values() {
            for p in tab.iter_panes_ignoring_zoom() {
                if p.pane.pane_id() == pane_id {
                    ids = Some((tab.tab_id(), p.pane.domain_id()));
                    break;
                }
            }
        }
        let (tab_id, domain_id) = ids?;
        let window_id = self.window_containing_tab(tab_id)?;
        Some((domain_id, window_id, tab_id))
    }

    pub fn domain_was_detached(&self, domain: DomainId) {
        let mut dead_panes = vec![];
        for pane in self.panes.borrow().values() {
            if pane.domain_id() == domain {
                dead_panes.push(pane.pane_id());
            }
        }

        {
            let mut windows = self.windows.borrow_mut();
            for (_, win) in windows.iter_mut() {
                for tab in win.iter() {
                    tab.kill_panes_in_domain(domain);
                }
            }
        }

        log::info!("domain detached panes: {:?}", dead_panes);
        for pane_id in dead_panes {
            self.remove_pane_internal(pane_id);
        }

        self.prune_dead_windows();
    }

    pub fn set_banner(&self, banner: Option<String>) {
        *self.banner.borrow_mut() = banner;
    }

    pub fn resolve_spawn_tab_domain(
        &self,
        // TODO: disambiguate with TabId
        pane_id: Option<PaneId>,
        domain: &config::keyassignment::SpawnTabDomain,
    ) -> anyhow::Result<Arc<dyn Domain>> {
        let domain = match domain {
            SpawnTabDomain::DefaultDomain => self.default_domain(),
            SpawnTabDomain::CurrentPaneDomain => {
                let pane_id = pane_id
                    .ok_or_else(|| anyhow!("CurrentPaneDomain used with no current pane"))?;
                let (pane_domain_id, _window_id, _tab_id) = self
                    .resolve_pane_id(pane_id)
                    .ok_or_else(|| anyhow!("pane_id {} invalid", pane_id))?;
                self.get_domain(pane_domain_id)
                    .expect("resolve_pane_id to give valid domain_id")
            }
            SpawnTabDomain::DomainId(domain_id) => self
                .get_domain(*domain_id)
                .ok_or_else(|| anyhow!("domain id {} is invalid", domain_id))?,
            SpawnTabDomain::DomainName(name) => self
                .get_domain_by_name(&name)
                .ok_or_else(|| anyhow!("domain name {} is invalid", name))?,
        };
        Ok(domain)
    }

    fn resolve_cwd(
        &self,
        command_dir: Option<String>,
        pane: Option<Rc<dyn Pane>>,
    ) -> Option<String> {
        command_dir.or_else(|| {
            match pane {
                Some(pane) => pane
                    .get_current_working_dir()
                    .and_then(|url| {
                        percent_decode_str(url.path())
                            .decode_utf8()
                            .ok()
                            .map(|path| path.into_owned())
                    })
                    .map(|path| {
                        // On Windows the file URI can produce a path like:
                        // `/C:\Users` which is valid in a file URI, but the leading slash
                        // is not liked by the windows file APIs, so we strip it off here.
                        let bytes = path.as_bytes();
                        if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' {
                            path[1..].to_owned()
                        } else {
                            path
                        }
                    }),
                None => None,
            }
        })
    }

    pub async fn split_pane(
        &self,
        // TODO: disambiguate with TabId
        pane_id: PaneId,
        request: SplitRequest,
        source: SplitSource,
        domain: config::keyassignment::SpawnTabDomain,
    ) -> anyhow::Result<(Rc<dyn Pane>, TerminalSize)> {
        let (_pane_domain_id, window_id, tab_id) = self
            .resolve_pane_id(pane_id)
            .ok_or_else(|| anyhow!("pane_id {} invalid", pane_id))?;

        let domain = self
            .resolve_spawn_tab_domain(Some(pane_id), &domain)
            .context("resolve_spawn_tab_domain")?;

        if domain.state() == DomainState::Detached {
            domain.attach(Some(window_id)).await?;
        }

        let current_pane = self
            .get_pane(pane_id)
            .ok_or_else(|| anyhow!("pane_id {} is invalid", pane_id))?;
        let term_config = current_pane.get_config();

        let source = match source {
            SplitSource::Spawn {
                command,
                command_dir,
            } => SplitSource::Spawn {
                command,
                command_dir: self.resolve_cwd(command_dir, Some(Rc::clone(&current_pane))),
            },
            other => other,
        };

        let pane = domain.split_pane(source, tab_id, pane_id, request).await?;
        if let Some(config) = term_config {
            pane.set_config(config);
        }

        // FIXME: clipboard

        let dims = pane.get_dimensions();

        let size = TerminalSize {
            cols: dims.cols,
            rows: dims.viewport_rows,
            pixel_height: 0, // FIXME: split pane pixel dimensions
            pixel_width: 0,
            dpi: dims.dpi,
        };

        Ok((pane, size))
    }

    pub async fn move_pane_to_new_tab(
        &self,
        pane_id: PaneId,
        window_id: Option<WindowId>,
        workspace_for_new_window: Option<String>,
    ) -> anyhow::Result<(Rc<Tab>, WindowId)> {
        let (_domain, _src_window, src_tab) = self
            .resolve_pane_id(pane_id)
            .ok_or_else(|| anyhow::anyhow!("pane {} not found", pane_id))?;
        let src_tab = match self.get_tab(src_tab) {
            Some(t) => t,
            None => anyhow::bail!("Invalid tab id {}", src_tab),
        };

        let window_builder;
        let (window_id, size) = if let Some(window_id) = window_id {
            let window = self
                .get_window_mut(window_id)
                .ok_or_else(|| anyhow!("window_id {} not found on this server", window_id))?;
            let tab = window
                .get_active()
                .ok_or_else(|| anyhow!("window {} has no tabs", window_id))?;
            let size = tab.get_size();

            (window_id, size)
        } else {
            window_builder = self.new_empty_window(workspace_for_new_window);
            (*window_builder, src_tab.get_size())
        };

        let pane = src_tab
            .remove_pane(pane_id)
            .ok_or_else(|| anyhow::anyhow!("pane {} wasn't in its containing tab!?", pane_id))?;

        let tab = Rc::new(Tab::new(&size));
        tab.assign_pane(&pane);
        pane.resize(size)?;
        self.add_tab_and_active_pane(&tab)?;
        self.add_tab_to_window(&tab, window_id)?;

        if src_tab.is_dead() {
            self.remove_tab(src_tab.tab_id());
        }

        Ok((tab, window_id))
    }

    pub async fn spawn_tab_or_window(
        &self,
        window_id: Option<WindowId>,
        domain: SpawnTabDomain,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        size: TerminalSize,
        current_pane_id: Option<PaneId>,
        workspace_for_new_window: String,
    ) -> anyhow::Result<(Rc<Tab>, Rc<dyn Pane>, WindowId)> {
        let domain = self
            .resolve_spawn_tab_domain(current_pane_id, &domain)
            .context("resolve_spawn_tab_domain")?;

        let window_builder;
        let term_config;

        let (window_id, size) = if let Some(window_id) = window_id {
            let window = self
                .get_window_mut(window_id)
                .ok_or_else(|| anyhow!("window_id {} not found on this server", window_id))?;
            let tab = window
                .get_active()
                .ok_or_else(|| anyhow!("window {} has no tabs", window_id))?;
            let pane = tab
                .get_active_pane()
                .ok_or_else(|| anyhow!("active tab in window {} has no panes", window_id))?;
            term_config = pane.get_config();

            let size = tab.get_size();

            (window_id, size)
        } else {
            term_config = None;
            window_builder = self.new_empty_window(Some(workspace_for_new_window));
            (*window_builder, size)
        };

        if domain.state() == DomainState::Detached {
            domain.attach(Some(window_id)).await?;
        }

        let cwd = self.resolve_cwd(
            command_dir,
            match current_pane_id {
                Some(id) => {
                    // Only use the cwd from the current pane if the domain
                    // is the same as the one we are spawning into
                    let (current_domain_id, _, _) = self
                        .resolve_pane_id(id)
                        .ok_or_else(|| anyhow!("pane_id {} invalid", id))?;
                    if current_domain_id == domain.domain_id() {
                        self.get_pane(id)
                    } else {
                        None
                    }
                }
                None => None,
            },
        );

        let tab = domain
            .spawn(size, command.clone(), cwd.clone(), window_id)
            .await
            .with_context(|| {
                format!(
                    "Spawning in domain `{}`: {size:?} command={command:?} cwd={cwd:?}",
                    domain.domain_name()
                )
            })?;

        let pane = tab
            .get_active_pane()
            .ok_or_else(|| anyhow!("missing active pane on tab!?"))?;

        if let Some(config) = term_config {
            pane.set_config(config);
        }

        // FIXME: clipboard?

        let mut window = self
            .get_window_mut(window_id)
            .ok_or_else(|| anyhow!("no such window!?"))?;
        if let Some(idx) = window.idx_by_id(tab.tab_id()) {
            window.save_and_then_set_active(idx);
        }

        Ok((tab, pane, window_id))
    }
}

pub struct IdentityHolder {
    prior: Option<Arc<ClientId>>,
}

impl Drop for IdentityHolder {
    fn drop(&mut self) {
        if let Some(mux) = Mux::get() {
            mux.replace_identity(self.prior.take());
        }
    }
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SessionTerminated {
    #[error("Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[error("Error: {:?}", err)]
    Error { err: Error },
    #[error("Window Closed")]
    WindowClosed,
}

pub(crate) fn terminal_size_to_pty_size(size: TerminalSize) -> anyhow::Result<PtySize> {
    Ok(PtySize {
        rows: size.rows.try_into()?,
        cols: size.cols.try_into()?,
        pixel_height: size.pixel_height.try_into()?,
        pixel_width: size.pixel_width.try_into()?,
    })
}

struct MuxClipboard {
    pane_id: PaneId,
}

impl Clipboard for MuxClipboard {
    fn set_contents(
        &self,
        selection: ClipboardSelection,
        clipboard: Option<String>,
    ) -> anyhow::Result<()> {
        let mux =
            Mux::get().ok_or_else(|| anyhow::anyhow!("MuxClipboard::set_contents: no Mux?"))?;
        mux.notify(MuxNotification::AssignClipboard {
            pane_id: self.pane_id,
            selection,
            clipboard,
        });
        Ok(())
    }
}

struct MuxDownloader {}

impl wezterm_term::DownloadHandler for MuxDownloader {
    fn save_to_downloads(&self, name: Option<String>, data: Vec<u8>) {
        if let Some(mux) = Mux::get() {
            mux.notify(MuxNotification::SaveToDownloads {
                name,
                data: Arc::new(data),
            });
        }
    }
}
