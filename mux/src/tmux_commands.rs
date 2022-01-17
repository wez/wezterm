use crate::domain::DomainId;
use crate::localpane::LocalPane;
use crate::pane::alloc_pane_id;
use crate::tab::{Tab, TabId};
use crate::tmux::{TmuxDomain, TmuxDomainState, TmuxRemotePane, TmuxTab};
use crate::tmux_pty::{TmuxChild, TmuxPty};
use crate::{Mux, Pane};
use anyhow::{anyhow, Context};
use portable_pty::{MasterPty, PtySize};
use std::collections::HashSet;
use std::fmt::{Debug, Write};
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use termwiz::tmux_cc::*;

pub(crate) trait TmuxCommand: Send + Debug {
    fn get_command(&self) -> String;
    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()>;
}

#[derive(Debug)]
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
}

impl TmuxDomainState {
    /// check if a PaneItem received from ListAllPanes has been attached
    fn check_pane_attached(&self, target: &PaneItem) -> bool {
        let pane_list = self.gui_tabs.borrow();
        let local_tab = match pane_list
            .iter()
            .find(|&x| x.tmux_window_id == target.window_id)
        {
            Some(x) => x,
            None => {
                return false;
            }
        };
        match local_tab.panes.get(&target.pane_id) {
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
        let mut pane_list = self.gui_tabs.borrow_mut();
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

    fn sync_pane_state(&self, panes: &[PaneItem]) -> anyhow::Result<()> {
        // TODO:
        // 1) iter over current session panes
        // 2) create pane if not exist
        // 3) fetch scroll buffer if new created
        // 4) update pane state if exist
        let current_session = self.tmux_session.borrow().unwrap_or(0);
        for pane in panes.iter() {
            if pane.session_id != current_session || self.check_pane_attached(&pane) {
                continue;
            }

            let local_pane_id = alloc_pane_id();
            let (output_read, output_write) = filedescriptor::socketpair()?;
            let active_lock = Arc::new((Mutex::new(false), Condvar::new()));

            let ref_pane = Arc::new(Mutex::new(TmuxRemotePane {
                local_pane_id,
                output_write,
                active_lock: active_lock.clone(),
                session_id: pane.session_id,
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
                let mut pane_map = self.remote_panes.borrow_mut();
                pane_map.insert(pane.pane_id, ref_pane.clone());
            }

            let pane_pty = TmuxPty {
                domain_id: self.domain_id,
                reader: output_read,
                cmd_queue: self.cmd_queue.clone(),
                master_pane: ref_pane,
            };
            let writer = pane_pty.try_clone_writer()?;
            let mux = Mux::get().expect("should be called at main thread");
            let size = PtySize {
                rows: pane.pane_height as u16,
                cols: pane.pane_width as u16,
                pixel_width: 0,
                pixel_height: 0,
            };

            let child = TmuxChild {
                active_lock: active_lock.clone(),
            };

            let terminal = wezterm_term::Terminal::new(
                crate::pty_size_to_terminal_size(size),
                std::sync::Arc::new(config::TermConfig::new()),
                "WezTerm",
                config::wezterm_version(),
                Box::new(writer),
            );

            let local_pane: Rc<dyn Pane> = Rc::new(LocalPane::new(
                local_pane_id,
                terminal,
                Box::new(child),
                Box::new(pane_pty),
                self.domain_id,
            ));

            let tab = Rc::new(Tab::new(&size));
            tab.assign_pane(&local_pane);

            self.create_gui_window();
            let mut gui_window = self.gui_window.borrow_mut();
            let gui_window_id = match gui_window.as_mut() {
                Some(x) => x,
                None => {
                    anyhow::bail!("No tmux gui created");
                }
            };

            mux.add_tab_and_active_pane(&tab)?;
            mux.add_tab_to_window(&tab, **gui_window_id)?;
            gui_window_id.notify();

            self.cmd_queue
                .lock()
                .unwrap()
                .push_back(Box::new(CapturePane(pane.pane_id)));
            TmuxDomainState::schedule_send_next_command(self.domain_id);

            self.add_attached_pane(&pane, &tab.tab_id())?;
            log::info!("new pane attached");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ListAllPanes;
impl TmuxCommand for ListAllPanes {
    fn get_command(&self) -> String {
        "list-panes -aF '#{session_id} #{window_id} #{pane_id} \
            #{pane_index} #{cursor_x} #{cursor_y} #{pane_width} #{pane_height} \
            #{pane_left} #{pane_top}'\n"
            .to_owned()
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

            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id = session_id[1..].parse()?;
            let window_id = window_id[1..].parse()?;
            let pane_id = pane_id[1..].parse()?;

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
            });
        }

        log::info!("panes in domain_id {}: {:?}", domain_id, items);
        let mux = Mux::get().expect("to be called on main thread");
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                return tmux_domain.inner.sync_pane_state(&items);
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}

#[derive(Debug)]
pub(crate) struct Resize {
    pub size: portable_pty::PtySize,
}

impl TmuxCommand for Resize {
    fn get_command(&self) -> String {
        format!("refresh-client -C {}x{}\n", self.size.cols, self.size.rows)
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
    fn get_command(&self) -> String {
        format!("capturep -p -t %{} -e -C\n", self.0)
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mux = Mux::get().expect("to be called on main thread");
        let domain = match mux.get_domain(domain_id) {
            Some(d) => d,
            None => anyhow::bail!("Tmux domain lost"),
        };
        let tmux_domain = match domain.downcast_ref::<TmuxDomain>() {
            Some(t) => t,
            None => anyhow::bail!("Tmux domain lost"),
        };

        let unescaped = termwiz::tmux_cc::unvis(&result.output).context("unescape pane content")?;
        let unescaped = unescaped.replace("\n", "\r\n");

        let pane_map = tmux_domain.inner.remote_panes.borrow();
        if let Some(pane) = pane_map.get(&self.0) {
            let mut pane = pane.lock().expect("Grant lock of tmux cmd queue failed");
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
    fn get_command(&self) -> String {
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
