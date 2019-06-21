#![allow(dead_code)]
use crate::config::Config;
use crate::frontend::gui_executor;
use crate::mux::Mux;
use crate::server::codec::*;
use crate::server::pollable::*;
use crate::server::tab::ClientTab;
use crate::server::UnixStream;
use crossbeam_channel::TryRecvError;
use failure::{bail, err_msg, format_err, Fallible};
use log::info;
use promise::{Future, Promise};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::thread;

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

fn process_unilateral(decoded: DecodedPdu) -> Fallible<()> {
    if let Some(tab_id) = decoded.pdu.tab_id() {
        let pdu = decoded.pdu;
        Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let tab = mux
                .get_tab(tab_id)
                .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
            let client_tab = tab.downcast_ref::<ClientTab>().unwrap();
            client_tab.process_unilateral(pdu)
        });
    } else {
        bail!("don't know how to handle {:?}", decoded);
    }
    Ok(())
}

fn client_thread(
    mut stream: Box<dyn ReadAndWrite>,
    rx: PollableReceiver<ReaderMessage>,
) -> Fallible<()> {
    let mut next_serial = 1u64;
    let mut promises = HashMap::new();
    let mut read_buffer = Vec::with_capacity(1024);
    loop {
        loop {
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
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => bail!("Client was destroyed"),
            };
        }

        let mut poll_array = [rx.as_poll_fd(), stream.as_poll_fd()];
        poll_for_read(&mut poll_array);

        if poll_array[1].revents != 0 || stream.has_read_buffered() {
            // When TLS is enabled on a stream, it may require a mixture of
            // reads AND writes in order to satisfy a given read or write.
            // As a result, we may appear ready to read a PDU, but may not
            // be able to read a complete PDU.
            // Set to non-blocking mode while we try to decode a packet to
            // avoid blocking.
            loop {
                stream.set_non_blocking(true)?;
                let res = Pdu::try_read_and_decode(&mut stream, &mut read_buffer);
                stream.set_non_blocking(false)?;
                if let Some(decoded) = res? {
                    log::trace!("decoded serial {}", decoded.serial);
                    if decoded.serial == 0 {
                        process_unilateral(decoded)?;
                    } else if let Some(mut promise) = promises.remove(&decoded.serial) {
                        promise.result(Ok(decoded.pdu));
                    } else {
                        log::error!(
                            "got serial {} without a corresponding promise",
                            decoded.serial
                        );
                    }
                } else {
                    break;
                }
            }
        }
    }
}

impl Client {
    pub fn new(stream: Box<dyn ReadAndWrite>) -> Self {
        let (sender, receiver) = pollable_channel().expect("failed to create pollable_channel");

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

    #[cfg(any(feature = "openssl", unix))]
    pub fn new_tls(config: &Arc<Config>) -> Fallible<Self> {
        use crate::server::listener::read_bytes;
        use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
        use openssl::x509::X509;

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

        let mut connector = SslConnector::builder(SslMethod::tls())?;

        if let Some(cert_file) = config.mux_client_pem_cert.as_ref() {
            connector.set_certificate_file(cert_file, SslFiletype::PEM)?;
        }
        if let Some(chain_file) = config.mux_client_pem_ca.as_ref() {
            connector.set_certificate_chain_file(chain_file)?;
        }
        if let Some(key_file) = config.mux_client_pem_private_key.as_ref() {
            connector.set_private_key_file(key_file, SslFiletype::PEM)?;
        }
        if let Some(root_certs) = config.mux_pem_root_certs.as_ref() {
            fn load_cert(name: &Path) -> Fallible<X509> {
                let cert_bytes = read_bytes(name)?;
                log::trace!("loaded {}", name.display());
                Ok(X509::from_pem(&cert_bytes)?)
            }
            for name in root_certs {
                if name.is_dir() {
                    for entry in std::fs::read_dir(name)? {
                        if let Ok(cert) = load_cert(&entry?.path()) {
                            connector.cert_store_mut().add_cert(cert).ok();
                        }
                    }
                } else {
                    connector.cert_store_mut().add_cert(load_cert(name)?)?;
                }
            }
        }

        let connector = connector.build();
        let connector = connector
            .configure()?
            .verify_hostname(!config.mux_client_accept_invalid_hostnames.unwrap_or(false));

        let stream = TcpStream::connect(remote_address)
            .map_err(|e| format_err!("connecting to {}: {}", remote_address, e))?;
        stream.set_nodelay(true)?;

        let stream = Box::new(
            connector
                .connect(
                    config
                        .mux_client_expected_cn
                        .as_ref()
                        .map(String::as_str)
                        .unwrap_or(remote_host_name),
                    stream,
                )
                .map_err(|e| {
                    format_err!(
                        "SslConnector for {} with host name {}: {} ({:?})",
                        remote_address,
                        remote_host_name,
                        e,
                        e
                    )
                })?,
        );
        Ok(Self::new(stream))
    }

    #[cfg(not(any(feature = "openssl", unix)))]
    pub fn new_tls(config: &Arc<Config>) -> Fallible<Self> {
        use crate::server::listener::IdentitySource;
        use native_tls::TlsConnector;
        use std::convert::TryInto;

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
    rpc!(get_tab_render_changes, GetTabRenderChanges, UnitResponse);
}
