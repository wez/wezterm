use crate::domain::DomainId;
use crate::pane::{
    CloseReason, ForEachPaneLogicalLine, LogicalLine, Pane, PaneId, Pattern, SearchResult,
    WithPaneLines,
};
use crate::renderable::*;
use crate::tmux::{TmuxDomain, TmuxDomainState};
use crate::{Domain, Mux, MuxNotification};
use anyhow::Error;
use async_trait::async_trait;
use config::keyassignment::ScrollbackEraseMode;
use config::{configuration, ExitBehavior};
use portable_pty::{Child, ChildKiller, ExitStatus, MasterPty, PtySize};
use procinfo::LocalProcessInfo;
use rangeset::RangeSet;
use smol::channel::{bounded, Receiver, TryRecvError};
use std::borrow::Cow;
use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryInto;
use std::io::{Result as IoResult, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::escape::csi::{Sgr, CSI};
use termwiz::escape::{Action, DeviceControlMode};
use termwiz::input::KeyboardEncoding;
use termwiz::surface::{Line, SequenceNo};
use url::Url;
use wezterm_dynamic::Value;
use wezterm_term::color::ColorPalette;
use wezterm_term::{
    Alert, AlertHandler, Clipboard, DownloadHandler, KeyCode, KeyModifiers, MouseEvent,
    SemanticZone, StableRowIndex, Terminal, TerminalConfiguration, TerminalSize,
};

#[derive(Debug)]
enum ProcessState {
    Running {
        child_waiter: Receiver<IoResult<ExitStatus>>,
        pid: Option<u32>,
        signaller: Box<dyn ChildKiller>,
        // Whether we've explicitly killed the child
        killed: bool,
    },
    DeadPendingClose {
        killed: bool,
    },
    Dead,
}

struct CachedProcInfo {
    root: LocalProcessInfo,
    updated: Instant,
    foreground: LocalProcessInfo,
}

pub struct LocalPane {
    pane_id: PaneId,
    terminal: RefCell<Terminal>,
    process: RefCell<ProcessState>,
    pty: RefCell<Box<dyn MasterPty>>,
    writer: RefCell<Box<dyn Write>>,
    domain_id: DomainId,
    tmux_domain: RefCell<Option<Arc<TmuxDomainState>>>,
    proc_list: RefCell<Option<CachedProcInfo>>,
    command_description: String,
}

#[async_trait(?Send)]
impl Pane for LocalPane {
    fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    fn get_metadata(&self) -> Value {
        #[allow(unused_mut)]
        let mut map: BTreeMap<Value, Value> = BTreeMap::new();

        #[cfg(unix)]
        if let Some(tio) = self.pty.borrow().get_termios() {
            use nix::sys::termios::LocalFlags;
            // Detect whether we might be in password input mode.
            // If local echo is disabled and canonical input mode
            // is enabled, then we assume that we're in some kind
            // of password-entry mode.
            let pw_input = !tio.local_flags.contains(LocalFlags::ECHO)
                && tio.local_flags.contains(LocalFlags::ICANON);
            map.insert(
                Value::String("password_input".to_string()),
                Value::Bool(pw_input),
            );
        }

        Value::Object(map.into())
    }

    fn get_cursor_position(&self) -> StableCursorPosition {
        let mut cursor = terminal_get_cursor_position(&mut self.terminal.borrow_mut());
        if self.tmux_domain.borrow().is_some() {
            cursor.visibility = termwiz::surface::CursorVisibility::Hidden;
        }
        cursor
    }

    fn get_keyboard_encoding(&self) -> KeyboardEncoding {
        self.terminal.borrow().get_keyboard_encoding()
    }

    fn get_current_seqno(&self) -> SequenceNo {
        self.terminal.borrow().current_seqno()
    }

    fn get_changed_since(
        &self,
        lines: Range<StableRowIndex>,
        seqno: SequenceNo,
    ) -> RangeSet<StableRowIndex> {
        terminal_get_dirty_lines(&mut self.terminal.borrow_mut(), lines, seqno)
    }

    fn for_each_logical_line_in_stable_range_mut(
        &self,
        lines: Range<StableRowIndex>,
        for_line: &mut dyn ForEachPaneLogicalLine,
    ) {
        terminal_for_each_logical_line_in_stable_range_mut(
            &mut self.terminal.borrow_mut(),
            lines,
            for_line,
        );
    }

    fn with_lines_mut(&self, lines: Range<StableRowIndex>, with_lines: &mut dyn WithPaneLines) {
        terminal_with_lines_mut(&mut self.terminal.borrow_mut(), lines, with_lines)
    }

    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>) {
        crate::pane::impl_get_lines_via_with_lines(self, lines)
    }

    fn get_logical_lines(&self, lines: Range<StableRowIndex>) -> Vec<LogicalLine> {
        crate::pane::impl_get_logical_lines_via_get_lines(self, lines)
    }

    fn get_dimensions(&self) -> RenderableDimensions {
        terminal_get_dimensions(&mut self.terminal.borrow_mut())
    }

    fn copy_user_vars(&self) -> HashMap<String, String> {
        self.terminal.borrow().user_vars().clone()
    }

    fn kill(&self) {
        let mut proc = self.process.borrow_mut();
        log::debug!(
            "killing process in pane {}, state is {:?}",
            self.pane_id,
            proc
        );
        match &mut *proc {
            ProcessState::Running {
                signaller, killed, ..
            } => {
                let _ = signaller.kill();
                *killed = true;
            }
            ProcessState::DeadPendingClose { killed } => {
                *killed = true;
            }
            _ => {}
        }
    }

    fn is_dead(&self) -> bool {
        let mut proc = self.process.borrow_mut();
        let mut notify = None;

        const EXIT_BEHAVIOR: &str = "This message is shown because \
            \x1b]8;;https://wezfurlong.org/wezterm/\
            config/lua/config/exit_behavior.html\
            \x1b\\exit_behavior\x1b]8;;\x1b\\";

        match &mut *proc {
            ProcessState::Running {
                child_waiter,
                killed,
                ..
            } => {
                let status = match child_waiter.try_recv() {
                    Ok(Ok(s)) => Some(s),
                    Err(TryRecvError::Empty) => None,
                    _ => Some(ExitStatus::with_exit_code(1)),
                };

                if let Some(status) = status {
                    let success = match status.success() {
                        true => true,
                        false => configuration()
                            .clean_exit_codes
                            .contains(&status.exit_code()),
                    };

                    match (configuration().exit_behavior, success, killed) {
                        (ExitBehavior::Close, _, _) => *proc = ProcessState::Dead,
                        (ExitBehavior::CloseOnCleanExit, false, false) => {
                            notify = Some(format!(
                                "\r\n⚠️  Process {} didn't exit cleanly\r\n{}.\r\n{}=\"CloseOnCleanExit\"\r\n",
                                self.command_description,
                                status,
                                EXIT_BEHAVIOR
                            ));
                            *proc = ProcessState::DeadPendingClose { killed: false }
                        }
                        (ExitBehavior::CloseOnCleanExit, ..) => *proc = ProcessState::Dead,
                        (ExitBehavior::Hold, success, false) => {
                            if success {
                                notify = Some(format!(
                                    "\r\n👍 Process {} completed.\r\n{}=\"Hold\"\r\n",
                                    self.command_description, EXIT_BEHAVIOR
                                ));
                            } else {
                                notify = Some(format!(
                                    "\r\n⚠️  Process {} didn't exit cleanly\r\n{}.\r\n{}=\"Hold\"\r\n",
                                    self.command_description,
                                    status,
                                    EXIT_BEHAVIOR
                                ));
                            }
                            *proc = ProcessState::DeadPendingClose { killed: false }
                        }
                        (ExitBehavior::Hold, _, true) => *proc = ProcessState::Dead,
                    }
                    log::debug!("child terminated, new state is {:?}", proc);
                }
            }
            ProcessState::DeadPendingClose { killed } => {
                if *killed {
                    *proc = ProcessState::Dead;
                    log::debug!("child state -> {:?}", proc);
                }
            }
            ProcessState::Dead => {}
        }

        if let Some(notify) = notify {
            emit_output_for_pane(self.pane_id, &notify);
        }

        match &*proc {
            ProcessState::Running { .. } => false,
            ProcessState::DeadPendingClose { .. } => false,
            ProcessState::Dead => true,
        }
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.terminal.borrow_mut().set_clipboard(clipboard);
    }

    fn set_download_handler(&self, handler: &Arc<dyn DownloadHandler>) {
        self.terminal.borrow_mut().set_download_handler(handler);
    }

    fn set_config(&self, config: Arc<dyn TerminalConfiguration>) {
        self.terminal.borrow_mut().set_config(config);
    }

    fn get_config(&self) -> Option<Arc<dyn TerminalConfiguration>> {
        Some(self.terminal.borrow().get_config())
    }

    fn perform_actions(&self, actions: Vec<termwiz::escape::Action>) {
        self.terminal.borrow_mut().perform_actions(actions)
    }

    fn mouse_event(&self, event: MouseEvent) -> Result<(), Error> {
        Mux::get().unwrap().record_input_for_current_identity();
        self.terminal.borrow_mut().mouse_event(event)
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        Mux::get().unwrap().record_input_for_current_identity();
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

    fn key_up(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
        Mux::get().unwrap().record_input_for_current_identity();
        self.terminal.borrow_mut().key_up(key, mods)
    }

    fn resize(&self, size: TerminalSize) -> Result<(), Error> {
        self.pty.borrow_mut().resize(PtySize {
            rows: size.rows.try_into()?,
            cols: size.cols.try_into()?,
            pixel_width: size.pixel_width.try_into()?,
            pixel_height: size.pixel_height.try_into()?,
        })?;
        self.terminal.borrow_mut().resize(size);
        Ok(())
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        Mux::get().unwrap().record_input_for_current_identity();
        self.writer.borrow_mut()
    }

    fn reader(&self) -> anyhow::Result<Option<Box<dyn std::io::Read + Send>>> {
        Ok(Some(self.pty.borrow_mut().try_clone_reader()?))
    }

    fn send_paste(&self, text: &str) -> Result<(), Error> {
        Mux::get().unwrap().record_input_for_current_identity();
        if self.tmux_domain.borrow().is_some() {
            Ok(())
        } else {
            self.terminal.borrow_mut().send_paste(text)
        }
    }

    fn get_title(&self) -> String {
        let title = self.terminal.borrow_mut().get_title().to_string();
        // If the title is the default pane title, then try to spice
        // things up a bit by returning the process basename instead
        if title == "wezterm" {
            if let Some(proc_name) = self.get_foreground_process_name() {
                let proc_name = std::path::Path::new(&proc_name);
                if let Some(name) = proc_name.file_name() {
                    return name.to_string_lossy().to_string();
                }
            }
        }

        title
    }

    fn palette(&self) -> ColorPalette {
        self.terminal.borrow().palette()
    }

    fn domain_id(&self) -> DomainId {
        self.domain_id
    }

    fn erase_scrollback(&self, erase_mode: ScrollbackEraseMode) {
        match erase_mode {
            ScrollbackEraseMode::ScrollbackOnly => {
                self.terminal.borrow_mut().erase_scrollback();
            }
            ScrollbackEraseMode::ScrollbackAndViewport => {
                self.terminal.borrow_mut().erase_scrollback_and_viewport();
            }
        }
    }

    fn focus_changed(&self, focused: bool) {
        self.terminal.borrow_mut().focus_changed(focused);
    }

    fn has_unseen_output(&self) -> bool {
        self.terminal.borrow().has_unseen_output()
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

    fn get_foreground_process_info(&self) -> Option<LocalProcessInfo> {
        #[cfg(unix)]
        if let Some(pid) = self.pty.borrow().process_group_leader() {
            return LocalProcessInfo::with_root_pid(pid as u32);
        }

        self.divine_foreground_process()
    }

    fn get_foreground_process_name(&self) -> Option<String> {
        #[cfg(unix)]
        if let Some(pid) = self.pty.borrow().process_group_leader() {
            if let Some(path) = LocalProcessInfo::executable_path(pid as u32) {
                return Some(path.to_string_lossy().to_string());
            }
        }

        #[cfg(windows)]
        if let Some(fg) = self.divine_foreground_process() {
            return Some(fg.executable.to_string_lossy().to_string());
        }

        None
    }

    fn can_close_without_prompting(&self, _reason: CloseReason) -> bool {
        if let Some(info) = self.divine_process_list(true) {
            log::trace!(
                "can_close_without_prompting? procs in pane {:#?}",
                info.root
            );

            let hook_result = config::run_immediate_with_lua_config(|lua| {
                let lua = match lua {
                    Some(lua) => lua,
                    None => return Ok(None),
                };
                let v = config::lua::emit_sync_callback(
                    &*lua,
                    ("mux-is-process-stateful".to_string(), (info.root.clone())),
                )?;
                match v {
                    mlua::Value::Nil => Ok(None),
                    mlua::Value::Boolean(v) => Ok(Some(v)),
                    _ => Ok(None),
                }
            });

            fn default_stateful_check(proc_list: &LocalProcessInfo) -> bool {
                let names = proc_list.flatten_to_exe_names();

                let skip = configuration()
                    .skip_close_confirmation_for_processes_named
                    .iter()
                    .cloned()
                    .collect::<HashSet<_>>();

                if !names.is_subset(&skip) {
                    // There are other processes running than are listed,
                    // so we consider this to be stateful
                    return true;
                }
                false
            }

            let is_stateful = match hook_result {
                Ok(None) => default_stateful_check(&info.root),
                Ok(Some(s)) => s,
                Err(err) => {
                    log::error!(
                        "Error while running mux-is-process-stateful \
                         hook: {:#}, falling back to default behavior",
                        err
                    );
                    default_stateful_check(&info.root)
                }
            };

            !is_stateful
        } else {
            #[cfg(unix)]
            {
                // If the process is dead but exit_behavior is holding the
                // window, we don't need to prompt to confirm closing.
                // That is detectable as no longer having a process group leader.
                if self.pty.borrow().process_group_leader().is_none() {
                    return true;
                }
            }

            false
        }
    }

    fn get_semantic_zones(&self) -> anyhow::Result<Vec<SemanticZone>> {
        let mut term = self.terminal.borrow_mut();
        term.get_semantic_zones()
    }

    async fn search(
        &self,
        pattern: Pattern,
        range: Range<StableRowIndex>,
        limit: Option<u32>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let term = self.terminal.borrow();
        let screen = term.screen();

        enum CompiledPattern {
            CaseSensitiveString(String),
            CaseInSensitiveString(String),
            Regex(regex::Regex),
        }

        let pattern = match pattern {
            Pattern::CaseSensitiveString(s) => CompiledPattern::CaseSensitiveString(s),
            Pattern::CaseInSensitiveString(s) => {
                // normalize the case so we match everything lowercase
                CompiledPattern::CaseInSensitiveString(s.to_lowercase())
            }
            Pattern::Regex(r) => CompiledPattern::Regex(regex::Regex::new(&r)?),
        };

        let mut results = vec![];
        let mut uniq_matches: HashMap<String, usize> = HashMap::new();

        screen.for_each_logical_line_in_stable_range(range, |sr, lines| {
            if let Some(limit) = limit {
                if results.len() == limit as usize {
                    // We've reach the limit, stop iteration.
                    return false;
                }
            }

            if lines.is_empty() {
                // Nothing to do on this iteration, carry on with the next.
                return true;
            }
            let haystack = if lines.len() == 1 {
                lines[0].as_str()
            } else {
                let mut s = String::new();
                for line in lines {
                    s.push_str(&line.as_str());
                }
                Cow::Owned(s)
            };
            let stable_idx = sr.start;

            if haystack.is_empty() {
                return true;
            }

            let haystack = match &pattern {
                CompiledPattern::CaseInSensitiveString(_) => Cow::Owned(haystack.to_lowercase()),
                _ => haystack,
            };
            let mut coords = None;

            match &pattern {
                CompiledPattern::CaseInSensitiveString(s)
                | CompiledPattern::CaseSensitiveString(s) => {
                    for (idx, s) in haystack.match_indices(s) {
                        found_match(
                            s,
                            idx,
                            lines,
                            stable_idx,
                            &mut uniq_matches,
                            &mut coords,
                            &mut results,
                        );
                    }
                }
                CompiledPattern::Regex(re) => {
                    // Allow for the regex to contain captures
                    for c in re.captures_iter(&haystack) {
                        // Look for the captures in reverse order, as index==0 is
                        // the whole matched string.  We can't just call
                        // `c.iter().rev()` as the capture iterator isn't double-ended.
                        for idx in (0..c.len()).rev() {
                            if let Some(m) = c.get(idx) {
                                found_match(
                                    m.as_str(),
                                    m.start(),
                                    lines,
                                    stable_idx,
                                    &mut uniq_matches,
                                    &mut coords,
                                    &mut results,
                                );
                                break;
                            }
                        }
                    }
                }
            }

            // Keep iterating
            true
        });

        #[derive(Copy, Clone, Debug)]
        struct Coord {
            byte_idx: usize,
            grapheme_idx: usize,
            stable_row: StableRowIndex,
        }

        fn found_match(
            text: &str,
            byte_idx: usize,
            lines: &[&Line],
            stable_idx: StableRowIndex,
            uniq_matches: &mut HashMap<String, usize>,
            coords: &mut Option<Vec<Coord>>,
            results: &mut Vec<SearchResult>,
        ) {
            if coords.is_none() {
                coords.replace(make_coords(lines, stable_idx));
            }
            let coords = coords.as_ref().unwrap();

            let match_id = match uniq_matches.get(text).copied() {
                Some(id) => id,
                None => {
                    let id = uniq_matches.len();
                    uniq_matches.insert(text.to_owned(), id);
                    id
                }
            };
            let (start_x, start_y) = haystack_idx_to_coord(byte_idx, coords);
            let (end_x, end_y) = haystack_idx_to_coord(byte_idx + text.len(), coords);
            results.push(SearchResult {
                start_x,
                start_y,
                end_x,
                end_y,
                match_id,
            });
        }

        fn make_coords(lines: &[&Line], stable_row: StableRowIndex) -> Vec<Coord> {
            let mut byte_idx = 0;
            let mut coords = vec![];

            for (row_idx, line) in lines.iter().enumerate() {
                for cell in line.visible_cells() {
                    coords.push(Coord {
                        byte_idx,
                        grapheme_idx: cell.cell_index(),
                        stable_row: stable_row + row_idx as StableRowIndex,
                    });
                    byte_idx += cell.str().len();
                }
            }

            coords
        }

        fn haystack_idx_to_coord(idx: usize, coords: &[Coord]) -> (usize, StableRowIndex) {
            let c = coords
                .binary_search_by(|ele| ele.byte_idx.cmp(&idx))
                .or_else(|i| -> Result<usize, usize> { Ok(i) })
                .unwrap();
            let coord = coords.get(c).map(|c| *c).unwrap_or_else(|| {
                let last = coords.last().unwrap();
                Coord {
                    grapheme_idx: last.grapheme_idx + 1,
                    ..*last
                }
            });
            (coord.grapheme_idx, coord.stable_row)
        }

        Ok(results)
    }
}

struct LocalPaneDCSHandler {
    pane_id: PaneId,
    tmux_domain: Option<Arc<TmuxDomainState>>,
}

pub(crate) fn emit_output_for_pane(pane_id: PaneId, message: &str) {
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut actions = vec![Action::CSI(CSI::Sgr(Sgr::Reset))];
    parser.parse(message.as_bytes(), |action| actions.push(action));

    promise::spawn::spawn_into_main_thread(async move {
        let mux = Mux::get().unwrap();
        if let Some(pane) = mux.get_pane(pane_id) {
            pane.perform_actions(actions);
            mux.notify(MuxNotification::PaneOutput(pane_id));
        }
    })
    .detach();
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
                    log::info!("tmux -CC mode requested");

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

                        emit_output_for_pane(
                            self.pane_id,
                            "\r\n[This pane is running tmux control mode. Press q to detach]",
                        );
                    }

                    self.tmux_domain.replace(tmux_domain);

                // TODO: do we need to proactively list available tabs here?
                // if so we should arrange to call domain.attach() and make
                // it do the right thing.
                } else {
                    log::warn!("unknown DeviceControlMode::Enter {:?}", mode,);
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
                log::warn!(
                    "unhandled DeviceControlMode::Data {:x} {}",
                    c,
                    (c as char).escape_debug()
                );
            }
            DeviceControlMode::TmuxEvents(events) => {
                if let Some(tmux) = self.tmux_domain.as_ref() {
                    tmux.advance(events);
                } else {
                    log::warn!("unhandled DeviceControlMode::TmuxEvents {:?}", &events);
                }
            }
            _ => {
                log::warn!("unhandled: {:?}", control);
            }
        }
    }
}

struct LocalPaneNotifHandler {
    pane_id: PaneId,
}

impl AlertHandler for LocalPaneNotifHandler {
    fn alert(&mut self, alert: Alert) {
        if let Some(mux) = Mux::get() {
            match &alert {
                Alert::WindowTitleChanged(title) => {
                    if let Some((_domain, window_id, _tab_id)) = mux.resolve_pane_id(self.pane_id) {
                        if let Some(mut window) = mux.get_window_mut(window_id) {
                            window.set_title(title);
                        }
                    }
                }
                Alert::TabTitleChanged(title) => {
                    if let Some((_domain, _window_id, tab_id)) = mux.resolve_pane_id(self.pane_id) {
                        if let Some(tab) = mux.get_tab(tab_id) {
                            tab.set_title(title.as_deref().unwrap_or(""));
                        }
                    }
                }
                _ => {}
            }

            mux.notify(MuxNotification::Alert {
                pane_id: self.pane_id,
                alert,
            });
        }
    }
}

/// This is a little gross; on some systems, our pipe reader will continue
/// to be blocked in read even after the child process has died.
/// We need to wake up and notice that the child terminated in order
/// for our state to wind down.
/// This block schedules a background thread to wait for the child
/// to terminate, and then nudge the muxer to check for dead processes.
/// Without this, typing `exit` in `cmd.exe` would keep the pane around
/// until something else triggered the mux to prune dead processes.
fn split_child(
    mut process: Box<dyn Child + Send>,
) -> (
    Receiver<IoResult<ExitStatus>>,
    Box<dyn ChildKiller>,
    Option<u32>,
) {
    let pid = process.process_id();
    let signaller = process.clone_killer();

    let (tx, rx) = bounded(1);

    std::thread::spawn(move || {
        let status = process.wait();
        tx.try_send(status).ok();
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().unwrap();
            mux.prune_dead_windows();
        })
        .detach();
    });

    (rx, signaller, pid)
}

impl LocalPane {
    pub fn new(
        pane_id: PaneId,
        mut terminal: Terminal,
        process: Box<dyn Child + Send>,
        pty: Box<dyn MasterPty>,
        writer: Box<dyn Write>,
        domain_id: DomainId,
        command_description: String,
    ) -> Self {
        let (process, signaller, pid) = split_child(process);

        terminal.set_device_control_handler(Box::new(LocalPaneDCSHandler {
            pane_id,
            tmux_domain: None,
        }));
        terminal.set_notification_handler(Box::new(LocalPaneNotifHandler { pane_id }));

        Self {
            pane_id,
            terminal: RefCell::new(terminal),
            process: RefCell::new(ProcessState::Running {
                child_waiter: process,
                pid,
                signaller,
                killed: false,
            }),
            pty: RefCell::new(pty),
            writer: RefCell::new(writer),
            domain_id,
            tmux_domain: RefCell::new(None),
            proc_list: RefCell::new(None),
            command_description,
        }
    }

    fn divine_current_working_dir(&self) -> Option<Url> {
        #[cfg(unix)]
        if let Some(pid) = self.pty.borrow().process_group_leader() {
            if let Some(path) = LocalProcessInfo::current_working_dir(pid as u32) {
                return Url::parse(&format!("file://localhost{}", path.display())).ok();
            }
        }

        #[cfg(windows)]
        if let Some(fg) = self.divine_foreground_process() {
            // Since windows paths typically start with something like C:\,
            // we cannot simply stick `localhost` on the front; we have to
            // omit the hostname otherwise the url parser is unhappy.
            return Url::parse(&format!("file://{}", fg.cwd.display())).ok();
        }

        #[allow(unreachable_code)]
        None
    }

    fn divine_process_list(&self, force_refresh: bool) -> Option<RefMut<CachedProcInfo>> {
        if let ProcessState::Running { pid: Some(pid), .. } = &*self.process.borrow() {
            let mut proc_list = self.proc_list.borrow_mut();

            let expired = force_refresh
                || proc_list
                    .as_ref()
                    .map(|info| info.updated.elapsed() > Duration::from_millis(300))
                    .unwrap_or(true);

            if expired {
                log::trace!("CachedProcInfo expired, refresh");
                let root = LocalProcessInfo::with_root_pid(*pid)?;

                // Windows doesn't have any job control or session concept,
                // so we infer that the equivalent to the process group
                // leader is the most recently spawned program running
                // in the console
                let mut youngest = &root;

                fn find_youngest<'a>(
                    proc: &'a LocalProcessInfo,
                    youngest: &mut &'a LocalProcessInfo,
                ) {
                    if proc.start_time >= youngest.start_time {
                        *youngest = proc;
                    }

                    for child in proc.children.values() {
                        #[cfg(windows)]
                        if child.console == 0 {
                            continue;
                        }
                        find_youngest(child, youngest);
                    }
                }

                find_youngest(&root, &mut youngest);
                let mut foreground = youngest.clone();
                foreground.children.clear();

                proc_list.replace(CachedProcInfo {
                    root,
                    foreground,
                    updated: Instant::now(),
                });
                log::trace!("CachedProcInfo updated");
            }

            return Some(RefMut::map(proc_list, |info| info.as_mut().unwrap()));
        }
        None
    }

    #[allow(dead_code)]
    fn divine_foreground_process(&self) -> Option<LocalProcessInfo> {
        if let Some(info) = self.divine_process_list(false) {
            Some(info.foreground.clone())
        } else {
            None
        }
    }
}

impl Drop for LocalPane {
    fn drop(&mut self) {
        // Avoid lingering zombies if we can, but don't block forever.
        // <https://github.com/wez/wezterm/issues/558>
        if let ProcessState::Running { signaller, .. } = &mut *self.process.borrow_mut() {
            let _ = signaller.kill();
        }
    }
}
