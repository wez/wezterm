//! Implements the multiplexer server frontend
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::Mux;
use crate::server::listener::spawn_listener;
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
    fn new(mux: &Rc<Mux>, start_listener: bool) -> Result<Rc<FrontEnd>, Error> {
        let (tx, rx) = mpsc::sync_channel(4);

        if start_listener {
            spawn_listener(mux.config(), Box::new(MuxExecutor { tx: tx.clone() }))?;
        }
        Ok(Rc::new(Self { tx, rx }))
    }

    pub fn try_new(mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        Self::new(mux, true)
    }

    pub fn new_null(mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        Self::new(mux, false)
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

            if Mux::get().unwrap().is_empty() {
                eprintln!("No more tabs; all done!");
                return Ok(());
            }
        }
    }

    fn spawn_new_window(
        &self,
        _config: &Arc<Config>,
        _fontconfig: &Rc<FontConfiguration>,
        _tab: &Rc<Tab>,
    ) -> Result<(), Error> {
        // The tab was already added to the mux, so we are a NOP
        Ok(())
    }
}
