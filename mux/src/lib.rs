use crate::pane::{Pane, PaneId};
use crate::tab::{Tab, TabId};
use crate::window::{Window, WindowId};
use anyhow::{anyhow, Error};
use config::{configuration, ExitBehavior};
use domain::{Domain, DomainId};
use log::error;
use portable_pty::ExitStatus;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Read;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use std::thread;
use termwiz::escape::Action;
use thiserror::*;

pub mod activity;
pub mod connui;
pub mod domain;
pub mod localpane;
pub mod pane;
pub mod renderable;
pub mod ssh;
pub mod tab;
pub mod termwiztermtab;
pub mod tmux;
pub mod window;

use crate::activity::Activity;

#[derive(Clone, Debug)]
pub enum MuxNotification {
    PaneOutput(PaneId),
    WindowCreated(WindowId),
    WindowInvalidated(WindowId),
    Alert {
        pane_id: PaneId,
        alert: wezterm_term::Alert,
    },
    Empty,
}

static SUB_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<Tab>>>,
    panes: RefCell<HashMap<PaneId, Rc<dyn Pane>>>,
    windows: RefCell<HashMap<WindowId, Window>>,
    default_domain: RefCell<Option<Arc<dyn Domain>>>,
    domains: RefCell<HashMap<DomainId, Arc<dyn Domain>>>,
    domains_by_name: RefCell<HashMap<String, Arc<dyn Domain>>>,
    subscribers: RefCell<HashMap<usize, Box<dyn Fn(MuxNotification) -> bool>>>,
    banner: RefCell<Option<String>>,
}

/// This function bounces parsed actions over to the main thread to feed to
/// the pty in the mux.
/// It blocks until the mux has finished consuming the data, which provides
/// some back-pressure so that eg: ctrl-c can remain responsive.
fn send_actions_to_mux(pane_id: PaneId, dead: &Arc<AtomicBool>, actions: Vec<Action>) {
    promise::spawn::block_on(promise::spawn::spawn_into_main_thread({
        let dead = Arc::clone(&dead);
        async move {
            let mux = Mux::get().unwrap();
            if let Some(pane) = mux.get_pane(pane_id) {
                pane.perform_actions(actions);
                mux.notify(MuxNotification::PaneOutput(pane_id));
            } else {
                // Something else removed the pane from
                // the mux, so signal that we should stop
                // trying to process it in read_from_pane_pty.
                dead.store(true, Ordering::Relaxed);
            }
        }
    }));
}

struct BufState {
    queue: Mutex<VecDeque<u8>>,
    cond: Condvar,
    dead: Arc<AtomicBool>,
}

impl BufState {
    fn write(&self, buf: &[u8]) {
        let mut queue = self.queue.lock().unwrap();
        queue.extend(buf);
        self.cond.notify_one();
    }
}

fn parse_buffered_data(pane_id: PaneId, state: &Arc<BufState>) {
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut queue = state.queue.lock().unwrap();

    loop {
        if queue.is_empty() {
            if state.dead.load(Ordering::Relaxed) {
                return;
            }
            queue = state.cond.wait(queue).unwrap();
            continue;
        }

        let mut actions = vec![];
        let buf = queue.make_contiguous();
        parser.parse(buf, |action| actions.push(action));
        queue.truncate(0);

        // Yield briefly to see if more data showed up and
        // lump it together with what we've got
        loop {
            let wait_res = state
                .cond
                .wait_timeout(queue, Duration::from_millis(1))
                .unwrap();
            queue = wait_res.0;
            if queue.is_empty() {
                break;
            }
            let buf = queue.make_contiguous();
            parser.parse(buf, |action| actions.push(action));
            queue.truncate(0);
            if !actions.is_empty() {
                // Don't delay very long if we've got stuff to display!
                break;
            }
        }

        if !actions.is_empty() {
            send_actions_to_mux(pane_id, &state.dead, actions);
        }
    }
}

/// This function is run in a separate thread; its purpose is to perform
/// blocking reads from the pty (non-blocking reads are not portable to
/// all platforms and pty/tty types), parse the escape sequences and
/// relay the actions to the mux thread to apply them to the pane.
fn read_from_pane_pty(pane_id: PaneId, banner: Option<String>, mut reader: Box<dyn std::io::Read>) {
    const BUFSIZE: usize = 4 * 1024;
    let mut buf = [0; BUFSIZE];

    // This is used to signal that an error occurred either in this thread,
    // or in the main mux thread.  If `true`, this thread will terminate.
    let dead = Arc::new(AtomicBool::new(false));

    let state = Arc::new(BufState {
        queue: Mutex::new(VecDeque::new()),
        cond: Condvar::new(),
        dead: Arc::clone(&dead),
    });

    std::thread::spawn({
        let state = Arc::clone(&state);
        move || parse_buffered_data(pane_id, &state)
    });

    if let Some(banner) = banner {
        state.write(banner.as_bytes());
    }

    while !dead.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                log::trace!("read_pty EOF: pane_id {}", pane_id);
                break;
            }
            Err(err) => {
                error!("read_pty failed: pane {} {:?}", pane_id, err);
                break;
            }
            Ok(size) => {
                state.write(&buf[..size]);
            }
        }
    }

    match configuration().exit_behavior {
        ExitBehavior::Hold | ExitBehavior::CloseOnCleanExit => {
            // We don't know if we can unilaterally close
            // this pane right now, so don't!
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                log::trace!("checking for dead windows after EOF on pane {}", pane_id);
                mux.prune_dead_windows();
            })
            .detach();
        }
        ExitBehavior::Close => {
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                mux.remove_pane(pane_id);
            })
            .detach();
        }
    }

    dead.store(true, Ordering::Relaxed);
}

thread_local! {
    static MUX: RefCell<Option<Rc<Mux>>> = RefCell::new(None);
}

pub struct MuxWindowBuilder {
    window_id: WindowId,
    activity: Option<Activity>,
    notified: bool,
}

impl MuxWindowBuilder {
    fn notify(&mut self) {
        if self.notified {
            return;
        }
        self.notified = true;
        let activity = self.activity.take().unwrap();
        let window_id = self.window_id;
        if let Some(mux) = Mux::get() {
            // If we're already on the mux thread, just send the notification
            // immediately.
            // This is super important for Wayland; if we push it to the
            // spawn queue below then the extra milliseconds of delay
            // causes it to get confused and shutdown the connection!?
            mux.notify(MuxNotification::WindowCreated(window_id));
        } else {
            promise::spawn::spawn_into_main_thread(async move {
                if let Some(mux) = Mux::get() {
                    mux.notify(MuxNotification::WindowCreated(window_id));
                    drop(activity);
                }
            })
            .detach();
        }
    }
}

impl Drop for MuxWindowBuilder {
    fn drop(&mut self) {
        self.notify();
    }
}

impl std::ops::Deref for MuxWindowBuilder {
    type Target = WindowId;

    fn deref(&self) -> &WindowId {
        &self.window_id
    }
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
            banner: RefCell::new(None),
        }
    }

    pub fn subscribe<F>(&self, subscriber: F)
    where
        F: Fn(MuxNotification) -> bool + 'static,
    {
        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
        self.subscribers
            .borrow_mut()
            .insert(sub_id, Box::new(subscriber));
    }

    pub fn notify(&self, notification: MuxNotification) {
        let mut subscribers = self.subscribers.borrow_mut();
        subscribers.retain(|_, notify| notify(notification.clone()));
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
        let banner = self.banner.borrow().clone();
        thread::spawn(move || read_from_pane_pty(pane_id, banner, reader));
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

    fn remove_pane_internal(&self, pane_id: PaneId) {
        log::debug!("removing pane {}", pane_id);
        if let Some(pane) = self.panes.borrow_mut().remove(&pane_id) {
            log::debug!("killing pane {}", pane_id);
            pane.kill();
        }
    }

    fn remove_tab_internal(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        log::debug!("remove_tab_internal tab {}", tab_id);

        let tab = self.tabs.borrow_mut().remove(&tab_id)?;

        let mut pane_ids = vec![];
        for pos in tab.iter_panes() {
            pane_ids.push(pos.pane.pane_id());
        }
        for pane_id in pane_ids {
            self.remove_pane_internal(pane_id);
        }

        Some(tab)
    }

    fn remove_window_internal(&self, window_id: WindowId) {
        log::debug!("remove_window_internal {}", window_id);
        let window = self.windows.borrow_mut().remove(&window_id);
        if let Some(window) = window {
            for tab in window.iter() {
                self.remove_tab_internal(tab.tab_id());
            }
        }
    }

    pub fn remove_pane(&self, pane_id: PaneId) {
        self.remove_pane_internal(pane_id);
        self.prune_dead_windows();
    }

    pub fn remove_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        let tab = self.remove_tab_internal(tab_id);
        self.prune_dead_windows();
        tab
    }

    pub fn prune_dead_windows(&self) {
        let live_tab_ids: Vec<TabId> = self.tabs.borrow().keys().cloned().collect();
        let mut dead_windows = vec![];
        let dead_tab_ids: Vec<TabId>;

        {
            let mut windows = match self.windows.try_borrow_mut() {
                Ok(w) => w,
                Err(_) => {
                    // It's ok if our caller already locked it; we can prune later.
                    return;
                }
            };
            for (window_id, win) in windows.iter_mut() {
                win.prune_dead_tabs(&live_tab_ids);
                if win.is_empty() {
                    log::debug!("prune_dead_windows: window is now empty");
                    dead_windows.push(*window_id);
                }
            }

            dead_tab_ids = self
                .tabs
                .borrow()
                .iter()
                .filter_map(|(&id, tab)| if tab.is_dead() { Some(id) } else { None })
                .collect();
        }

        for tab_id in dead_tab_ids {
            log::trace!("tab {} is dead", tab_id);
            self.remove_tab_internal(tab_id);
        }

        for window_id in dead_windows {
            log::trace!("window {} is dead", window_id);
            self.remove_window_internal(window_id);
        }

        if self.is_empty() {
            self.notify(MuxNotification::Empty);
        }
    }

    pub fn kill_window(&self, window_id: WindowId) {
        self.remove_window_internal(window_id);
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

    pub fn new_empty_window(&self) -> MuxWindowBuilder {
        let window = Window::new();
        let window_id = window.window_id();
        self.windows.borrow_mut().insert(window_id, window);
        MuxWindowBuilder {
            window_id,
            activity: Some(Activity::new()),
            notified: false,
        }
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
        let mut dead_panes = vec![];
        for pane in self.panes.borrow().values() {
            if pane.domain_id() == domain {
                dead_panes.push(pane.pane_id());
            }
        }

        {
            let mut windows = self.windows.borrow_mut();
            for (_, win) in windows.iter_mut() {
                for tab in win.iter() {
                    tab.kill_panes_in_domain(domain);
                }
            }
        }

        log::error!("domain detached panes: {:?}", dead_panes);
        for pane_id in dead_panes {
            self.remove_pane_internal(pane_id);
        }

        self.prune_dead_windows();
    }

    pub fn set_banner(&self, banner: Option<String>) {
        *self.banner.borrow_mut() = banner;
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

pub(crate) fn pty_size_to_terminal_size(size: portable_pty::PtySize) -> wezterm_term::TerminalSize {
    wezterm_term::TerminalSize {
        physical_rows: size.rows as usize,
        physical_cols: size.cols as usize,
        pixel_width: size.pixel_width as usize,
        pixel_height: size.pixel_height as usize,
    }
}
