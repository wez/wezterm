//! Implements the multiplexer server frontend
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::server::listener::spawn_listener;
use anyhow::{bail, Error};
use async_task::JoinHandle;
use crossbeam_channel::{unbounded as channel, Receiver, Sender};
use log::info;
use promise::*;
use std::rc::Rc;

#[derive(Clone)]
struct MuxExecutor {
    tx: Sender<SpawnFunc>,
}

impl BasicExecutor for MuxExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("MuxExecutor execute failed");
    }
}

impl Executor for MuxExecutor {
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }
}

pub struct MuxServerFrontEnd {
    tx: Sender<SpawnFunc>,
    rx: Receiver<SpawnFunc>,
}

impl MuxServerFrontEnd {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    fn new(start_listener: bool) -> Result<Rc<dyn FrontEnd>, Error> {
        let (tx, rx) = channel();

        if start_listener {
            spawn_listener()?;
        }
        Ok(Rc::new(Self { tx, rx }))
    }

    pub fn try_new() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(true)
    }

    pub fn new_null() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(false)
    }

    pub fn spawn_task<F: std::future::Future<Output = ()> + 'static>(
        &self,
        future: F,
    ) -> JoinHandle<(), ()> {
        let tx = self.tx.clone();
        let (task, handle) = async_task::spawn_local(
            future,
            move |task| tx.send(Box::new(move || task.run())).unwrap(),
            (),
        );
        task.schedule();
        handle
    }
}

impl FrontEnd for MuxServerFrontEnd {
    fn executor(&self) -> Box<dyn Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }

    fn low_pri_executor(&self) -> Box<dyn Executor> {
        self.executor()
    }

    fn run_forever(&self) -> Result<(), Error> {
        loop {
            match self.rx.recv() {
                Ok(func) => func(),
                Err(err) => bail!("while waiting for events: {:?}", err),
            }

            if Mux::get().unwrap().is_empty() && crate::frontend::activity::Activity::count() == 0 {
                info!("No more tabs; all done!");
                return Ok(());
            }
        }
    }

    fn spawn_new_window(
        &self,
        _fontconfig: &Rc<FontConfiguration>,
        _tab: &Rc<dyn Tab>,
        _window_id: WindowId,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
