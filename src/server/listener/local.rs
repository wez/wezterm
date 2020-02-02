use crate::config::UnixDomain;
use crate::create_user_owned_dirs;
use crate::server::listener::{clientsession, umask};
use crate::server::UnixListener;
use anyhow::{anyhow, Context as _};
use promise::spawn::spawn_into_main_thread;

pub struct LocalListener {
    listener: UnixListener,
}

impl LocalListener {
    pub fn new(listener: UnixListener) -> Self {
        Self { listener }
    }

    pub fn with_domain(unix_dom: &UnixDomain) -> anyhow::Result<Self> {
        let listener = safely_create_sock_path(unix_dom)?;
        Ok(Self::new(listener))
    }

    pub fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    spawn_into_main_thread(async move {
                        let mut session = clientsession::ClientSession::new(stream);
                        std::thread::spawn(move || session.run());
                    });
                }
                Err(err) => {
                    log::error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }
}

/// Take care when setting up the listener socket;
/// we need to be sure that the directory that we create it in
/// is owned by the user and has appropriate file permissions
/// that prevent other users from manipulating its contents.
fn safely_create_sock_path(unix_dom: &UnixDomain) -> anyhow::Result<UnixListener> {
    let sock_path = &unix_dom.socket_path();
    log::debug!("setting up {}", sock_path.display());

    let _saver = umask::UmaskSaver::new();

    let sock_dir = sock_path
        .parent()
        .ok_or_else(|| anyhow!("sock_path {} has no parent dir", sock_path.display()))?;

    create_user_owned_dirs(sock_dir)?;

    #[cfg(unix)]
    {
        use crate::running_under_wsl;
        use std::os::unix::fs::PermissionsExt;

        if !running_under_wsl() && !unix_dom.skip_permissions_check {
            // Let's be sure that the ownership looks sane
            let meta = sock_dir.symlink_metadata()?;

            let permissions = meta.permissions();
            if (permissions.mode() & 0o22) != 0 {
                anyhow::bail!(
                    "The permissions for {} are insecure and currently \
                     allow other users to write to it (permissions={:?})",
                    sock_dir.display(),
                    permissions
                );
            }
        }
    }

    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }

    UnixListener::bind(sock_path)
        .with_context(|| format!("Failed to bind to {}", sock_path.display()))
}
