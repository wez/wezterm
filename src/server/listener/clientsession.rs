use crate::mux::renderable::{RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{Tab, TabId};
use crate::mux::{Mux, MuxNotification, MuxSubscriber};
use crate::server::codec::*;
use crate::server::pollable::*;
use anyhow::{anyhow, bail, Context, Error};
use crossbeam::channel::TryRecvError;
use log::error;
use portable_pty::PtySize;
use promise::spawn::spawn_into_main_thread;
use rangeset::RangeSet;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use term::terminal::Clipboard;
use term::StableRowIndex;
use url::Url;

pub struct ClientSession<S: ReadAndWrite> {
    stream: S,
    to_write_rx: PollableReceiver<DecodedPdu>,
    to_write_tx: PollableSender<DecodedPdu>,
    mux_rx: MuxSubscriber,
    per_tab: HashMap<TabId, Arc<Mutex<PerTab>>>,
}

#[derive(Default, Debug)]
struct PerTab {
    cursor_position: StableCursorPosition,
    title: String,
    working_dir: Option<Url>,
    dimensions: RenderableDimensions,
    dirty_lines: RangeSet<StableRowIndex>,
    mouse_grabbed: bool,
}

impl PerTab {
    fn compute_changes(&mut self, tab: &Rc<dyn Tab>) -> Option<GetTabRenderChangesResponse> {
        let mut changed = false;
        let mouse_grabbed = tab.is_mouse_grabbed();
        if mouse_grabbed != self.mouse_grabbed {
            changed = true;
        }

        let dims = tab.renderer().get_dimensions();
        if dims != self.dimensions {
            changed = true;
        }

        let cursor_position = tab.renderer().get_cursor_position();
        if cursor_position != self.cursor_position {
            changed = true;
        }

        let title = tab.get_title();
        if title != self.title {
            changed = true;
        }

        let working_dir = tab.get_current_working_dir();
        if working_dir != self.working_dir {
            changed = true;
        }

        let mut all_dirty_lines = tab
            .renderer()
            .get_dirty_lines(0..dims.physical_top + dims.viewport_rows as StableRowIndex);
        let dirty_delta = all_dirty_lines.difference(&self.dirty_lines);
        if !dirty_delta.is_empty() {
            changed = true;
        }

        if !changed {
            return None;
        }

        // Figure out what we're going to send as dirty lines vs bonus lines
        let viewport_range =
            dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex;

        let (first_line, lines) = tab.renderer().get_lines(viewport_range);
        let mut bonus_lines = lines
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let stable_row = first_line + idx as StableRowIndex;
                all_dirty_lines.remove(stable_row);
                (stable_row, line)
            })
            .collect::<Vec<_>>();

        // Always send the cursor's row, as that tends to the busiest and we don't
        // have a sequencing concept for our idea of the remote state.
        let (cursor_line, lines) = tab
            .renderer()
            .get_lines(cursor_position.y..cursor_position.y + 1);
        bonus_lines.push((cursor_line, lines[0].clone()));

        self.cursor_position = cursor_position;
        self.title = title.clone();
        self.working_dir = working_dir.clone();
        self.dimensions = dims;
        self.dirty_lines = all_dirty_lines;
        self.mouse_grabbed = mouse_grabbed;

        let dirty_lines = dirty_delta.iter().cloned().collect();
        let bonus_lines = bonus_lines.into();
        Some(GetTabRenderChangesResponse {
            tab_id: tab.tab_id(),
            mouse_grabbed,
            dirty_lines,
            dimensions: dims,
            cursor_position,
            title,
            bonus_lines,
            working_dir: working_dir.map(Into::into),
        })
    }

    fn mark_clean(&mut self, stable_row: StableRowIndex) {
        self.dirty_lines.remove(stable_row);
    }
}

fn maybe_push_tab_changes(
    tab: &Rc<dyn Tab>,
    sender: PollableSender<DecodedPdu>,
    per_tab: Arc<Mutex<PerTab>>,
) -> anyhow::Result<()> {
    let mut per_tab = per_tab.lock().unwrap();
    if let Some(resp) = per_tab.compute_changes(tab) {
        sender.send(DecodedPdu {
            pdu: Pdu::GetTabRenderChangesResponse(resp),
            serial: 0,
        })?;
    }
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
    pub fn new(stream: S) -> Self {
        let (to_write_tx, to_write_rx) =
            pollable_channel().expect("failed to create pollable_channel");
        let mux = Mux::get().expect("to be running on gui thread");
        let mux_rx = mux.subscribe().expect("Mux::subscribe to succeed");
        Self {
            stream,
            to_write_rx,
            to_write_tx,
            mux_rx,
            per_tab: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        if let Err(e) = self.process() {
            error!("While processing session loop: {}", e);
        }
    }

    fn per_tab(&mut self, tab_id: TabId) -> Arc<Mutex<PerTab>> {
        Arc::clone(
            self.per_tab
                .entry(tab_id)
                .or_insert_with(|| Arc::new(Mutex::new(PerTab::default()))),
        )
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
                        self.stream.flush().context("while flushing stream")?;
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
                    Err(TryRecvError::Disconnected) => bail!("mux_rx is Disconnected"),
                };
            }

            for tab_id in tabs_to_output.drain() {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    let mux = Mux::get().unwrap();
                    let tab = mux
                        .get_tab(tab_id)
                        .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                    maybe_push_tab_changes(&tab, sender, per_tab)?;
                    Ok::<(), anyhow::Error>(())
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
                    match res {
                        Ok(Some(decoded)) => self.process_one(decoded),
                        Ok(None) => break,
                        Err(err) => {
                            log::error!("Error decoding: {}", err);
                            return Err(err);
                        }
                    }
                }
            }
        }
    }

    fn process_one(&mut self, decoded: DecodedPdu) {
        let start = Instant::now();
        let sender = self.to_write_tx.clone();
        let serial = decoded.serial;

        let send_response = move |result: anyhow::Result<Pdu>| {
            let pdu = match result {
                Ok(pdu) => pdu,
                Err(err) => Pdu::ErrorResponse(ErrorResponse {
                    reason: format!("Error: {}", err),
                }),
            };
            log::trace!("{} processing time {:?}", serial, start.elapsed());
            sender
                .send(DecodedPdu { pdu, serial })
                .expect("failed to send DecodedPdu to sender")
        };

        fn catch<F, SND>(f: F, send_response: SND)
        where
            F: FnOnce() -> anyhow::Result<Pdu>,
            SND: Fn(anyhow::Result<Pdu>),
        {
            send_response(f());
        }

        match decoded.pdu {
            Pdu::Ping(Ping {}) => send_response(Ok(Pdu::Pong(Pong {}))),
            Pdu::ListTabs(ListTabs {}) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let mut tabs = vec![];
                            for window_id in mux.iter_windows().into_iter() {
                                let window = mux.get_window(window_id).unwrap();
                                for tab in window.iter() {
                                    let dims = tab.renderer().get_dimensions();
                                    let working_dir = tab.get_current_working_dir();
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
                                        working_dir: working_dir.map(Into::into),
                                    });
                                }
                            }
                            log::error!("ListTabs {:#?}", tabs);
                            Ok(Pdu::ListTabsResponse(ListTabsResponse { tabs }))
                        },
                        send_response,
                    )
                });
            }

            Pdu::WriteToTab(WriteToTab { tab_id, data }) => {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            tab.writer().write_all(&data)?;
                            maybe_push_tab_changes(&tab, sender, per_tab)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    );
                });
            }
            Pdu::SendPaste(SendPaste { tab_id, data }) => {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            tab.send_paste(&data)?;
                            maybe_push_tab_changes(&tab, sender, per_tab)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                });
            }

            Pdu::Resize(Resize { tab_id, size }) => {
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            tab.resize(size)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                });
            }

            Pdu::SendKeyDown(SendKeyDown { tab_id, event }) => {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            tab.key_down(event.key, event.modifiers)?;
                            maybe_push_tab_changes(&tab, sender, per_tab)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                });
            }
            Pdu::SendMouseEvent(SendMouseEvent { tab_id, event }) => {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            let mut host = BufferedTerminalHost {
                                write: tab.writer(),
                                title: None,
                            };
                            tab.mouse_event(event, &mut host)?;
                            maybe_push_tab_changes(&tab, sender, per_tab)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                });
            }

            Pdu::Spawn(spawn) => {
                let sender = self.to_write_tx.clone();
                spawn_into_main_thread(async move {
                    schedule_domain_spawn(spawn, sender, send_response);
                });
            }

            Pdu::GetTabRenderChanges(GetTabRenderChanges { tab_id, .. }) => {
                let sender = self.to_write_tx.clone();
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            maybe_push_tab_changes(&tab, sender, per_tab)?;
                            Ok(Pdu::UnitResponse(UnitResponse {}))
                        },
                        send_response,
                    )
                });
            }

            Pdu::GetLines(GetLines { tab_id, lines }) => {
                let per_tab = self.per_tab(tab_id);
                spawn_into_main_thread(async move {
                    catch(
                        move || {
                            let mux = Mux::get().unwrap();
                            let tab = mux
                                .get_tab(tab_id)
                                .ok_or_else(|| anyhow!("no such tab {}", tab_id))?;
                            let mut renderer = tab.renderer();

                            let mut lines_and_indices = vec![];
                            let mut per_tab = per_tab.lock().unwrap();

                            for range in lines {
                                let (first_row, lines) = renderer.get_lines(range);
                                for (idx, line) in lines.into_iter().enumerate() {
                                    let stable_row = first_row + idx as StableRowIndex;
                                    per_tab.mark_clean(stable_row);
                                    lines_and_indices.push((stable_row, line));
                                }
                            }
                            Ok(Pdu::GetLinesResponse(GetLinesResponse {
                                tab_id,
                                lines: lines_and_indices.into(),
                            }))
                        },
                        send_response,
                    )
                });
            }

            Pdu::GetCodecVersion(_) => {
                send_response(Ok(Pdu::GetCodecVersionResponse(GetCodecVersionResponse {
                    codec_vers: CODEC_VERSION,
                    version_string: crate::wezterm_version().to_owned(),
                })))
            }

            Pdu::Invalid { .. } => send_response(Err(anyhow!("invalid PDU {:?}", decoded.pdu))),
            Pdu::Pong { .. }
            | Pdu::ListTabsResponse { .. }
            | Pdu::SetClipboard { .. }
            | Pdu::SpawnResponse { .. }
            | Pdu::GetTabRenderChangesResponse { .. }
            | Pdu::UnitResponse { .. }
            | Pdu::GetLinesResponse { .. }
            | Pdu::GetCodecVersionResponse { .. }
            | Pdu::ErrorResponse { .. } => {
                send_response(Err(anyhow!("expected a request, got {:?}", decoded.pdu)))
            }
        }
    }
}

// Dancing around a little bit here; we can't directly spawn_into_main_thread the domain_spawn
// function below because the compiler thinks that all of its locals then need to be Send.
// We need to shimmy through this helper to break that aspect of the compiler flow
// analysis and allow things to compile.
fn schedule_domain_spawn<SND>(spawn: Spawn, sender: PollableSender<DecodedPdu>, send_response: SND)
where
    SND: Fn(anyhow::Result<Pdu>) + 'static,
{
    promise::spawn::spawn(async move { send_response(domain_spawn(spawn, sender).await) });
}

async fn domain_spawn(spawn: Spawn, sender: PollableSender<DecodedPdu>) -> anyhow::Result<Pdu> {
    let mux = Mux::get().unwrap();
    let domain = mux
        .get_domain(spawn.domain_id)
        .ok_or_else(|| anyhow!("domain {} not found on this server", spawn.domain_id))?;

    let window_id = if let Some(window_id) = spawn.window_id {
        mux.get_window_mut(window_id)
            .ok_or_else(|| anyhow!("window_id {} not found on this server", window_id))?;
        window_id
    } else {
        mux.new_empty_window()
    };

    let tab = domain
        .spawn(spawn.size, spawn.command, spawn.command_dir, window_id)
        .await?;

    let clip: Arc<dyn Clipboard> = Arc::new(RemoteClipboard {
        tab_id: tab.tab_id(),
        sender,
    });
    tab.set_clipboard(&clip);

    Ok::<Pdu, anyhow::Error>(Pdu::SpawnResponse(SpawnResponse {
        tab_id: tab.tab_id(),
        window_id,
    }))
}
