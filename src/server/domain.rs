use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::front_end;
use crate::mux::domain::{alloc_domain_id, Domain, DomainId};
use crate::mux::tab::Tab;
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
    pub client: Mutex<Client>,
    pub local_domain_id: DomainId,
    pub remote_domain_id: DomainId,
}
pub struct ClientDomain {
    inner: Arc<ClientInner>,
}

impl ClientInner {
    pub fn new(client: Client) -> Self {
        let local_domain_id = alloc_domain_id();
        // Assumption: that the domain id on the other end is
        // always the first created default domain.  In the future
        // we'll add a way to discover/enumerate domains to populate
        // this a bit rigorously.
        let remote_domain_id = 0;
        Self {
            client: Mutex::new(client),
            local_domain_id,
            remote_domain_id,
        }
    }
}

impl ClientDomain {
    pub fn new(client: Client) -> Self {
        let inner = Arc::new(ClientInner::new(client));
        Self { inner }
    }
}

impl Domain for ClientDomain {
    fn domain_id(&self) -> DomainId {
        self.inner.local_domain_id
    }

    fn spawn(&self, size: PtySize, command: Option<CommandBuilder>) -> Fallible<Rc<dyn Tab>> {
        let remote_tab_id = {
            let mut client = self.inner.client.lock().unwrap();
            client
                .spawn(Spawn {
                    domain_id: self.inner.remote_domain_id,
                    window_id: None,
                    size,
                    command,
                })?
                .tab_id
        };
        let tab: Rc<dyn Tab> = Rc::new(ClientTab::new(&self.inner, remote_tab_id));
        Mux::get().unwrap().add_tab(&tab)?;
        Ok(tab)
    }

    fn attach(&self) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let mut client = self.inner.client.lock().unwrap();
        let tabs = client.list_tabs()?;

        let mut windows = HashMap::new();

        for entry in tabs.tabs.iter() {
            log::error!("attaching to remote tab {} {}", entry.tab_id, entry.title);
            let tab: Rc<dyn Tab> = Rc::new(ClientTab::new(&self.inner, entry.tab_id));
            mux.add_tab(&tab)?;

            windows
                .entry(entry.window_id)
                .and_modify(|local_window_id| {
                    let mut window = mux
                        .get_window_mut(*local_window_id)
                        .expect("no such window!?");
                    window.push(&tab);
                })
                .or_insert_with(|| {
                    let fonts = Rc::new(FontConfiguration::new(
                        Arc::clone(mux.config()),
                        FontSystemSelection::get_default(),
                    ));
                    front_end()
                        .unwrap()
                        .spawn_new_window(mux.config(), &fonts, &tab)
                        .unwrap()
                });
        }
        Ok(())
    }
}
