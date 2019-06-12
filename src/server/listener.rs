use crate::config::Config;
use crate::mux::Mux;
use crate::server::codec::*;
use crate::server::{UnixListener, UnixStream};
use failure::{bail, err_msg, format_err, Error, Fallible};
#[cfg(unix)]
use libc::{mode_t, umask};
use log::{debug, error, warn};
use promise::{Executor, Future};
use std::fs::{remove_file, DirBuilder};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;
use std::thread;

pub struct Listener {
    acceptor: UnixListener,
    executor: Box<dyn Executor>,
}

impl Listener {
    pub fn new(acceptor: UnixListener, executor: Box<dyn Executor>) -> Self {
        Self { acceptor, executor }
    }

    fn run(&mut self) {
        for stream in self.acceptor.incoming() {
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

pub struct ClientSession {
    stream: UnixStream,
    executor: Box<dyn Executor>,
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

impl ClientSession {
    fn new(stream: UnixStream, executor: Box<dyn Executor>) -> Self {
        Self { stream, executor }
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

            Pdu::Invalid { .. } => bail!("invalid PDU {:?}", pdu),
            Pdu::Pong { .. }
            | Pdu::ListTabsResponse { .. }
            | Pdu::SendMouseEventResponse { .. }
            | Pdu::GetCoarseTabRenderableDataResponse { .. }
            | Pdu::SpawnResponse { .. }
            | Pdu::UnitResponse { .. }
            | Pdu::ErrorResponse { .. } => bail!("expected a request, got {:?}", pdu),
        })
    }

    fn process_one(&mut self) -> Fallible<()> {
        let decoded = Pdu::decode(&mut self.stream)?;
        debug!("got pdu {:?} from client", decoded);
        let response = self.process_pdu(decoded.pdu).unwrap_or_else(|e| {
            Pdu::ErrorResponse(ErrorResponse {
                reason: format!("Error: {}", e),
            })
        });
        response.encode(&mut self.stream, decoded.serial)?;
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
        if !std::env::var_os("WEZTERM_SKIP_MUX_SOCK_PERMISSIONS_CHECK").is_some() {
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
    let mut listener = Listener::new(safely_create_sock_path(sock_path)?, executor);
    thread::spawn(move || {
        listener.run();
    });
    Ok(())
}
