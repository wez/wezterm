use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::server::domain::ClientInner;
use failure::{bail, Fallible};
use portable_pty::PtySize;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ops::Range;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{CursorPosition, Line};
use term::{KeyCode, KeyModifiers, MouseEvent, TerminalHost};
use termwiz::hyperlink::Hyperlink;

pub struct ClientTab {
    client: Arc<ClientInner>,
    local_tab_id: TabId,
    remote_tab_id: TabId,
    renderable: RefCell<RenderableState>,
}

impl ClientTab {
    pub fn new(client: &Arc<ClientInner>, remote_tab_id: TabId) -> Self {
        let local_tab_id = alloc_tab_id();
        Self {
            client: Arc::clone(client),
            remote_tab_id,
            local_tab_id,
            renderable: RefCell::new(RenderableState { dirty: true }),
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
        bail!("ClientTab::send_paste not impl");
    }

    fn reader(&self) -> Fallible<Box<dyn std::io::Read + Send>> {
        bail!("ClientTab::reader not impl");
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        panic!("ClientTab::writer not impl");
    }

    fn resize(&self, size: PtySize) -> Fallible<()> {
        bail!("ClientTab::resize not impl");
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Fallible<()> {
        bail!("ClientTab::key_down not impl");
    }

    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> Fallible<()> {
        bail!("ClientTab::mouse_event not impl");
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
    dirty: bool,
}

impl Renderable for RenderableState {
    fn get_cursor_position(&self) -> CursorPosition {
        CursorPosition::default()
    }

    fn get_dirty_lines(&self) -> Vec<(usize, &Line, Range<usize>)> {
        vec![]
    }

    fn has_dirty_lines(&self) -> bool {
        self.dirty
    }

    fn make_all_lines_dirty(&mut self) {
        self.dirty = true;
    }

    fn clean_dirty_lines(&mut self) {
        self.dirty = false;
    }

    fn current_highlight(&self) -> Option<Arc<Hyperlink>> {
        None
    }

    fn physical_dimensions(&self) -> (usize, usize) {
        (24, 80)
    }
}
