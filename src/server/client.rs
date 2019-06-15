#![allow(dead_code)]
use crate::config::Config;
use crate::server::codec::*;
use crate::server::listener::IdentitySource;
use crate::server::UnixStream;
use failure::{bail, err_msg, format_err, Fallible};
use log::info;
use native_tls::TlsConnector;
use promise::{Future, Promise};
use std::collections::HashMap;
use std::convert::TryInto;
use std::net::TcpStream;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

pub trait ReadAndWrite: std::io::Read + std::io::Write + Send {}
impl ReadAndWrite for UnixStream {}
impl ReadAndWrite for native_tls::TlsStream<std::net::TcpStream> {}

enum ReaderMessage {
    SendPdu { pdu: Pdu, promise: Promise<Pdu> },
}

pub struct Client {
    sender: Sender<ReaderMessage>,
}

macro_rules! rpc {
    ($method_name:ident, $request_type:ident, $response_type:ident) => {
        pub fn $method_name(&mut self, pdu: $request_type) -> Future<$response_type> {
            self.send_pdu(Pdu::$request_type(pdu)).then(|result| {
            match result {
                Ok(Pdu::$response_type(res)) => Ok(res),
                Ok(_) => bail!("unexpected response {:?}", result),
                Err(err) => Err(err),
            }
        })
        }
    };

    // This variant allows omitting the request parameter; this is useful
    // in the case where the struct is empty and present only for the purpose
    // of typing the request.
    ($method_name:ident, $request_type:ident=(), $response_type:ident) => {
        pub fn $method_name(&mut self) -> Future<$response_type> {
            self.send_pdu(Pdu::$request_type($request_type{})).then(|result| {
            match result {
                Ok(Pdu::$response_type(res)) => Ok(res),
                Ok(_) => bail!("unexpected response {:?}", result),
                Err(err) => Err(err),
            }
            })
        }
    };
}

fn client_thread(mut stream: Box<dyn ReadAndWrite>, rx: Receiver<ReaderMessage>) -> Fallible<()> {
    let mut next_serial = 0u64;
    let mut promises = HashMap::new();
    loop {
        let msg = if promises.is_empty() {
            // If we don't have any results to read back, then we can and
            // should block on an incoming request, otherwise we'll busy
            // wait in this loop
            match rx.recv() {
                Ok(msg) => Some(msg),
                Err(err) => bail!("Client was destroyed: {}", err),
            }
        } else {
            match rx.try_recv() {
                Ok(msg) => Some(msg),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => bail!("Client was destroyed"),
            }
        };
        if let Some(msg) = msg {
            match msg {
                ReaderMessage::SendPdu { pdu, promise } => {
                    let serial = next_serial;
                    next_serial += 1;
                    promises.insert(serial, promise);

                    pdu.encode(&mut stream, serial)?;
                    stream.flush()?;
                }
            }
        }

        if !promises.is_empty() {
            let decoded = Pdu::decode(&mut stream)?;
            if let Some(mut promise) = promises.remove(&decoded.serial) {
                promise.result(Ok(decoded.pdu));
            } else {
                log::error!(
                    "got serial {} without a corresponding promise",
                    decoded.serial
                );
            }
        }
    }
}

impl Client {
    pub fn new(stream: Box<dyn ReadAndWrite>) -> Self {
        let (sender, receiver) = channel();

        thread::spawn(move || {
            if let Err(e) = client_thread(stream, receiver) {
                log::error!("client thread ended: {}", e);
            }
        });

        Self { sender }
    }

    pub fn new_unix_domain(config: &Arc<Config>) -> Fallible<Self> {
        let sock_path = Path::new(
            config
                .mux_server_unix_domain_socket_path
                .as_ref()
                .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?,
        );
        info!("connect to {}", sock_path.display());
        let stream = Box::new(UnixStream::connect(sock_path)?);
        Ok(Self::new(stream))
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
        stream.set_nodelay(true)?;

        let stream = Box::new(connector.connect(remote_host_name, stream).map_err(|e| {
            format_err!(
                "TlsConnector for {} with host name {}: {} ({:?})",
                remote_address,
                remote_host_name,
                e,
                e
            )
        })?);
        Ok(Self::new(stream))
    }

    pub fn send_pdu(&mut self, pdu: Pdu) -> Future<Pdu> {
        let mut promise = Promise::new();
        let future = promise.get_future().expect("future already taken!?");
        match self.sender.send(ReaderMessage::SendPdu { pdu, promise }) {
            Ok(_) => future,
            Err(err) => Future::err(format_err!("{}", err)),
        }
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
    rpc!(
        get_tab_render_changes,
        GetTabRenderChanges,
        GetTabRenderChangesResponse
    );
}
