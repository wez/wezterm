use crate::connui::ConnectionUI;
use crate::server::client::Client;
use crate::server::tab::ClientPane;
use anyhow::{anyhow, bail};
use async_trait::async_trait;
use codec::{ListPanesResponse, Spawn, SplitPane};
use config::keyassignment::SpawnTabDomain;
use config::{SshDomain, TlsDomainClient, UnixDomain};
use mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use mux::pane::{Pane, PaneId};
use mux::tab::{SplitDirection, Tab, TabId};
use mux::window::WindowId;
use mux::Mux;
use portable_pty::{CommandBuilder, PtySize};
use promise::spawn::{join_handle_result, spawn_into_new_thread};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct ClientInner {
    pub client: Client,
    pub local_domain_id: DomainId,
    pub remote_domain_id: DomainId,
    remote_to_local_window: Mutex<HashMap<WindowId, WindowId>>,
    remote_to_local_tab: Mutex<HashMap<TabId, TabId>>,
    remote_to_local_pane: Mutex<HashMap<PaneId, PaneId>>,
}

impl ClientInner {
    fn remote_to_local_window(&self, remote_window_id: WindowId) -> Option<WindowId> {
        let map = self.remote_to_local_window.lock().unwrap();
        map.get(&remote_window_id).cloned()
    }

    fn record_remote_to_local_window_mapping(
        &self,
        remote_window_id: WindowId,
        local_window_id: WindowId,
    ) {
        let mut map = self.remote_to_local_window.lock().unwrap();
        map.insert(remote_window_id, local_window_id);
        log::info!(
            "record_remote_to_local_window_mapping: {} -> {}",
            remote_window_id,
            local_window_id
        );
    }

    fn local_to_remote_window(&self, local_window_id: WindowId) -> Option<WindowId> {
        let map = self.remote_to_local_window.lock().unwrap();
        for (remote, local) in map.iter() {
            if *local == local_window_id {
                return Some(*remote);
            }
        }
        None
    }

    pub fn remote_to_local_pane_id(&self, remote_pane_id: PaneId) -> Option<TabId> {
        let mut pane_map = self.remote_to_local_pane.lock().unwrap();

        if let Some(id) = pane_map.get(&remote_pane_id) {
            return Some(*id);
        }

        let mux = Mux::get().unwrap();

        for pane in mux.iter_panes() {
            if let Some(pane) = pane.downcast_ref::<ClientPane>() {
                if pane.remote_pane_id() == remote_pane_id {
                    let local_pane_id = pane.pane_id();
                    pane_map.insert(remote_pane_id, local_pane_id);
                    return Some(local_pane_id);
                }
            }
        }
        None
    }
    pub fn remove_old_pane_mapping(&self, remote_pane_id: PaneId) {
        let mut pane_map = self.remote_to_local_pane.lock().unwrap();
        pane_map.remove(&remote_pane_id);
    }

    pub fn remove_old_tab_mapping(&self, remote_tab_id: TabId) {
        let mut tab_map = self.remote_to_local_tab.lock().unwrap();
        tab_map.remove(&remote_tab_id);
    }

    fn record_remote_to_local_tab_mapping(&self, remote_tab_id: TabId, local_tab_id: TabId) {
        let mut map = self.remote_to_local_tab.lock().unwrap();
        map.insert(remote_tab_id, local_tab_id);
        log::info!(
            "record_remote_to_local_tab_mapping: {} -> {}",
            remote_tab_id,
            local_tab_id
        );
    }

    pub fn remote_to_local_tab_id(&self, remote_tab_id: TabId) -> Option<TabId> {
        let map = self.remote_to_local_tab.lock().unwrap();
        for (remote, local) in map.iter() {
            if *remote == remote_tab_id {
                return Some(*local);
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
pub enum ClientDomainConfig {
    Unix(UnixDomain),
    Tls(TlsDomainClient),
    Ssh(SshDomain),
}

impl ClientDomainConfig {
    pub fn name(&self) -> &str {
        match self {
            ClientDomainConfig::Unix(unix) => &unix.name,
            ClientDomainConfig::Tls(tls) => &tls.name,
            ClientDomainConfig::Ssh(ssh) => &ssh.name,
        }
    }

    pub fn label(&self) -> String {
        match self {
            ClientDomainConfig::Unix(unix) => format!("unix mux {}", unix.socket_path().display()),
            ClientDomainConfig::Tls(tls) => format!("TLS mux {}", tls.remote_address),
            ClientDomainConfig::Ssh(ssh) => {
                format!("SSH mux {}@{}", ssh.username, ssh.remote_address)
            }
        }
    }

    pub fn connect_automatically(&self) -> bool {
        match self {
            ClientDomainConfig::Unix(unix) => unix.connect_automatically,
            ClientDomainConfig::Tls(tls) => tls.connect_automatically,
            ClientDomainConfig::Ssh(ssh) => ssh.connect_automatically,
        }
    }
}

impl ClientInner {
    pub fn new(local_domain_id: DomainId, client: Client) -> Self {
        // Assumption: that the domain id on the other end is
        // always the first created default domain.  In the future
        // we'll add a way to discover/enumerate domains to populate
        // this a bit rigorously.
        let remote_domain_id = 0;
        Self {
            client,
            local_domain_id,
            remote_domain_id,
            remote_to_local_window: Mutex::new(HashMap::new()),
            remote_to_local_tab: Mutex::new(HashMap::new()),
            remote_to_local_pane: Mutex::new(HashMap::new()),
        }
    }
}

pub struct ClientDomain {
    config: ClientDomainConfig,
    label: String,
    inner: RefCell<Option<Arc<ClientInner>>>,
    local_domain_id: DomainId,
}

impl ClientDomain {
    pub fn new(config: ClientDomainConfig) -> Self {
        let local_domain_id = alloc_domain_id();
        let label = config.label();
        Self {
            config,
            label,
            inner: RefCell::new(None),
            local_domain_id,
        }
    }

    fn inner(&self) -> Option<Arc<ClientInner>> {
        self.inner.borrow().as_ref().map(|i| Arc::clone(i))
    }

    pub fn perform_detach(&self) {
        log::error!("detached domain {}", self.local_domain_id);
        self.inner.borrow_mut().take();
        let mux = Mux::get().unwrap();
        mux.domain_was_detached(self.local_domain_id);
    }

    pub fn remote_to_local_pane_id(&self, remote_pane_id: TabId) -> Option<TabId> {
        let inner = self.inner()?;
        inner.remote_to_local_pane_id(remote_pane_id)
    }

    pub fn get_client_inner_for_domain(domain_id: DomainId) -> anyhow::Result<Arc<ClientInner>> {
        let mux = Mux::get().unwrap();
        let domain = mux
            .get_domain(domain_id)
            .ok_or_else(|| anyhow!("invalid domain id {}", domain_id))?;
        let domain = domain
            .downcast_ref::<Self>()
            .ok_or_else(|| anyhow!("domain {} is not a ClientDomain", domain_id))?;

        if let Some(inner) = domain.inner() {
            Ok(inner)
        } else {
            bail!("domain has no assigned client");
        }
    }

    /// The reader in the mux may have decided to give up on one or
    /// more tabs at the time that a disconnect was detected, and
    /// it's also possible that another client connected and adjusted
    /// the set of tabs since we were connected, so we need to re-sync.
    pub async fn reattach(domain_id: DomainId, ui: ConnectionUI) -> anyhow::Result<()> {
        let inner = Self::get_client_inner_for_domain(domain_id)?;

        let panes = inner.client.list_panes().await?;
        Self::process_pane_list(inner, panes)?;

        ui.close();
        Ok(())
    }

    pub async fn resync(&self) -> anyhow::Result<()> {
        if let Some(inner) = self.inner.borrow().as_ref() {
            let panes = inner.client.list_panes().await?;
            Self::process_pane_list(Arc::clone(inner), panes)?;
        }
        Ok(())
    }

    fn process_pane_list(inner: Arc<ClientInner>, panes: ListPanesResponse) -> anyhow::Result<()> {
        let mux = Mux::get().expect("to be called on main thread");
        log::debug!("ListPanes result {:#?}", panes);

        for tabroot in panes.tabs {
            let root_size = match tabroot.root_size() {
                Some(size) => size,
                None => continue,
            };

            if let Some((remote_window_id, remote_tab_id)) = tabroot.window_and_tab_ids() {
                let tab;

                if let Some(tab_id) = inner.remote_to_local_tab_id(remote_tab_id) {
                    match mux.get_tab(tab_id) {
                        Some(t) => tab = t,
                        None => {
                            // We likely decided that we hit EOF on the tab and
                            // removed it from the mux.  Let's add it back, but
                            // with a new id.
                            inner.remove_old_tab_mapping(remote_tab_id);
                            tab = Rc::new(Tab::new(&root_size));
                            inner.record_remote_to_local_tab_mapping(remote_tab_id, tab.tab_id());
                            mux.add_tab_no_panes(&tab);
                        }
                    };
                } else {
                    tab = Rc::new(Tab::new(&root_size));
                    mux.add_tab_no_panes(&tab);
                    inner.record_remote_to_local_tab_mapping(remote_tab_id, tab.tab_id());
                }

                log::debug!("tree: {:#?}", tabroot);
                tab.sync_with_pane_tree(root_size, tabroot, |entry| {
                    if let Some(pane_id) = inner.remote_to_local_pane_id(entry.pane_id) {
                        match mux.get_pane(pane_id) {
                            Some(pane) => pane,
                            None => {
                                // We likely decided that we hit EOF on the tab and
                                // removed it from the mux.  Let's add it back, but
                                // with a new id.
                                inner.remove_old_pane_mapping(entry.pane_id);
                                let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
                                    &inner,
                                    entry.tab_id,
                                    entry.pane_id,
                                    entry.size,
                                    &entry.title,
                                ));
                                mux.add_pane(&pane).expect("failed to add pane to mux");
                                pane
                            }
                        }
                    } else {
                        let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
                            &inner,
                            entry.tab_id,
                            entry.pane_id,
                            entry.size,
                            &entry.title,
                        ));
                        log::debug!(
                            "attaching to remote pane {:?} -> local pane_id {}",
                            entry,
                            pane.pane_id()
                        );
                        mux.add_pane(&pane).expect("failed to add pane to mux");
                        pane
                    }
                });

                if let Some(local_window_id) = inner.remote_to_local_window(remote_window_id) {
                    let mut window = mux
                        .get_window_mut(local_window_id)
                        .expect("no such window!?");
                    if window.idx_by_id(tab.tab_id()).is_none() {
                        window.push(&tab);
                    }
                } else {
                    let local_window_id = mux.new_empty_window();
                    inner.record_remote_to_local_window_mapping(remote_window_id, *local_window_id);
                    mux.add_tab_to_window(&tab, *local_window_id)?;
                }
            }
        }

        Ok(())
    }

    fn finish_attach(
        domain_id: DomainId,
        client: Client,
        panes: ListPanesResponse,
    ) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let domain = mux
            .get_domain(domain_id)
            .ok_or_else(|| anyhow!("invalid domain id {}", domain_id))?;
        let domain = domain
            .downcast_ref::<Self>()
            .ok_or_else(|| anyhow!("domain {} is not a ClientDomain", domain_id))?;

        let inner = Arc::new(ClientInner::new(domain_id, client));
        *domain.inner.borrow_mut() = Some(Arc::clone(&inner));

        Self::process_pane_list(inner, panes)?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl Domain for ClientDomain {
    fn domain_id(&self) -> DomainId {
        self.local_domain_id
    }

    fn domain_name(&self) -> &str {
        self.config.name()
    }

    fn domain_label(&self) -> &str {
        &self.label
    }

    async fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        window: WindowId,
    ) -> anyhow::Result<Rc<Tab>> {
        let inner = self
            .inner()
            .ok_or_else(|| anyhow!("domain is not attached"))?;
        let result = inner
            .client
            .spawn(Spawn {
                domain_id: inner.remote_domain_id,
                window_id: inner.local_to_remote_window(window),
                size,
                command,
                command_dir,
            })
            .await?;

        inner.record_remote_to_local_window_mapping(result.window_id, window);

        let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
            &inner,
            result.tab_id,
            result.pane_id,
            size,
            "wezterm",
        ));
        let tab = Rc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        let mux = Mux::get().unwrap();
        mux.add_tab_and_active_pane(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    async fn split_pane(
        &self,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        tab_id: TabId,
        pane_id: PaneId,
        direction: SplitDirection,
    ) -> anyhow::Result<Rc<dyn Pane>> {
        let inner = self
            .inner()
            .ok_or_else(|| anyhow!("domain is not attached"))?;

        let mux = Mux::get().unwrap();

        let tab = mux
            .get_tab(tab_id)
            .ok_or_else(|| anyhow!("tab_id {} is invalid", tab_id))?;
        let local_pane = mux
            .get_pane(pane_id)
            .ok_or_else(|| anyhow!("pane_id {} is invalid", pane_id))?;
        let pane = local_pane
            .downcast_ref::<ClientPane>()
            .ok_or_else(|| anyhow!("pane_id {} is not a ClientPane", pane_id))?;

        let result = inner
            .client
            .split_pane(SplitPane {
                domain: SpawnTabDomain::CurrentPaneDomain,
                pane_id: pane.remote_tab_id,
                direction,
                command,
                command_dir,
            })
            .await?;

        let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
            &inner,
            result.tab_id,
            result.pane_id,
            result.size,
            "wezterm",
        ));

        let pane_index = match tab
            .iter_panes()
            .iter()
            .find(|p| p.pane.pane_id() == pane_id)
        {
            Some(p) => p.index,
            None => anyhow::bail!("invalid pane id {}", pane_id),
        };

        tab.split_and_insert(pane_index, direction, Rc::clone(&pane))
            .ok();

        mux.add_pane(&pane)?;

        Ok(pane)
    }

    async fn attach(&self) -> anyhow::Result<()> {
        let domain_id = self.local_domain_id;
        let config = self.config.clone();

        let activity = mux::activity::Activity::new();
        let ui = ConnectionUI::new();
        ui.title("wezterm: Connecting...");

        ui.async_run_and_log_error({
            let ui = ui.clone();
            async move {
                let mut cloned_ui = ui.clone();
                let client = join_handle_result(spawn_into_new_thread(move || match &config {
                    ClientDomainConfig::Unix(unix) => {
                        let initial = true;
                        Client::new_unix_domain(domain_id, unix, initial, &mut cloned_ui)
                    }
                    ClientDomainConfig::Tls(tls) => Client::new_tls(domain_id, tls, &mut cloned_ui),
                    ClientDomainConfig::Ssh(ssh) => Client::new_ssh(domain_id, ssh, &mut cloned_ui),
                }))
                .await?;

                ui.output_str("Checking server version\n");
                client.verify_version_compat(&ui).await?;

                ui.output_str("Version check OK!  Requesting pane list...\n");
                let panes = client.list_panes().await?;
                ui.output_str(&format!(
                    "Server has {} tabs.  Attaching to local UI...\n",
                    panes.tabs.len()
                ));
                ClientDomain::finish_attach(domain_id, client, panes)
            }
        })
        .await
        .map_err(|e| {
            ui.output_str(&format!("Error during attach: {:#}\n", e));
            e
        })?;

        ui.output_str("Attached!\n");
        drop(activity);
        ui.close();
        Ok(())
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented");
    }

    fn state(&self) -> DomainState {
        if self.inner.borrow().is_some() {
            DomainState::Attached
        } else {
            DomainState::Detached
        }
    }
}
