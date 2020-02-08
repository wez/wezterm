use crate::config::{configuration, SshDomain, TlsDomainClient, UnixDomain};
use crate::connui::ConnectionUI;
use crate::mux::domain::alloc_domain_id;
use crate::mux::domain::DomainId;
use crate::mux::Mux;
use crate::server::codec::*;
use crate::server::domain::{ClientDomain, ClientDomainConfig};
use crate::server::pollable::*;
use crate::server::tab::ClientTab;
use crate::server::UnixStream;
use crate::ssh::ssh_connect_with_ui;
use anyhow::{anyhow, bail, Context, Error};
use crossbeam::channel::TryRecvError;
use filedescriptor::{pollfd, AsRawSocketDescriptor};
use log::info;
use portable_pty::{CommandBuilder, NativePtySystem, PtySystem};
use promise::{Future, Promise};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

enum ReaderMessage {
    SendPdu { pdu: Pdu, promise: Promise<Pdu> },
}

#[derive(Clone)]
pub struct Client {
    sender: PollableSender<ReaderMessage>,
    local_domain_id: DomainId,
}

macro_rules! rpc {
    ($method_name:ident, $request_type:ident, $response_type:ident) => {
        pub async fn $method_name(&self, pdu: $request_type) -> anyhow::Result<$response_type> {
            let start = std::time::Instant::now();
            let result = self.send_pdu(Pdu::$request_type(pdu)).await;
            let elapsed = start.elapsed();
            metrics::value!("rpc", elapsed, "method" => stringify!($method_name));
            match result {
                Ok(Pdu::$response_type(res)) => Ok(res),
                Ok(_) => bail!("unexpected response {:?}", result),
                Err(err) => Err(err),
            }
        }
    };

    // This variant allows omitting the request parameter; this is useful
    // in the case where the struct is empty and present only for the purpose
    // of typing the request.
    ($method_name:ident, $request_type:ident=(), $response_type:ident) => {
        #[allow(dead_code)]
        pub async fn $method_name(&self) -> anyhow::Result<$response_type> {
            let start = std::time::Instant::now();
            let result = self.send_pdu(Pdu::$request_type($request_type{})).await;
            let elapsed = start.elapsed();
            metrics::value!("rpc", elapsed, "method" => stringify!($method_name));
            match result {
                Ok(Pdu::$response_type(res)) => Ok(res),
                Ok(_) => bail!("unexpected response {:?}", result),
                Err(err) => Err(err),
            }
        }
    };
}

fn process_unilateral(local_domain_id: DomainId, decoded: DecodedPdu) -> anyhow::Result<()> {
    if let Some(tab_id) = decoded.pdu.tab_id() {
        let pdu = decoded.pdu;
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().unwrap();
            let client_domain = mux
                .get_domain(local_domain_id)
                .ok_or_else(|| anyhow!("no such domain {}", local_domain_id))?;
            let client_domain = client_domain
                .downcast_ref::<ClientDomain>()
                .ok_or_else(|| {
                    anyhow!("domain {} is not a ClientDomain instance", local_domain_id)
                })?;

            let local_tab_id = client_domain
                .remote_to_local_tab_id(tab_id)
                .ok_or_else(|| anyhow!("remote tab id {} does not have a local tab id", tab_id))?;
            let tab = mux
                .get_tab(local_tab_id)
                .ok_or_else(|| anyhow!("no such tab {}", local_tab_id))?;
            let client_tab = tab.downcast_ref::<ClientTab>().ok_or_else(|| {
                log::error!(
                    "received unilateral PDU for tab {} which is \
                     not an instance of ClientTab: {:?}",
                    local_tab_id,
                    pdu
                );
                anyhow!(
                    "received unilateral PDU for tab {} which is \
                     not an instance of ClientTab: {:?}",
                    local_tab_id,
                    pdu
                )
            })?;
            client_tab.process_unilateral(pdu)
        });
    } else {
        bail!("don't know how to handle {:?}", decoded);
    }
    Ok(())
}

fn client_thread(
    reconnectable: &mut Reconnectable,
    local_domain_id: DomainId,
    rx: &mut PollableReceiver<ReaderMessage>,
) -> anyhow::Result<()> {
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

                        pdu.encode(reconnectable.stream(), serial)
                            .context("encoding a PDU to send to the server")?;
                        reconnectable
                            .stream()
                            .flush()
                            .context("flushing PDU to server")?;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    for (_, mut promise) in promises.into_iter() {
                        promise.result(Err(anyhow!("Client was destroyed")));
                    }
                    bail!("Client was destroyed");
                }
            };
        }

        let mut poll_array = [rx.as_poll_fd(), reconnectable.stream().as_poll_fd()];
        if !reconnectable.stream().has_read_buffered() {
            poll_for_read(&mut poll_array);
        }

        if poll_array[1].revents != 0 || reconnectable.stream().has_read_buffered() {
            // When TLS is enabled on a stream, it may require a mixture of
            // reads AND writes in order to satisfy a given read or write.
            // As a result, we may appear ready to read a PDU, but may not
            // be able to read a complete PDU.
            // Set to non-blocking mode while we try to decode a packet to
            // avoid blocking.
            loop {
                reconnectable.stream().set_non_blocking(true)?;
                let res = Pdu::try_read_and_decode(reconnectable.stream(), &mut read_buffer);
                reconnectable.stream().set_non_blocking(false)?;
                match res {
                    Ok(None) => {
                        /* no data available right now; try again later! */
                        break;
                    }
                    Ok(Some(decoded)) => {
                        log::trace!("decoded serial {}", decoded.serial);
                        if decoded.serial == 0 {
                            process_unilateral(local_domain_id, decoded)
                                .context("processing unilateral PDU from server")?;
                        } else if let Some(mut promise) = promises.remove(&decoded.serial) {
                            promise.result(Ok(decoded.pdu));
                        } else {
                            log::error!(
                                "got serial {} without a corresponding promise",
                                decoded.serial
                            );
                        }
                        break;
                    }
                    Err(err) => {
                        let reason = format!("Error while decoding response pdu: {}", err);
                        log::error!("{}", reason);
                        for (_, mut promise) in promises.into_iter() {
                            promise.result(Err(anyhow!("{}", reason)));
                        }
                        bail!(reason);
                    }
                }
            }
        }
    }
}

pub fn unix_connect_with_retry(path: &Path) -> Result<UnixStream, std::io::Error> {
    let mut error = std::io::Error::last_os_error();

    for iter in 0..10 {
        if iter > 0 {
            std::thread::sleep(std::time::Duration::from_millis(iter * 10));
        }
        match UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(err) => error = err,
        }
    }

    Err(error)
}

struct Reconnectable {
    config: ClientDomainConfig,
    stream: Option<Box<dyn ReadAndWrite>>,
    tls_creds: Option<GetTlsCredsResponse>,
}

struct SshStream {
    chan: ssh2::Channel,
    sess: ssh2::Session,
}

// This is a bit horrible, but is needed because the Channel type embeds
// a raw pointer to chan and that trips the borrow checker.
// Since we move both the session and channel together, it is safe
// to mark SshStream as Send.
unsafe impl Send for SshStream {}

impl SshStream {
    fn process_stderr(&mut self) {
        let blocking = self.sess.is_blocking();
        self.sess.set_blocking(false);

        loop {
            let mut buf = [0u8; 1024];
            match self.chan.stderr().read(&mut buf) {
                Ok(size) => {
                    if size == 0 {
                        break;
                    } else {
                        let stderr = &buf[0..size];
                        log::error!("ssh stderr: {}", String::from_utf8_lossy(stderr));
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        log::error!("ssh error reading stderr: {}", e);
                    }
                    break;
                }
            }
        }

        self.sess.set_blocking(blocking);
    }
}

impl Read for SshStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        // Take the opportunity to read and show data from stderr
        self.process_stderr();
        self.chan.read(buf)
    }
}

impl AsPollFd for SshStream {
    fn as_poll_fd(&self) -> pollfd {
        self.sess
            .tcp_stream()
            .as_ref()
            .unwrap()
            .as_socket_descriptor()
            .as_poll_fd()
    }
}

impl Write for SshStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.chan.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.chan.flush()
    }
}

impl ReadAndWrite for SshStream {
    fn set_non_blocking(&self, non_blocking: bool) -> anyhow::Result<()> {
        self.sess.set_blocking(!non_blocking);
        Ok(())
    }

    fn has_read_buffered(&self) -> bool {
        false
    }
}

impl Reconnectable {
    fn new(config: ClientDomainConfig, stream: Option<Box<dyn ReadAndWrite>>) -> Self {
        Self {
            config,
            stream,
            tls_creds: None,
        }
    }

    fn tls_creds_path(&self) -> anyhow::Result<PathBuf> {
        let path = crate::config::pki_dir()?.join(self.config.name());
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    fn tls_creds_ca_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.tls_creds_path()?.join("ca.pem"))
    }

    fn tls_creds_cert_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.tls_creds_path()?.join("cert.pem"))
    }

    // Clippy thinks we should return &ReadAndWrite here, but the caller
    // needs to know the size of the returned type in a number of situations,
    // so suppress that lint
    #[allow(clippy::borrowed_box)]
    fn stream(&mut self) -> &mut Box<dyn ReadAndWrite> {
        self.stream.as_mut().unwrap()
    }

    fn reconnectable(&mut self) -> bool {
        match &self.config {
            // It doesn't make sense to reconnect to a unix socket; we only
            // get disconnected it it dies, so respawning it would not preserve
            // the set of tabs and we'd have confusing and inconsistent state
            ClientDomainConfig::Unix(_) => false,
            ClientDomainConfig::Tls(_) => true,
            // It *does* make sense to reconnect with an ssh session, but we
            // need to grow some smarts about whether the disconnect was because
            // we sent CTRL-D to close the last session, or whether it was a network
            // level disconnect, because we will otherwise throw up authentication
            // dialogs that would be annoying
            ClientDomainConfig::Ssh(_) => false,
        }
    }

    fn connect(&mut self, initial: bool, ui: &mut ConnectionUI) -> anyhow::Result<()> {
        match self.config.clone() {
            ClientDomainConfig::Unix(unix_dom) => self.unix_connect(unix_dom, initial, ui),
            ClientDomainConfig::Tls(tls) => self.tls_connect(tls, initial, ui),
            ClientDomainConfig::Ssh(ssh) => self.ssh_connect(ssh, initial, ui),
        }
    }

    /// If debugging on wez's machine, use a path specific to that machine.
    fn wezterm_bin_path() -> &'static str {
        if !configuration().use_local_build_for_proxy {
            "wezterm"
        } else if cfg!(debug_assertions) {
            "/home/wez/wez-personal/wezterm/target/debug/wezterm"
        } else {
            "/home/wez/wez-personal/wezterm/target/release/wezterm"
        }
    }

    fn ssh_connect(
        &mut self,
        ssh_dom: SshDomain,
        initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<()> {
        let sess = ssh_connect_with_ui(&ssh_dom.remote_address, &ssh_dom.username, ui)?;
        sess.set_timeout(ssh_dom.timeout.as_secs().try_into()?);

        let mut chan = sess.channel_session()?;

        let proxy_bin = Self::wezterm_bin_path();

        let cmd = if initial {
            format!("{} cli proxy", proxy_bin)
        } else {
            format!("{} cli --no-auto-start proxy", proxy_bin)
        };
        ui.output_str(&format!("Running: {}\n", cmd));
        log::error!("going to run {}", cmd);
        chan.exec(&cmd)?;

        let stream: Box<dyn ReadAndWrite> = Box::new(SshStream { sess, chan });
        self.stream.replace(stream);
        Ok(())
    }

    fn unix_connect(
        &mut self,
        unix_dom: UnixDomain,
        initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<()> {
        let sock_path = unix_dom.socket_path();
        ui.output_str(&format!("Connect to {}\n", sock_path.display()));
        info!("connect to {}", sock_path.display());

        let stream = match unix_connect_with_retry(&sock_path) {
            Ok(stream) => stream,
            Err(e) => {
                if unix_dom.no_serve_automatically || !initial {
                    bail!("failed to connect to {}: {}", sock_path.display(), e);
                }
                log::error!(
                    "While connecting to {}: {}.  Will try spawning the server.",
                    sock_path.display(),
                    e
                );
                ui.output_str(&format!("Error: {}.  Will try spawning server.\n", e));

                let argv = unix_dom.serve_command()?;

                // We need to use a pty to spawn the command because,
                // on Windows, when spawned from the gui with no pre-existing
                // conhost.exe, `wsl.exe` will fail to start up correctly.
                // This also has a nice side effect of not flashing up a
                // console window when we first spin up the wsl instance.
                let pty_system = NativePtySystem::default();
                let pair = pty_system.openpty(Default::default())?;
                let mut cmd = CommandBuilder::new(&argv[0]);
                cmd.args(&argv[1..]);
                let mut child = pair.slave.spawn_command(cmd)?;
                let status = child.wait()?;
                if !status.success() {
                    log::error!("{:?} failed with status {:?}", argv, status);
                }
                drop(child);
                drop(pair.slave);

                unix_connect_with_retry(&sock_path)
                    .with_context(|| format!("failed to connect to {}", sock_path.display()))?
            }
        };

        ui.output_str("Connected!\n");
        stream.set_read_timeout(Some(unix_dom.read_timeout))?;
        stream.set_write_timeout(Some(unix_dom.write_timeout))?;
        let stream: Box<dyn ReadAndWrite> = Box::new(stream);
        self.stream.replace(stream);
        Ok(())
    }

    pub fn tls_connect(
        &mut self,
        tls_client: TlsDomainClient,
        _initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<()> {
        use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
        use openssl::x509::X509;

        openssl::init();

        let remote_address = &tls_client.remote_address;

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            anyhow!(
                "expected mux_server_remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

        if let Some(Ok(ssh_params)) = tls_client.ssh_parameters() {
            if self.tls_creds.is_none() {
                // We need to bootstrap via an ssh session
                let sess =
                    ssh_connect_with_ui(&ssh_params.host_and_port, &ssh_params.username, ui)?;
                let mut chan = sess.channel_session()?;

                // The `tlscreds` command will start the server if needed and then
                // obtain client credentials that we can use for tls.
                let cmd = format!("{} cli tlscreds", Self::wezterm_bin_path());
                ui.output_str(&format!("Running: {}\n", cmd));
                chan.exec(&cmd)
                    .with_context(|| format!("executing `{}` on remote host", cmd))?;

                // stdout holds an encoded pdu
                let mut buf = Vec::new();
                chan.read_to_end(&mut buf)
                    .context("reading tlscreds response to buffer")?;

                // stderr is ideally empty
                let mut err = String::new();
                chan.stderr()
                    .read_to_string(&mut err)
                    .context("reading tlscreds stderr")?;
                if !err.is_empty() {
                    log::error!("remote: `{}` stderr -> `{}`", cmd, err);
                }

                let creds = match Pdu::decode(buf.as_slice())
                    .with_context(|| format!("reading tlscreds response. stderr={}", err))?
                    .pdu
                {
                    Pdu::GetTlsCredsResponse(creds) => creds,
                    _ => bail!("unexpected response to tlscreds, stderr={}", err),
                };

                // Save the credentials to disk, as that is currently the easiest
                // way to get them into openssl.  Ideally we'd keep these entirely
                // in memory.
                std::fs::write(&self.tls_creds_ca_path()?, creds.ca_cert_pem.as_bytes())?;
                std::fs::write(
                    &self.tls_creds_cert_path()?,
                    creds.client_cert_pem.as_bytes(),
                )?;
                self.tls_creds.replace(creds);
            }
        }

        let mut connector = SslConnector::builder(SslMethod::tls())?;

        let cert_file = match tls_client.pem_cert.clone() {
            Some(cert) => cert,
            None if self.tls_creds.is_some() => self.tls_creds_cert_path()?,
            None => bail!("no pem_cert configured"),
        };

        connector
            .set_certificate_file(&cert_file, SslFiletype::PEM)
            .context(format!(
                "set_certificate_file to {} for TLS client",
                cert_file.display()
            ))?;

        if let Some(chain_file) = tls_client.pem_ca.as_ref() {
            connector
                .set_certificate_chain_file(&chain_file)
                .context(format!(
                    "set_certificate_chain_file to {} for TLS client",
                    chain_file.display()
                ))?;
        }

        let key_file = match tls_client.pem_private_key.clone() {
            Some(key) => key,
            None if self.tls_creds.is_some() => self.tls_creds_cert_path()?,
            None => bail!("no pem_private_key configured"),
        };
        connector
            .set_private_key_file(&key_file, SslFiletype::PEM)
            .context(format!(
                "set_private_key_file to {} for TLS client",
                key_file.display()
            ))?;

        fn load_cert(name: &Path) -> anyhow::Result<X509> {
            let cert_bytes = std::fs::read(name)?;
            log::trace!("loaded {}", name.display());
            Ok(X509::from_pem(&cert_bytes)?)
        }
        for name in &tls_client.pem_root_certs {
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

        if self.tls_creds.is_some() {
            connector
                .cert_store_mut()
                .add_cert(load_cert(&self.tls_creds_ca_path()?)?)?;
        }

        let connector = connector.build();
        let connector = connector
            .configure()?
            .verify_hostname(!tls_client.accept_invalid_hostnames);

        ui.output_str(&format!("Connecting to {} using TLS\n", remote_address));
        let stream = TcpStream::connect(remote_address)
            .with_context(|| format!("connecting to {}", remote_address))?;
        stream.set_nodelay(true)?;
        stream.set_write_timeout(Some(tls_client.write_timeout))?;
        stream.set_read_timeout(Some(tls_client.read_timeout))?;

        let stream = Box::new(
            connector
                .connect(
                    tls_client
                        .expected_cn
                        .as_ref()
                        .map(String::as_str)
                        .unwrap_or(remote_host_name),
                    stream,
                )
                .with_context(|| {
                    format!(
                        "SslConnector for {} with host name {}",
                        remote_address, remote_host_name,
                    )
                })?,
        );
        ui.output_str("TLS Connected!\n");
        self.stream.replace(stream);
        Ok(())
    }
}

impl Client {
    fn new(local_domain_id: DomainId, mut reconnectable: Reconnectable) -> Self {
        let (sender, mut receiver) = pollable_channel().expect("failed to create pollable_channel");

        thread::spawn(move || {
            const BASE_INTERVAL: Duration = Duration::from_secs(1);
            const MAX_INTERVAL: Duration = Duration::from_secs(10);

            let mut backoff = BASE_INTERVAL;
            loop {
                if let Err(e) = client_thread(&mut reconnectable, local_domain_id, &mut receiver) {
                    if !reconnectable.reconnectable() {
                        log::debug!("client thread ended: {}", e);
                        break;
                    }

                    let mut ui = ConnectionUI::new();
                    ui.title("wezterm: Reconnecting...");

                    ui.output_str(&format!(
                        "client disconnected {}; will reconnect in {:?}\n",
                        e, backoff
                    ));

                    loop {
                        std::thread::sleep(backoff);
                        match reconnectable.connect(false, &mut ui) {
                            Ok(_) => {
                                backoff = BASE_INTERVAL;
                                log::error!("Reconnected!");
                                promise::spawn::spawn_into_main_thread(async move {
                                    ClientDomain::reattach(local_domain_id, ui).await.ok();
                                });
                                break;
                            }
                            Err(err) => {
                                backoff = (backoff + backoff).min(MAX_INTERVAL);
                                ui.output_str(&format!(
                                    "problem reconnecting: {}; will reconnect in {:?}\n",
                                    err, backoff
                                ));
                            }
                        }
                    }
                } else {
                    log::error!("client_thread returned without any error condition");
                    break;
                }
            }

            async fn detach(local_domain_id: DomainId) -> anyhow::Result<()> {
                let mux = Mux::get().unwrap();
                let client_domain = mux
                    .get_domain(local_domain_id)
                    .ok_or_else(|| anyhow!("no such domain {}", local_domain_id))?;
                let client_domain =
                    client_domain
                        .downcast_ref::<ClientDomain>()
                        .ok_or_else(|| {
                            anyhow!("domain {} is not a ClientDomain instance", local_domain_id)
                        })?;
                client_domain.perform_detach();
                Ok(())
            }
            promise::spawn::spawn_into_main_thread(async move {
                detach(local_domain_id).await.ok();
            });
        });

        Self {
            sender,
            local_domain_id,
        }
    }

    pub async fn verify_version_compat(&self, ui: &ConnectionUI) -> anyhow::Result<()> {
        match self.get_codec_version(GetCodecVersion {}).await {
            Ok(info) if info.codec_vers == CODEC_VERSION => {
                log::info!(
                    "Server version is {} (codec version {})",
                    info.version_string,
                    info.codec_vers
                );
                Ok(())
            }
            Ok(info) => {
                let msg = format!(
                    "Please install the same version of wezterm on both \
                     the client and server! \
                     The server verson is {} (codec version {}), which is not \
                     compatible with our version {} (codec version {}).",
                    info.version_string,
                    info.codec_vers,
                    crate::wezterm_version(),
                    CODEC_VERSION
                );
                ui.output_str(&msg);
                bail!("{}", msg);
            }
            Err(err) => {
                let msg = format!(
                    "Please install the same version of wezterm on both \
                     the client and server! \
                     The server reported error {} while being asked for its \
                     version.  This likely means that the server is older \
                     than the client.",
                    err
                );
                ui.output_str(&msg);
                bail!("{}", msg);
            }
        }
    }

    #[allow(dead_code)]
    pub fn local_domain_id(&self) -> DomainId {
        self.local_domain_id
    }

    pub fn new_default_unix_domain(initial: bool, ui: &mut ConnectionUI) -> anyhow::Result<Self> {
        let config = configuration();
        let unix_dom = config
            .unix_domains
            .first()
            .ok_or_else(|| anyhow!("no default unix domain is configured"))?;
        Self::new_unix_domain(alloc_domain_id(), unix_dom, initial, ui)
    }

    pub fn new_unix_domain(
        local_domain_id: DomainId,
        unix_dom: &UnixDomain,
        initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<Self> {
        let mut reconnectable =
            Reconnectable::new(ClientDomainConfig::Unix(unix_dom.clone()), None);
        reconnectable.connect(initial, ui)?;
        Ok(Self::new(local_domain_id, reconnectable))
    }

    pub fn new_tls(
        local_domain_id: DomainId,
        tls_client: &TlsDomainClient,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<Self> {
        let mut reconnectable =
            Reconnectable::new(ClientDomainConfig::Tls(tls_client.clone()), None);
        reconnectable.connect(true, ui)?;
        Ok(Self::new(local_domain_id, reconnectable))
    }

    pub fn new_ssh(
        local_domain_id: DomainId,
        ssh_dom: &SshDomain,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<Self> {
        let mut reconnectable = Reconnectable::new(ClientDomainConfig::Ssh(ssh_dom.clone()), None);
        reconnectable.connect(true, ui)?;
        Ok(Self::new(local_domain_id, reconnectable))
    }

    pub fn send_pdu(&self, pdu: Pdu) -> Future<Pdu> {
        let mut promise = Promise::new();
        let future = promise.get_future().expect("future already taken!?");
        match self.sender.send(ReaderMessage::SendPdu { pdu, promise }) {
            Ok(_) => future,
            Err(err) => Future::err(Error::msg(err)),
        }
    }

    rpc!(ping, Ping = (), Pong);
    rpc!(list_tabs, ListTabs = (), ListTabsResponse);
    rpc!(spawn, Spawn, SpawnResponse);
    rpc!(write_to_tab, WriteToTab, UnitResponse);
    rpc!(send_paste, SendPaste, UnitResponse);
    rpc!(key_down, SendKeyDown, UnitResponse);
    rpc!(mouse_event, SendMouseEvent, UnitResponse);
    rpc!(resize, Resize, UnitResponse);
    rpc!(get_tab_render_changes, GetTabRenderChanges, UnitResponse);
    rpc!(get_lines, GetLines, GetLinesResponse);
    rpc!(get_codec_version, GetCodecVersion, GetCodecVersionResponse);
    rpc!(get_tls_creds, GetTlsCreds = (), GetTlsCredsResponse);
}
