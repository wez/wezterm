use crate::mux::domain::{alloc_domain_id, Domain, DomainId};
use crate::mux::tab::Tab;
use crate::server::client::Client;
use crate::server::codec::Spawn;
use failure::{bail, Fallible};
use portable_pty::{CommandBuilder, PtySize};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct ClientDomain {
    client: Arc<Mutex<Client>>,
    local_domain_id: DomainId,
    remote_domain_id: DomainId,
}

impl ClientDomain {
    pub fn new(client: &Arc<Mutex<Client>>) -> Self {
        let local_domain_id = alloc_domain_id();
        // Assumption: that the domain id on the other end is
        // always the first created default domain.  In the future
        // we'll add a way to discover/enumerate domains to populate
        // this a bit rigorously.
        let remote_domain_id = 0;
        Self {
            client: Arc::clone(client),
            local_domain_id,
            remote_domain_id,
        }
    }
}

impl Domain for ClientDomain {
    fn domain_id(&self) -> DomainId {
        self.local_domain_id
    }

    fn spawn(&self, size: PtySize, command: Option<CommandBuilder>) -> Fallible<Rc<dyn Tab>> {
        let mut client = self.client.lock().unwrap();
        let remote_tab_id = client.spawn(Spawn {
            domain_id: self.remote_domain_id,
            size,
            command,
        });
        bail!("need to wrap in a tab proxy");
    }
}
