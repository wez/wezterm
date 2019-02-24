use crate::{Child, MasterPty};
use failure::Error;
use std::cell::{RefCell, RefMut};
use term::Terminal;

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

    pub fn terminal(&self) -> RefMut<Terminal> {
        self.terminal.borrow_mut()
    }

    pub fn process(&self) -> RefMut<Child> {
        self.process.borrow_mut()
    }

    pub fn pty(&self) -> RefMut<MasterPty> {
        self.pty.borrow_mut()
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
    tabs: Vec<Tab>,
    active: usize,
}

impl Tabs {
    pub fn new(tab: Tab) -> Self {
        Self {
            tabs: vec![tab],
            active: 0,
        }
    }

    pub fn push(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_by_idx(&self, idx: usize) -> Option<&Tab> {
        self.tabs.get(idx)
    }

    pub fn get_by_id(&self, id: TabId) -> Result<&Tab, Error> {
        for t in &self.tabs {
            if t.tab_id == id {
                return Ok(t);
            }
        }
        bail!("no such tab id {}", id)
    }

    pub fn idx_by_id(&self, id: TabId) -> Option<usize> {
        for (idx, t) in self.tabs.iter().enumerate() {
            if t.tab_id == id {
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

    pub fn get_active(&self) -> &Tab {
        // FIXME: Optional
        &self.tabs[self.active]
    }

    #[inline]
    pub fn get_active_idx(&self) -> usize {
        self.active
    }

    pub fn set_active(&mut self, idx: usize) {
        assert!(idx < self.tabs.len());
        self.active = idx;
        self.tabs[idx].terminal.borrow_mut().make_all_lines_dirty();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter()
    }
}
