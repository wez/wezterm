use crate::domain::DomainId;
use crate::pane::{Pane, PaneId, Pattern, SearchResult};
use crate::renderable::*;
use crate::tmux::{TmuxDomain, TmuxDomainState};
use crate::{Domain, Mux};
use anyhow::Error;
use async_trait::async_trait;
use portable_pty::{Child, MasterPty, PtySize};
use rangeset::RangeSet;
use std::cell::{RefCell, RefMut};
use std::ops::Range;
use std::sync::Arc;
use termwiz::escape::DeviceControlMode;
use termwiz::surface::Line;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    CellAttributes, Clipboard, KeyCode, KeyModifiers, MouseEvent, SemanticZone, StableRowIndex,
    Terminal,
};

pub struct LocalPane {
    pane_id: PaneId,
    terminal: RefCell<Terminal>,
    process: RefCell<Box<dyn Child>>,
    pty: RefCell<Box<dyn MasterPty>>,
    domain_id: DomainId,
    tmux_domain: RefCell<Option<Arc<TmuxDomainState>>>,
}

#[async_trait(?Send)]
impl Pane for LocalPane {
    fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        let mut cursor = terminal_get_cursor_position(&mut self.terminal.borrow_mut());
        if self.tmux_domain.borrow().is_some() {
            cursor.visibility = termwiz::surface::CursorVisibility::Hidden;
        }
        cursor
    }

    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex> {
        terminal_get_dirty_lines(&mut self.terminal.borrow_mut(), lines)
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        let (first, mut lines) = terminal_get_lines(&mut self.terminal.borrow_mut(), lines);

        if self.tmux_domain.borrow().is_some() {
            let cursor = terminal_get_cursor_position(&mut self.terminal.borrow_mut());
            let idx = cursor.y as isize - first as isize;
            if idx > 0 {
                if let Some(line) = lines.get_mut(idx as usize) {
                    line.overlay_text_with_attribute(
                        0,
                        "This pane is running tmux control mode. Press q to detach.",
                        CellAttributes::default(),
                    );
                }
            }
        }

        (first, lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        terminal_get_dimensions(&mut self.terminal.borrow_mut())
    }

    fn kill(&self) {
        log::debug!("killing process in pane {}", self.pane_id);
        self.process.borrow_mut().kill().ok();
    }

    fn is_dead(&self) -> bool {
        if let Ok(None) = self.process.borrow_mut().try_wait() {
            false
        } else {
            log::trace!("Pane id {} is_dead", self.pane_id);
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
        if self.tmux_domain.borrow().is_some() {
            log::error!("key: {:?}", key);
            if key == KeyCode::Char('q') {
                self.terminal.borrow_mut().send_paste("detach\n")?;
            }
            return Ok(());
        } else {
            self.terminal.borrow_mut().key_down(key, mods)
        }
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
        if self.tmux_domain.borrow().is_some() {
            Ok(())
        } else {
            self.terminal.borrow_mut().send_paste(text)
        }
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
        if self.tmux_domain.borrow().is_some() {
            false
        } else {
            self.terminal.borrow().is_mouse_grabbed()
        }
    }

    fn is_alt_screen_active(&self) -> bool {
        if self.tmux_domain.borrow().is_some() {
            false
        } else {
            self.terminal.borrow().is_alt_screen_active()
        }
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.terminal
            .borrow()
            .get_current_dir()
            .cloned()
            .or_else(|| self.divine_current_working_dir())
    }

    fn get_semantic_zones(&self) -> anyhow::Result<Vec<SemanticZone>> {
        let term = self.terminal.borrow();
        term.get_semantic_zones()
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

struct LocalPaneDCSHandler {
    pane_id: PaneId,
    tmux_domain: Option<Arc<TmuxDomainState>>,
}

impl wezterm_term::DeviceControlHandler for LocalPaneDCSHandler {
    fn handle_device_control(&mut self, control: termwiz::escape::DeviceControlMode) {
        match control {
            DeviceControlMode::Enter(mode) => {
                if !mode.ignored_extra_intermediates
                    && mode.params.len() == 1
                    && mode.params[0] == 1000
                    && mode.intermediates.is_empty()
                {
                    log::error!("tmux -CC mode requested");

                    // Create a new domain to host these tmux tabs
                    let domain = TmuxDomain::new(self.pane_id);
                    let tmux_domain = Arc::clone(&domain.inner);

                    let domain: Arc<dyn Domain> = Arc::new(domain);
                    let mux = Mux::get().expect("to be called on main thread");
                    mux.add_domain(&domain);

                    if let Some(pane) = mux.get_pane(self.pane_id) {
                        let pane = pane.downcast_ref::<LocalPane>().unwrap();
                        pane.tmux_domain
                            .borrow_mut()
                            .replace(Arc::clone(&tmux_domain));
                    }

                    self.tmux_domain.replace(tmux_domain);

                // TODO: do we need to proactively list available tabs here?
                // if so we should arrange to call domain.attach() and make
                // it do the right thing.
                } else {
                    log::error!("unknown DeviceControlMode::Enter {:?}", mode,);
                }
            }
            DeviceControlMode::Exit => {
                if let Some(tmux) = self.tmux_domain.take() {
                    let mux = Mux::get().expect("to be called on main thread");
                    if let Some(pane) = mux.get_pane(self.pane_id) {
                        let pane = pane.downcast_ref::<LocalPane>().unwrap();
                        pane.tmux_domain.borrow_mut().take();
                    }
                    mux.domain_was_detached(tmux.domain_id);
                }
            }
            DeviceControlMode::Data(c) => {
                if let Some(tmux) = self.tmux_domain.as_ref() {
                    tmux.advance(c);
                } else {
                    log::error!(
                        "unhandled DeviceControlMode::Data {:x} {}",
                        c,
                        (c as char).escape_debug()
                    );
                }
            }
            _ => {
                log::error!("unhandled: {:?}", control);
            }
        }
    }
}

impl LocalPane {
    pub fn new(
        pane_id: PaneId,
        mut terminal: Terminal,
        process: Box<dyn Child>,
        pty: Box<dyn MasterPty>,
        domain_id: DomainId,
    ) -> Self {
        terminal.set_device_control_handler(Box::new(LocalPaneDCSHandler {
            pane_id,
            tmux_domain: None,
        }));
        Self {
            pane_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(process),
            pty: RefCell::new(pty),
            domain_id,
            tmux_domain: RefCell::new(None),
        }
    }

    #[cfg(target_os = "macos")]
    fn divine_current_working_dir_macos(&self) -> Option<Url> {
        if let Some(pid) = self.pty.borrow().process_group_leader() {
            extern "C" {
                fn proc_pidinfo(
                    pid: libc::pid_t,
                    flavor: libc::c_int,
                    arg: u64,
                    buffer: *mut proc_vnodepathinfo,
                    buffersize: libc::c_int,
                ) -> libc::c_int;
            }
            const PROC_PIDVNODEPATHINFO: libc::c_int = 9;
            #[repr(C)]
            struct vinfo_stat {
                vst_dev: u32,
                vst_mode: u16,
                vst_nlink: u16,
                vst_ino: u64,
                vst_uid: libc::uid_t,
                vst_gid: libc::gid_t,
                vst_atime: i64,
                vst_atimensec: i64,
                vst_mtime: i64,
                vst_mtimensec: i64,
                vst_ctime: i64,
                vst_ctimensec: i64,
                vst_birthtime: i64,
                vst_birthtimensec: i64,
                vst_size: libc::off_t,
                vst_blocks: i64,
                vst_blksize: i32,
                vst_flags: u32,
                vst_gen: u32,
                vst_rdev: u32,
                vst_qspare_1: i64,
                vst_qspare_2: i64,
            }
            #[repr(C)]
            struct vnode_info {
                vi_stat: vinfo_stat,
                vi_type: libc::c_int,
                vi_pad: libc::c_int,
                vi_fsid: libc::fsid_t,
            }

            const MAXPATHLEN: usize = 1024;
            #[repr(C)]
            struct vnode_info_path {
                vip_vi: vnode_info,
                vip_path: [i8; MAXPATHLEN],
            }

            #[repr(C)]
            struct proc_vnodepathinfo {
                pvi_cdir: vnode_info_path,
                pvi_rdir: vnode_info_path,
            }

            let mut pathinfo: proc_vnodepathinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&pathinfo) as libc::c_int;
            let ret = unsafe { proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &mut pathinfo, size) };
            if ret == size {
                let path = unsafe { std::ffi::CStr::from_ptr(pathinfo.pvi_cdir.vip_path.as_ptr()) };
                if let Ok(s) = path.to_str() {
                    return Url::parse(&format!("file://localhost{}", s)).ok();
                }
            }
        }
        None
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

        #[cfg(target_os = "macos")]
        {
            return self.divine_current_working_dir_macos();
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
