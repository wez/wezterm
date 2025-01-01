use crate::PaneId;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

static CLIENT_ID: AtomicUsize = AtomicUsize::new(0);
lazy_static::lazy_static! {
    static ref EPOCH: u64 = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap().as_secs();
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClientId {
    pub hostname: String,
    pub username: String,
    pub pid: u32,
    pub epoch: u64,
    pub id: usize,
    pub ssh_auth_sock: Option<String>,
}

impl ClientId {
    pub fn new() -> Self {
        let id = CLIENT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            hostname: hostname::get()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|_| "localhost".to_string()),
            username: config::username_from_env().unwrap_or_else(|_| "somebody".to_string()),
            pid: unsafe { libc::getpid() as u32 },
            epoch: *EPOCH,
            id,
            ssh_auth_sock: crate::AgentProxy::default_ssh_auth_sock(),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct ClientInfo {
    pub client_id: Arc<ClientId>,
    /// The time this client last connected
    #[serde(with = "ts_seconds")]
    pub connected_at: DateTime<Utc>,
    /// Which workspace is active
    pub active_workspace: Option<String>,
    /// The last time we received input from this client
    #[serde(with = "ts_seconds")]
    pub last_input: DateTime<Utc>,
    /// The currently-focused pane
    pub focused_pane_id: Option<PaneId>,
}

impl ClientInfo {
    pub fn new(client_id: Arc<ClientId>) -> Self {
        Self {
            client_id,
            connected_at: Utc::now(),
            active_workspace: None,
            last_input: Utc::now(),
            focused_pane_id: None,
        }
    }

    pub fn update_last_input(&mut self) {
        self.last_input = Utc::now();
    }

    pub fn update_focused_pane(&mut self, pane_id: PaneId) {
        self.focused_pane_id.replace(pane_id);
    }
}
