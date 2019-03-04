use crate::guicommon::tabs::{Tab, TabId};
use failure::Error;
use promise::{Executor, Future};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use term::TerminalHost;
use termwiz::hyperlink::Hyperlink;

pub mod renderable;

#[derive(Default)]
pub struct Mux {
    tabs: RefCell<HashMap<TabId, Rc<Tab>>>,
}

fn read_from_tab_pty(executor: Arc<Executor>, tab_id: TabId, mut reader: Box<std::io::Read>) {
    const BUFSIZE: usize = 32 * 1024;
    let mut buf = [0; BUFSIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                eprintln!("read_pty EOF: tab_id {}", tab_id);
                Future::with_executor(Arc::clone(&executor), move || {
                    let mux = Mux::get().unwrap();
                    mux.remove_tab(tab_id);
                    Ok(())
                });
                return;
            }
            Ok(size) => {
                let data = buf[0..size].to_vec();
                Future::with_executor(Arc::clone(&executor), move || {
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
            Err(err) => {
                eprintln!("read_pty failed: tab {} {:?}", tab_id, err);
                Future::with_executor(Arc::clone(&executor), move || {
                    let mux = Mux::get().unwrap();
                    mux.remove_tab(tab_id);
                    Ok(())
                });
                return;
            }
        }
    }
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

    pub fn add_tab(&self, executor: Arc<Executor>, tab: &Rc<Tab>) -> Result<(), Error> {
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tabs.borrow().is_empty()
    }
}
