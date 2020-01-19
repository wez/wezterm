use crate::config::{configuration, TlsDomainServer, UnixDomain};
use crate::create_user_owned_dirs;
use crate::server::UnixListener;
use anyhow::{anyhow, bail, Context, Error};
use log::{debug, error};
use native_tls::Identity;
use promise::spawn::spawn_into_main_thread;
use std::convert::TryFrom;
use std::fs::remove_file;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

mod clientsession;
mod not_ossl;
mod ossl;
mod umask;

#[cfg(not(any(feature = "openssl", unix)))]
use not_ossl as tls_impl;
#[cfg(any(feature = "openssl", unix))]
use ossl as tls_impl;

struct LocalListener {
    listener: UnixListener,
}

impl LocalListener {
    pub fn new(listener: UnixListener) -> Self {
        Self { listener }
    }

    fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    spawn_into_main_thread(async move {
                        let mut session = clientsession::ClientSession::new(stream);
                        thread::spawn(move || session.run());
                    });
                }
                Err(err) => {
                    error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum IdentitySource {
    Pkcs12File {
        path: PathBuf,
        password: String,
    },
    PemFiles {
        key: PathBuf,
        cert: Option<PathBuf>,
        chain: Option<PathBuf>,
    },
}

impl TryFrom<IdentitySource> for Identity {
    type Error = Error;

    fn try_from(source: IdentitySource) -> anyhow::Result<Identity> {
        match source {
            IdentitySource::Pkcs12File { path, password } => {
                let bytes = std::fs::read(&path)?;
                Identity::from_pkcs12(&bytes, &password)
                    .with_context(|| format!("error loading pkcs12 file '{}'", path.display()))
            }
            IdentitySource::PemFiles { key, cert, chain } => {
                tls_impl::pem_files_to_identity(key, cert, chain)
            }
        }
    }
}

/// Take care when setting up the listener socket;
/// we need to be sure that the directory that we create it in
/// is owned by the user and has appropriate file permissions
/// that prevent other users from manipulating its contents.
fn safely_create_sock_path(unix_dom: &UnixDomain) -> Result<UnixListener, Error> {
    let sock_path = &unix_dom.socket_path();
    debug!("setting up {}", sock_path.display());

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
                bail!(
                    "The permissions for {} are insecure and currently \
                     allow other users to write to it (permissions={:?})",
                    sock_dir.display(),
                    permissions
                );
            }
        }
    }

    if sock_path.exists() {
        remove_file(sock_path)?;
    }

    UnixListener::bind(sock_path)
        .with_context(|| format!("Failed to bind to {}", sock_path.display()))
}

pub fn spawn_listener() -> anyhow::Result<()> {
    let config = configuration();
    for unix_dom in &config.unix_domains {
        let mut listener = LocalListener::new(safely_create_sock_path(unix_dom)?);
        thread::spawn(move || {
            listener.run();
        });
    }

    for tls_server in &config.tls_servers {
        tls_impl::spawn_tls_listener(tls_server)?;
    }
    Ok(())
}
