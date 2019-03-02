use crate::{Child, MasterPty};
use failure::Error;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use term::{KeyCode, KeyModifiers, MouseEvent, Terminal, TerminalHost};

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::ATOMIC_USIZE_INIT;
pub type TabId = usize;

pub struct Tab {
    tab_id: TabId,
    terminal: RefCell<Terminal>,
    process: RefCell<Child>,
    pty: RefCell<MasterPty>,
}

impl Tab {
    pub fn new(terminal: Terminal, process: Child, pty: MasterPty) -> Self {
        let tab_id = TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
        Self {
            tab_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
        }
    }

    #[inline]
    pub fn tab_id(&self) -> TabId {
        self.tab_id
    }

    #[deprecated]
    pub fn terminal(&self) -> RefMut<Terminal> {
        self.terminal.borrow_mut()
    }

    pub fn renderer(&self) -> RefMut<Terminal> {
        self.terminal.borrow_mut()
    }

    pub fn is_dead(&self) -> bool {
        if let Ok(None) = self.process.borrow_mut().try_wait() {
            false
        } else {
            true
        }
    }

    pub fn advance_bytes(&self, buf: &[u8], host: &mut TerminalHost) {
        self.terminal.borrow_mut().advance_bytes(buf, host)
    }

    pub fn mouse_event(&self, event: MouseEvent, host: &mut TerminalHost) -> Result<(), Error> {
        self.terminal.borrow_mut().mouse_event(event, host)
    }

    pub fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        self.terminal
            .borrow_mut()
            .key_down(key, mods, &mut *self.pty.borrow_mut())
    }

    pub fn resize(
        &self,
        rows: u16,
        cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.pty
            .borrow_mut()
            .resize(rows, cols, pixel_width, pixel_height)?;
        self.terminal
            .borrow_mut()
            .resize(rows as usize, cols as usize);
        Ok(())
    }

    pub fn writer(&self) -> RefMut<MasterPty> {
        self.pty.borrow_mut()
    }

    pub fn reader(&self) -> Result<Box<std::io::Read + Send>, Error> {
        self.pty.borrow_mut().try_clone_reader()
    }

    pub fn send_paste(&self, text: &str) -> Result<(), Error> {
        self.terminal
            .borrow_mut()
            .send_paste(text, &mut *self.pty.borrow_mut())
    }

    pub fn get_title(&self) -> String {
        self.terminal.borrow_mut().get_title().to_string()
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
        // Avoid lingering zombies
        self.process.borrow_mut().kill().ok();
        self.process.borrow_mut().wait().ok();
    }
}

pub struct Tabs {
    tabs: Vec<Rc<Tab>>,
    active: usize,
}

impl Tabs {
    pub fn new(tab: &Rc<Tab>) -> Self {
        Self {
            tabs: vec![Rc::clone(tab)],
            active: 0,
        }
    }

    pub fn push(&mut self, tab: &Rc<Tab>) {
        self.tabs.push(Rc::clone(tab))
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_by_idx(&self, idx: usize) -> Option<&Rc<Tab>> {
        self.tabs.get(idx)
    }

    pub fn idx_by_id(&self, id: TabId) -> Option<usize> {
        for (idx, t) in self.tabs.iter().enumerate() {
            if t.tab_id() == id {
                return Some(idx);
            }
        }
        None
    }

    pub fn remove_by_id(&mut self, id: TabId) {
        if let Some(idx) = self.idx_by_id(id) {
            self.tabs.remove(idx);
            let len = self.tabs.len();
            if len > 0 && self.active == idx && idx >= len {
                self.set_active(len - 1);
            }
        }
    }

    pub fn get_active(&self) -> Option<&Rc<Tab>> {
        self.get_by_idx(self.active)
    }

    #[inline]
    pub fn get_active_idx(&self) -> usize {
        self.active
    }

    pub fn set_active(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.active = idx;
        self.get_by_idx(idx)
            .unwrap()
            .terminal
            .borrow_mut()
            .make_all_lines_dirty();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Tab>> {
        self.tabs.iter()
    }
}
