#![allow(dead_code)]
use crate::config::Config;
use crate::server::codec::*;
use crate::server::listener::IdentitySource;
use crate::server::pollable::*;
use crate::server::UnixStream;
use crossbeam_channel::{Sender, TryRecvError};
use failure::{bail, err_msg, format_err, Fallible};
use log::info;
use native_tls::TlsConnector;
use promise::{Future, Promise};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::thread;

pub trait ReadAndWrite: std::io::Read + std::io::Write + Send + AsPollFd {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()>;
}
impl ReadAndWrite for UnixStream {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()> {
        self.set_nonblocking(non_blocking)?;
        Ok(())
    }
}
impl ReadAndWrite for native_tls::TlsStream<std::net::TcpStream> {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()> {
        self.get_ref().set_nonblocking(non_blocking)?;
        Ok(())
    }
}

enum ReaderMessage {
    SendPdu { pdu: Pdu, promise: Promise<Pdu> },
}

#[derive(Clone)]
pub struct Client {
    sender: PollableSender<ReaderMessage>,
}

macro_rules! rpc {
    ($method_name:ident, $request_type:ident, $response_type:ident) => {
        pub fn $method_name(&self, pdu: $request_type) -> Future<$response_type> {
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
        pub fn $method_name(&self) -> Future<$response_type> {
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

fn client_thread(
    mut stream: Box<dyn ReadAndWrite>,
    rx: PollableReceiver<ReaderMessage>,
    mut unilaterals: Option<Sender<Pdu>>,
) -> Fallible<()> {
    let mut next_serial = 1u64;
    let mut promises = HashMap::new();
    let mut read_buffer = Vec::with_capacity(1024);
    loop {
        let mut poll_array = [rx.as_poll_fd(), stream.as_poll_fd()];
        poll_for_read(&mut poll_array);
        log::trace!(
            "out: {}, in: {}",
            poll_array[0].revents,
            poll_array[1].revents
        );
        if poll_array[0].revents != 0 {
            match rx.try_recv() {
                Ok(msg) => match msg {
                    ReaderMessage::SendPdu { pdu, promise } => {
                        let serial = next_serial;
                        next_serial += 1;
                        promises.insert(serial, promise);

                        pdu.encode(&mut stream, serial)?;
                        stream.flush()?;
                    }
                },
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => bail!("Client was destroyed"),
            };
        }

        if poll_array[1].revents != 0 {
            // When TLS is enabled on a stream, it may require a mixture of
            // reads AND writes in order to satisfy a given read or write.
            // As a result, we may appear ready to read a PDU, but may not
            // be able to read a complete PDU.
            // Set to non-blocking mode while we try to decode a packet to
            // avoid blocking.
            stream.set_non_blocking(true)?;
            let res = Pdu::try_read_and_decode(&mut stream, &mut read_buffer);
            stream.set_non_blocking(false)?;
            if let Some(decoded) = res? {
                if decoded.serial == 0 {
                    if let Some(uni) = unilaterals.as_mut() {
                        uni.send(decoded.pdu)?;
                    } else {
                        log::error!("got unilateral, but there is no handler");
                    }
                } else if let Some(mut promise) = promises.remove(&decoded.serial) {
                    promise.result(Ok(decoded.pdu));
                } else {
                    log::error!(
                        "got serial {} without a corresponding promise",
                        decoded.serial
                    );
                }
            } else {
                log::trace!("spurious/incomplete read wakeup");
            }
        }
    }
}

impl Client {
    pub fn new(stream: Box<dyn ReadAndWrite>, unilaterals: Option<Sender<Pdu>>) -> Self {
        let (sender, receiver) = pollable_channel().expect("failed to create pollable_channel");

        thread::spawn(move || {
            if let Err(e) = client_thread(stream, receiver, unilaterals) {
                log::error!("client thread ended: {}", e);
            }
        });

        Self { sender }
    }

    pub fn new_unix_domain(
        config: &Arc<Config>,
        unilaterals: Option<Sender<Pdu>>,
    ) -> Fallible<Self> {
        let sock_path = Path::new(
            config
                .mux_server_unix_domain_socket_path
                .as_ref()
                .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?,
        );
        info!("connect to {}", sock_path.display());
        let stream = Box::new(UnixStream::connect(sock_path)?);
        Ok(Self::new(stream, unilaterals))
    }

    pub fn new_tls(config: &Arc<Config>, unilaterals: Option<Sender<Pdu>>) -> Fallible<Self> {
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
        Ok(Self::new(stream, unilaterals))
    }

    pub fn send_pdu(&self, pdu: Pdu) -> Future<Pdu> {
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
