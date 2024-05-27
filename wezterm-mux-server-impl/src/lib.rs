use config::{ConfigHandle, SshMultiplexing};
use mux::domain::{Domain, LocalDomain};
use mux::ssh::RemoteSshDomain;
use mux::Mux;
use std::sync::Arc;
use wezterm_client::domain::{ClientDomain, ClientDomainConfig};

pub mod dispatch;
pub mod local;
pub mod pki;
pub mod sessionhandler;

fn client_domains(config: &config::ConfigHandle) -> Vec<ClientDomainConfig> {
    let mut domains = vec![];
    for unix_dom in &config.unix_domains {
        domains.push(ClientDomainConfig::Unix(unix_dom.clone()));
    }

    for ssh_dom in config.ssh_domains().into_iter() {
        if ssh_dom.multiplexing == SshMultiplexing::WezTerm {
            domains.push(ClientDomainConfig::Ssh(ssh_dom.clone()));
        }
    }

    for tls_client in &config.tls_clients {
        domains.push(ClientDomainConfig::Tls(tls_client.clone()));
    }
    domains
}

pub fn update_mux_domains(config: &ConfigHandle) -> anyhow::Result<()> {
    update_mux_domains_impl(config, false)
}

pub fn update_mux_domains_for_server(config: &ConfigHandle) -> anyhow::Result<()> {
    update_mux_domains_impl(config, true)
}

fn update_mux_domains_impl(config: &ConfigHandle, is_standalone_mux: bool) -> anyhow::Result<()> {
    let mux = Mux::get();

    for client_config in client_domains(&config) {
        if mux.get_domain_by_name(client_config.name()).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(ClientDomain::new(client_config));
        mux.add_domain(&domain);
    }

    for ssh_dom in config.ssh_domains().into_iter() {
        if ssh_dom.multiplexing != SshMultiplexing::None {
            continue;
        }

        if mux.get_domain_by_name(&ssh_dom.name).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(RemoteSshDomain::with_ssh_domain(&ssh_dom)?);
        mux.add_domain(&domain);
    }

    for wsl_dom in config.wsl_domains() {
        if mux.get_domain_by_name(&wsl_dom.name).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new_wsl(wsl_dom.clone())?);
        mux.add_domain(&domain);
    }

    for exec_dom in &config.exec_domains {
        if mux.get_domain_by_name(&exec_dom.name).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new_exec_domain(exec_dom.clone())?);
        mux.add_domain(&domain);
    }

    for serial in &config.serial_ports {
        if mux.get_domain_by_name(&serial.name).is_some() {
            continue;
        }

        let domain: Arc<dyn Domain> = Arc::new(LocalDomain::new_serial_domain(serial.clone())?);
        mux.add_domain(&domain);
    }

    if is_standalone_mux {
        if let Some(name) = &config.default_mux_server_domain {
            if let Some(dom) = mux.get_domain_by_name(name) {
                if dom.is::<ClientDomain>() {
                    anyhow::bail!("default_mux_server_domain cannot be set to a client domain!");
                }
                mux.set_default_domain(&dom);
            }
        }
    } else {
        if let Some(name) = &config.default_domain {
            if let Some(dom) = mux.get_domain_by_name(name) {
                mux.set_default_domain(&dom);
            }
        }
    }

    Ok(())
}

lazy_static::lazy_static! {
    pub static ref PKI: pki::Pki = pki::Pki::init().expect("failed to initialize PKI");
}
