//! Implements the multiplexer server frontend
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::Mux;
use failure::Error;
use promise::Executor;
use promise::SpawnFunc;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;

#[derive(Clone)]
struct MuxExecutor {
    tx: SyncSender<SpawnFunc>,
}

impl Executor for MuxExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("GlutinExecutor execute failed");
    }
    fn clone_executor(&self) -> Box<Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }
}

pub struct MuxServerFrontEnd {
    tx: SyncSender<SpawnFunc>,
    rx: Receiver<SpawnFunc>,
}

impl MuxServerFrontEnd {
    pub fn try_new(_mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        let (tx, rx) = mpsc::sync_channel(4);
        Ok(Rc::new(Self { tx, rx }))
    }
}

impl FrontEnd for MuxServerFrontEnd {
    fn gui_executor(&self) -> Box<Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }

    fn run_forever(&self) -> Result<(), Error> {
        loop {
            match self.rx.recv() {
                Ok(func) => func.call(),
                Err(err) => bail!("while waiting for events: {:?}", err),
            }
        }
    }

    fn spawn_new_window(
        &self,
        _config: &Arc<Config>,
        _fontconfig: &Rc<FontConfiguration>,
        _tab: &Rc<Tab>,
    ) -> Result<(), Error> {
        unimplemented!();
    }
}
