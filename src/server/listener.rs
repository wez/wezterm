use crate::config::Config;
use crate::mux::renderable::Renderable;
use crate::mux::tab::TabId;
use crate::mux::Mux;
use crate::server::codec::*;
use crate::server::UnixListener;
use failure::{bail, err_msg, format_err, Error, Fallible};
#[cfg(unix)]
use libc::{mode_t, umask};
use log::{debug, error, warn};
use native_tls::{Identity, TlsAcceptor};
use promise::{Executor, Future};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fs::{remove_file, DirBuilder};
use std::io::Read;
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use termwiz::surface::{Change, Position, SequenceNo, Surface};

struct LocalListener {
    listener: UnixListener,
    executor: Box<dyn Executor>,
}

impl LocalListener {
    pub fn new(listener: UnixListener, executor: Box<dyn Executor>) -> Self {
        Self { listener, executor }
    }

    fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let executor = self.executor.clone_executor();
                    let mut session = ClientSession::new(stream, executor);
                    thread::spawn(move || session.run());
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

fn read_bytes<T: AsRef<Path>>(path: T) -> Fallible<Vec<u8>> {
    let path = path.as_ref();
    let mut f = std::fs::File::open(path)
        .map_err(|e| format_err!("opening file {}: {}", path.display(), e))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(any(feature = "openssl", all(unix, not(target_os = "macos"))))]
fn pem_files_to_identity(
    key: PathBuf,
    cert: Option<PathBuf>,
    chain: Option<PathBuf>,
) -> Fallible<Identity> {
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
    Identity::from_pkcs12(&der, password).map_err(|e| {
        format_err!(
            "error creating identity from pkcs12 generated \
             from PemFiles {}, {:?}, {:?}: {}",
            key.display(),
            cert,
            chain,
            e
        )
    })
}

#[cfg(not(any(feature = "openssl", all(unix, not(target_os = "macos")))))]
fn pem_files_to_identity(
    _key: PathBuf,
    _cert: Option<PathBuf>,
    _chain: Option<PathBuf>,
) -> Fallible<Identity> {
    bail!("recompile wezterm using --features openssl")
}

impl TryFrom<IdentitySource> for Identity {
    type Error = Error;

    fn try_from(source: IdentitySource) -> Fallible<Identity> {
        match source {
            IdentitySource::Pkcs12File { path, password } => {
                let bytes = read_bytes(&path)?;
                Identity::from_pkcs12(&bytes, &password).map_err(|e| {
                    format_err!("error loading pkcs12 file '{}': {}", path.display(), e)
                })
            }
            IdentitySource::PemFiles { key, cert, chain } => {
                pem_files_to_identity(key, cert, chain)
            }
        }
    }
}

struct NetListener {
    acceptor: Arc<TlsAcceptor>,
    listener: TcpListener,
    executor: Box<dyn Executor>,
}

impl NetListener {
    pub fn new(listener: TcpListener, acceptor: TlsAcceptor, executor: Box<dyn Executor>) -> Self {
        Self {
            listener,
            acceptor: Arc::new(acceptor),
            executor,
        }
    }

    fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    stream.set_nodelay(true).ok();
                    let executor = self.executor.clone_executor();
                    let acceptor = self.acceptor.clone();
                    thread::spawn(move || match acceptor.accept(stream) {
                        Ok(stream) => {
                            let mut session = ClientSession::new(stream, executor);
                            session.run();
                        }
                        Err(e) => {
                            error!("failed TlsAcceptor: {}", e);
                        }
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

pub struct ClientSession<S: std::io::Read + std::io::Write> {
    stream: S,
    executor: Box<dyn Executor>,
    surfaces_by_tab: Arc<Mutex<HashMap<TabId, Surface>>>,
}

struct BufferedTerminalHost<'a> {
    write: std::cell::RefMut<'a, dyn std::io::Write>,
    clipboard: Option<String>,
    title: Option<String>,
}

impl<'a> term::TerminalHost for BufferedTerminalHost<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        &mut *self.write
    }

    fn click_link(&mut self, link: &Arc<term::cell::Hyperlink>) {
        error!("ignoring url open of {:?}", link.uri());
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        warn!("peer requested clipboard; ignoring");
        Ok("".into())
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        if let Some(clip) = clip {
            self.clipboard.replace(clip);
        }
        Ok(())
    }

    fn set_title(&mut self, title: &str) {
        self.title.replace(title.to_owned());
    }
}

fn update_surface_from_screen(
    surface: &mut Surface,
    renderable: &mut dyn Renderable,
) -> SequenceNo {
    let (rows, cols) = renderable.physical_dimensions();
    let (surface_width, surface_height) = surface.dimensions();

    if (rows != surface_height) || (cols != surface_width) {
        surface.resize(cols, rows);
        renderable.make_all_lines_dirty();
    }

    let (x, y) = surface.cursor_position();
    let cursor = renderable.get_cursor_position();
    if (x != cursor.x) || (y as i64 != cursor.y) {
        surface.add_change(Change::CursorPosition {
            x: Position::Absolute(cursor.x),
            y: Position::Absolute(cursor.y as usize),
        });
    }

    let mut changes = vec![];

    for (line_idx, line, _selrange) in renderable.get_dirty_lines() {
        changes.append(&mut surface.diff_against_numbered_line(line_idx, &line));
    }

    surface.add_changes(changes)
}

impl<S: std::io::Read + std::io::Write> ClientSession<S> {
    fn new(stream: S, executor: Box<dyn Executor>) -> Self {
        Self {
            stream,
            executor,
            surfaces_by_tab: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn process(&mut self) -> Result<(), Error> {
        loop {
            self.process_one()?;
        }
    }

    fn process_pdu(&mut self, pdu: Pdu) -> Fallible<Pdu> {
        Ok(match pdu {
            Pdu::Ping(Ping {}) => Pdu::Pong(Pong {}),
            Pdu::ListTabs(ListTabs {}) => {
                let result = Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let mut tabs = vec![];
                    for window_id in mux.iter_windows().into_iter() {
                        let window = mux.get_window(window_id).unwrap();
                        for tab in window.iter() {
                            tabs.push(WindowAndTabEntry {
                                window_id,
                                tab_id: tab.tab_id(),
                                title: tab.get_title(),
                            });
                        }
                    }
                    log::error!("ListTabs {:#?}", tabs);
                    Ok(ListTabsResponse { tabs })
                })
                .wait()?;
                Pdu::ListTabsResponse(result)
            }
            Pdu::GetCoarseTabRenderableData(GetCoarseTabRenderableData { tab_id, dirty_all }) => {
                let result = Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    let title = tab.get_title();
                    let mut renderable = tab.renderer();
                    if dirty_all {
                        renderable.make_all_lines_dirty();
                    }

                    let dirty_lines = renderable
                        .get_dirty_lines()
                        .iter()
                        .map(|(line_idx, line, sel)| DirtyLine {
                            line_idx: *line_idx,
                            line: (*line).clone(),
                            selection_col_from: sel.start,
                            selection_col_to: sel.end,
                        })
                        .collect();
                    renderable.clean_dirty_lines();

                    let (physical_rows, physical_cols) = renderable.physical_dimensions();

                    Ok(GetCoarseTabRenderableDataResponse {
                        dirty_lines,
                        current_highlight: renderable.current_highlight(),
                        cursor_position: renderable.get_cursor_position(),
                        physical_rows,
                        physical_cols,
                        title,
                    })
                })
                .wait()?;
                Pdu::GetCoarseTabRenderableDataResponse(result)
            }

            Pdu::WriteToTab(WriteToTab { tab_id, data }) => {
                Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    tab.writer().write_all(&data)?;
                    Ok(())
                })
                .wait()?;
                Pdu::UnitResponse(UnitResponse {})
            }
            Pdu::SendPaste(SendPaste { tab_id, data }) => {
                Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    tab.send_paste(&data)?;
                    Ok(())
                })
                .wait()?;
                Pdu::UnitResponse(UnitResponse {})
            }

            Pdu::Resize(Resize { tab_id, size }) => {
                Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    tab.resize(size)?;
                    Ok(())
                })
                .wait()?;
                Pdu::UnitResponse(UnitResponse {})
            }

            Pdu::SendKeyDown(SendKeyDown { tab_id, event }) => {
                Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    tab.key_down(event.key, event.modifiers)?;
                    Ok(())
                })
                .wait()?;
                Pdu::UnitResponse(UnitResponse {})
            }
            Pdu::SendMouseEvent(SendMouseEvent { tab_id, event }) => {
                let clipboard = Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    let mut host = BufferedTerminalHost {
                        write: tab.writer(),
                        clipboard: None,
                        title: None,
                    };
                    tab.mouse_event(event, &mut host)?;
                    Ok(host.clipboard)
                })
                .wait()?;
                Pdu::SendMouseEventResponse(SendMouseEventResponse { clipboard })
            }

            Pdu::Spawn(spawn) => {
                let result = Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let domain = mux.get_domain(spawn.domain_id).ok_or_else(|| {
                        format_err!("domain {} not found on this server", spawn.domain_id)
                    })?;

                    let window_id = if let Some(window_id) = spawn.window_id {
                        mux.get_window_mut(window_id).ok_or_else(|| {
                            format_err!("window_id {} not found on this server", window_id)
                        })?;
                        window_id
                    } else {
                        mux.new_empty_window()
                    };

                    let tab = domain.spawn(spawn.size, spawn.command, window_id)?;
                    Ok(SpawnResponse {
                        tab_id: tab.tab_id(),
                        window_id,
                    })
                })
                .wait()?;
                Pdu::SpawnResponse(result)
            }

            Pdu::GetTabRenderChanges(GetTabRenderChanges {
                tab_id,
                sequence_no,
            }) => {
                let surfaces = Arc::clone(&self.surfaces_by_tab);
                Future::with_executor(self.executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| format_err!("no such tab {}", tab_id))?;
                    let mut surfaces = surfaces.lock().unwrap();
                    let (rows, cols) = tab.renderer().physical_dimensions();
                    let surface = surfaces
                        .entry(tab_id)
                        .or_insert_with(|| Surface::new(cols, rows));
                    update_surface_from_screen(surface, &mut *tab.renderer());
                    let title = tab.get_title();
                    if title != surface.title() {
                        log::error!("recording surface title {}", title);
                        surface.add_change(Change::Title(title));
                    }

                    let (new_seq, changes) = surface.get_changes(sequence_no);
                    let changes = changes.into_owned();

                    // Keep the change log in the surface bounded;
                    // we don't completely blow away the log each time
                    // so that multiple clients have an opportunity to
                    // resync from a smaller delta
                    surface.flush_changes_older_than(new_seq - (rows * cols * 2));
                    Ok(Pdu::GetTabRenderChangesResponse(
                        GetTabRenderChangesResponse {
                            sequence_no: new_seq,
                            changes,
                        },
                    ))
                })
                .wait()?
            }

            Pdu::Invalid { .. } => bail!("invalid PDU {:?}", pdu),
            Pdu::Pong { .. }
            | Pdu::ListTabsResponse { .. }
            | Pdu::SendMouseEventResponse { .. }
            | Pdu::GetCoarseTabRenderableDataResponse { .. }
            | Pdu::SpawnResponse { .. }
            | Pdu::GetTabRenderChangesResponse { .. }
            | Pdu::UnitResponse { .. }
            | Pdu::ErrorResponse { .. } => bail!("expected a request, got {:?}", pdu),
        })
    }

    fn process_one(&mut self) -> Fallible<()> {
        let start = Instant::now();
        let decoded = Pdu::decode(&mut self.stream)?;
        debug!("got pdu {:?} from client in {:?}", decoded, start.elapsed());

        let start = Instant::now();
        let response = self.process_pdu(decoded.pdu).unwrap_or_else(|e| {
            Pdu::ErrorResponse(ErrorResponse {
                reason: format!("Error: {}", e),
            })
        });
        log::trace!("processing time {:?}", start.elapsed());

        let start = Instant::now();
        response.encode(&mut self.stream, decoded.serial)?;
        self.stream.flush()?;
        log::trace!("encode and send in {:?}", start.elapsed());

        Ok(())
    }

    fn run(&mut self) {
        if let Err(e) = self.process() {
            error!("While processing session loop: {}", e);
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
fn safely_create_sock_path(sock_path: &str) -> Result<UnixListener, Error> {
    let sock_path = Path::new(sock_path);

    debug!("setting up {}", sock_path.display());

    let _saver = UmaskSaver::new();

    let sock_dir = sock_path
        .parent()
        .ok_or_else(|| format_err!("sock_path {} has no parent dir", sock_path.display()))?;

    let mut builder = DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    {
        builder.mode(0o700);
    }

    builder.create(sock_dir)?;

    #[cfg(unix)]
    {
        if std::env::var_os("WEZTERM_SKIP_MUX_SOCK_PERMISSIONS_CHECK").is_none() {
            // Let's be sure that the ownership looks sane
            let meta = sock_dir.symlink_metadata()?;

            let permissions = meta.permissions();
            if (permissions.mode() & 0o22) != 0 {
                bail!(
                    "The permissions for {} are insecure and currently
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
        .map_err(|e| format_err!("Failed to bind to {}: {}", sock_path.display(), e))
}

pub fn spawn_listener(config: &Arc<Config>, executor: Box<dyn Executor>) -> Result<(), Error> {
    let sock_path = config
        .mux_server_unix_domain_socket_path
        .as_ref()
        .ok_or_else(|| err_msg("no mux_server_unix_domain_socket_path"))?;
    let mut listener = LocalListener::new(
        safely_create_sock_path(sock_path)?,
        executor.clone_executor(),
    );
    thread::spawn(move || {
        listener.run();
    });

    if let Some(address) = &config.mux_server_bind_address {
        let identity = IdentitySource::PemFiles {
            key: config
                .mux_server_pem_private_key
                .as_ref()
                .ok_or_else(|| err_msg("missing mux_server_pem_private_key config value"))?
                .into(),
            cert: config.mux_server_pem_cert.clone(),
            chain: config.mux_server_pem_ca.clone(),
        };

        let mut net_listener = NetListener::new(
            TcpListener::bind(address).map_err(|e| {
                format_err!(
                    "error binding to mux_server_bind_address {}: {}",
                    address,
                    e
                )
            })?,
            TlsAcceptor::new(identity.try_into()?)?,
            executor,
        );
        thread::spawn(move || {
            net_listener.run();
        });
    }

    Ok(())
}
