use crate::config::Config;
use crate::server::codec::*;
use crate::server::UnixStream;
use failure::{err_msg, Error};
use std::path::Path;
use std::sync::Arc;

pub struct Client {
    stream: UnixStream,
    serial: u64,
}

impl Client {
    pub fn new(config: &Arc<Config>) -> Result<Self, Error> {
        let sock_path = Path::new(
            config
                .mux_server_unix_domain_socket_path
                .as_ref()
                .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?,
        );
        eprintln!("connect to {}", sock_path.display());
        let stream = UnixStream::connect(sock_path)?;
        Ok(Self { stream, serial: 0 })
    }

    pub fn ping(&mut self) -> Result<(), Error> {
        let ping_serial = self.serial;
        self.serial += 1;
        Pdu::Ping(Ping {
            serial: ping_serial,
        })
        .encode(&mut self.stream)?;
        let pdu = Pdu::decode(&mut self.stream)?;
        match pdu {
            Pdu::Pong(Pong { serial }) if serial == ping_serial => Ok(()),
            Pdu::Pong(Pong { serial }) => bail!(
                "expected pong with serial {} but got {}",
                ping_serial,
                serial
            ),
            _ => bail!("expected Pong response, got {:?}", pdu),
        }
    }
}
