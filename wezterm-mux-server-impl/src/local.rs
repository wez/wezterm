use anyhow::{anyhow, Context as _};
use config::{create_user_owned_dirs, UnixDomain};
use promise::spawn::spawn_into_main_thread;
use wezterm_uds::UnixListener;

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
                        crate::dispatch::process(stream).await.map_err(|e| {
                            log::error!("{:#}", e);
                            e
                        })
                    })
                    .detach();
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
    log::trace!("setting up {}", sock_path.display());

    let sock_dir = sock_path
        .parent()
        .ok_or_else(|| anyhow!("sock_path {} has no parent dir", sock_path.display()))?;

    create_user_owned_dirs(sock_dir)?;

    #[cfg(unix)]
    {
        use config::running_under_wsl;
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

    // We want to remove the socket if it exists.
    // However, on windows, we can't tell if the unix domain socket
    // exists using the methods on Path, so instead we just unconditionally
    // remove it and see what error occurs.
    match std::fs::remove_file(sock_path) {
        Ok(_) => {}
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {}
            _ => return Err(err).context(format!("Unable to remove {}", sock_path.display())),
        },
    }

    let listener = UnixListener::bind(sock_path)
        .with_context(|| format!("Failed to bind to {}", sock_path.display()))?;

    config::set_sticky_bit(&sock_path);

    Ok(listener)
}
