use crate::config::Config;
use crate::ExitStatus;
use failure::Error;
use promise::{Executor, Future};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use term::TerminalHost;
use termwiz::hyperlink::Hyperlink;

pub mod renderable;
pub mod tab;
pub mod window;

use crate::mux::tab::{Tab, TabId};
use crate::mux::window::{Window, WindowId};

pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<Tab>>>,
    windows: RefCell<HashMap<WindowId, Window>>,
    config: Arc<Config>,
}

fn read_from_tab_pty(executor: Box<Executor>, tab_id: TabId, mut reader: Box<std::io::Read>) {
    const BUFSIZE: usize = 32 * 1024;
    let mut buf = [0; BUFSIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                eprintln!("read_pty EOF: tab_id {}", tab_id);
                break;
            }
            Err(err) => {
                eprintln!("read_pty failed: tab {} {:?}", tab_id, err);
                break;
            }
            Ok(size) => {
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
    writer: &'a mut std::io::Write,
}

impl<'a> TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut std::io::Write {
        &mut self.writer
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        match open::that(link.uri()) {
            Ok(_) => {}
            Err(err) => eprintln!("failed to open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        eprintln!("peer requested clipboard; ignoring");
        Ok("".into())
    }

    fn set_clipboard(&mut self, _clip: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    fn set_title(&mut self, _title: &str) {}
}

thread_local! {
    static MUX: RefCell<Option<Rc<Mux>>> = RefCell::new(None);
}

impl Mux {
    pub fn new(config: &Arc<Config>) -> Self {
        Self {
            tabs: RefCell::new(HashMap::new()),
            windows: RefCell::new(HashMap::new()),
            config: Arc::clone(config),
        }
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

    pub fn get_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        self.tabs.borrow().get(&tab_id).map(Rc::clone)
    }

    pub fn add_tab(&self, executor: Box<Executor>, tab: &Rc<Tab>) -> Result<(), Error> {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));

        let reader = tab.reader()?;
        let tab_id = tab.tab_id();
        thread::spawn(move || read_from_tab_pty(executor, tab_id, reader));

        Ok(())
    }

    pub fn remove_tab(&self, tab_id: TabId) {
        eprintln!("removing tab {}", tab_id);
        self.tabs.borrow_mut().remove(&tab_id);
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

    pub fn add_new_window_with_tab(&self, tab: &Rc<Tab>) -> Result<WindowId, Error> {
        let window = Window::new(tab);
        let window_id = window.window_id();
        self.windows.borrow_mut().insert(window_id, window);
        Ok(window_id)
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tabs.borrow().is_empty()
    }

    pub fn iter_tabs(&self) -> Vec<Rc<Tab>> {
        self.tabs
            .borrow()
            .iter()
            .map(|(_, v)| Rc::clone(v))
            .collect()
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
