use crate::config::{configuration, TlsDomainServer};
use anyhow::{anyhow, bail, Context, Error};
use log::error;
use promise::spawn::spawn_into_main_thread;
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::thread;

mod clientsession;
mod local;
mod ossl;
mod pki;
mod sessionhandler;
pub mod umask;

lazy_static::lazy_static! {
    static ref PKI: pki::Pki = pki::Pki::init().expect("failed to initialize PKI");
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
        ossl::spawn_tls_listener(tls_server)?;
    }
    Ok(())
}
