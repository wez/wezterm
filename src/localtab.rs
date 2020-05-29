use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::mux::tab::{Pattern, SearchResult};
use anyhow::Error;
use portable_pty::{Child, MasterPty, PtySize};
use std::cell::{RefCell, RefMut};
use std::sync::Arc;
use term::color::ColorPalette;
use term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, StableRowIndex, Terminal, TerminalHost};
use url::Url;

pub struct LocalTab {
    tab_id: TabId,
    terminal: RefCell<Terminal>,
    process: RefCell<Box<dyn Child>>,
    pty: RefCell<Box<dyn MasterPty>>,
    domain_id: DomainId,
}

impl Tab for LocalTab {
    #[inline]
    fn tab_id(&self) -> TabId {
        self.tab_id
    }

    fn renderer(&self) -> RefMut<dyn Renderable> {
        RefMut::map(self.terminal.borrow_mut(), |t| &mut *t)
    }

    fn is_dead(&self) -> bool {
        if let Ok(None) = self.process.borrow_mut().try_wait() {
            false
        } else {
            log::error!("is_dead: {:?}", self.tab_id);
            true
        }
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.terminal.borrow_mut().set_clipboard(clipboard);
    }

    fn advance_bytes(&self, buf: &[u8], host: &mut dyn TerminalHost) {
        self.terminal.borrow_mut().advance_bytes(buf, host)
    }

    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> Result<(), Error> {
        self.terminal.borrow_mut().mouse_event(event, host)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        self.terminal
            .borrow_mut()
            .key_down(key, mods, &mut *self.pty.borrow_mut())
    }

    fn resize(&self, size: PtySize) -> Result<(), Error> {
        self.pty.borrow_mut().resize(size)?;
        self.terminal.borrow_mut().resize(
            size.rows as usize,
            size.cols as usize,
            size.pixel_width as usize,
            size.pixel_height as usize,
        );
        Ok(())
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.pty.borrow_mut()
    }

    fn reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error> {
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

    fn palette(&self) -> ColorPalette {
        self.terminal.borrow().palette()
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn erase_scrollback(&self) {
        self.terminal.borrow_mut().erase_scrollback();
    }

    fn is_mouse_grabbed(&self) -> bool {
        self.terminal.borrow().is_mouse_grabbed()
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.terminal.borrow().get_current_dir().cloned()
    }

    fn search(&self, pattern: &Pattern) -> Vec<SearchResult> {
        let term = self.terminal.borrow();
        let screen = term.screen();

        let mut results = vec![];
        let mut haystack = String::new();
        let mut byte_pos_to_stable_idx = vec![];

        fn haystack_idx_to_coord(
            idx: usize,
            byte_pos_to_stable_idx: &[(usize, StableRowIndex)],
        ) -> (usize, StableRowIndex) {
            for (start, row) in byte_pos_to_stable_idx.iter().rev() {
                if idx >= *start {
                    return (idx - *start, *row);
                }
            }
            unreachable!();
        }

        fn collect_matches(
            results: &mut Vec<SearchResult>,
            pattern: &Pattern,
            haystack: &str,
            byte_pos_to_stable_idx: &[(usize, StableRowIndex)],
        ) {
            if haystack.is_empty() {
                return;
            }
            match pattern {
                Pattern::String(s) => {
                    for (idx, s) in haystack.match_indices(s) {
                        let (start_x, start_y) = haystack_idx_to_coord(idx, byte_pos_to_stable_idx);
                        let (end_x, end_y) =
                            haystack_idx_to_coord(idx + s.len(), byte_pos_to_stable_idx);
                        results.push(SearchResult {
                            start_x,
                            start_y,
                            end_x,
                            end_y,
                        });
                    }
                } /*
                  Pattern::Regex(r) => {
                      // TODO
                  }
                  */
            }
        }

        for (idx, line) in screen.lines.iter().enumerate() {
            byte_pos_to_stable_idx.push((haystack.len(), screen.phys_to_stable_row_index(idx)));
            let mut wrapped = false;
            for (_, cell) in line.visible_cells() {
                haystack.push_str(cell.str());
                wrapped = cell.attrs().wrapped();
            }

            if !wrapped {
                collect_matches(&mut results, pattern, &haystack, &byte_pos_to_stable_idx);
                haystack.clear();
                byte_pos_to_stable_idx.clear();
            }
        }

        collect_matches(&mut results, pattern, &haystack, &byte_pos_to_stable_idx);
        results
    }
}

impl LocalTab {
    pub fn new(
        terminal: Terminal,
        process: Box<dyn Child>,
        pty: Box<dyn MasterPty>,
        domain_id: DomainId,
    ) -> Self {
        let tab_id = alloc_tab_id();
        Self {
            tab_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
            domain_id,
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
