use crate::config::Config;
use crate::server::codec::*;
use crate::server::UnixStream;
use failure::{bail, ensure, err_msg, Error};
use log::info;
use std::path::Path;
use std::sync::Arc;

pub struct Client {
    stream: UnixStream,
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
    pub fn new(config: &Arc<Config>) -> Result<Self, Error> {
        let sock_path = Path::new(
            config
                .mux_server_unix_domain_socket_path
                .as_ref()
                .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?,
        );
        info!("connect to {}", sock_path.display());
        let stream = UnixStream::connect(sock_path)?;
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
}
