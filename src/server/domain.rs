use crate::config::{SshDomain, TlsDomainClient, UnixDomain};
use crate::connui::ConnectionUI;
use crate::font::FontConfiguration;
use crate::frontend::front_end;
use crate::mux::domain::{alloc_domain_id, Domain, DomainId, DomainState};
use crate::mux::tab::{Pane, PaneId, Tab, TabId};
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::server::client::Client;
use crate::server::codec::{ListTabsResponse, Spawn};
use crate::server::tab::ClientPane;
use anyhow::{anyhow, bail};
use async_trait::async_trait;
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

        let tabs = inner.client.list_tabs().await?;
        Self::process_tab_list(inner, tabs)?;

        ui.close();
        Ok(())
    }

    fn process_tab_list(inner: Arc<ClientInner>, tabs: ListTabsResponse) -> anyhow::Result<()> {
        let mux = Mux::get().expect("to be called on main thread");
        log::debug!("ListTabs result {:#?}", tabs);

        for entry in tabs.tabs.iter() {
            let tab;

            if let Some(tab_id) = inner.remote_to_local_tab_id(entry.tab_id) {
                match mux.get_tab(tab_id) {
                    Some(t) => tab = t,
                    None => {
                        // We likely decided that we hit EOF on the tab and
                        // removed it from the mux.  Let's add it back, but
                        // with a new id.
                        inner.remove_old_tab_mapping(entry.tab_id);
                        tab = Rc::new(Tab::new(&entry.size));
                        inner.record_remote_to_local_tab_mapping(entry.tab_id, tab.tab_id());
                    }
                };
            } else {
                tab = Rc::new(Tab::new(&entry.size));
                inner.record_remote_to_local_tab_mapping(entry.tab_id, tab.tab_id());
            }

            if let Some(pane_id) = inner.remote_to_local_pane_id(entry.pane_id) {
                match mux.get_pane(pane_id) {
                    Some(_pane) => {}
                    None => {
                        // We likely decided that we hit EOF on the tab and
                        // removed it from the mux.  Let's add it back, but
                        // with a new id.
                        inner.remove_old_pane_mapping(entry.pane_id);
                        let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
                            &inner,
                            entry.pane_id,
                            entry.size,
                            &entry.title,
                        ));
                        tab.assign_pane(&pane);
                        mux.add_pane(&pane)?;
                    }
                };
            } else {
                log::info!(
                    "attaching to remote pane {} in remote window {} {}",
                    entry.pane_id,
                    entry.window_id,
                    entry.title
                );
                let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(
                    &inner,
                    entry.pane_id,
                    entry.size,
                    &entry.title,
                ));
                tab.assign_pane(&pane);
                mux.add_tab(&tab)?;
            }

            if let Some(local_window_id) = inner.remote_to_local_window(entry.window_id) {
                let mut window = mux
                    .get_window_mut(local_window_id)
                    .expect("no such window!?");
                log::info!("already have a local window for this one");
                if window.idx_by_id(tab.tab_id()).is_none() {
                    window.push(&tab);
                }
            } else {
                log::info!("spawn new local window");
                let fonts = Rc::new(FontConfiguration::new());
                let local_window_id = mux.new_empty_window();
                inner.record_remote_to_local_window_mapping(entry.window_id, local_window_id);
                mux.add_tab_to_window(&tab, local_window_id)?;

                front_end()
                    .unwrap()
                    .spawn_new_window(&fonts, &tab, local_window_id)
                    .unwrap();
            }
        }
        Ok(())
    }

    fn finish_attach(
        domain_id: DomainId,
        client: Client,
        tabs: ListTabsResponse,
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

        Self::process_tab_list(inner, tabs)?;

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
        let remote_tab_id = {
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

            result.tab_id
        };
        let pane: Rc<dyn Pane> = Rc::new(ClientPane::new(&inner, remote_tab_id, size, "wezterm"));
        let tab = Rc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        let mux = Mux::get().unwrap();
        mux.add_tab(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    async fn attach(&self) -> anyhow::Result<()> {
        let domain_id = self.local_domain_id;
        let config = self.config.clone();

        let activity = crate::frontend::activity::Activity::new();
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

                client.verify_version_compat(&ui).await?;

                ui.output_str("Version check OK!  Requesting tab list...\n");
                let tabs = client.list_tabs().await?;
                ui.output_str(&format!(
                    "Server has {} tabs.  Attaching to local UI...\n",
                    tabs.tabs.len()
                ));
                ClientDomain::finish_attach(domain_id, client, tabs)
            }
        })
        .await?;

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
