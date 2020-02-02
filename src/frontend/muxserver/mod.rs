//! Implements the multiplexer server frontend
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::server::listener::spawn_listener;
use anyhow::{bail, Error};
use crossbeam::channel::{unbounded as channel, Receiver};
use log::info;
use promise::*;
use std::rc::Rc;

pub struct MuxServerFrontEnd {
    rx: Receiver<SpawnFunc>,
}

impl MuxServerFrontEnd {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    fn new(start_listener: bool) -> Result<Rc<dyn FrontEnd>, Error> {
        let (tx, rx) = channel();

        let tx_main = tx.clone();
        let tx_low = tx.clone();
        let queue_func = move |f: SpawnFunc| {
            tx_main.send(f).ok();
        };
        let queue_func_low = move |f: SpawnFunc| {
            tx_low.send(f).ok();
        };
        promise::spawn::set_schedulers(
            Box::new(move |task| queue_func(Box::new(move || task.run()))),
            Box::new(move |task| queue_func_low(Box::new(move || task.run()))),
        );

        if start_listener {
            spawn_listener()?;
        }
        Ok(Rc::new(Self { rx }))
    }

    pub fn try_new() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(true)
    }

    pub fn new_null() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(false)
    }
}

impl FrontEnd for MuxServerFrontEnd {
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
