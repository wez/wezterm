use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::*;
use failure::Fallible;
use promise::{BasicExecutor, Executor, SpawnFunc};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod glyphcache;
mod termwindow;

pub struct SoftwareFrontEnd {
    connection: Rc<Connection>,
}

lazy_static::lazy_static! {
static ref USE_OPENGL: AtomicBool = AtomicBool::new(true);
}

pub fn is_opengl_enabled() -> bool {
    USE_OPENGL.load(Ordering::Acquire)
}

impl SoftwareFrontEnd {
    pub fn try_new_no_opengl(mux: &Rc<Mux>) -> Fallible<Rc<dyn FrontEnd>> {
        USE_OPENGL.store(false, Ordering::Release);
        Self::try_new(mux)
    }

    pub fn try_new(_mux: &Rc<Mux>) -> Fallible<Rc<dyn FrontEnd>> {
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
        self.connection
            .schedule_timer(std::time::Duration::from_millis(200), move || {
                let mux = Mux::get().unwrap();
                mux.prune_dead_windows();
                if mux.is_empty() {
                    Connection::get().unwrap().terminate_message_loop();
                }
            });

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
