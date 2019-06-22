use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::front_end;
use crate::mux::domain::{Domain, DomainId};
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::server::client::Client;
use crate::server::codec::Spawn;
use crate::server::tab::ClientTab;
use failure::Fallible;
use portable_pty::{CommandBuilder, PtySize};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct ClientInner {
    pub client: Client,
    pub local_domain_id: DomainId,
    pub remote_domain_id: DomainId,
    remote_to_local_window: Mutex<HashMap<WindowId, WindowId>>,
    remote_to_local_tab: Mutex<HashMap<TabId, TabId>>,
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
        log::error!(
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
}

pub struct ClientDomain {
    inner: Arc<ClientInner>,
}

impl ClientInner {
    pub fn new(client: Client) -> Self {
        let local_domain_id = client.local_domain_id();
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
        }
    }
}

impl ClientDomain {
    pub fn new(client: Client) -> Self {
        let inner = Arc::new(ClientInner::new(client));
        Self { inner }
    }

    pub fn remote_to_local_tab_id(&self, remote_tab_id: TabId) -> Option<TabId> {
        let mut tab_map = self.inner.remote_to_local_tab.lock().unwrap();

        if let Some(id) = tab_map.get(&remote_tab_id) {
            return Some(*id);
        }

        let mux = Mux::get().unwrap();

        for tab in mux.iter_tabs() {
            if let Some(tab) = tab.downcast_ref::<ClientTab>() {
                if tab.remote_tab_id() == remote_tab_id {
                    let local_tab_id = tab.tab_id();
                    tab_map.insert(remote_tab_id, local_tab_id);
                    return Some(local_tab_id);
                }
            }
        }
        None
    }
}

impl Domain for ClientDomain {
    fn domain_id(&self) -> DomainId {
        self.inner.local_domain_id
    }

    fn spawn(
        &self,
        size: PtySize,
        command: Option<CommandBuilder>,
        window: WindowId,
    ) -> Fallible<Rc<dyn Tab>> {
        let remote_tab_id = {
            let result = self
                .inner
                .client
                .spawn(Spawn {
                    domain_id: self.inner.remote_domain_id,
                    window_id: self.inner.local_to_remote_window(window),
                    size,
                    command,
                })
                .wait()?;

            self.inner
                .record_remote_to_local_window_mapping(result.window_id, window);

            result.tab_id
        };
        let tab: Rc<dyn Tab> = Rc::new(ClientTab::new(&self.inner, remote_tab_id, size));
        let mux = Mux::get().unwrap();
        mux.add_tab(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    fn attach(&self) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let tabs = self.inner.client.list_tabs().wait()?;
        log::error!("ListTabs result {:#?}", tabs);

        for entry in tabs.tabs.iter() {
            log::error!(
                "attaching to remote tab {} in remote window {} {}",
                entry.tab_id,
                entry.window_id,
                entry.title
            );
            let tab: Rc<dyn Tab> = Rc::new(ClientTab::new(&self.inner, entry.tab_id, entry.size));
            mux.add_tab(&tab)?;

            if let Some(local_window_id) = self.inner.remote_to_local_window(entry.window_id) {
                let mut window = mux
                    .get_window_mut(local_window_id)
                    .expect("no such window!?");
                log::error!("already have a local window for this one");
                window.push(&tab);
            } else {
                log::error!("spawn new local window");
                let fonts = Rc::new(FontConfiguration::new(
                    Arc::clone(mux.config()),
                    FontSystemSelection::get_default(),
                ));
                let local_window_id = mux.new_empty_window();
                self.inner
                    .record_remote_to_local_window_mapping(entry.window_id, local_window_id);
                mux.add_tab_to_window(&tab, local_window_id)?;

                front_end()
                    .unwrap()
                    .spawn_new_window(mux.config(), &fonts, &tab, local_window_id)
                    .unwrap();
            }
        }
        Ok(())
    }
}
