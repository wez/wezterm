use crate::domain::{ClientDomain, ClientDomainConfig};
use crate::pane::ClientPane;
use crate::UnixStream;
use anyhow::{anyhow, bail, Context};
use async_ossl::AsyncSslStream;
use async_trait::async_trait;
use codec::*;
use config::{configuration, SshDomain, TlsDomainClient, UnixDomain};
use filedescriptor::FileDescriptor;
use futures::FutureExt;
use mux::connui::ConnectionUI;
use mux::domain::{alloc_domain_id, DomainId};
use mux::pane::PaneId;
use mux::ssh::ssh_connect_with_ui;
use mux::Mux;
use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
use openssl::x509::X509;
use smol::channel::{bounded, unbounded, Receiver, Sender};
use smol::prelude::*;
use smol::{block_on, Async};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::marker::Unpin;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use thiserror::Error;

enum ReaderMessage {
    SendPdu {
        pdu: Pdu,
        promise: Sender<anyhow::Result<Pdu>>,
    },
    Readable,
}

#[derive(Clone)]
pub struct Client {
    sender: Sender<ReaderMessage>,
    local_domain_id: DomainId,
    pub is_reconnectable: bool,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "Please install the same version of wezterm on both the client and server!\n\
     The server version is {} (codec version {}),\n\
     which is not compatible with our version \n\
     {} (codec version {}).",
    version,
    codec_vers,
    config::wezterm_version(),
    CODEC_VERSION
)]
pub struct IncompatibleVersionError {
    pub version: String,
    pub codec_vers: usize,
}

macro_rules! rpc {
    ($method_name:ident, $request_type:ident, $response_type:ident) => {
        pub async fn $method_name(&self, pdu: $request_type) -> anyhow::Result<$response_type> {
            let start = std::time::Instant::now();
            let result = self.send_pdu(Pdu::$request_type(pdu)).await;
            let elapsed = start.elapsed();
            metrics::histogram!("rpc", elapsed, "method" => stringify!($method_name));
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
            metrics::histogram!("rpc", elapsed, "method" => stringify!($method_name));
            match result {
                Ok(Pdu::$response_type(res)) => Ok(res),
                Ok(_) => bail!("unexpected response {:?}", result),
                Err(err) => Err(err),
            }
        }
    };
}

fn process_unilateral_inner(pane_id: PaneId, local_domain_id: DomainId, decoded: DecodedPdu) {
    promise::spawn::spawn(async move {
        process_unilateral_inner_async(pane_id, local_domain_id, decoded).await?;
        Ok::<(), anyhow::Error>(())
    })
    .detach();
}

async fn process_unilateral_inner_async(
    pane_id: PaneId,
    local_domain_id: DomainId,
    decoded: DecodedPdu,
) -> anyhow::Result<()> {
    let mux = match Mux::get() {
        Some(mux) => mux,
        None => {
            // This can happen for some client scenarios; it is ok to ignore it.
            return Ok(());
        }
    };

    let client_domain = mux
        .get_domain(local_domain_id)
        .ok_or_else(|| anyhow!("no such domain {}", local_domain_id))?;
    let client_domain = client_domain
        .downcast_ref::<ClientDomain>()
        .ok_or_else(|| anyhow!("domain {} is not a ClientDomain instance", local_domain_id))?;

    // If we get a push for a pane that we don't yet know about,
    // it means that some other client has manipulated the mux
    // topology; we need to re-sync.
    let local_pane_id = match client_domain.remote_to_local_pane_id(pane_id) {
        Some(p) => p,
        None => {
            client_domain.resync().await?;
            client_domain
                .remote_to_local_pane_id(pane_id)
                .ok_or_else(|| {
                    anyhow!("remote pane id {} does not have a local pane id", pane_id)
                })?
        }
    };

    let pane = mux
        .get_pane(local_pane_id)
        .ok_or_else(|| anyhow!("no such pane {}", local_pane_id))?;
    let client_pane = pane.downcast_ref::<ClientPane>().ok_or_else(|| {
        log::error!(
            "received unilateral PDU for pane {} which is \
                     not an instance of ClientPane: {:?}",
            local_pane_id,
            decoded.pdu
        );
        anyhow!(
            "received unilateral PDU for pane {} which is \
                     not an instance of ClientPane: {:?}",
            local_pane_id,
            decoded.pdu
        )
    })?;
    client_pane.process_unilateral(decoded.pdu)
}

fn process_unilateral(local_domain_id: DomainId, decoded: DecodedPdu) -> anyhow::Result<()> {
    if let Some(pane_id) = decoded.pdu.pane_id() {
        promise::spawn::spawn_into_main_thread(async move {
            process_unilateral_inner(pane_id, local_domain_id, decoded)
        })
        .detach();
    } else {
        bail!("don't know how to handle {:?}", decoded);
    }
    Ok(())
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
enum NotReconnectableError {
    #[error("Client was destroyed")]
    ClientWasDestroyed,
}

fn client_thread(
    reconnectable: &mut Reconnectable,
    local_domain_id: DomainId,
    rx: &mut Receiver<ReaderMessage>,
) -> anyhow::Result<()> {
    block_on(client_thread_async(reconnectable, local_domain_id, rx))
}

async fn client_thread_async(
    reconnectable: &mut Reconnectable,
    local_domain_id: DomainId,
    rx: &mut Receiver<ReaderMessage>,
) -> anyhow::Result<()> {
    let mut next_serial = 1u64;

    struct Promises {
        map: HashMap<u64, Sender<anyhow::Result<Pdu>>>,
    }

    impl Promises {
        fn fail_all(&mut self, reason: &str) {
            log::trace!("failing all promises: {}", reason);
            for (_, promise) in self.map.drain() {
                promise.try_send(Err(anyhow!("{}", reason))).unwrap();
            }
        }
    }

    impl Drop for Promises {
        fn drop(&mut self) {
            self.fail_all("Client was destroyed");
        }
    }
    let mut promises = Promises {
        map: HashMap::new(),
    };

    let mut stream = reconnectable.take_stream().unwrap();

    loop {
        let rx_msg = rx.recv();
        let wait_for_read = stream
            .wait_for_readable()
            .map(|_| Ok(ReaderMessage::Readable));

        match smol::future::or(rx_msg, wait_for_read).await {
            Ok(ReaderMessage::SendPdu { pdu, promise }) => {
                let serial = next_serial;
                next_serial += 1;
                promises.map.insert(serial, promise);

                pdu.encode_async(&mut stream, serial)
                    .await
                    .context("encoding a PDU to send to the server")?;
                stream.flush().await.context("flushing PDU to server")?;
            }
            Ok(ReaderMessage::Readable) => match Pdu::decode_async(&mut stream).await {
                Ok(decoded) => {
                    log::trace!("decoded serial {}", decoded.serial);
                    if decoded.serial == 0 {
                        process_unilateral(local_domain_id, decoded)
                            .context("processing unilateral PDU from server")
                            .map_err(|e| {
                                log::error!("process_unilateral: {:?}", e);
                                e
                            })?;
                    } else if let Some(promise) = promises.map.remove(&decoded.serial) {
                        promise.try_send(Ok(decoded.pdu)).unwrap();
                    } else {
                        log::error!(
                            "got serial {} without a corresponding promise",
                            decoded.serial
                        );
                    }
                }
                Err(err) => {
                    let reason = format!("Error while decoding response pdu: {:#}", err);
                    log::error!("{}", reason);
                    promises.fail_all(&reason);
                    return Err(err).context("Error while decoding response pdu");
                }
            },
            Err(_) => {
                return Err(NotReconnectableError::ClientWasDestroyed.into());
            }
        }
    }
}

pub fn unix_connect_with_retry(
    path: &Path,
    just_spawned: bool,
) -> Result<UnixStream, std::io::Error> {
    let mut error = std::io::Error::last_os_error();

    if just_spawned {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

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

#[async_trait(?Send)]
pub trait AsyncReadAndWrite: Unpin + AsyncRead + AsyncWrite + std::fmt::Debug + Send {
    async fn wait_for_readable(&self) -> anyhow::Result<()>;
}

#[async_trait(?Send)]
impl<T> AsyncReadAndWrite for Async<T>
where
    T: std::fmt::Debug,
    T: std::io::Write,
    T: std::io::Read,
    T: Send,
{
    async fn wait_for_readable(&self) -> anyhow::Result<()> {
        Ok(self.readable().await?)
    }
}

#[derive(Debug)]
struct Reconnectable {
    config: ClientDomainConfig,
    stream: Option<Box<dyn AsyncReadAndWrite>>,
    tls_creds: Option<GetTlsCredsResponse>,
}

struct SshStream {
    stdin: FileDescriptor,
    stdout: FileDescriptor,
    _child: wezterm_ssh::SshChildProcess,
}

impl std::fmt::Debug for SshStream {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(fmt, "SshStream {{...}}")
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for SshStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.stdout.as_raw_fd()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawSocket for SshStream {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        self.stdout.as_raw_socket()
    }
}

impl Read for SshStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.stdout.read(buf)
    }
}

impl Write for SshStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.stdin.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.stdin.flush()
    }
}

impl Reconnectable {
    fn new(config: ClientDomainConfig, stream: Option<Box<dyn AsyncReadAndWrite>>) -> Self {
        Self {
            config,
            stream,
            tls_creds: None,
        }
    }

    fn tls_creds_path(&self) -> anyhow::Result<PathBuf> {
        let path = config::pki_dir()?.join(self.config.name());
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    fn tls_creds_ca_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.tls_creds_path()?.join("ca.pem"))
    }

    fn tls_creds_cert_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.tls_creds_path()?.join("cert.pem"))
    }

    fn take_stream(&mut self) -> Option<Box<dyn AsyncReadAndWrite>> {
        self.stream.take()
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
    fn wezterm_bin_path(path: &Option<String>) -> &str {
        match path.as_ref() {
            Some(p) => p,
            None => {
                if !configuration().use_local_build_for_proxy {
                    "wezterm"
                } else if cfg!(debug_assertions) {
                    "/home/wez/wez-personal/wezterm/target/debug/wezterm"
                } else {
                    "/home/wez/wez-personal/wezterm/target/release/wezterm"
                }
            }
        }
    }

    fn ssh_connect(
        &mut self,
        ssh_dom: SshDomain,
        initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<()> {
        let sess = ssh_connect_with_ui(&ssh_dom.remote_address, &ssh_dom.username, ui)?;
        let proxy_bin = Self::wezterm_bin_path(&ssh_dom.remote_wezterm_path);

        let cmd = if initial {
            format!("{} cli proxy", proxy_bin)
        } else {
            format!("{} cli --no-auto-start proxy", proxy_bin)
        };
        ui.output_str(&format!("Running: {}\n", cmd));
        log::error!("going to run {}", cmd);

        let exec = smol::block_on(sess.exec(&cmd, None))?;

        let mut stderr = exec.stderr;
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(len) = stderr.read(&mut buf) {
                if len == 0 {
                    break;
                } else {
                    let stderr = &buf[0..len];
                    log::error!("ssh stderr: {}", String::from_utf8_lossy(stderr));
                }
            }
        });

        let stream: Box<dyn AsyncReadAndWrite> = Box::new(Async::new(SshStream {
            stdin: exec.stdin,
            stdout: exec.stdout,
            _child: exec.child,
        })?);
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
        log::trace!("connect to {}", sock_path.display());

        let stream = match unix_connect_with_retry(&sock_path, false) {
            Ok(stream) => stream,
            Err(e) => {
                if unix_dom.no_serve_automatically || !initial {
                    bail!("failed to connect to {}: {}", sock_path.display(), e);
                }
                log::warn!(
                    "While connecting to {}: {}.  Will try spawning the server.",
                    sock_path.display(),
                    e
                );
                ui.output_str(&format!("Error: {}.  Will try spawning server.\n", e));

                let argv = unix_dom.serve_command()?;

                let mut cmd = std::process::Command::new(&argv[0]);
                cmd.args(&argv[1..]);
                let child = cmd
                    .spawn()
                    .with_context(|| format!("while spawning {:?}", cmd))?;
                std::thread::spawn(move || match child.wait_with_output() {
                    Ok(out) => {
                        if let Ok(stdout) = std::str::from_utf8(&out.stdout) {
                            if !stdout.is_empty() {
                                log::warn!("stdout: {}", stdout);
                            }
                        }
                        if let Ok(stderr) = std::str::from_utf8(&out.stderr) {
                            if !stderr.is_empty() {
                                log::warn!("stderr: {}", stderr);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("spawn: {:#}", err);
                    }
                });

                unix_connect_with_retry(&sock_path, true).with_context(|| {
                    format!(
                        "(after spawning server) failed to connect to {}",
                        sock_path.display()
                    )
                })?
            }
        };

        ui.output_str("Connected!\n");
        stream.set_read_timeout(Some(unix_dom.read_timeout))?;
        stream.set_write_timeout(Some(unix_dom.write_timeout))?;
        let stream: Box<dyn AsyncReadAndWrite> = Box::new(Async::new(stream)?);
        self.stream.replace(stream);
        Ok(())
    }

    pub fn tls_connect(
        &mut self,
        tls_client: TlsDomainClient,
        _initial: bool,
        ui: &mut ConnectionUI,
    ) -> anyhow::Result<()> {
        openssl::init();

        let remote_address = &tls_client.remote_address;

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            anyhow!(
                "expected mux_server_remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

        // If we are reconnecting and already bootstrapped via SSH, let's see if
        // we can connect using those same credentials and avoid running through
        // the SSH authentication flow.
        if let Some(Ok(_)) = tls_client.ssh_parameters() {
            match self.try_connect(&tls_client, ui, &remote_address, remote_host_name) {
                Ok(stream) => {
                    self.stream.replace(stream);
                    return Ok(());
                }
                Err(err) => {
                    if let Some(ioerr) = err.root_cause().downcast_ref::<std::io::Error>() {
                        match ioerr.kind() {
                            std::io::ErrorKind::ConnectionRefused => {
                                // Server isn't up yet; let's proceed with bootstrap
                            }
                            _ => {
                                // If it is an IO error that implies that we had an issue
                                // reaching or otherwise talking to the remote host.
                                // Re-attempting the SSH bootstrap most likely will not
                                // succeed so we let this bubble up.
                                return Err(err);
                            }
                        }
                    }
                    ui.output_str(&format!(
                        "Failed to reuse creds: {:?}\nWill retry bootstrap via SSH\n",
                        err
                    ));
                }
            }
        }

        if let Some(Ok(ssh_params)) = tls_client.ssh_parameters() {
            if self.tls_creds.is_none() {
                // We need to bootstrap via an ssh session
                let sess =
                    ssh_connect_with_ui(&ssh_params.host_and_port, &ssh_params.username, ui)?;

                let creds = ui.run_and_log_error(|| {
                    // The `tlscreds` command will start the server if needed and then
                    // obtain client credentials that we can use for tls.
                    let cmd = format!(
                        "{} cli tlscreds",
                        Self::wezterm_bin_path(&tls_client.remote_wezterm_path)
                    );

                    ui.output_str(&format!("Running: {}\n", cmd));
                    let mut exec = smol::block_on(sess.exec(&cmd, None))
                        .with_context(|| format!("executing `{}` on remote host", cmd))?;

                    // stdout holds an encoded pdu
                    let mut buf = Vec::new();
                    exec.stdout
                        .read_to_end(&mut buf)
                        .context("reading tlscreds response to buffer")?;

                    drop(exec.stdin);

                    // stderr is ideally empty
                    let mut err = String::new();
                    exec.stderr
                        .read_to_string(&mut err)
                        .context("reading tlscreds stderr")?;
                    if !err.is_empty() {
                        log::error!("remote: `{}` stderr -> `{}`", cmd, err);
                    }

                    let creds = match Pdu::decode(buf.as_slice())
                        .context("reading tlscreds response")?
                        .pdu
                    {
                        Pdu::GetTlsCredsResponse(creds) => creds,
                        _ => bail!("unexpected response to tlscreds"),
                    };

                    // Save the credentials to disk, as that is currently the easiest
                    // way to get them into openssl.  Ideally we'd keep these entirely
                    // in memory.
                    std::fs::write(&self.tls_creds_ca_path()?, creds.ca_cert_pem.as_bytes())?;
                    std::fs::write(
                        &self.tls_creds_cert_path()?,
                        creds.client_cert_pem.as_bytes(),
                    )?;
                    log::info!("got TLS creds");
                    Ok(creds)
                })?;
                self.tls_creds.replace(creds);
            }
        }

        let cloned_ui = ui.clone();
        let stream = cloned_ui.run_and_log_error({
            || self.try_connect(&tls_client, ui, &remote_address, remote_host_name)
        })?;
        self.stream.replace(stream);
        Ok(())
    }

    fn try_connect(
        &mut self,
        tls_client: &TlsDomainClient,
        ui: &mut ConnectionUI,
        remote_address: &str,
        remote_host_name: &str,
    ) -> anyhow::Result<Box<dyn AsyncReadAndWrite>> {
        let mut connector = SslConnector::builder(SslMethod::tls())?;

        let cert_file = match tls_client.pem_cert.clone() {
            Some(cert) => cert,
            None => self.tls_creds_cert_path()?,
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
            None => self.tls_creds_cert_path()?,
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

        if let Ok(ca_path) = self.tls_creds_ca_path() {
            if ca_path.exists() {
                connector.cert_store_mut().add_cert(load_cert(&ca_path)?)?;
            }
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

        let stream = Box::new(Async::new(AsyncSslStream::new(
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
        ))?);
        ui.output_str("TLS Connected!\n");
        Ok(stream)
    }
}

impl Client {
    fn new(local_domain_id: DomainId, mut reconnectable: Reconnectable) -> Self {
        let is_reconnectable = reconnectable.reconnectable();
        let (sender, mut receiver) = unbounded();

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

                    if let Some(ioerr) = e.root_cause().downcast_ref::<std::io::Error>() {
                        if let std::io::ErrorKind::UnexpectedEof = ioerr.kind() {
                            // Don't reconnect for a simple EOF
                            log::error!("server closed connection ({})", e);
                            break;
                        }
                    }

                    if let Some(err) = e.root_cause().downcast_ref::<NotReconnectableError>() {
                        log::error!("{}; won't try to reconnect", err);
                        break;
                    }

                    let mut ui = ConnectionUI::new();
                    ui.title("wezterm: Reconnecting...");

                    loop {
                        ui.sleep_with_reason(
                            &format!("client disconnected {}; will reconnect", e),
                            backoff,
                        )
                        .ok();
                        match reconnectable.connect(false, &mut ui) {
                            Ok(_) => {
                                backoff = BASE_INTERVAL;
                                log::error!("Reconnected!");
                                promise::spawn::spawn_into_main_thread(async move {
                                    ClientDomain::reattach(local_domain_id, ui).await.ok();
                                })
                                .detach();
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
                if let Some(mux) = Mux::get() {
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
                }
                Ok(())
            }
            promise::spawn::spawn_into_main_thread(async move {
                detach(local_domain_id).await.ok();
            })
            .detach();
        });

        Self {
            sender,
            local_domain_id,
            is_reconnectable,
        }
    }

    pub async fn verify_version_compat(&self, ui: &ConnectionUI) -> anyhow::Result<()> {
        match self.get_codec_version(GetCodecVersion {}).await {
            Ok(info) if info.codec_vers == CODEC_VERSION => {
                log::trace!(
                    "Server version is {} (codec version {})",
                    info.version_string,
                    info.codec_vers
                );
                Ok(())
            }
            Ok(info) => {
                let err = IncompatibleVersionError {
                    version: info.version_string,
                    codec_vers: info.codec_vers,
                };
                ui.output_str(&err.to_string());
                log::error!("{:?}", err);
                return Err(err.into());
            }
            Err(err) => {
                let msg = format!(
                    "Please install the same version of wezterm on both \
                     the client and server! \
                     The server reported error '{}' while being asked for its \
                     version.  This likely means that the server is older \
                     than the client.\n",
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

        let unix_dom = match std::env::var_os("WEZTERM_UNIX_SOCKET") {
            Some(path) => config::UnixDomain {
                socket_path: Some(path.into()),
                ..Default::default()
            },
            None => config
                .unix_domains
                .first()
                .ok_or_else(|| {
                    anyhow!(
                        "no default unix domain is configured and WEZTERM_UNIX_SOCKET \
                        is not set in the environment"
                    )
                })?
                .clone(),
        };

        Self::new_unix_domain(alloc_domain_id(), &unix_dom, initial, ui)
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

    pub async fn send_pdu(&self, pdu: Pdu) -> anyhow::Result<Pdu> {
        let (promise, rx) = bounded(1);
        self.sender
            .send(ReaderMessage::SendPdu { pdu, promise })
            .await?;
        rx.recv().await?
    }

    rpc!(ping, Ping = (), Pong);
    rpc!(list_panes, ListPanes = (), ListPanesResponse);
    rpc!(spawn, Spawn, SpawnResponse);
    rpc!(spawn_v2, SpawnV2, SpawnResponse);
    rpc!(split_pane, SplitPane, SpawnResponse);
    rpc!(write_to_pane, WriteToPane, UnitResponse);
    rpc!(send_paste, SendPaste, UnitResponse);
    rpc!(key_down, SendKeyDown, UnitResponse);
    rpc!(mouse_event, SendMouseEvent, UnitResponse);
    rpc!(resize, Resize, UnitResponse);
    rpc!(set_zoomed, SetPaneZoomed, UnitResponse);
    rpc!(
        get_tab_render_changes,
        GetPaneRenderChanges,
        LivenessResponse
    );
    rpc!(get_lines, GetLines, GetLinesResponse);
    rpc!(get_codec_version, GetCodecVersion, GetCodecVersionResponse);
    rpc!(get_tls_creds, GetTlsCreds = (), GetTlsCredsResponse);
    rpc!(
        search_scrollback,
        SearchScrollbackRequest,
        SearchScrollbackResponse
    );
    rpc!(kill_pane, KillPane, UnitResponse);
}
