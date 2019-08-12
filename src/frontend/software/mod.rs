use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use failure::Fallible;
use promise::{BasicExecutor, Executor, SpawnFunc};
use std::rc::Rc;
use std::sync::Arc;
use window::Connection;

mod termwindow;

pub struct SoftwareFrontEnd {
    connection: Rc<Connection>,
}

impl SoftwareFrontEnd {
    pub fn try_new(mux: &Rc<Mux>) -> Fallible<Rc<dyn FrontEnd>> {
        let connection = Connection::init()?;
        let front_end = Rc::new(SoftwareFrontEnd { connection });
        Ok(front_end)
    }
}

struct GuiExecutor {}
impl BasicExecutor for GuiExecutor {
    fn execute(&self, f: SpawnFunc) {
        Connection::executor().execute(f)
    }
}

impl Executor for GuiExecutor {
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(GuiExecutor {})
    }
}

impl FrontEnd for SoftwareFrontEnd {
    fn gui_executor(&self) -> Box<dyn Executor> {
        Box::new(GuiExecutor {})
    }

    fn run_forever(&self) -> Fallible<()> {
        self.connection.run_message_loop()
    }

    fn spawn_new_window(
        &self,
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        window_id: MuxWindowId,
    ) -> Fallible<()> {
        termwindow::TermWindow::new_window(config, fontconfig, tab, window_id)
    }
}
