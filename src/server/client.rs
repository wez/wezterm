#![allow(dead_code)]
use crate::config::{Config, SshDomain, TlsDomainClient, UnixDomain};
use crate::frontend::gui_executor;
use crate::mux::domain::alloc_domain_id;
use crate::mux::domain::DomainId;
use crate::mux::Mux;
use crate::server::codec::*;
use crate::server::domain::{ClientDomain, ClientDomainConfig};
use crate::server::pollable::*;
use crate::server::tab::ClientTab;
use crate::server::UnixStream;
use crossbeam_channel::TryRecvError;
use failure::{bail, err_msg, format_err, Fallible};
use filedescriptor::{pollfd, AsRawSocketDescriptor};
use log::info;
use promise::{Future, Promise};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
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

fn process_unilateral(local_domain_id: DomainId, decoded: DecodedPdu) -> Fallible<()> {
    if let Some(tab_id) = decoded.pdu.tab_id() {
        let pdu = decoded.pdu;
        Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let client_domain = mux
                .get_domain(local_domain_id)
                .ok_or_else(|| format_err!("no such domain {}", local_domain_id))?;
            let client_domain = client_domain
                .downcast_ref::<ClientDomain>()
                .ok_or_else(|| {
                    format_err!("domain {} is not a ClientDomain instance", local_domain_id)
                })?;

            let local_tab_id = client_domain
                .remote_to_local_tab_id(tab_id)
                .ok_or_else(|| {
                    format_err!("remote tab id {} does not have a local tab id", tab_id)
                })?;
            let tab = mux
                .get_tab(local_tab_id)
                .ok_or_else(|| format_err!("no such tab {}", local_tab_id))?;
            let client_tab = tab.downcast_ref::<ClientTab>().ok_or_else(|| {
                log::error!(
                    "received unilateral PDU for tab {} which is \
                     not an instance of ClientTab: {:?}",
                    local_tab_id,
                    pdu
                );
                format_err!(
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

                        pdu.encode(reconnectable.stream(), serial)?;
                        reconnectable.stream().flush()?;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => bail!("Client was destroyed"),
            };
        }

        let mut poll_array = [rx.as_poll_fd(), reconnectable.stream().as_poll_fd()];
        poll_for_read(&mut poll_array);

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
                if let Some(decoded) = res? {
                    log::trace!("decoded serial {}", decoded.serial);
                    if decoded.serial == 0 {
                        process_unilateral(local_domain_id, decoded)?;
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

impl std::io::Read for SshStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
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

impl std::io::Write for SshStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.chan.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.chan.flush()
    }
}

impl ReadAndWrite for SshStream {
    fn set_non_blocking(&self, non_blocking: bool) -> Fallible<()> {
        self.sess.set_blocking(!non_blocking);
        Ok(())
    }

    fn has_read_buffered(&self) -> bool {
        false
    }
}

impl Reconnectable {
    fn new(config: ClientDomainConfig, stream: Option<Box<dyn ReadAndWrite>>) -> Self {
        Self { config, stream }
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
            ClientDomainConfig::Ssh(_) => false,
        }
    }

    fn reconnect(&mut self) -> Fallible<bool> {
        if !self.reconnectable() {
            return Ok(false);
        }
        self.connect()?;
        Ok(true)
    }

    fn connect(&mut self) -> Fallible<()> {
        match self.config.clone() {
            ClientDomainConfig::Unix(unix_dom) => self.unix_connect(unix_dom),
            ClientDomainConfig::Tls(tls) => self.tls_connect(tls),
            ClientDomainConfig::Ssh(ssh) => self.ssh_connect(ssh),
        }
    }

    fn ssh_connect(&mut self, ssh_dom: SshDomain) -> Fallible<()> {
        let mut sess = ssh2::Session::new()?;

        let tcp = TcpStream::connect(&ssh_dom.remote_address)?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        if let Ok(mut known_hosts) = sess.known_hosts() {
            let varname = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            let var = std::env::var_os(varname).ok_or_else(|| {
                failure::format_err!("environment variable {} is missing", varname)
            })?;
            let file = Path::new(&var).join(".ssh/known_hosts");
            if file.exists() {
                known_hosts
                    .read_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                    .map_err(|e| {
                        failure::format_err!("reading known_hosts file {}: {}", file.display(), e)
                    })?;
            }

            let remote_host_name = ssh_dom.remote_address.split(':').next().ok_or_else(|| {
                format_err!(
                    "expected remote_address to have the form 'host:port', but have {}",
                    ssh_dom.remote_address
                )
            })?;

            let (key, key_type) = sess
                .host_key()
                .ok_or_else(|| failure::err_msg("failed to get ssh host key"))?;

            let fingerprint = sess
                .host_key_hash(ssh2::HashType::Sha256)
                .map(|fingerprint| {
                    format!(
                        "SHA256:{}",
                        base64::encode_config(
                            fingerprint,
                            base64::Config::new(base64::CharacterSet::Standard, false)
                        )
                    )
                })
                .or_else(|| {
                    // Querying for the Sha256 can fail if for example we were linked
                    // against libssh < 1.9, so let's fall back to Sha1 in that case.
                    sess.host_key_hash(ssh2::HashType::Sha1)
                        .map(|fingerprint| {
                            let mut res = vec![];
                            write!(&mut res, "SHA1").ok();
                            for b in fingerprint {
                                write!(&mut res, ":{:02x}", *b).ok();
                            }
                            String::from_utf8(res).unwrap()
                        })
                })
                .ok_or_else(|| failure::err_msg("failed to get host fingerprint"))?;

            use ssh2::CheckResult;
            match known_hosts.check(&remote_host_name, key) {
                CheckResult::Match => {}
                CheckResult::NotFound => {
                    let allow = tinyfiledialogs::message_box_yes_no(
                        "wezterm",
                        &format!(
                            "SSH host {} is not yet trusted.\n\
                             {:?} Fingerprint: {}.\n\
                             Trust and continue connecting?",
                            ssh_dom.remote_address, key_type, fingerprint
                        ),
                        tinyfiledialogs::MessageBoxIcon::Question,
                        tinyfiledialogs::YesNo::No,
                    );

                    if tinyfiledialogs::YesNo::No == allow {
                        bail!("user declined to trust host");
                    }

                    known_hosts
                        .add(
                            remote_host_name,
                            key,
                            &ssh_dom.remote_address,
                            key_type.into(),
                        )
                        .map_err(|e| {
                            failure::format_err!("adding known_hosts entry in memory: {}", e)
                        })?;

                    known_hosts
                        .write_file(&file, ssh2::KnownHostFileKind::OpenSSH)
                        .map_err(|e| {
                            failure::format_err!(
                                "writing known_hosts file {}: {}",
                                file.display(),
                                e
                            )
                        })?;
                }
                CheckResult::Mismatch => {
                    tinyfiledialogs::message_box_ok(
                        "wezterm",
                        &format!(
                            "host key mismatch for ssh server {}.\n\
                             Got fingerprint {} instead of expected value from known_hosts\n\
                             file {}.\n\
                             Refusing to connect.",
                            ssh_dom.remote_address,
                            fingerprint,
                            file.display()
                        ),
                        tinyfiledialogs::MessageBoxIcon::Error,
                    );
                    bail!("host mismatch, man in the middle attack?!");
                }
                CheckResult::Failure => {
                    tinyfiledialogs::message_box_ok(
                        "wezterm",
                        "Failed to load and check known ssh hosts",
                        tinyfiledialogs::MessageBoxIcon::Error,
                    );
                    bail!("failed to check the known hosts");
                }
            }
        }

        let methods: HashSet<&str> = sess.auth_methods(&ssh_dom.username)?.split(',').collect();

        if !sess.authenticated() && methods.contains("publickey") {
            if let Err(err) = sess.userauth_agent(&ssh_dom.username) {
                log::info!("while attempting agent auth: {}", err);
            }
        }

        fn password_prompt(instructions: &str, prompt: &str, dom: &SshDomain) -> Option<String> {
            let text = format!(
                "SSH Authentication for {} @ {}\n{}\n{}",
                dom.username, dom.remote_address, instructions, prompt
            );
            tinyfiledialogs::password_box("wezterm", &text)
        }

        fn input_prompt(instructions: &str, prompt: &str, dom: &SshDomain) -> Option<String> {
            let text = format!(
                "SSH Authentication for {} @ {}\n{}\n{}",
                dom.username, dom.remote_address, instructions, prompt
            );
            tinyfiledialogs::input_box("wezterm", &text, "")
        }

        if !sess.authenticated() && methods.contains("keyboard-interactive") {
            struct Prompt<'a> {
                dom: &'a SshDomain,
            }

            let mut prompt = Prompt { dom: &ssh_dom };
            impl<'a> ssh2::KeyboardInteractivePrompt for Prompt<'a> {
                fn prompt<'b>(
                    &mut self,
                    _username: &str,
                    instructions: &str,
                    prompts: &[ssh2::Prompt<'b>],
                ) -> Vec<String> {
                    prompts
                        .iter()
                        .map(|p| {
                            let func = if p.echo {
                                input_prompt
                            } else {
                                password_prompt
                            };

                            func(instructions, &p.text, &self.dom).unwrap_or_else(String::new)
                        })
                        .collect()
                }
            }

            if let Err(err) = sess.userauth_keyboard_interactive(&ssh_dom.username, &mut prompt) {
                log::error!("while attempting keyboard-interactive auth: {}", err);
            }
        }

        if !sess.authenticated() && methods.contains("password") {
            let pass = password_prompt("", "Password", &ssh_dom)
                .ok_or_else(|| failure::err_msg("password entry was cancelled"))?;
            if let Err(err) = sess.userauth_password(&ssh_dom.username, &pass) {
                log::error!("while attempting password auth: {}", err);
            }
        }

        if !sess.authenticated() {
            failure::bail!("unable to authenticate session");
        }

        let mut chan = sess.channel_session()?;
        chan.exec("wezterm cli proxy")?;

        let stream: Box<dyn ReadAndWrite> = Box::new(SshStream { sess, chan });
        self.stream.replace(stream);
        Ok(())
    }

    fn unix_connect(&mut self, unix_dom: UnixDomain) -> Fallible<()> {
        let sock_path = unix_dom.socket_path();
        info!("connect to {}", sock_path.display());

        let stream = match unix_connect_with_retry(&sock_path) {
            Ok(stream) => stream,
            Err(e) => {
                if unix_dom.no_serve_automatically {
                    bail!("failed to connect to {}: {}", sock_path.display(), e);
                }
                log::error!(
                    "While connecting to {}: {}.  Will try spawning the server.",
                    sock_path.display(),
                    e
                );

                let argv = unix_dom.serve_command()?;

                // We need to use a pty to spawn the command because,
                // on Windows, when spawned from the gui with no pre-existing
                // conhost.exe, `wsl.exe` will fail to start up correctly.
                // This also has a nice side effect of not flashing up a
                // console window when we first spin up the wsl instance.
                let pty_system = portable_pty::PtySystemSelection::default().get()?;
                let pair = pty_system.openpty(Default::default())?;
                let mut cmd = portable_pty::CommandBuilder::new(&argv[0]);
                cmd.args(&argv[1..]);
                let mut child = pair.slave.spawn_command(cmd)?;
                let status = child.wait()?;
                if !status.success() {
                    log::error!("{:?} failed with status {:?}", argv, status);
                }
                drop(child);
                drop(pair.slave);
                // Gross bug workaround: ClosePsuedoConsole can get confused about the
                // processes attached to the console when using wsl; the scenario
                // is that we use wsl.exe to invoke wezterm, daemonize it (which forks
                // and detaches from the pty) and then we can discard the pty.
                // The ClosePsuedoConsole call should not need to wait for any clients,
                // but blocks forever.
                // The workaround is to leak the console handle.  The associated conhost
                // process will show up in the process tree until this instance of
                // wezterm is terminated, but it otherwise invisible.
                std::mem::forget(pair.master);

                unix_connect_with_retry(&sock_path).map_err(|e| {
                    format_err!("failed to connect to {}: {}", sock_path.display(), e)
                })?
            }
        };

        let stream: Box<dyn ReadAndWrite> = Box::new(stream);
        self.stream.replace(stream);
        Ok(())
    }

    #[cfg(any(feature = "openssl", unix))]
    pub fn tls_connect(&mut self, tls_client: TlsDomainClient) -> Fallible<()> {
        use crate::server::listener::read_bytes;
        use openssl::ssl::{SslConnector, SslFiletype, SslMethod};
        use openssl::x509::X509;

        openssl::init();

        let remote_address = &tls_client.remote_address;

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            format_err!(
                "expected mux_server_remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

        let mut connector = SslConnector::builder(SslMethod::tls())?;

        if let Some(cert_file) = tls_client.pem_cert.as_ref() {
            connector.set_certificate_file(cert_file, SslFiletype::PEM)?;
        }
        if let Some(chain_file) = tls_client.pem_ca.as_ref() {
            connector.set_certificate_chain_file(chain_file)?;
        }
        if let Some(key_file) = tls_client.pem_private_key.as_ref() {
            connector.set_private_key_file(key_file, SslFiletype::PEM)?;
        }
        fn load_cert(name: &Path) -> Fallible<X509> {
            let cert_bytes = read_bytes(name)?;
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

        let connector = connector.build();
        let connector = connector
            .configure()?
            .verify_hostname(!tls_client.accept_invalid_hostnames);

        let stream = TcpStream::connect(remote_address)
            .map_err(|e| format_err!("connecting to {}: {}", remote_address, e))?;
        stream.set_nodelay(true)?;

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
        self.stream.replace(stream);
        Ok(())
    }

    #[cfg(not(any(feature = "openssl", unix)))]
    pub fn tls_connect(&mut self, tls_client: TlsDomainClient) -> Fallible<()> {
        use crate::server::listener::IdentitySource;
        use native_tls::TlsConnector;
        use std::convert::TryInto;

        let remote_address = &tls_client.remote_address;

        let remote_host_name = remote_address.split(':').next().ok_or_else(|| {
            format_err!(
                "expected mux_server_remote_address to have the form 'host:port', but have {}",
                remote_address
            )
        })?;

        let identity = IdentitySource::PemFiles {
            key: tls_client
                .pem_private_key
                .as_ref()
                .ok_or_else(|| failure::err_msg("missing pem_private_key config value"))?
                .into(),
            cert: tls_client.pem_cert.clone(),
            chain: tls_client.pem_ca.clone(),
        };

        let connector = TlsConnector::builder()
            .identity(identity.try_into()?)
            .danger_accept_invalid_hostnames(tls_client.accept_invalid_hostnames)
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

                    log::error!("client disconnected {}; will reconnect in {:?}", e, backoff);

                    loop {
                        std::thread::sleep(backoff);
                        match reconnectable.connect() {
                            Ok(_) => {
                                backoff = BASE_INTERVAL;
                                log::error!("Reconnected!");
                                break;
                            }
                            Err(err) => {
                                backoff = (backoff + backoff).min(MAX_INTERVAL);
                                log::error!(
                                    "problem reconnecting: {}; will reconnect in {:?}",
                                    err,
                                    backoff
                                );
                            }
                        }
                    }
                }
            }
            Future::with_executor(gui_executor().unwrap(), move || {
                let mux = Mux::get().unwrap();
                let client_domain = mux
                    .get_domain(local_domain_id)
                    .ok_or_else(|| format_err!("no such domain {}", local_domain_id))?;
                let client_domain =
                    client_domain
                        .downcast_ref::<ClientDomain>()
                        .ok_or_else(|| {
                            format_err!("domain {} is not a ClientDomain instance", local_domain_id)
                        })?;
                client_domain.perform_detach();
                Ok(())
            });
        });

        Self {
            sender,
            local_domain_id,
        }
    }

    pub fn local_domain_id(&self) -> DomainId {
        self.local_domain_id
    }

    pub fn new_default_unix_domain(config: &Arc<Config>) -> Fallible<Self> {
        let unix_dom = config
            .unix_domains
            .first()
            .ok_or_else(|| err_msg("no default unix domain is configured"))?;
        Self::new_unix_domain(alloc_domain_id(), config, unix_dom)
    }

    pub fn new_unix_domain(
        local_domain_id: DomainId,
        _config: &Arc<Config>,
        unix_dom: &UnixDomain,
    ) -> Fallible<Self> {
        let mut reconnectable =
            Reconnectable::new(ClientDomainConfig::Unix(unix_dom.clone()), None);
        reconnectable.connect()?;
        Ok(Self::new(local_domain_id, reconnectable))
    }

    pub fn new_tls(
        local_domain_id: DomainId,
        _config: &Arc<Config>,
        tls_client: &TlsDomainClient,
    ) -> Fallible<Self> {
        let mut reconnectable =
            Reconnectable::new(ClientDomainConfig::Tls(tls_client.clone()), None);
        reconnectable.connect()?;
        Ok(Self::new(local_domain_id, reconnectable))
    }

    pub fn new_ssh(
        local_domain_id: DomainId,
        _config: &Arc<Config>,
        ssh_dom: &SshDomain,
    ) -> Fallible<Self> {
        let mut reconnectable = Reconnectable::new(ClientDomainConfig::Ssh(ssh_dom.clone()), None);
        reconnectable.connect()?;
        Ok(Self::new(local_domain_id, reconnectable))
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
    rpc!(spawn, Spawn, SpawnResponse);
    rpc!(write_to_tab, WriteToTab, UnitResponse);
    rpc!(send_paste, SendPaste, UnitResponse);
    rpc!(key_down, SendKeyDown, UnitResponse);
    rpc!(mouse_event, SendMouseEvent, SendMouseEventResponse);
    rpc!(resize, Resize, UnitResponse);
    rpc!(get_tab_render_changes, GetTabRenderChanges, UnitResponse);
}
