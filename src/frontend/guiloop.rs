use crate::config::Config;
use crate::font::FontConfiguration;
use crate::mux::tab::Tab;
use failure::Error;
use promise::Executor;
use std::rc::Rc;
use std::sync::Arc;

pub trait GuiSystem {
    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    fn run_forever(&self) -> Result<(), Error>;

    fn spawn_new_window(
        &self,
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<Tab>,
    ) -> Result<(), Error>;

    fn gui_executor(&self) -> Box<Executor>;
}
