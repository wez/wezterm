use crate::domain::{DomainId, WriterWrapper};
use crate::localpane::LocalPane;
use crate::pane::{alloc_pane_id, PaneId};
use crate::tab::{SplitDirection, SplitRequest, SplitSize, Tab, TabId};
use crate::tmux::{TmuxDomain, TmuxDomainState, TmuxRemotePane, TmuxTab};
use crate::tmux_pty::{TmuxChild, TmuxPty};
use crate::{Mux, Pane};
use anyhow::{anyhow, Context};
use fancy_regex::Regex;
use parking_lot::{Condvar, Mutex};
use portable_pty::{MasterPty, PtySize};
use std::collections::HashSet;
use std::fmt::{Debug, Write};
use std::io::Write as _;
use std::sync::Arc;
use termwiz::tmux_cc::*;
use wezterm_term::TerminalSize;

pub(crate) trait TmuxCommand: Send + Debug {
    fn get_command(&self, domain_id: DomainId) -> String;
    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaneItem {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    pane_id: TmuxPaneId,
    _pane_index: u64,
    cursor_x: u64,
    cursor_y: u64,
    pane_width: u64,
    pane_height: u64,
    pane_left: u64,
    pane_top: u64,
    _pane_active: bool,
}

#[derive(Debug)]
enum TmuxLayout {
    SplitVertical(Vec<PaneItem>),
    SplitHorizontal(Vec<PaneItem>),
    SinglePane(PaneItem),
}

#[derive(Debug)]
struct WindowItem {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    window_width: u64,
    window_height: u64,
    window_active: bool,
    layout: Vec<TmuxLayout>,
}

impl TmuxDomainState {
    /// check if a PaneItem received from ListAllPanes has been attached
    pub fn check_pane_attached(&self, window_id: TmuxWindowId, pane_id: TmuxPaneId) -> bool {
        let pane_list = self.gui_tabs.lock();
        let local_tab = match pane_list.iter().find(|&x| x.tmux_window_id == window_id) {
            Some(x) => x,
            None => {
                return false;
            }
        };
        match local_tab.panes.get(&pane_id) {
            Some(_) => {
                return true;
            }
            None => {
                return false;
            }
        }
    }

    /// after we create a tab for a remote pane, save its ID into the
    /// TmuxPane-TmuxPane tree, so we can ref it later.
    fn add_attached_pane(&self, target: &PaneItem, tab_id: &TabId) -> anyhow::Result<()> {
        let mut pane_list = self.gui_tabs.lock();
        let local_tab = match pane_list
            .iter_mut()
            .find(|x| x.tmux_window_id == target.window_id)
        {
            Some(x) => x,
            None => {
                pane_list.push(TmuxTab {
                    tab_id: *tab_id,
                    tmux_window_id: target.window_id,
                    panes: HashSet::new(),
                });
                pane_list.last_mut().unwrap()
            }
        };
        match local_tab.panes.get(&target.pane_id) {
            Some(_) => {
                anyhow::bail!("Tmux pane already attached");
            }
            None => {
                local_tab.panes.insert(target.pane_id);
                return Ok(());
            }
        }
    }

    fn remove_detached_pane(
        &self,
        window_id: TmuxWindowId,
        new_set: &HashSet<TmuxPaneId>,
    ) -> anyhow::Result<()> {
        let mut pane_list = self.gui_tabs.lock();
        let local_tab = match pane_list.iter_mut().find(|x| x.tmux_window_id == window_id) {
            Some(x) => x,
            None => {
                anyhow::bail!("Cannot find the window {window_id}")
            }
        };

        let to_remove: Vec<_> = local_tab.panes.difference(new_set).cloned().collect();
        for p in to_remove {
            let pane_map = self.remote_panes.lock();
            let local_pane_id = pane_map.get(&p).unwrap().lock().local_pane_id;
            let mux = Mux::get();
            mux.remove_pane(local_pane_id);
            local_tab.panes.remove(&p);

            if local_tab.panes.is_empty() {
                mux.remove_tab(local_tab.tab_id);
                let mut pane_list = self.gui_tabs.lock();
                pane_list.retain(|x| x.tmux_window_id != window_id);
            }
        }
        Ok(())
    }

    pub fn remove_detached_window(&self, window_id: TmuxWindowId) -> anyhow::Result<()> {
        let mut pane_list = self.gui_tabs.lock();
        let local_tab = match pane_list.iter_mut().find(|x| x.tmux_window_id == window_id) {
            Some(x) => x,
            None => {
                anyhow::bail!("Cannot find the window {window_id}")
            }
        };

        let mux = Mux::get();
        mux.remove_tab(local_tab.tab_id);

        Ok(())
    }

    fn create_pane(&self, pane: &PaneItem) -> anyhow::Result<Arc<dyn Pane>> {
        let local_pane_id = alloc_pane_id();
        let active_lock = Arc::new((Mutex::new(false), Condvar::new()));
        let (output_read, output_write) = filedescriptor::socketpair()?;
        let ref_pane = Arc::new(Mutex::new(TmuxRemotePane {
            local_pane_id,
            output_write,
            active_lock: active_lock.clone(),
            session_id: 0,
            window_id: pane.window_id,
            pane_id: pane.pane_id,
            cursor_x: pane.cursor_x,
            cursor_y: pane.cursor_y,
            pane_width: pane.pane_width,
            pane_height: pane.pane_height,
            pane_left: pane.pane_left,
            pane_top: pane.pane_top,
        }));

        {
            let mut pane_map = self.remote_panes.lock();
            pane_map.insert(pane.pane_id, ref_pane.clone());
        }

        let pane_pty = TmuxPty {
            domain_id: self.domain_id,
            reader: output_read,
            cmd_queue: self.cmd_queue.clone(),
            master_pane: ref_pane,
        };

        let writer = WriterWrapper::new(pane_pty.take_writer()?);

        let size = TerminalSize {
            rows: pane.pane_height as usize,
            cols: pane.pane_width as usize,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        };

        let child = TmuxChild {
            active_lock: active_lock.clone(),
        };

        let terminal = wezterm_term::Terminal::new(
            size,
            std::sync::Arc::new(config::TermConfig::new()),
            "WezTerm",
            config::wezterm_version(),
            Box::new(writer.clone()),
        );

        Ok(Arc::new(LocalPane::new(
            local_pane_id,
            terminal,
            Box::new(child),
            Box::new(pane_pty),
            Box::new(writer),
            self.domain_id,
            "tmux pane".to_string(),
        )))
    }

    pub fn split_pane(
        &self,
        tab_id: TabId,
        pane_id: PaneId,
        remote_id: TmuxPaneId,
        split_request: SplitRequest,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        let mux = Mux::get();
        let tab = match mux.get_tab(tab_id) {
            Some(t) => t,
            None => anyhow::bail!("Invalid tab id {}", tab_id),
        };

        let pane_index = match tab
            .iter_panes_ignoring_zoom()
            .iter()
            .find(|p| p.pane.pane_id() == pane_id)
        {
            Some(p) => p.index,
            None => anyhow::bail!("invalid pane id {}", pane_id),
        };

        let split_size = match tab.compute_split_size(pane_index, split_request) {
            Some(s) => s,
            None => anyhow::bail!("invalid pane index {}", pane_index),
        };

        let window_id;
        {
            let mut pane_list = self.gui_tabs.lock();
            window_id = match pane_list.iter_mut().find(|x| x.tab_id == tab_id) {
                Some(x) => x.tmux_window_id,
                None => 0,
            };
        }

        let p = PaneItem {
            session_id: 0,
            window_id: window_id,
            pane_id: remote_id,
            _pane_index: 0,
            cursor_x: 0,
            cursor_y: 0,
            pane_width: split_size.second.cols as u64,
            pane_height: split_size.second.rows as u64,
            pane_left: 0,
            pane_top: 0,
            _pane_active: false,
        };

        let pane = self.create_pane(&p).unwrap();
        tab.split_and_insert(pane_index, split_request, Arc::clone(&pane))?;

        self.add_attached_pane(&p, &tab_id)?;
        let _ = mux.add_pane(&pane);

        return Ok(pane);
    }

    fn sync_pane_state(&self, panes: &[PaneItem]) -> anyhow::Result<()> {
        let current_session = self.tmux_session.lock().unwrap_or(0);
        let mux = Mux::get();

        for pane in panes.iter() {
            if pane.session_id != current_session
                || !self.check_pane_attached(pane.window_id, pane.pane_id)
            {
                continue;
            }

            // We now have the cursor information, fix the cursor position
            let pane_map = self.remote_panes.lock();
            let local_pane_id = pane_map.get(&pane.pane_id).unwrap().lock().local_pane_id;
            let local_pane = mux.get_pane(local_pane_id).unwrap();
            local_pane.set_cursor_position(pane.cursor_x as usize, pane.cursor_y as usize);

            // TODO: Set active pane
            log::info!("new pane synced, id: {}", pane.pane_id);
        }
        Ok(())
    }

    fn sync_window_state(&self, windows: &[WindowItem], new_window: bool) -> anyhow::Result<()> {
        let current_session = self.tmux_session.lock().unwrap_or(0);
        let mux = Mux::get();

        self.create_gui_window();
        let mut gui_window = self.gui_window.lock();
        let gui_window_id = match gui_window.as_mut() {
            Some(x) => x,
            None => {
                anyhow::bail!("No tmux gui created");
            }
        };

        let mut session_id = 0;
        let mut active_tab_id = None;
        for window in windows.iter() {
            if window.session_id != current_session {
                continue;
            }

            let size = TerminalSize {
                rows: window.window_height as usize,
                cols: window.window_width as usize,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 0,
            };

            let tab = Arc::new(Tab::new(&size));
            tab.set_title(&format!("Tmux window: {}", window.window_id));
            mux.add_tab_no_panes(&tab);

            if window.window_active {
                active_tab_id = Some(tab.tab_id());
            }

            let mut split_stack;
            let mut split_direction;

            let mut split_pane_index = 1;
            for l in &window.layout {
                match l {
                    TmuxLayout::SinglePane(p) => {
                        let local_pane = self.create_pane(&p).unwrap();
                        tab.assign_pane(&local_pane);
                        self.add_attached_pane(&p, &tab.tab_id())?;
                        let _ = mux.add_pane(&local_pane);
                        if !new_window {
                            self.cmd_queue
                                .lock()
                                .push_back(Box::new(CapturePane(p.pane_id)));
                            TmuxDomainState::schedule_send_next_command(self.domain_id);
                        }
                        break;
                    }

                    TmuxLayout::SplitHorizontal(x) => {
                        split_direction = SplitDirection::Horizontal;
                        split_stack = x;
                    }

                    TmuxLayout::SplitVertical(x) => {
                        split_direction = SplitDirection::Vertical;
                        split_stack = x;
                    }
                }

                for p in split_stack {
                    let local_pane;
                    if !self.check_pane_attached(p.window_id, p.pane_id) {
                        local_pane = self.create_pane(&p).unwrap();
                        self.add_attached_pane(&p, &tab.tab_id())?;
                        let _ = mux.add_pane(&local_pane);
                        if let None = tab.get_active_pane() {
                            tab.assign_pane(&local_pane);
                            split_pane_index = tab.get_active_idx();
                            self.cmd_queue
                                .lock()
                                .push_back(Box::new(CapturePane(p.pane_id)));
                            TmuxDomainState::schedule_send_next_command(self.domain_id);
                            continue;
                        }

                        split_pane_index = tab.split_and_insert(
                            split_pane_index,
                            SplitRequest {
                                direction: split_direction,
                                target_is_second: false,
                                top_level: false,
                                size: SplitSize::Cells(
                                    if split_direction == SplitDirection::Horizontal {
                                        p.pane_width as usize
                                    } else {
                                        p.pane_height as usize
                                    },
                                ),
                            },
                            local_pane.clone(),
                        )? + 1;

                        self.cmd_queue
                            .lock()
                            .push_back(Box::new(CapturePane(p.pane_id)));
                        TmuxDomainState::schedule_send_next_command(self.domain_id);
                    } else {
                        let pane_map = self.remote_panes.lock();
                        let local_pane_id = pane_map.get(&p.pane_id).unwrap().lock().local_pane_id;
                        split_pane_index = match tab
                            .iter_panes_ignoring_zoom()
                            .iter()
                            .find(|x| x.pane.pane_id() == local_pane_id)
                        {
                            Some(x) => x.index,
                            None => anyhow::bail!("invalid pane id {}", local_pane_id),
                        };
                        continue;
                    }
                }
            }

            mux.add_tab_to_window(&tab, **gui_window_id)?;
            gui_window_id.notify();

            session_id = window.session_id;

            self.cmd_queue.lock().push_back(Box::new(ListAllPanes {
                window_id: window.window_id,
                prune: false,
            }));
            TmuxDomainState::schedule_send_next_command(self.domain_id);
        }

        if let Some(mut window) = mux.get_window_mut(**gui_window_id) {
            window.set_title(&format!("Tmux Session: {session_id}"));

            if let Some(idx) = active_tab_id {
                let tab_idx = window
                    .idx_by_id(idx)
                    .ok_or_else(|| anyhow::anyhow!("tab {idx} not in {}", **gui_window_id))?;
                window.save_and_then_set_active(tab_idx);
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ListAllPanes {
    pub window_id: TmuxWindowId,
    pub prune: bool,
}

impl TmuxCommand for ListAllPanes {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!(
            "list-panes -F '#{{session_id}} #{{window_id}} #{{pane_id}} \
            #{{pane_index}} #{{cursor_x}} #{{cursor_y}} #{{pane_width}} #{{pane_height}} \
            #{{pane_left}} #{{pane_top}} #{{pane_active}}' -t @{}\n",
            self.window_id
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mut items = vec![];
        let mut pane_set = HashSet::new();
        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            let session_id = fields.next().ok_or_else(|| anyhow!("missing session_id"))?;
            let window_id = fields.next().ok_or_else(|| anyhow!("missing window_id"))?;
            let pane_id = fields.next().ok_or_else(|| anyhow!("missing pane_id"))?;
            let _pane_index = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_index"))?
                .parse()?;
            let cursor_x = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_x"))?
                .parse()?;
            let cursor_y = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_y"))?
                .parse()?;
            let pane_width = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_width"))?
                .parse()?;
            let pane_height = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_height"))?
                .parse()?;
            let pane_left = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_left"))?
                .parse()?;
            let pane_top = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_top"))?
                .parse()?;
            let pane_active = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_active"))?
                .parse::<usize>()?;

            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id = session_id[1..].parse()?;
            let window_id = window_id[1..].parse()?;
            let pane_id = pane_id[1..].parse()?;
            let _pane_active = if pane_active == 1 { true } else { false };

            pane_set.insert(pane_id);

            items.push(PaneItem {
                session_id,
                window_id,
                pane_id,
                _pane_index,
                cursor_x,
                cursor_y,
                pane_width,
                pane_height,
                pane_left,
                pane_top,
                _pane_active,
            });
        }

        log::debug!("panes in domain_id {}: {:?}", domain_id, items);
        let mux = Mux::get();
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                if !self.prune {
                    return tmux_domain.inner.sync_pane_state(&items);
                } else {
                    return tmux_domain
                        .inner
                        .remove_detached_pane(self.window_id, &pane_set);
                }
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}

fn parse_pane(layout: &str) -> anyhow::Result<PaneItem> {
    let re_pane = Regex::new(r"^,?(\d+)x(\d+),(\d+),(\d+)(,(\d+))?").unwrap();

    if let Some(caps) = re_pane.captures(layout).unwrap() {
        let pane_width = caps
            .get(1)
            .unwrap()
            .as_str()
            .parse()
            .expect("Wrong pane width");
        let pane_height = caps
            .get(2)
            .unwrap()
            .as_str()
            .parse()
            .expect("Wrong pane height");
        let pane_left = caps
            .get(3)
            .unwrap()
            .as_str()
            .parse()
            .expect("Wrong pane left");
        let pane_top = caps
            .get(4)
            .unwrap()
            .as_str()
            .parse()
            .expect("Wrong pane top");
        let pane_id = match caps.get(6) {
            Some(x) => x.as_str().parse().expect(""),
            None => 0,
        };

        return Ok(PaneItem {
            _pane_index: 0, // Don't care
            session_id: 0,  // Will fill it later
            window_id: 0,
            pane_id,
            pane_width,
            pane_height,
            pane_left,
            pane_top,
            cursor_x: 0, // the layout doesn't include this information, will set by list-panes
            cursor_y: 0,
            _pane_active: false, // same as above
        });
    }

    anyhow::bail!("Wrong pane layout format");
}

fn parse_layout(
    mut layout: &str,
    result: &mut Vec<TmuxLayout>,
) -> anyhow::Result<(Option<TmuxLayout>, usize)> {
    log::debug!("Parsing tmux layout: '{}'", layout);

    let re_pane = Regex::new(r"^,?(\d+x\d+,\d+,\d+,\d+)").unwrap();
    let re_split_push = Regex::new(r"^,?(\d+x\d+,\d+,\d+)[\{|\[]").unwrap();
    let re_split_h_pop = Regex::new(r"^\}").unwrap();
    let re_split_v_pop = Regex::new(r"^\]").unwrap();

    let mut parse_len = 0;
    let mut stack = Vec::new();

    while layout.len() > 0 {
        if let Some(caps) = re_split_push.captures(layout).unwrap() {
            log::debug!("Tmux layout split pane push");
            let len = caps.get(0).unwrap().as_str().len();
            parse_len += len;
            let mut pane = parse_pane(caps.get(1).unwrap().as_str()).unwrap();

            layout = layout.get(len..).unwrap();
            if result.is_empty() {
                // Fake one, to flag it is not a TmuxLayout::SinglePane will pop
                result.push(TmuxLayout::SplitHorizontal(vec![]));
            }

            let (split_pane, len) = parse_layout(layout, result).unwrap();
            let mut split_pane = split_pane.unwrap();

            match split_pane {
                TmuxLayout::SplitHorizontal(ref mut x) => {
                    let last_item = x.pop().unwrap();
                    pane.pane_id = last_item.pane_id;
                    x.insert(0, pane.clone());
                }
                TmuxLayout::SplitVertical(ref mut x) => {
                    let last_item = x.pop().unwrap();
                    pane.pane_id = last_item.pane_id;
                    x.insert(0, pane.clone());
                }
                TmuxLayout::SinglePane(_x) => {
                    anyhow::bail!("The tmux layout is not right")
                }
            }

            result.insert(0, split_pane);

            stack.push(pane);

            layout = layout.get(len..).unwrap();
            parse_len += len;
        } else if let Some(caps) = re_pane.captures(layout).unwrap() {
            log::debug!("Tmux layout pane");
            let len = caps.get(0).unwrap().as_str().len();
            let pane = parse_pane(caps.get(1).unwrap().as_str()).unwrap();

            // SinglePane
            if result.is_empty() {
                result.insert(0, TmuxLayout::SinglePane(pane));
                return Ok((None, len));
            }

            stack.push(pane);
            parse_len += len;
            layout = layout.get(len..).unwrap();
        } else if let Some(_caps) = re_split_h_pop.captures(layout).unwrap() {
            log::debug!("Tmux layout split horizontal pop");
            return Ok((Some(TmuxLayout::SplitHorizontal(stack)), parse_len + 1));
        } else if let Some(_caps) = re_split_v_pop.captures(layout).unwrap() {
            log::debug!("Tmux layout split vertical pop");
            return Ok((Some(TmuxLayout::SplitVertical(stack)), parse_len + 1));
        }
    }

    // Pop the fake one
    let _ = result.pop();

    Ok((None, 0))
}

#[derive(Debug)]
pub(crate) struct ListAllWindows {
    pub session_id: TmuxSessionId,
    pub window_id: Option<TmuxWindowId>,
}

impl TmuxCommand for ListAllWindows {
    fn get_command(&self, _domain_id: DomainId) -> String {
        format!(
            "list-windows -F \
                '#{{session_id}} #{{window_id}} \
                #{{window_width}} #{{window_height}} \
                #{{window_active}} \
                #{{window_layout}} -t {}'\n",
            self.session_id
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mut items = vec![];

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            let session_id = fields.next().ok_or_else(|| anyhow!("missing session_id"))?;
            let window_id = fields.next().ok_or_else(|| anyhow!("missing window_id"))?;
            let window_width = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_width"))?
                .parse()?;
            let window_height = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_height"))?
                .parse()?;
            let window_active = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_active"))?
                .parse::<usize>()?;

            let window_layout = fields
                .next()
                .ok_or_else(|| anyhow!("missing window_layout"))?;

            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id = session_id[1..].parse()?;
            let window_id = window_id[1..].parse()?;

            if let Some(x) = self.window_id {
                if x != window_id {
                    continue;
                }
            }

            let window_layout = window_layout.get(5..).unwrap();

            let mut layout = Vec::<TmuxLayout>::new();

            let _ = parse_layout(window_layout, &mut layout)?;
            // Fill in the session_id and window_id
            for l in &mut layout {
                match l {
                    TmuxLayout::SinglePane(ref mut p) => {
                        p.session_id = session_id;
                        p.window_id = window_id;
                    }
                    TmuxLayout::SplitHorizontal(ref mut v) => {
                        for p in v {
                            p.session_id = session_id;
                            p.window_id = window_id;
                        }
                    }
                    TmuxLayout::SplitVertical(ref mut v) => {
                        for p in v {
                            p.session_id = session_id;
                            p.window_id = window_id;
                        }
                    }
                }
            }

            let window_active = if window_active == 1 { true } else { false };

            items.push(WindowItem {
                session_id,
                window_id,
                window_width,
                window_height,
                window_active,
                layout,
            });
        }

        log::debug!("layout in domain_id {}: {:#?}", domain_id, items);
        let mux = Mux::get();
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                return tmux_domain.inner.sync_window_state(
                    &items,
                    if let Some(_) = self.window_id {
                        true
                    } else {
                        false
                    },
                );
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}

#[derive(Debug)]
pub(crate) struct Resize {
    pub pane_id: TmuxPaneId,
    pub size: PtySize,
}

impl TmuxCommand for Resize {
    fn get_command(&self, domain_id: DomainId) -> String {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => panic!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => panic!("Tmux domain lost"),
        };

        let pane_map = tmux_domain.inner.remote_panes.lock();

        let tmux_window_id = match pane_map.get(&self.pane_id) {
            Some(x) => x.lock().window_id,
            None => panic!("No this pane"),
        };

        let pane_list = tmux_domain.inner.gui_tabs.lock();
        let local_tab = match pane_list
            .iter()
            .find(|&x| x.tmux_window_id == tmux_window_id)
        {
            Some(x) => x.tab_id,
            None => panic!("Could not find the tab for this pane"),
        };

        let size = match mux.get_tab(local_tab) {
            Some(x) => x.get_size(),
            None => TerminalSize {
                rows: 80,
                cols: 24,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 0,
            },
        };

        let support_commands = tmux_domain.inner.support_commands.lock();

        if let Some(_x) = support_commands.get("resize-window") {
            format!(
                "resize-window -x {} -y {} -t @{};resize-pane -x {} -y {} -t %{}\n",
                size.cols, size.rows, tmux_window_id, self.size.cols, self.size.rows, self.pane_id
            )
        } else if let Some(x) = support_commands.get("refresh-client") {
            if x.contains("-C XxY") {
                format!(
                    "refresh-client -C {}x{}\nresize-pane -x {} -y {} -t %{}\n",
                    size.cols, size.rows, self.size.cols, self.size.rows, self.pane_id
                )
            } else {
                format!(
                    "refresh-client -C {},{}\nresize-pane -x {} -y {} -t %{}\n",
                    size.cols, size.rows, self.size.cols, self.size.rows, self.pane_id
                )
            }
        } else {
            panic!("The tmux version is not supported");
        }
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        if result.error {
            log::error!(
                "Error resizing: domain_id={} result={:?}",
                domain_id,
                result
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct CapturePane(TmuxPaneId);
impl TmuxCommand for CapturePane {
    fn get_command(&self, _domain_id: DomainId) -> String {
        const HISTORY_LINES: isize = -2000;
        format!(
            "capture-pane -p -t %{} -e -C -S {}\n",
            self.0, HISTORY_LINES
        )
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => anyhow::bail!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => anyhow::bail!("Tmux domain lost"),
        };

        //let unescaped = termwiz::tmux_cc::unvis(&result.output.trim_end_matches('\n')).context("unescape pane content")?;
        let unescaped = termwiz::tmux_cc::unvis(&result.output).context("unescape pane content")?;
        // capturep contents returned from guarded lines which always contain a tailing '\n'
        let unescaped = &unescaped[0..unescaped.len().saturating_sub(1)].replace("\n", "\r\n");

        let pane_map = tmux_domain.inner.remote_panes.lock();
        if let Some(pane) = pane_map.get(&self.0) {
            let mut pane = pane.lock();
            pane.output_write
                .write_all(unescaped.as_bytes())
                .context("writing capture pane result to output")?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SendKeys {
    pub keys: Vec<u8>,
    pub pane: TmuxPaneId,
}
impl TmuxCommand for SendKeys {
    fn get_command(&self, _domain_id: DomainId) -> String {
        let mut s = String::new();
        for &byte in self.keys.iter() {
            write!(&mut s, "0x{:X} ", byte).expect("unable to write key");
        }
        format!("send-keys -t %{} {}\r", self.pane, s)
    }

    fn process_result(&self, _domain_id: DomainId, _result: &Guarded) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct NewWindow;
impl TmuxCommand for NewWindow {
    fn get_command(&self, _domain_id: DomainId) -> String {
        "new-window\n".to_owned()
    }

    fn process_result(&self, _domain_id: DomainId, _result: &Guarded) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ResizeWindow {
    pub window_id: TmuxWindowId,
    pub width: usize,
    pub height: usize,
}

impl TmuxCommand for ResizeWindow {
    fn get_command(&self, domain_id: DomainId) -> String {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => panic!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => panic!("Tmux domain lost"),
        };

        let support_commands = tmux_domain.inner.support_commands.lock();

        if let Some(_x) = support_commands.get("resize-window") {
            format!(
                "resize-window -x {} -y {} -t @{}\n",
                self.width, self.height, self.window_id
            )
        } else if let Some(x) = support_commands.get("refresh-client") {
            if x.contains("-C XxY") {
                format!("refresh-client -C {}x{}\n", self.width, self.height)
            } else {
                format!("refresh-client -C {},{}\n", self.width, self.height)
            }
        } else {
            panic!("The tmux version is not supported");
        }
    }

    fn process_result(&self, _domain_id: DomainId, _result: &Guarded) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ListCommands;
impl TmuxCommand for ListCommands {
    fn get_command(&self, _domain_id: DomainId) -> String {
        "list-commands\n".to_owned()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mux = Mux::get();
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => panic!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => panic!("Tmux domain lost"),
        };

        let mut support_commands = tmux_domain.inner.support_commands.lock();

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let v: Vec<&str> = line.split(' ').collect();
            support_commands.insert(v[0].to_string(), line.to_string());
        }

        let mut cmd_queue = tmux_domain.inner.cmd_queue.as_ref().lock();
        let session = tmux_domain.inner.tmux_session.lock().unwrap();
        cmd_queue.push_back(Box::new(ListAllWindows {
            session_id: session,
            window_id: None,
        }));
        TmuxDomainState::schedule_send_next_command(domain_id);

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SplitPane {
    pub pane_id: TmuxPaneId,
    pub direction: SplitDirection,
}

impl TmuxCommand for SplitPane {
    fn get_command(&self, _domain_id: DomainId) -> String {
        if self.direction == SplitDirection::Horizontal {
            format!("split-window -h -t %{}\n", self.pane_id)
        } else {
            format!("split-window -v -t %{}\n", self.pane_id)
        }
    }

    fn process_result(&self, _domain_id: DomainId, _result: &Guarded) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pane() {
        let pane_case1 = "100x200,0,0".to_string();
        let pane_case2 = "300x400,10,20,17".to_string();

        let p = parse_pane(pane_case1.get(0..).unwrap()).unwrap();
        assert_eq!(p.pane_id, 0);
        assert_eq!(p.pane_width, 100);
        assert_eq!(p.pane_height, 200);
        assert_eq!(p.pane_left, 0);
        assert_eq!(p.pane_top, 0);

        let p = parse_pane(pane_case2.get(0..).unwrap()).unwrap();
        assert_eq!(p.pane_id, 17);
        assert_eq!(p.pane_width, 300);
        assert_eq!(p.pane_height, 400);
        assert_eq!(p.pane_left, 10);
        assert_eq!(p.pane_top, 20);
    }

    #[test]
    fn test_parse_layout() {
        let layout_case1 = "158x40,0,0,72".to_string();
        let layout_case2 = "158x40,0,0[158x20,0,0,69,158x19,0,21{79x19,0,21,70,78x19,80,21[78x9,80,21,71,78x9,80,31,73]}]".to_string();
        let layout_case3 = "158x40,0,0{79x40,0,0[79x20,0,0,74,79x19,0,21{39x19,0,21,76,39x19,40,21,77}],78x40,80,0,75}".to_string();

        let mut layout = Vec::new();
        let _ = parse_layout(layout_case1.get(0..).unwrap(), &mut layout).unwrap();
        let l = layout.pop().unwrap();
        assert!(if let TmuxLayout::SinglePane(_x) = l {
            true
        } else {
            false
        });

        layout = Vec::new();
        let _ = parse_layout(layout_case2.get(0..).unwrap(), &mut layout).unwrap();
        assert!(if let TmuxLayout::SplitVertical(_x) = &layout[0] {
            true
        } else {
            false
        });
        assert!(if let TmuxLayout::SplitHorizontal(_x) = &layout[1] {
            true
        } else {
            false
        });
        assert!(if let TmuxLayout::SplitVertical(_x) = &layout[2] {
            true
        } else {
            false
        });

        layout = Vec::new();
        let _ = parse_layout(layout_case3.get(0..).unwrap(), &mut layout).unwrap();
        assert!(if let TmuxLayout::SplitHorizontal(_x) = &layout[0] {
            true
        } else {
            false
        });
        assert!(if let TmuxLayout::SplitVertical(_x) = &layout[1] {
            true
        } else {
            false
        });
        assert!(if let TmuxLayout::SplitHorizontal(_x) = &layout[2] {
            true
        } else {
            false
        });
    }
}
