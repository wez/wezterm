//! Implements the multiplexer server frontend
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::Mux;
use failure::Error;
use promise::Executor;
use std::rc::Rc;
use std::sync::Arc;

pub struct MuxServerFrontEnd {}

impl MuxServerFrontEnd {
    pub fn try_new(_mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        Ok(Rc::new(Self {}))
    }
}

impl FrontEnd for MuxServerFrontEnd {
    fn gui_executor(&self) -> Box<Executor> {
        unimplemented!();
    }

    fn run_forever(&self) -> Result<(), Error> {
        unimplemented!();
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
