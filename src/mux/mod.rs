use crate::mux::tab::{Pane, PaneId};
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::{Window, WindowId};
use crate::ratelim::RateLimiter;
use crate::server::pollable::{pollable_channel, PollableReceiver, PollableSender};
use anyhow::{anyhow, Error};
use domain::{Domain, DomainId};
use log::{debug, error};
use portable_pty::ExitStatus;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use thiserror::*;

pub mod domain;
pub mod renderable;
pub mod tab;
pub mod window;

#[derive(Clone, Debug)]
pub enum MuxNotification {
    PaneOutput(PaneId),
}

static SUB_ID: AtomicUsize = AtomicUsize::new(0);

pub type MuxSubscriber = PollableReceiver<MuxNotification>;

pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<Tab>>>,
    panes: RefCell<HashMap<PaneId, Rc<dyn Pane>>>,
    windows: RefCell<HashMap<WindowId, Window>>,
    default_domain: RefCell<Option<Arc<dyn Domain>>>,
    domains: RefCell<HashMap<DomainId, Arc<dyn Domain>>>,
    domains_by_name: RefCell<HashMap<String, Arc<dyn Domain>>>,
    subscribers: RefCell<HashMap<usize, PollableSender<MuxNotification>>>,
}

fn read_from_pane_pty(pane_id: PaneId, mut reader: Box<dyn std::io::Read>) {
    const BUFSIZE: usize = 32 * 1024;
    let mut buf = [0; BUFSIZE];

    let mut lim = RateLimiter::new(|config| config.ratelimit_output_bytes_per_second);
    let dead = Arc::new(AtomicBool::new(false));

    'outer: while !dead.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                error!("read_pty EOF: pane_id {}", pane_id);
                break;
            }
            Err(err) => {
                error!("read_pty failed: pane {} {:?}", pane_id, err);
                break;
            }
            Ok(size) => {
                let buf = &buf[..size];
                let mut pos = 0;

                while pos < size {
                    if dead.load(Ordering::Relaxed) {
                        break 'outer;
                    }
                    match lim.admit_check((size - pos) as u32) {
                        Ok(len) => {
                            let len = len as usize;
                            let data = buf[pos..pos + len].to_vec();
                            pos += len;
                            promise::spawn::spawn_into_main_thread_with_low_priority({
                                let dead = Arc::clone(&dead);
                                async move {
                                    let mux = Mux::get().unwrap();
                                    if let Some(pane) = mux.get_pane(pane_id) {
                                        pane.advance_bytes(&data);
                                        mux.notify(MuxNotification::PaneOutput(pane_id));
                                    } else {
                                        // Something else removed the pane from
                                        // the mux, so we should stop trying to
                                        // process it.
                                        dead.store(true, Ordering::Relaxed);
                                    }
                                }
                            });
                        }
                        Err(delay) => {
                            log::trace!("RateLimiter: sleep for {:?}", delay);
                            std::thread::sleep(delay);
                        }
                    }
                }
            }
        }
    }
    promise::spawn::spawn_into_main_thread(async move {
        let mux = Mux::get().unwrap();
        mux.remove_pane(pane_id);
    });
}

thread_local! {
    static MUX: RefCell<Option<Rc<Mux>>> = RefCell::new(None);
}

impl Mux {
    pub fn new(default_domain: Option<Arc<dyn Domain>>) -> Self {
        let mut domains = HashMap::new();
        let mut domains_by_name = HashMap::new();
        if let Some(default_domain) = default_domain.as_ref() {
            domains.insert(default_domain.domain_id(), Arc::clone(default_domain));

            domains_by_name.insert(
                default_domain.domain_name().to_string(),
                Arc::clone(default_domain),
            );
        }

        Self {
            tabs: RefCell::new(HashMap::new()),
            panes: RefCell::new(HashMap::new()),
            windows: RefCell::new(HashMap::new()),
            default_domain: RefCell::new(default_domain),
            domains_by_name: RefCell::new(domains_by_name),
            domains: RefCell::new(domains),
            subscribers: RefCell::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self) -> anyhow::Result<MuxSubscriber> {
        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = pollable_channel()?;
        self.subscribers.borrow_mut().insert(sub_id, tx);
        Ok(rx)
    }

    pub fn notify(&self, notification: MuxNotification) {
        let mut subscribers = self.subscribers.borrow_mut();
        subscribers.retain(|_, tx| tx.send(notification.clone()).is_ok());
    }

    pub fn default_domain(&self) -> Arc<dyn Domain> {
        self.default_domain
            .borrow()
            .as_ref()
            .map(Arc::clone)
            .unwrap()
    }

    pub fn set_default_domain(&self, domain: &Arc<dyn Domain>) {
        *self.default_domain.borrow_mut() = Some(Arc::clone(domain));
    }

    pub fn get_domain(&self, id: DomainId) -> Option<Arc<dyn Domain>> {
        self.domains.borrow().get(&id).cloned()
    }

    pub fn get_domain_by_name(&self, name: &str) -> Option<Arc<dyn Domain>> {
        self.domains_by_name.borrow().get(name).cloned()
    }

    pub fn add_domain(&self, domain: &Arc<dyn Domain>) {
        if self.default_domain.borrow().is_none() {
            *self.default_domain.borrow_mut() = Some(Arc::clone(domain));
        }
        self.domains
            .borrow_mut()
            .insert(domain.domain_id(), Arc::clone(domain));
        self.domains_by_name
            .borrow_mut()
            .insert(domain.domain_name().to_string(), Arc::clone(domain));
    }

    pub fn set_mux(mux: &Rc<Mux>) {
        MUX.with(|m| {
            *m.borrow_mut() = Some(Rc::clone(mux));
        });
    }

    pub fn shutdown() {
        MUX.with(|m| drop(m.borrow_mut().take()));
    }

    pub fn get() -> Option<Rc<Mux>> {
        let mut res = None;
        MUX.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Rc::clone(mux));
            }
        });
        res
    }

    pub fn get_pane(&self, pane_id: PaneId) -> Option<Rc<dyn Pane>> {
        self.panes.borrow().get(&pane_id).map(Rc::clone)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        self.tabs.borrow().get(&tab_id).map(Rc::clone)
    }

    pub fn add_pane(&self, pane: &Rc<dyn Pane>) -> Result<(), Error> {
        self.panes
            .borrow_mut()
            .insert(pane.pane_id(), Rc::clone(pane));
        let reader = pane.reader()?;
        let pane_id = pane.pane_id();
        thread::spawn(move || read_from_pane_pty(pane_id, reader));
        Ok(())
    }

    pub fn add_tab_no_panes(&self, tab: &Rc<Tab>) {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));
    }

    pub fn add_tab_and_active_pane(&self, tab: &Rc<Tab>) -> Result<(), Error> {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));
        let pane = tab
            .get_active_pane()
            .ok_or_else(|| anyhow!("tab MUST have an active pane"))?;
        self.add_pane(&pane)
    }

    pub fn remove_pane(&self, pane_id: PaneId) {
        debug!("removing pane {}", pane_id);
        if let Some(pane) = self.panes.borrow_mut().remove(&pane_id) {
            pane.kill();
        }
        self.prune_dead_windows();
    }

    pub fn remove_tab(&self, tab_id: TabId) {
        debug!("removing tab {}", tab_id);
        let mut pane_ids = vec![];
        if let Some(tab) = self.tabs.borrow_mut().remove(&tab_id) {
            for pos in tab.iter_panes() {
                pane_ids.push(pos.pane.pane_id());
            }
        }
        for pane_id in pane_ids {
            self.remove_pane(pane_id);
        }
        self.prune_dead_windows();
    }

    pub fn prune_dead_windows(&self) {
        let live_tab_ids: Vec<TabId> = self.tabs.borrow().keys().cloned().collect();
        let mut windows = self.windows.borrow_mut();
        let mut dead_windows = vec![];
        for (window_id, win) in windows.iter_mut() {
            win.prune_dead_tabs(&live_tab_ids);
            if win.is_empty() {
                log::error!("prune_dead_windows: window is now empty");
                dead_windows.push(*window_id);
            }
        }

        let dead_tab_ids: Vec<TabId> = self
            .tabs
            .borrow()
            .iter()
            .filter_map(|(&id, tab)| if tab.is_dead() { Some(id) } else { None })
            .collect();

        for tab_id in dead_tab_ids {
            log::error!("tab {} is dead", tab_id);
            self.tabs.borrow_mut().remove(&tab_id);
        }

        /*
        let dead_pane_ids: Vec<TabId> = self
            .panes
            .borrow()
            .iter()
            .filter_map(|(&id, pane)| if pane.is_dead() { Some(id) } else { None })
            .collect();

        for pane_id in dead_pane_ids {
            self.panes.borrow_mut().remove(&pane_id);
        }
        */

        for window_id in dead_windows {
            error!("removing window {}", window_id);
            windows.remove(&window_id);
        }
    }

    pub fn kill_window(&self, window_id: WindowId) {
        let mut windows = self.windows.borrow_mut();
        if let Some(window) = windows.remove(&window_id) {
            for tab in window.iter() {
                self.tabs.borrow_mut().remove(&tab.tab_id());
            }
        }
    }

    pub fn get_window(&self, window_id: WindowId) -> Option<Ref<Window>> {
        if !self.windows.borrow().contains_key(&window_id) {
            return None;
        }
        Some(Ref::map(self.windows.borrow(), |windows| {
            windows.get(&window_id).unwrap()
        }))
    }

    pub fn get_window_mut(&self, window_id: WindowId) -> Option<RefMut<Window>> {
        if !self.windows.borrow().contains_key(&window_id) {
            return None;
        }
        Some(RefMut::map(self.windows.borrow_mut(), |windows| {
            windows.get_mut(&window_id).unwrap()
        }))
    }

    pub fn get_active_tab_for_window(&self, window_id: WindowId) -> Option<Rc<Tab>> {
        let window = self.get_window(window_id)?;
        window.get_active().map(Rc::clone)
    }

    pub fn new_empty_window(&self) -> WindowId {
        let window = Window::new();
        let window_id = window.window_id();
        self.windows.borrow_mut().insert(window_id, window);
        window_id
    }

    pub fn add_tab_to_window(&self, tab: &Rc<Tab>, window_id: WindowId) -> anyhow::Result<()> {
        let mut window = self
            .get_window_mut(window_id)
            .ok_or_else(|| anyhow!("add_tab_to_window: no such window_id {}", window_id))?;
        window.push(tab);
        Ok(())
    }

    pub fn window_containing_tab(&self, tab_id: TabId) -> Option<WindowId> {
        for w in self.windows.borrow().values() {
            for t in w.iter() {
                if t.tab_id() == tab_id {
                    return Some(w.window_id());
                }
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.panes.borrow().is_empty()
    }

    pub fn iter_panes(&self) -> Vec<Rc<dyn Pane>> {
        self.panes
            .borrow()
            .iter()
            .map(|(_, v)| Rc::clone(v))
            .collect()
    }

    pub fn iter_windows(&self) -> Vec<WindowId> {
        self.windows.borrow().keys().cloned().collect()
    }

    pub fn iter_domains(&self) -> Vec<Arc<dyn Domain>> {
        self.domains.borrow().values().cloned().collect()
    }

    pub fn resolve_pane_id(&self, pane_id: PaneId) -> Option<(DomainId, WindowId, TabId)> {
        let mut ids = None;
        for tab in self.tabs.borrow().values() {
            for p in tab.iter_panes() {
                if p.pane.pane_id() == pane_id {
                    ids = Some((tab.tab_id(), p.pane.domain_id()));
                    break;
                }
            }
        }
        let (tab_id, domain_id) = ids?;
        let window_id = self.window_containing_tab(tab_id)?;
        Some((domain_id, window_id, tab_id))
    }

    pub fn domain_was_detached(&self, domain: DomainId) {
        self.panes
            .borrow_mut()
            .retain(|_pane_id, pane| pane.domain_id() != domain);
        // Ideally we'd do this here, but that seems to cause problems
        // at the moment:
        // self.prune_dead_windows();
    }
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SessionTerminated {
    #[error("Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[error("Error: {:?}", err)]
    Error { err: Error },
    #[error("Window Closed")]
    WindowClosed,
}
