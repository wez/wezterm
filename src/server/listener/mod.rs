use crate::config::{configuration, TlsDomainServer};
use anyhow::{anyhow, bail, Context, Error};
use log::error;
use native_tls::Identity;
use promise::spawn::spawn_into_main_thread;
use std::convert::TryFrom;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

mod clientsession;
mod local;
mod not_ossl;
mod ossl;
mod pki;
mod umask;

#[cfg(not(any(feature = "openssl", unix)))]
use not_ossl as tls_impl;
#[cfg(any(feature = "openssl", unix))]
use ossl as tls_impl;

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

pub fn spawn_listener() -> anyhow::Result<()> {
    let config = configuration();
    for unix_dom in &config.unix_domains {
        let mut listener = local::LocalListener::with_domain(unix_dom)?;
        thread::spawn(move || {
            listener.run();
        });
    }

    for tls_server in &config.tls_servers {
        tls_impl::spawn_tls_listener(tls_server)?;
    }
    Ok(())
}
