use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use crate::mux::tab::{alloc_pane_id, Pane, PaneId};
use crate::mux::tab::{Pattern, SearchResult};
use anyhow::Error;
use async_trait::async_trait;
use portable_pty::{Child, MasterPty, PtySize};
use std::cell::{RefCell, RefMut};
use std::sync::Arc;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, StableRowIndex, Terminal};

pub struct LocalPane {
    pane_id: PaneId,
    terminal: RefCell<Terminal>,
    process: RefCell<Box<dyn Child>>,
    pty: RefCell<Box<dyn MasterPty>>,
    domain_id: DomainId,
}

#[async_trait(?Send)]
impl Pane for LocalPane {
    fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    fn renderer(&self) -> RefMut<dyn Renderable> {
        RefMut::map(self.terminal.borrow_mut(), |t| &mut *t)
    }

    fn is_dead(&self) -> bool {
        if let Ok(None) = self.process.borrow_mut().try_wait() {
            false
        } else {
            log::error!("Pane id {} is_dead", self.pane_id);
            true
        }
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.terminal.borrow_mut().set_clipboard(clipboard);
    }

    fn advance_bytes(&self, buf: &[u8]) {
        self.terminal.borrow_mut().advance_bytes(buf)
    }

    fn mouse_event(&self, event: MouseEvent) -> Result<(), Error> {
        self.terminal.borrow_mut().mouse_event(event)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        self.terminal.borrow_mut().key_down(key, mods)
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
        self.terminal.borrow_mut().send_paste(text)
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

    fn focus_changed(&self, focused: bool) {
        self.terminal.borrow_mut().focus_changed(focused);
    }

    fn is_mouse_grabbed(&self) -> bool {
        self.terminal.borrow().is_mouse_grabbed()
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.terminal
            .borrow()
            .get_current_dir()
            .cloned()
            .or_else(|| self.divine_current_working_dir())
    }

    async fn search(&self, mut pattern: Pattern) -> anyhow::Result<Vec<SearchResult>> {
        let term = self.terminal.borrow();
        let screen = term.screen();

        if let Pattern::CaseInSensitiveString(s) = &mut pattern {
            // normalize the case so we match everything lowercase
            *s = s.to_lowercase()
        }

        let mut results = vec![];
        let mut haystack = String::new();
        let mut coords = vec![];

        struct Coord {
            byte_idx: usize,
            grapheme_idx: usize,
            stable_row: StableRowIndex,
        }

        fn haystack_idx_to_coord(idx: usize, coords: &[Coord]) -> (usize, StableRowIndex) {
            let c = coords
                .binary_search_by(|ele| ele.byte_idx.cmp(&idx))
                .or_else(|i| -> Result<usize, usize> { Ok(i) })
                .unwrap();
            let coord = coords.get(c).or_else(|| coords.last()).unwrap();
            (coord.grapheme_idx, coord.stable_row)
        }

        fn collect_matches(
            results: &mut Vec<SearchResult>,
            pattern: &Pattern,
            haystack: &str,
            coords: &[Coord],
        ) {
            if haystack.is_empty() {
                return;
            }
            match pattern {
                // Rust only provides a case sensitive match_indices function, so
                // we have to pre-arrange to lowercase both the pattern and the
                // haystack strings
                Pattern::CaseInSensitiveString(s) | Pattern::CaseSensitiveString(s) => {
                    for (idx, s) in haystack.match_indices(s) {
                        let (start_x, start_y) = haystack_idx_to_coord(idx, coords);
                        let (end_x, end_y) = haystack_idx_to_coord(idx + s.len(), coords);
                        results.push(SearchResult {
                            start_x,
                            start_y,
                            end_x,
                            end_y,
                        });
                    }
                }
                Pattern::Regex(r) => {
                    if let Ok(re) = regex::Regex::new(r) {
                        for m in re.find_iter(haystack) {
                            let (start_x, start_y) = haystack_idx_to_coord(m.start(), coords);
                            let (end_x, end_y) = haystack_idx_to_coord(m.end(), coords);
                            results.push(SearchResult {
                                start_x,
                                start_y,
                                end_x,
                                end_y,
                            });
                        }
                    }
                }
            }
        }

        for (idx, line) in screen.lines.iter().enumerate() {
            let stable_row = screen.phys_to_stable_row_index(idx);

            let mut wrapped = false;
            for (grapheme_idx, cell) in line.visible_cells() {
                coords.push(Coord {
                    byte_idx: haystack.len(),
                    grapheme_idx,
                    stable_row,
                });

                let s = cell.str();
                if let Pattern::CaseInSensitiveString(_) = &pattern {
                    // normalize the case so we match everything lowercase
                    haystack.push_str(&s.to_lowercase());
                } else {
                    haystack.push_str(cell.str());
                }
                wrapped = cell.attrs().wrapped();
            }

            if !wrapped {
                if let Pattern::Regex(_) = &pattern {
                    haystack.push('\n');
                } else {
                    collect_matches(&mut results, &pattern, &haystack, &coords);
                    haystack.clear();
                    coords.clear();
                }
            }
        }

        collect_matches(&mut results, &pattern, &haystack, &coords);
        Ok(results)
    }
}

impl LocalPane {
    pub fn new(
        terminal: Terminal,
        process: Box<dyn Child>,
        pty: Box<dyn MasterPty>,
        domain_id: DomainId,
    ) -> Self {
        let pane_id = alloc_pane_id();
        Self {
            pane_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
            domain_id,
        }
    }

    #[cfg(target_os = "linux")]
    fn divine_current_working_dir_linux(&self) -> Option<Url> {
        if let Some(pid) = self.pty.borrow().process_group_leader() {
            if let Ok(path) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
                return Url::parse(&format!("file://localhost{}", path.display())).ok();
            }
        }
        None
    }

    fn divine_current_working_dir(&self) -> Option<Url> {
        #[cfg(target_os = "linux")]
        {
            return self.divine_current_working_dir_linux();
        }

        #[allow(unreachable_code)]
        None
    }
}

impl Drop for LocalPane {
    fn drop(&mut self) {
        // Avoid lingering zombies
        self.process.borrow_mut().kill().ok();
        self.process.borrow_mut().wait().ok();
    }
}
