#![allow(dead_code)]
use crate::config::Config;
use crate::server::codec::*;
use crate::server::listener::IdentitySource;
use crate::server::UnixStream;
use failure::{bail, ensure, err_msg, format_err, Error, Fallible};
use log::info;
use native_tls::TlsConnector;
use std::convert::TryInto;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

pub trait ReadAndWrite: std::io::Read + std::io::Write {}
impl ReadAndWrite for UnixStream {}
impl ReadAndWrite for native_tls::TlsStream<std::net::TcpStream> {}

pub struct Client {
    stream: Box<dyn ReadAndWrite>,
    serial: u64,
}

macro_rules! rpc {
    ($method_name:ident, $request_type:ident, $response_type:ident) => {
        pub fn $method_name(&mut self, pdu: $request_type) -> Result<$response_type, Error> {
            let result = self.send_pdu(Pdu::$request_type(pdu))?;
            match result {
                Pdu::$response_type(res) => Ok(res),
                _ => bail!("unexpected response {:?}", result),
            }
        }
    };

    // This variant allows omitting the request parameter; this is useful
    // in the case where the struct is empty and present only for the purpose
    // of typing the request.
    ($method_name:ident, $request_type:ident=(), $response_type:ident) => {
        pub fn $method_name(&mut self) -> Result<$response_type, Error> {
            let result = self.send_pdu(Pdu::$request_type($request_type{}))?;
            match result {
                Pdu::$response_type(res) => Ok(res),
                _ => bail!("unexpected response {:?}", result),
            }
        }
    };
}

impl Client {
    pub fn new_unix_domain(config: &Arc<Config>) -> Fallible<Self> {
        let sock_path = Path::new(
            config
                .mux_server_unix_domain_socket_path
                .as_ref()
                .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?,
        );
        info!("connect to {}", sock_path.display());
        let stream = Box::new(UnixStream::connect(sock_path)?);
        Ok(Self { stream, serial: 0 })
    }

    pub fn new_tls(config: &Arc<Config>) -> Fallible<Self> {
        let remote_address = config
            .mux_server_remote_address
            .as_ref()
            .ok_or_else(|| err_msg("missing mux_server_remote_address config value"))?;

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            format_err!(
                "expected mux_server_remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

        let identity = IdentitySource::PemFiles {
            key: config
                .mux_client_pem_private_key
                .as_ref()
                .ok_or_else(|| err_msg("missing mux_client_pem_private_key config value"))?
                .into(),
            cert: config.mux_client_pem_cert.clone(),
            chain: config.mux_client_pem_ca.clone(),
        };

        let connector = TlsConnector::builder()
            .identity(identity.try_into()?)
            .danger_accept_invalid_hostnames(
                config.mux_client_accept_invalid_hostnames.unwrap_or(false),
            )
            .build()?;

        let stream = TcpStream::connect(remote_address)
            .map_err(|e| format_err!("connecting to {}: {}", remote_address, e))?;

        let stream = Box::new(connector.connect(remote_host_name, stream).map_err(|e| {
            format_err!(
                "TlsConnector for {} with host name {}: {} ({:?})",
                remote_address,
                remote_host_name,
                e,
                e
            )
        })?);
        Ok(Self { stream, serial: 0 })
    }

    pub fn send_pdu(&mut self, pdu: Pdu) -> Result<Pdu, Error> {
        let serial = self.serial;
        self.serial += 1;
        pdu.encode(&mut self.stream, serial)?;
        let decoded = Pdu::decode(&mut self.stream)?;
        ensure!(
            decoded.serial == serial,
            "got out of order response (expected serial {} but got {:?}",
            serial,
            decoded
        );
        Ok(decoded.pdu)
    }

    rpc!(ping, Ping = (), Pong);
    rpc!(list_tabs, ListTabs = (), ListTabsResponse);
    rpc!(
        get_coarse_tab_renderable_data,
        GetCoarseTabRenderableData,
        GetCoarseTabRenderableDataResponse
    );
    rpc!(spawn, Spawn, SpawnResponse);
    rpc!(write_to_tab, WriteToTab, UnitResponse);
    rpc!(send_paste, SendPaste, UnitResponse);
    rpc!(key_down, SendKeyDown, UnitResponse);
    rpc!(mouse_event, SendMouseEvent, SendMouseEventResponse);
    rpc!(resize, Resize, UnitResponse);
}
