use crate::config::Config;
use crate::frontend::gui_executor;
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::{Window, WindowId};
use crate::ratelim::RateLimiter;
use crate::server::pollable::{pollable_channel, PollableReceiver, PollableSender};
use domain::{Domain, DomainId};
use failure::{bail, format_err, Error, Fallible};
use failure_derive::*;
use log::{debug, error};
use portable_pty::ExitStatus;
use promise::{Executor, Future};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use term::terminal::Clipboard;
use term::TerminalHost;
use termwiz::hyperlink::Hyperlink;

pub mod domain;
pub mod renderable;
pub mod tab;
pub mod window;

#[derive(Clone, Debug)]
pub enum MuxNotification {
    TabOutput(TabId),
}

static SUB_ID: AtomicUsize = AtomicUsize::new(0);

pub type MuxSubscriber = PollableReceiver<MuxNotification>;

pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<dyn Tab>>>,
    windows: RefCell<HashMap<WindowId, Window>>,
    config: Arc<Config>,
    default_domain: Arc<dyn Domain>,
    domains: RefCell<HashMap<DomainId, Arc<dyn Domain>>>,
    domains_by_name: RefCell<HashMap<String, Arc<dyn Domain>>>,
    subscribers: RefCell<HashMap<usize, PollableSender<MuxNotification>>>,
}

fn read_from_tab_pty(config: Arc<Config>, tab_id: TabId, mut reader: Box<dyn std::io::Read>) {
    let executor = gui_executor().expect("gui_executor was not registered yet!?");
    const BUFSIZE: usize = 32 * 1024;
    let mut buf = [0; BUFSIZE];

    let mut lim = RateLimiter::new(
        config
            .ratelimit_output_bytes_per_second
            .unwrap_or(2 * 1024 * 1024),
    );

    loop {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                error!("read_pty EOF: tab_id {}", tab_id);
                break;
            }
            Err(err) => {
                error!("read_pty failed: tab {} {:?}", tab_id, err);
                break;
            }
            Ok(size) => {
                lim.blocking_admittance_check(size as u32);
                let data = buf[0..size].to_vec();
                Future::with_executor(executor.clone_executor(), move || {
                    let mux = Mux::get().unwrap();
                    if let Some(tab) = mux.get_tab(tab_id) {
                        tab.advance_bytes(
                            &data,
                            &mut Host {
                                writer: &mut *tab.writer(),
                            },
                        );
                        mux.notify(MuxNotification::TabOutput(tab_id));
                    }
                    Ok(())
                });
            }
        }
    }
    Future::with_executor(executor.clone_executor(), move || {
        let mux = Mux::get().unwrap();
        mux.remove_tab(tab_id);
        Ok(())
    });
}

/// This is just a stub impl of TerminalHost; it really only exists
/// in order to parse data sent by the peer (so, just to parse output).
/// As such it only really has Host::writer get called.
/// The GUI driven flows provide their own impl of TerminalHost.
struct Host<'a> {
    writer: &'a mut dyn std::io::Write,
}

impl<'a> TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        &mut self.writer
    }

    fn click_link(&mut self, link: &Arc<Hyperlink>) {
        match open::that(link.uri()) {
            Ok(_) => {}
            Err(err) => error!("failed to open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Fallible<Arc<dyn Clipboard>> {
        bail!("peer requested clipboard; ignoring");
    }

    fn set_title(&mut self, _title: &str) {}
}

thread_local! {
    static MUX: RefCell<Option<Rc<Mux>>> = RefCell::new(None);
}

impl Mux {
    pub fn new(config: &Arc<Config>, default_domain: &Arc<dyn Domain>) -> Self {
        let mut domains = HashMap::new();
        domains.insert(default_domain.domain_id(), Arc::clone(default_domain));

        let mut domains_by_name = HashMap::new();
        domains_by_name.insert(
            default_domain.domain_name().to_string(),
            Arc::clone(default_domain),
        );

        Self {
            tabs: RefCell::new(HashMap::new()),
            windows: RefCell::new(HashMap::new()),
            config: Arc::clone(config),
            default_domain: Arc::clone(default_domain),
            domains_by_name: RefCell::new(domains_by_name),
            domains: RefCell::new(domains),
            subscribers: RefCell::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self) -> Fallible<MuxSubscriber> {
        let sub_id = SUB_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = pollable_channel()?;
        self.subscribers.borrow_mut().insert(sub_id, tx);
        Ok(rx)
    }

    pub fn notify(&self, notification: MuxNotification) {
        let mut subscribers = self.subscribers.borrow_mut();
        subscribers.retain(|_, tx| tx.send(notification.clone()).is_ok());
    }

    pub fn default_domain(&self) -> &Arc<dyn Domain> {
        &self.default_domain
    }

    pub fn get_domain(&self, id: DomainId) -> Option<Arc<dyn Domain>> {
        self.domains.borrow().get(&id).cloned()
    }

    pub fn get_domain_by_name(&self, name: &str) -> Option<Arc<dyn Domain>> {
        self.domains_by_name.borrow().get(name).cloned()
    }

    pub fn add_domain(&self, domain: &Arc<dyn Domain>) {
        self.domains
            .borrow_mut()
            .insert(domain.domain_id(), Arc::clone(domain));
        self.domains_by_name
            .borrow_mut()
            .insert(domain.domain_name().to_string(), Arc::clone(domain));
    }

    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }

    pub fn set_mux(mux: &Rc<Mux>) {
        MUX.with(|m| {
            *m.borrow_mut() = Some(Rc::clone(mux));
        });
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

    pub fn get_tab(&self, tab_id: TabId) -> Option<Rc<dyn Tab>> {
        self.tabs.borrow().get(&tab_id).map(Rc::clone)
    }

    pub fn add_tab(&self, tab: &Rc<dyn Tab>) -> Result<(), Error> {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));

        let reader = tab.reader()?;
        let tab_id = tab.tab_id();
        let config = Arc::clone(&self.config);
        thread::spawn(move || read_from_tab_pty(config, tab_id, reader));

        Ok(())
    }

    pub fn remove_tab(&self, tab_id: TabId) {
        debug!("removing tab {}", tab_id);
        self.tabs.borrow_mut().remove(&tab_id);
        self.prune_dead_windows();
    }

    pub fn prune_dead_windows(&self) {
        let live_tab_ids: Vec<TabId> = self.tabs.borrow().keys().cloned().collect();
        let mut windows = self.windows.borrow_mut();
        let mut dead_windows = vec![];
        for (window_id, win) in windows.iter_mut() {
            win.prune_dead_tabs(&live_tab_ids);
            if win.is_empty() {
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
            self.tabs.borrow_mut().remove(&tab_id);
        }

        for window_id in dead_windows {
            error!("removing window {}", window_id);
            windows.remove(&window_id);
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

    pub fn get_active_tab_for_window(&self, window_id: WindowId) -> Option<Rc<dyn Tab>> {
        let window = self.get_window(window_id)?;
        window.get_active().map(Rc::clone)
    }

    pub fn new_empty_window(&self) -> WindowId {
        let window = Window::new();
        let window_id = window.window_id();
        self.windows.borrow_mut().insert(window_id, window);
        window_id
    }

    pub fn add_tab_to_window(&self, tab: &Rc<dyn Tab>, window_id: WindowId) -> Fallible<()> {
        let mut window = self
            .get_window_mut(window_id)
            .ok_or_else(|| format_err!("add_tab_to_window: no such window_id {}", window_id))?;
        window.push(tab);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.borrow().is_empty()
    }

    pub fn iter_tabs(&self) -> Vec<Rc<dyn Tab>> {
        self.tabs
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

    pub fn domain_was_detached(&self, domain: DomainId) {
        self.tabs
            .borrow_mut()
            .retain(|_tab_id, tab| tab.domain_id() != domain);
        // Ideally we'd do this here, but that seems to cause problems
        // at the moment:
        // self.prune_dead_windows();
    }
}

#[derive(Debug, Fail)]
#[allow(dead_code)]
pub enum SessionTerminated {
    #[fail(display = "Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[fail(display = "Error: {:?}", err)]
    Error { err: Error },
    #[fail(display = "Window Closed")]
    WindowClosed,
}
