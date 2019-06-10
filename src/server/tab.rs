use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use failure::Fallible;
use filedescriptor::Pipe;
use log::error;
use portable_pty::PtySize;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ops::Range;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{CursorPosition, Line};
use term::{KeyCode, KeyModifiers, MouseEvent, TerminalHost};
use termwiz::hyperlink::Hyperlink;
use termwiz::input::KeyEvent;

pub struct ClientTab {
    client: Arc<ClientInner>,
    local_tab_id: TabId,
    remote_tab_id: TabId,
    renderable: RefCell<RenderableState>,
    writer: RefCell<TabWriter>,
    reader: Pipe,
}

impl ClientTab {
    pub fn new(client: &Arc<ClientInner>, remote_tab_id: TabId) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };
        let render = RenderableState {
            client: Arc::clone(client),
            remote_tab_id,
            coarse: RefCell::new(None),
        };

        let reader = Pipe::new().expect("Pipe::new failed");

        Self {
            client: Arc::clone(client),
            remote_tab_id,
            local_tab_id,
            renderable: RefCell::new(render),
            writer: RefCell::new(writer),
            reader,
        }
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
        "a client tab".to_owned()
    }

    fn send_paste(&self, text: &str) -> Fallible<()> {
        let mut client = self.client.client.lock().unwrap();
        client.send_paste(SendPaste {
            tab_id: self.remote_tab_id,
            data: text.to_owned(),
        })?;
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
        let mut client = self.client.client.lock().unwrap();
        client.resize(Resize {
            tab_id: self.remote_tab_id,
            size,
        })?;
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Fallible<()> {
        let mut client = self.client.client.lock().unwrap();
        client.key_down(SendKeyDown {
            tab_id: self.remote_tab_id,
            event: KeyEvent {
                key: key,
                modifiers: mods,
            },
        })?;
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> Fallible<()> {
        let mut client = self.client.client.lock().unwrap();
        client.mouse_event(SendMouseEvent {
            tab_id: self.remote_tab_id,
            event,
        })?;
        Ok(())
    }

    fn advance_bytes(&self, buf: &[u8], host: &mut dyn TerminalHost) {
        panic!("ClientTab::advance_bytes not impl");
    }

    fn is_dead(&self) -> bool {
        false
    }

    fn palette(&self) -> ColorPalette {
        Default::default()
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }
}

struct RenderableState {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
    coarse: RefCell<Option<GetCoarseTabRenderableDataResponse>>,
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> CursorPosition {
        let coarse = self.coarse.borrow();
        if let Some(coarse) = coarse.as_ref() {
            coarse.cursor_position.clone()
        } else {
            CursorPosition::default()
        }
    }

    fn get_dirty_lines(&self) -> Vec<(usize, Line, Range<usize>)> {
        let coarse = self.coarse.borrow();
        if let Some(coarse) = coarse.as_ref() {
            coarse
                .dirty_lines
                .iter()
                .map(|dl| {
                    (
                        dl.line_idx,
                        dl.line.clone(),
                        dl.selection_col_from..dl.selection_col_to,
                    )
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn has_dirty_lines(&self) -> bool {
        let mut client = self.client.client.lock().unwrap();
        if let Ok(resp) = client.get_coarse_tab_renderable_data(GetCoarseTabRenderableData {
            tab_id: self.remote_tab_id,
        }) {
            let dirty = !resp.dirty_lines.is_empty();
            self.coarse.borrow_mut().replace(resp);
            dirty
        } else {
            self.coarse.borrow_mut().take();
            false
        }
    }

    fn make_all_lines_dirty(&mut self) {}

    fn clean_dirty_lines(&mut self) {
        self.coarse.borrow_mut().take();
    }

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        let coarse = self.coarse.borrow();
        coarse
            .as_ref()
            .and_then(|coarse| coarse.current_highlight.clone())
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        let coarse = self.coarse.borrow();
        if let Some(coarse) = coarse.as_ref() {
            (coarse.physical_rows, coarse.physical_cols)
        } else {
            (24, 80)
        }
    }
}

struct TabWriter {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
}

impl std::io::Write for TabWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        let mut client = self.client.client.lock().unwrap();
        client
            .write_to_tab(WriteToTab {
                tab_id: self.remote_tab_id,
                data: data.to_vec(),
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
