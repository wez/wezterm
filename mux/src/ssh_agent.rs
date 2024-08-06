use crate::{ClientId, Mux};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_file;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;

/// AgentProxy manages an agent.PID symlink in the wezterm runtime
/// directory.
/// The intent is to maintain the symlink and have it point to the
/// appropriate ssh agent socket path for the most recently active
/// mux client.
///
/// Why symlink rather than running an agent proxy socket of our own?
/// Some agent implementations use low level unix socket operations
/// to decide whether the client process is allowed to consume
/// the agent or not, and us sitting in the middle breaks that.
///
/// As a further complication, when a wezterm proxy client is
/// present, both the proxy and the mux instance inside a gui
/// tend to be updated together, with the gui often being
/// touched last.
///
/// To deal with that we de-bounce input events and weight
/// proxy clients higher so that we can avoid thrashing
/// between gui and proxy.
///
/// The consequence of this is that there is 100ms of artificial
/// latency to detect a change in the active client.
/// This number was selected because it is unlike for a human
/// to be able to switch devices that quickly.
///
/// How is this used? The Mux::client_had_input function
/// will call AgentProxy::update_target to signal when
/// the active client may have changed.

pub struct AgentProxy {
    sock_path: PathBuf,
    current_target: RwLock<Option<Arc<ClientId>>>,
    sender: SyncSender<()>,
}

impl Drop for AgentProxy {
    fn drop(&mut self) {
        std::fs::remove_file(&self.sock_path).ok();
    }
}

fn update_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> anyhow::Result<()> {
    let original = original.as_ref();
    let link = link.as_ref();

    match symlink_file(original, link) {
        Ok(()) => Ok(()),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                std::fs::remove_file(link)
                    .with_context(|| format!("failed to remove {}", link.display()))?;
                symlink_file(original, link).with_context(|| {
                    format!(
                        "failed to create symlink {} -> {}: {err:#}",
                        link.display(),
                        original.display()
                    )
                })
            } else {
                anyhow::bail!(
                    "failed to create symlink {} -> {}: {err:#}",
                    link.display(),
                    original.display()
                );
            }
        }
    }
}

impl AgentProxy {
    pub fn new() -> Self {
        let pid = unsafe { libc::getpid() };
        let sock_path = config::RUNTIME_DIR.join(format!("agent.{pid}"));

        if let Some(inherited) = Self::default_ssh_auth_sock() {
            if let Err(err) = update_symlink(&inherited, &sock_path) {
                log::error!("failed to set {sock_path:?} to initial inherited SSH_AUTH_SOCK value of {inherited:?}: {err:#}");
            }
        }

        let (sender, receiver) = sync_channel(16);

        std::thread::spawn(move || Self::process_updates(receiver));

        Self {
            sock_path,
            current_target: RwLock::new(None),
            sender,
        }
    }

    pub fn default_ssh_auth_sock() -> Option<String> {
        match &config::configuration().default_ssh_auth_sock {
            Some(value) => Some(value.to_string()),
            None => std::env::var("SSH_AUTH_SOCK").ok(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.sock_path
    }

    pub fn update_target(&self) {
        // If the send fails, the channel is most likely
        // full, which means that the updater thread is
        // going to observe the now-current state when
        // it wakes up, so we needn't try any harder
        self.sender.try_send(()).ok();
    }

    fn process_updates(receiver: Receiver<()>) {
        while let Ok(_) = receiver.recv() {
            // De-bounce multiple input events so that we don't quickly
            // thrash between the host and proxy value
            std::thread::sleep(std::time::Duration::from_millis(100));
            while receiver.try_recv().is_ok() {}

            if let Some(mux) = Mux::try_get() {
                if let Some(agent) = &mux.agent {
                    agent.update_now();
                }
            }
        }
    }

    fn update_now(&self) {
        // Get list of clients from mux
        // Order by most recent activity
        // Take first one with auth sock -> that's the path
        // If we find none, then we print an error and drop
        // this stream.

        let mut clients = Mux::get().iter_clients();
        clients.retain(|info| info.client_id.ssh_auth_sock.is_some());

        clients.sort_by(|a, b| {
            // The biggest last_input time is most recent, so it sorts sooner.
            // However, when using a proxy into a gui mux, both the proxy and the
            // gui will update around the same time, with the gui often being
            // updated fractionally after the proxy.
            // In this situation we want the proxy to be selected, so we weight
            // proxy entries slightly higher by adding a small Duration to
            // the actual observed value.
            // `via proxy pid` is coupled with the Pdu::SetClientId logic
            // in wezterm-mux-server-impl/src/sessionhandler.rs
            const PROXY_MARKER: &str = "via proxy pid";
            let a_proxy = a.client_id.hostname.contains(PROXY_MARKER);
            let b_proxy = b.client_id.hostname.contains(PROXY_MARKER);

            fn adjust_for_proxy(time: DateTime<Utc>, is_proxy: bool) -> DateTime<Utc> {
                if is_proxy {
                    time + Duration::milliseconds(100)
                } else {
                    time
                }
            }

            let a_time = adjust_for_proxy(a.last_input, a_proxy);
            let b_time = adjust_for_proxy(b.last_input, b_proxy);

            b_time.cmp(&a_time)
        });

        log::trace!("filtered to {clients:#?}");
        match clients.get(0) {
            Some(info) => {
                let current = self.current_target.read().clone();
                let needs_update = match (current, &info.client_id) {
                    (None, _) => true,
                    (Some(prior), current) => prior != *current,
                };

                if needs_update {
                    let ssh_auth_sock = info
                        .client_id
                        .ssh_auth_sock
                        .as_ref()
                        .expect("we checked in the retain above");
                    log::trace!(
                        "Will update {} -> {ssh_auth_sock}",
                        self.sock_path.display(),
                    );
                    self.current_target.write().replace(info.client_id.clone());

                    if let Err(err) = update_symlink(ssh_auth_sock, &self.sock_path) {
                        log::error!(
                            "Problem updating {} -> {ssh_auth_sock}: {err:#}",
                            self.sock_path.display(),
                        );
                    }
                }
            }
            None => {
                if self.current_target.write().take().is_some() {
                    log::trace!("Updating agent to be bogus");
                    if let Err(err) = update_symlink(".", &self.sock_path) {
                        log::error!(
                            "Problem updating {} -> .: {err:#}",
                            self.sock_path.display()
                        );
                    }
                }
            }
        }
    }
}
