use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::pty::{Child, MasterPty, PtySize};
use failure::Error;
use std::cell::{RefCell, RefMut};
use term::{KeyCode, KeyModifiers, MouseEvent, Terminal, TerminalHost};

pub struct LocalTab {
    tab_id: TabId,
    terminal: RefCell<Terminal>,
    process: RefCell<Box<Child>>,
    pty: RefCell<Box<MasterPty>>,
}

impl Tab for LocalTab {
    #[inline]
    fn tab_id(&self) -> TabId {
        self.tab_id
    }

    fn renderer(&self) -> RefMut<Renderable> {
        RefMut::map(self.terminal.borrow_mut(), |t| &mut *t)
    }

    fn is_dead(&self) -> bool {
        if let Ok(None) = self.process.borrow_mut().try_wait() {
            false
        } else {
            true
        }
    }

    fn advance_bytes(&self, buf: &[u8], host: &mut TerminalHost) {
        self.terminal.borrow_mut().advance_bytes(buf, host)
    }

    fn mouse_event(&self, event: MouseEvent, host: &mut TerminalHost) -> Result<(), Error> {
        self.terminal.borrow_mut().mouse_event(event, host)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        self.terminal
            .borrow_mut()
            .key_down(key, mods, &mut *self.pty.borrow_mut())
    }

    fn resize(
        &self,
        rows: u16,
        cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.pty.borrow_mut().resize(PtySize {
            rows,
            cols,
            pixel_width,
            pixel_height,
        })?;
        self.terminal
            .borrow_mut()
            .resize(rows as usize, cols as usize);
        Ok(())
    }

    fn writer(&self) -> RefMut<std::io::Write> {
        self.pty.borrow_mut()
    }

    fn reader(&self) -> Result<Box<std::io::Read + Send>, Error> {
        self.pty.borrow_mut().try_clone_reader()
    }

    fn send_paste(&self, text: &str) -> Result<(), Error> {
        self.terminal
            .borrow_mut()
            .send_paste(text, &mut *self.pty.borrow_mut())
    }

    fn get_title(&self) -> String {
        self.terminal.borrow_mut().get_title().to_string()
    }
}

impl LocalTab {
    pub fn new(terminal: Terminal, process: Box<Child>, pty: Box<MasterPty>) -> Self {
        let tab_id = alloc_tab_id();
        Self {
            tab_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
        }
    }
}

impl Drop for LocalTab {
    fn drop(&mut self) {
        // Avoid lingering zombies
        self.process.borrow_mut().kill().ok();
        self.process.borrow_mut().wait().ok();
    }
}
