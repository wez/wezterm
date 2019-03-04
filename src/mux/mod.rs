use crate::futurecore::Spawner;
use crate::guicommon::tabs::{Tab, TabId};
use failure::Error;
use promise::Executor;
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
    spawner: RefCell<Option<Spawner>>,
}

#[derive(Clone)]
pub enum PtyEvent {
    Data { tab_id: TabId, data: Vec<u8> },
    Terminated { tab_id: TabId },
}

pub trait PtyEventSender: Send {
    fn send(&self, event: PtyEvent) -> Result<(), Error>;
}

fn read_from_tab_pty(
    executor: Arc<Executor>,
    spawner: Spawner,
    sender: Box<PtyEventSender>,
    tab_id: TabId,
    mut reader: Box<std::io::Read>,
) {
    const BUFSIZE: usize = 32 * 1024;
    let mut buf = [0; BUFSIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                eprintln!("read_pty EOF: tab_id {}", tab_id);
                sender.send(PtyEvent::Terminated { tab_id }).ok();
                return;
            }
            Ok(size) => {
                spawner.spawn(Box::new(futures::future::lazy(|| {
                    eprintln!("I was spawned from a pty thread");
                    futures::future::ok(())
                })));
                if sender
                    .send(PtyEvent::Data {
                        tab_id,
                        data: buf[0..size].to_vec(),
                    })
                    .is_err()
                {
                    return;
                }
            }
            Err(err) => {
                eprintln!("read_pty failed: tab {} {:?}", tab_id, err);
                sender.send(PtyEvent::Terminated { tab_id }).ok();
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

impl Mux {
    pub fn set_spawner(&self, spawner: Spawner) {
        *self.spawner.borrow_mut() = Some(spawner);
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<Rc<Tab>> {
        self.tabs.borrow().get(&tab_id).map(Rc::clone)
    }

    pub fn add_tab(
        &self,
        executor: Arc<Executor + Send + Sync>,
        sender: Box<PtyEventSender>,
        tab: &Rc<Tab>,
    ) -> Result<(), Error> {
        self.tabs.borrow_mut().insert(tab.tab_id(), Rc::clone(tab));

        let reader = tab.reader()?;
        let tab_id = tab.tab_id();
        let spawner = self.spawner.borrow().as_ref().unwrap().clone();
        thread::spawn(move || read_from_tab_pty(executor, spawner, sender, tab_id, reader));

        Ok(())
    }

    pub fn remove_tab(&self, tab_id: TabId) {
        eprintln!("removing tab {}", tab_id);
        self.tabs.borrow_mut().remove(&tab_id);
    }

    pub fn process_pty_event(&self, event: PtyEvent) -> Result<(), Error> {
        match event {
            PtyEvent::Data { tab_id, data } => {
                if let Some(tab) = self.get_tab(tab_id) {
                    tab.advance_bytes(
                        &data,
                        &mut Host {
                            writer: &mut *tab.writer(),
                        },
                    );
                }
            }
            PtyEvent::Terminated { tab_id } => {
                // The fact that we woke up is enough to trigger each
                // window to check for termination
                eprintln!("tab {} terminated", tab_id);
                self.remove_tab(tab_id);
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tabs.borrow().is_empty()
    }
}
