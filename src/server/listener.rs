use crate::config::{configuration, TlsDomainServer, UnixDomain};
use crate::create_user_owned_dirs;
use crate::frontend::executor;
use crate::mux::tab::{Tab, TabId};
use crate::mux::{Mux, MuxNotification, MuxSubscriber};
use crate::server::codec::*;
use crate::server::pollable::*;
use crate::server::UnixListener;
use anyhow::{anyhow, bail, Context, Error};
use crossbeam_channel::TryRecvError;
#[cfg(unix)]
use libc::{mode_t, umask};
use log::{debug, error};
use native_tls::Identity;
use portable_pty::PtySize;
use promise::Future;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::fs::remove_file;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use term::terminal::Clipboard;
use term::StableRowIndex;

struct LocalListener {
    listener: UnixListener,
}

impl LocalListener {
    pub fn new(listener: UnixListener) -> Self {
        Self { listener }
    }

    fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    Future::with_executor(executor(), move || {
                        let mut session = ClientSession::new(stream);
                        thread::spawn(move || session.run());
                        Ok(())
                    });
                }
                Err(err) => {
                    error!("accept failed: {}", err);
                    return;
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum IdentitySource {
    Pkcs12File {
        path: PathBuf,
        password: String,
    },
    PemFiles {
        key: PathBuf,
        cert: Option<PathBuf>,
        chain: Option<PathBuf>,
    },
}

pub fn read_bytes<T: AsRef<Path>>(path: T) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();
    let mut f =
        std::fs::File::open(path).with_context(|| format!("opening file {}", path.display()))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(any(feature = "openssl", unix))]
fn pem_files_to_identity(
    key: PathBuf,
    cert: Option<PathBuf>,
    chain: Option<PathBuf>,
) -> anyhow::Result<Identity> {
    // This is a bit of a redundant dance around;
    // the native_tls interface only allows for pkcs12
    // encoded identity information, but in my use case
    // I only have pem encoded identity information.
    // We can use openssl to convert the data to pkcs12
    // so that we can then pass it on using the Identity
    // type that native_tls requires.
    use openssl::pkcs12::Pkcs12;
    use openssl::pkey::PKey;
    use openssl::x509::X509;
    let key_bytes = read_bytes(&key)?;
    let pkey = PKey::private_key_from_pem(&key_bytes)?;

    let cert_bytes = read_bytes(cert.as_ref().unwrap_or(&key))?;
    let x509_cert = X509::from_pem(&cert_bytes)?;

    let chain_bytes = read_bytes(chain.as_ref().unwrap_or(&key))?;
    let x509_chain = X509::stack_from_pem(&chain_bytes)?;

    let password = "internal";
    let mut ca_stack = openssl::stack::Stack::new()?;
    for ca in x509_chain.into_iter() {
        ca_stack.push(ca)?;
    }
    let mut builder = Pkcs12::builder();
    builder.ca(ca_stack);
    let pkcs12 = builder.build(password, "", &pkey, &x509_cert)?;

    let der = pkcs12.to_der()?;
    Identity::from_pkcs12(&der, password).with_context(|| {
        format!(
            "error creating identity from pkcs12 generated \
             from PemFiles {}, {:?}, {:?}",
            key.display(),
            cert,
            chain,
        )
    })
}

#[cfg(not(any(feature = "openssl", unix)))]
fn pem_files_to_identity(
    _key: PathBuf,
    _cert: Option<PathBuf>,
    _chain: Option<PathBuf>,
) -> anyhow::Result<Identity> {
    bail!("recompile wezterm using --features openssl")
}

impl TryFrom<IdentitySource> for Identity {
    type Error = Error;

    fn try_from(source: IdentitySource) -> anyhow::Result<Identity> {
        match source {
            IdentitySource::Pkcs12File { path, password } => {
                let bytes = read_bytes(&path)?;
                Identity::from_pkcs12(&bytes, &password)
                    .with_context(|| format!("error loading pkcs12 file '{}'", path.display()))
            }
            IdentitySource::PemFiles { key, cert, chain } => {
                pem_files_to_identity(key, cert, chain)
            }
        }
    }
}

#[cfg(not(any(feature = "openssl", unix)))]
mod not_ossl {
    use super::*;
    use native_tls::TlsAcceptor;
    use std::convert::TryInto;

    struct NetListener {
        acceptor: Arc<TlsAcceptor>,
        listener: TcpListener,
    }

    impl NetListener {
        pub fn new(listener: TcpListener, acceptor: TlsAcceptor) -> Self {
            Self {
                listener,
                acceptor: Arc::new(acceptor),
            }
        }

        fn run(&mut self) {
            for stream in self.listener.incoming() {
                match stream {
                    Ok(stream) => {
                        stream.set_nodelay(true).ok();
                        let acceptor = self.acceptor.clone();

                        match acceptor.accept(stream) {
                            Ok(stream) => {
                                Future::with_executor(executor(), move || {
                                    let mut session = ClientSession::new(stream);
                                    thread::spawn(move || session.run());
                                    Ok(())
                                });
                            }
                            Err(e) => {
                                error!("failed TlsAcceptor: {}", e);
                            }
                        }
                    }
                    Err(err) => {
                        error!("accept failed: {}", err);
                        return;
                    }
                }
            }
        }
    }

    pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> anyhow::Result<()> {
        let identity = IdentitySource::PemFiles {
            key: tls_server
                .pem_private_key
                .as_ref()
                .ok_or_else(|| anyhow!("missing pem_private_key config value"))?
                .into(),
            cert: tls_server.pem_cert.clone(),
            chain: tls_server.pem_ca.clone(),
        };

        let mut net_listener = NetListener::new(
            TcpListener::bind(&tls_server.bind_address).with_context(|| {
                format!("error binding to bind_address {}", tls_server.bind_address,)
            })?,
            TlsAcceptor::new(identity.try_into()?)?,
        );
        thread::spawn(move || {
            net_listener.run();
        });
        Ok(())
    }
}

#[cfg(any(feature = "openssl", unix))]
mod ossl {
    use super::*;
    use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream, SslVerifyMode};
    use openssl::x509::X509;

    struct OpenSSLNetListener {
        acceptor: Arc<SslAcceptor>,
        listener: TcpListener,
    }

    impl OpenSSLNetListener {
        pub fn new(listener: TcpListener, acceptor: SslAcceptor) -> Self {
            Self {
                listener,
                acceptor: Arc::new(acceptor),
            }
        }

        /// Authenticates the peer.
        /// The requirements are:
        /// * The peer must have a certificate
        /// * The peer certificate must be trusted
        /// * The peer certificate must include a CN string that is
        ///   either an exact match for the unix username of the
        ///   user running this mux server instance, or must match
        ///   a special encoded prefix set up by a proprietary PKI
        ///   infrastructure in an environment used by the author.
        fn verify_peer_cert<T>(stream: &SslStream<T>) -> anyhow::Result<()> {
            let cert = stream
                .ssl()
                .peer_certificate()
                .ok_or_else(|| anyhow!("no peer cert"))?;
            let subject = cert.subject_name();
            let cn = subject
                .entries_by_nid(openssl::nid::Nid::COMMONNAME)
                .next()
                .ok_or_else(|| anyhow!("cert has no CN"))?;
            let cn_str = cn.data().as_utf8()?.to_string();

            let wanted_unix_name = std::env::var("USER")?;

            if wanted_unix_name == cn_str {
                log::info!(
                    "Peer certificate CN `{}` == $USER `{}`",
                    cn_str,
                    wanted_unix_name
                );
                Ok(())
            } else {
                // Some environments that are used by the author of this
                // program encode the CN in the form `user:unixname/DATA`
                let maybe_encoded = format!("user:{}/", wanted_unix_name);
                if cn_str.starts_with(&maybe_encoded) {
                    log::info!(
                        "Peer certificate CN `{}` matches $USER `{}`",
                        cn_str,
                        wanted_unix_name
                    );
                    Ok(())
                } else {
                    bail!("CN `{}` did not match $USER `{}`", cn_str, wanted_unix_name);
                }
            }
        }

        fn run(&mut self) {
            for stream in self.listener.incoming() {
                match stream {
                    Ok(stream) => {
                        stream.set_nodelay(true).ok();
                        let acceptor = self.acceptor.clone();

                        match acceptor.accept(stream) {
                            Ok(stream) => {
                                if let Err(err) = Self::verify_peer_cert(&stream) {
                                    error!("problem with peer cert: {}", err);
                                    break;
                                }

                                Future::with_executor(executor(), move || {
                                    let mut session = ClientSession::new(stream);
                                    thread::spawn(move || session.run());
                                    Ok(())
                                });
                            }
                            Err(e) => {
                                error!("failed TlsAcceptor: {}", e);
                            }
                        }
                    }
                    Err(err) => {
                        error!("accept failed: {}", err);
                        return;
                    }
                }
            }
        }
    }

    pub fn spawn_tls_listener(tls_server: &TlsDomainServer) -> Result<(), Error> {
        openssl::init();

        let mut acceptor = SslAcceptor::mozilla_modern(SslMethod::tls())?;

        if let Some(cert_file) = tls_server.pem_cert.as_ref() {
            acceptor.set_certificate_file(cert_file, SslFiletype::PEM)?;
        }
        if let Some(chain_file) = tls_server.pem_ca.as_ref() {
            acceptor.set_certificate_chain_file(chain_file)?;
        }
        if let Some(key_file) = tls_server.pem_private_key.as_ref() {
            acceptor.set_private_key_file(key_file, SslFiletype::PEM)?;
        }
        fn load_cert(name: &Path) -> anyhow::Result<X509> {
            let cert_bytes = read_bytes(name)?;
            log::trace!("loaded {}", name.display());
            Ok(X509::from_pem(&cert_bytes)?)
        }
        for name in &tls_server.pem_root_certs {
            if name.is_dir() {
                for entry in std::fs::read_dir(name)? {
                    if let Ok(cert) = load_cert(&entry?.path()) {
                        acceptor.cert_store_mut().add_cert(cert).ok();
                    }
                }
            } else {
                acceptor.cert_store_mut().add_cert(load_cert(name)?)?;
            }
        }

        acceptor.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

        let acceptor = acceptor.build();

        let mut net_listener = OpenSSLNetListener::new(
            TcpListener::bind(&tls_server.bind_address).with_context(|| {
                format!(
                    "error binding to mux_server_bind_address {}",
                    tls_server.bind_address,
                )
            })?,
            acceptor,
        );
        thread::spawn(move || {
            net_listener.run();
        });
        Ok(())
    }
}

pub struct ClientSession<S: ReadAndWrite> {
    stream: S,
    to_write_rx: PollableReceiver<DecodedPdu>,
    to_write_tx: PollableSender<DecodedPdu>,
    mux_rx: MuxSubscriber,
}

fn maybe_push_tab_changes(
    tab: &Rc<dyn Tab>,
    sender: PollableSender<DecodedPdu>,
) -> anyhow::Result<()> {
    let tab_id = tab.tab_id();
    let mouse_grabbed = tab.is_mouse_grabbed();
    let dims = tab.renderer().get_dimensions();

    let mut dirty_rows = tab.renderer().get_dirty_lines(
        dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
    );
    // Make sure we pick up "damage" done by switching between alt and primary screen
    dirty_rows.add_set(
        &tab.renderer()
            .get_dirty_lines(0..dims.viewport_rows as StableRowIndex),
    );

    let dirty_lines = dirty_rows.iter().cloned().collect();

    let cursor_position = tab.renderer().get_cursor_position();

    let title = tab.get_title();

    let (cursor_line, lines) = tab
        .renderer()
        .get_lines(cursor_position.y..cursor_position.y + 1);
    let bonus_lines = lines
        .into_iter()
        .enumerate()
        .map(|(idx, line)| (cursor_line + idx as StableRowIndex, line))
        .collect::<Vec<_>>()
        .into();

    sender.send(DecodedPdu {
        pdu: Pdu::GetTabRenderChangesResponse(GetTabRenderChangesResponse {
            tab_id,
            mouse_grabbed,
            dirty_lines,
            dimensions: dims,
            cursor_position,
            title,
            bonus_lines,
        }),
        serial: 0,
    })?;
    Ok(())
}

struct RemoteClipboard {
    sender: PollableSender<DecodedPdu>,
    tab_id: TabId,
}

impl Clipboard for RemoteClipboard {
    fn get_contents(&self) -> anyhow::Result<String> {
        Ok("".to_owned())
    }

    fn set_contents(&self, clipboard: Option<String>) -> anyhow::Result<()> {
        self.sender.send(DecodedPdu {
            serial: 0,
            pdu: Pdu::SetClipboard(SetClipboard {
                tab_id: self.tab_id,
                clipboard,
            }),
        })?;
        Ok(())
    }
}

struct BufferedTerminalHost<'a> {
    write: std::cell::RefMut<'a, dyn std::io::Write>,
    title: Option<String>,
}

impl<'a> term::TerminalHost for BufferedTerminalHost<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        &mut *self.write
    }

    fn click_link(&mut self, link: &Arc<term::cell::Hyperlink>) {
        log::error!(
            "nothing should call BufferedTerminalHost::click_link, but something did with {:?}",
            link
        );
    }

    fn set_title(&mut self, title: &str) {
        self.title.replace(title.to_owned());
    }
}

impl<S: ReadAndWrite> ClientSession<S> {
    fn new(stream: S) -> Self {
        let (to_write_tx, to_write_rx) =
            pollable_channel().expect("failed to create pollable_channel");
        let mux = Mux::get().expect("to be running on gui thread");
        let mux_rx = mux.subscribe().expect("Mux::subscribe to succeed");
        Self {
            stream,
            to_write_rx,
            to_write_tx,
            mux_rx,
        }
    }

    fn run(&mut self) {
        if let Err(e) = self.process() {
            error!("While processing session loop: {}", e);
        }
    }

    fn process(&mut self) -> Result<(), Error> {
        let mut read_buffer = Vec::with_capacity(1024);
        let mut tabs_to_output = HashSet::new();

        loop {
            loop {
                match self.to_write_rx.try_recv() {
                    Ok(decoded) => {
                        log::trace!("writing pdu with serial {}", decoded.serial);
                        decoded.pdu.encode(&mut self.stream, decoded.serial)?;
                        self.stream.flush()?;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => bail!("ClientSession was destroyed"),
                };
            }
            loop {
                match self.mux_rx.try_recv() {
                    Ok(notif) => match notif {
                        // Coalesce multiple TabOutputs for the same tab
                        MuxNotification::TabOutput(tab_id) => tabs_to_output.insert(tab_id),
                    },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => bail!("ClientSession was destroyed"),
                };
            }

            for tab_id in tabs_to_output.drain() {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(())
                });
            }

            let mut poll_array = [
                self.to_write_rx.as_poll_fd(),
                self.stream.as_poll_fd(),
                self.mux_rx.as_poll_fd(),
            ];
            poll_for_read(&mut poll_array);

            if poll_array[1].revents != 0 || self.stream.has_read_buffered() {
                loop {
                    self.stream.set_non_blocking(true)?;
                    let res = Pdu::try_read_and_decode(&mut self.stream, &mut read_buffer);
                    self.stream.set_non_blocking(false)?;
                    if let Some(decoded) = res? {
                        self.process_one(decoded)?;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn process_one(&mut self, decoded: DecodedPdu) -> anyhow::Result<()> {
        let start = Instant::now();
        let sender = self.to_write_tx.clone();
        let serial = decoded.serial;
        self.process_pdu(decoded.pdu).then(move |result| {
            let pdu = match result {
                Ok(pdu) => pdu,
                Err(err) => Pdu::ErrorResponse(ErrorResponse {
                    reason: format!("Error: {}", err),
                }),
            };
            log::trace!("{} processing time {:?}", serial, start.elapsed());
            sender.send(DecodedPdu { pdu, serial })
        });
        Ok(())
    }

    fn process_pdu(&mut self, pdu: Pdu) -> Future<Pdu> {
        match pdu {
            Pdu::Ping(Ping {}) => Future::ok(Pdu::Pong(Pong {})),
            Pdu::ListTabs(ListTabs {}) => Future::with_executor(executor(), move || {
                let mux = Mux::get().unwrap();
                let mut tabs = vec![];
                for window_id in mux.iter_windows().into_iter() {
                    let window = mux.get_window(window_id).unwrap();
                    for tab in window.iter() {
                        let dims = tab.renderer().get_dimensions();
                        tabs.push(WindowAndTabEntry {
                            window_id,
                            tab_id: tab.tab_id(),
                            title: tab.get_title(),
                            size: PtySize {
                                cols: dims.cols as u16,
                                rows: dims.viewport_rows as u16,
                                pixel_height: 0,
                                pixel_width: 0,
                            },
                        });
                    }
                }
                log::error!("ListTabs {:#?}", tabs);
                Ok(Pdu::ListTabsResponse(ListTabsResponse { tabs }))
            }),

            Pdu::WriteToTab(WriteToTab { tab_id, data }) => {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    tab.writer().write_all(&data)?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(Pdu::UnitResponse(UnitResponse {}))
                })
            }
            Pdu::SendPaste(SendPaste { tab_id, data }) => {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    tab.send_paste(&data)?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(Pdu::UnitResponse(UnitResponse {}))
                })
            }

            Pdu::Resize(Resize { tab_id, size }) => Future::with_executor(executor(), move || {
                let mux = Mux::get().unwrap();
                let tab = mux
                    .get_tab(tab_id)
                    .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                tab.resize(size)?;
                Ok(Pdu::UnitResponse(UnitResponse {}))
            }),

            Pdu::SendKeyDown(SendKeyDown { tab_id, event }) => {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    tab.key_down(event.key, event.modifiers)?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(Pdu::UnitResponse(UnitResponse {}))
                })
            }
            Pdu::SendMouseEvent(SendMouseEvent { tab_id, event }) => {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    let mut host = BufferedTerminalHost {
                        write: tab.writer(),
                        title: None,
                    };
                    tab.mouse_event(event, &mut host)?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(Pdu::UnitResponse(UnitResponse {}))
                })
            }

            Pdu::Spawn(spawn) => Future::with_executor(executor(), {
                let sender = self.to_write_tx.clone();
                move || {
                    let mux = Mux::get().unwrap();
                    let domain = mux.get_domain(spawn.domain_id).ok_or_else(|| {
                        anyhow!("domain {} not found on this server", spawn.domain_id)
                    })?;

                    let window_id = if let Some(window_id) = spawn.window_id {
                        mux.get_window_mut(window_id).ok_or_else(|| {
                            anyhow!("window_id {} not found on this server", window_id)
                        })?;
                        window_id
                    } else {
                        mux.new_empty_window()
                    };

                    let tab = domain.spawn(spawn.size, spawn.command, window_id)?;

                    let clip: Arc<dyn Clipboard> = Arc::new(RemoteClipboard {
                        tab_id: tab.tab_id(),
                        sender,
                    });
                    tab.set_clipboard(&clip);

                    Ok(Pdu::SpawnResponse(SpawnResponse {
                        tab_id: tab.tab_id(),
                        window_id,
                    }))
                }
            }),

            Pdu::GetTabRenderChanges(GetTabRenderChanges { tab_id, .. }) => {
                let sender = self.to_write_tx.clone();
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    maybe_push_tab_changes(&tab, sender)?;
                    Ok(Pdu::UnitResponse(UnitResponse {}))
                })
            }

            Pdu::GetLines(GetLines { tab_id, lines }) => {
                Future::with_executor(executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    let mut renderer = tab.renderer();

                    let mut lines_and_indices = vec![];

                    for range in lines {
                        let (first_row, lines) = renderer.get_lines(range);
                        for (idx, line) in lines.into_iter().enumerate() {
                            let stable_row = first_row + idx as StableRowIndex;
                            lines_and_indices.push((stable_row, line));
                        }
                    }
                    Ok(Pdu::GetLinesResponse(GetLinesResponse {
                        tab_id,
                        lines: lines_and_indices.into(),
                    }))
                })
            }

            Pdu::Invalid { .. } => Future::err(anyhow!("invalid PDU {:?}", pdu)),
            Pdu::Pong { .. }
            | Pdu::ListTabsResponse { .. }
            | Pdu::SetClipboard { .. }
            | Pdu::SpawnResponse { .. }
            | Pdu::GetTabRenderChangesResponse { .. }
            | Pdu::UnitResponse { .. }
            | Pdu::GetLinesResponse { .. }
            | Pdu::ErrorResponse { .. } => {
                Future::err(anyhow!("expected a request, got {:?}", pdu))
            }
        }
    }
}

/// Unfortunately, novice unix users can sometimes be running
/// with an overly permissive umask so we take care to install
/// a more restrictive mask while we might be creating things
/// in the filesystem.
/// This struct locks down the umask for its lifetime, restoring
/// the prior umask when it is dropped.
struct UmaskSaver {
    #[cfg(unix)]
    mask: mode_t,
}

impl UmaskSaver {
    fn new() -> Self {
        Self {
            #[cfg(unix)]
            mask: unsafe { umask(0o077) },
        }
    }
}

impl Drop for UmaskSaver {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            umask(self.mask);
        }
    }
}

/// Take care when setting up the listener socket;
/// we need to be sure that the directory that we create it in
/// is owned by the user and has appropriate file permissions
/// that prevent other users from manipulating its contents.
fn safely_create_sock_path(unix_dom: &UnixDomain) -> Result<UnixListener, Error> {
    let sock_path = &unix_dom.socket_path();
    debug!("setting up {}", sock_path.display());

    let _saver = UmaskSaver::new();

    let sock_dir = sock_path
        .parent()
        .ok_or_else(|| anyhow!("sock_path {} has no parent dir", sock_path.display()))?;

    create_user_owned_dirs(sock_dir)?;

    #[cfg(unix)]
    {
        use crate::running_under_wsl;
        use std::os::unix::fs::PermissionsExt;

        if !running_under_wsl() && !unix_dom.skip_permissions_check {
            // Let's be sure that the ownership looks sane
            let meta = sock_dir.symlink_metadata()?;

            let permissions = meta.permissions();
            if (permissions.mode() & 0o22) != 0 {
                bail!(
                    "The permissions for {} are insecure and currently \
                     allow other users to write to it (permissions={:?})",
                    sock_dir.display(),
                    permissions
                );
            }
        }
    }

    if sock_path.exists() {
        remove_file(sock_path)?;
    }

    UnixListener::bind(sock_path)
        .with_context(|| format!("Failed to bind to {}", sock_path.display()))
}

#[cfg(any(feature = "openssl", unix))]
fn spawn_tls_listener(tls_server: &TlsDomainServer) -> anyhow::Result<()> {
    ossl::spawn_tls_listener(tls_server)
}

#[cfg(not(any(feature = "openssl", unix)))]
fn spawn_tls_listener(tls_server: &TlsDomainServer) -> anyhow::Result<()> {
    not_ossl::spawn_tls_listener(tls_server)
}

pub fn spawn_listener() -> anyhow::Result<()> {
    let config = configuration();
    for unix_dom in &config.unix_domains {
        let mut listener = LocalListener::new(safely_create_sock_path(unix_dom)?);
        thread::spawn(move || {
            listener.run();
        });
    }

    for tls_server in &config.tls_servers {
        spawn_tls_listener(tls_server)?;
    }
    Ok(())
}
