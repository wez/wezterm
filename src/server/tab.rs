use crate::frontend::gui_executor;
use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::server::client::Client;
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use failure::{bail, Fallible};
use filedescriptor::Pipe;
use log::error;
use portable_pty::PtySize;
use promise::Future;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use term::color::ColorPalette;
use term::selection::SelectionRange;
use term::{CursorPosition, Line};
use term::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TerminalHost};
use termwiz::hyperlink::Hyperlink;
use termwiz::input::KeyEvent;
use termwiz::surface::{Change, SequenceNo, Surface};

struct MouseState {
    future: Option<Future<()>>,
    queue: VecDeque<MouseEvent>,
    selection_range: Arc<Mutex<Option<SelectionRange>>>,
    something_changed: Arc<Mutex<bool>>,
    client: Client,
    remote_tab_id: TabId,
}

impl MouseState {
    fn append(&mut self, event: MouseEvent) {
        if let Some(last) = self.queue.back_mut() {
            if last.modifiers == event.modifiers {
                if last.kind == MouseEventKind::Move
                    && event.kind == MouseEventKind::Move
                    && last.button == event.button
                {
                    // Collapse any interim moves and just buffer up
                    // the last of them
                    *last = event;
                    return;
                }

                // Similarly, for repeated wheel scrolls, add up the deltas
                // rather than swamping the queue
                match (&last.button, &event.button) {
                    (MouseButton::WheelUp(a), MouseButton::WheelUp(b)) => {
                        last.button = MouseButton::WheelUp(a + b);
                        return;
                    }
                    (MouseButton::WheelDown(a), MouseButton::WheelDown(b)) => {
                        last.button = MouseButton::WheelDown(a + b);
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.queue.push_back(event);
        log::trace!("MouseEvent {}: queued", self.queue.len());
    }

    fn pop(&mut self) -> Fallible<Option<MouseEvent>> {
        if self.can_send()? {
            Ok(self.queue.pop_front())
        } else {
            Ok(None)
        }
    }

    fn can_send(&mut self) -> Fallible<bool> {
        if self.future.is_none() {
            Ok(true)
        } else {
            let ready = self.future.as_ref().map(Future::is_ready).unwrap_or(false);
            if ready {
                self.future.take().unwrap().wait()?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    fn next(state: &Arc<Mutex<Self>>) -> Fallible<()> {
        let mut mouse = state.lock().unwrap();
        if let Some(event) = mouse.pop()? {
            let selection_range = Arc::clone(&mouse.selection_range);
            let something_changed = Arc::clone(&mouse.something_changed);
            let state = Arc::clone(state);

            mouse.future = Some(
                mouse
                    .client
                    .mouse_event(SendMouseEvent {
                        tab_id: mouse.remote_tab_id,
                        event,
                    })
                    .then(move |resp| {
                        if let Ok(r) = resp.as_ref() {
                            *selection_range.lock().unwrap() = r.selection_range;
                            *something_changed.lock().unwrap() = true;
                        }
                        Future::with_executor(gui_executor().unwrap(), move || {
                            Self::next(&state)?;
                            Ok(())
                        });
                        Ok(())
                    }),
            );
        }
        Ok(())
    }
}

pub struct ClientTab {
    client: Arc<ClientInner>,
    local_tab_id: TabId,
    remote_tab_id: TabId,
    renderable: RefCell<RenderableState>,
    writer: RefCell<TabWriter>,
    reader: Pipe,
    mouse: Arc<Mutex<MouseState>>,
}

impl ClientTab {
    pub fn new(client: &Arc<ClientInner>, remote_tab_id: TabId) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };
        let selection_range = Arc::new(Mutex::new(None));
        let something_changed = Arc::new(Mutex::new(true));
        let mouse = Arc::new(Mutex::new(MouseState {
            selection_range: Arc::clone(&selection_range),
            something_changed: Arc::clone(&something_changed),
            remote_tab_id,
            client: client.client.clone(),
            future: None,
            queue: VecDeque::new(),
        }));
        let render = RenderableState {
            client: Arc::clone(client),
            remote_tab_id,
            last_poll: RefCell::new(Instant::now()),
            dead: RefCell::new(false),
            poll_future: RefCell::new(None),
            surface: RefCell::new(Surface::new(80, 24)),
            remote_sequence: RefCell::new(0),
            local_sequence: RefCell::new(0),
            selection_range,
            something_changed,
        };

        let reader = Pipe::new().expect("Pipe::new failed");

        Self {
            client: Arc::clone(client),
            mouse,
            remote_tab_id,
            local_tab_id,
            renderable: RefCell::new(render),
            writer: RefCell::new(writer),
            reader,
        }
    }

    pub fn process_unilateral(&self, pdu: Pdu) -> Fallible<()> {
        match pdu {
            Pdu::GetTabRenderChangesResponse(delta) => {
                log::trace!("new delta {}", delta.sequence_no);
                self.renderable
                    .borrow()
                    .apply_changes_to_surface(delta.sequence_no, delta.changes);
            }
            _ => bail!("unhandled unilateral pdu: {:?}", pdu),
        };
        Ok(())
    }
}

impl Tab for ClientTab {
    fn tab_id(&self) -> TabId {
        self.local_tab_id
    }
    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.renderable.borrow_mut()
    }

    fn get_title(&self) -> String {
        let renderable = self.renderable.borrow();
        let surface = renderable.surface.borrow();
        format!("[muxed] {} {}", surface.current_seqno(), surface.title())
    }

    fn send_paste(&self, text: &str) -> Fallible<()> {
        self.client.client.send_paste(SendPaste {
            tab_id: self.remote_tab_id,
            data: text.to_owned(),
        });
        Ok(())
    }

    fn reader(&self) -> Fallible<Box<dyn std::io::Read + Send>> {
        error!("made reader for ClientTab");
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> Fallible<()> {
        self.renderable
            .borrow()
            .surface
            .borrow_mut()
            .resize(size.cols as usize, size.rows as usize);
        self.client.client.resize(Resize {
            tab_id: self.remote_tab_id,
            size,
        });
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Fallible<()> {
        self.client.client.key_down(SendKeyDown {
            tab_id: self.remote_tab_id,
            event: KeyEvent {
                key,
                modifiers: mods,
            },
        });
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, _host: &mut dyn TerminalHost) -> Fallible<()> {
        self.mouse.lock().unwrap().append(event);
        MouseState::next(&self.mouse)?;
        Ok(())

        /*
        if resp.clipboard.is_some() {
            host.set_clipboard(resp.clipboard)?;
        }
        *self.renderable.borrow().selection_range.lock().unwrap() = resp.selection_range;
        */
    }

    fn advance_bytes(&self, _buf: &[u8], _host: &mut dyn TerminalHost) {
        panic!("ClientTab::advance_bytes not impl");
    }

    // clippy is wrong: the borrow checker hates returning the value directly
    #[allow(clippy::let_and_return)]
    fn is_dead(&self) -> bool {
        let renderable = self.renderable.borrow();
        let dead = *renderable.dead.borrow();
        dead
    }

    fn palette(&self) -> ColorPalette {
        Default::default()
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        *self.renderable.borrow().selection_range.lock().unwrap()
    }
}

struct RenderableState {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
    last_poll: RefCell<Instant>,
    dead: RefCell<bool>,
    poll_future: RefCell<Option<Future<UnitResponse>>>,
    surface: RefCell<Surface>,
    remote_sequence: RefCell<SequenceNo>,
    local_sequence: RefCell<SequenceNo>,
    selection_range: Arc<Mutex<Option<SelectionRange>>>,
    something_changed: Arc<Mutex<bool>>,
}

const POLL_INTERVAL: Duration = Duration::from_millis(50);

impl RenderableState {
    fn apply_changes_to_surface(&self, remote_seq: SequenceNo, changes: Vec<Change>) {
        if let Some(first) = changes.first().as_ref() {
            log::trace!("{:?}", first);
        }
        self.surface.borrow_mut().add_changes(changes);
        *self.remote_sequence.borrow_mut() = remote_seq;
    }

    fn poll(&self) -> Fallible<()> {
        let ready = self
            .poll_future
            .borrow()
            .as_ref()
            .map(Future::is_ready)
            .unwrap_or(false);
        if ready {
            self.poll_future.borrow_mut().take().unwrap().wait()?;
            *self.last_poll.borrow_mut() = Instant::now();
        } else if self.poll_future.borrow().is_some() {
            // We have a poll in progress
            return Ok(());
        }

        let last = *self.last_poll.borrow();
        if last.elapsed() < POLL_INTERVAL {
            return Ok(());
        }

        {
            *self.last_poll.borrow_mut() = Instant::now();
            *self.poll_future.borrow_mut() = Some(self.client.client.get_tab_render_changes(
                GetTabRenderChanges {
                    tab_id: self.remote_tab_id,
                    sequence_no: *self.remote_sequence.borrow(),
                },
            ));
        }
        Ok(())
    }
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> CursorPosition {
        let (x, y) = self.surface.borrow().cursor_position();
        CursorPosition { x, y: y as i64 }
    }

    fn get_dirty_lines(&self) -> Vec<(usize, Line, Range<usize>)> {
        let mut surface = self.surface.borrow_mut();
        let seq = surface.current_seqno();
        surface.flush_changes_older_than(seq);
        let selection = *self.selection_range.lock().unwrap();
        *self.something_changed.lock().unwrap() = false;
        *self.local_sequence.borrow_mut() = seq;
        surface
            .screen_lines()
            .into_iter()
            .enumerate()
            .map(|(idx, line)| {
                let r = match selection {
                    None => 0..0,
                    Some(sel) => sel.cols_for_row(idx as i32),
                };
                (idx, line, r)
            })
            .collect()
    }

    fn has_dirty_lines(&self) -> bool {
        if self.poll().is_err() {
            *self.dead.borrow_mut() = true;
        }
        if *self.something_changed.lock().unwrap() {
            return true;
        }
        self.surface
            .borrow()
            .has_changes(*self.local_sequence.borrow())
    }

    fn make_all_lines_dirty(&mut self) {}

    fn clean_dirty_lines(&mut self) {}

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        None
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        let (cols, rows) = self.surface.borrow().dimensions();
        (rows, cols)
    }
}

struct TabWriter {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
}

impl std::io::Write for TabWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        self.client
            .client
            .write_to_tab(WriteToTab {
                tab_id: self.remote_tab_id,
                data: data.to_vec(),
            })
            .wait()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
