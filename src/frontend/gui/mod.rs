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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod glyphcache;
mod quad;
mod renderstate;
mod tabbar;
mod termwindow;
mod utilsprites;

pub struct GuiFrontEnd {
    connection: Rc<Connection>,
}

lazy_static::lazy_static! {
static ref USE_OPENGL: AtomicBool = AtomicBool::new(true);
}

pub fn is_opengl_enabled() -> bool {
    USE_OPENGL.load(Ordering::Acquire)
}

impl GuiFrontEnd {
    pub fn try_new_no_opengl() -> Fallible<Rc<dyn FrontEnd>> {
        USE_OPENGL.store(false, Ordering::Release);
        Self::try_new()
    }

    pub fn try_new() -> Fallible<Rc<dyn FrontEnd>> {
        let connection = Connection::init()?;
        let front_end = Rc::new(GuiFrontEnd { connection });
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

impl FrontEnd for GuiFrontEnd {
    fn executor(&self) -> Box<dyn Executor> {
        Box::new(GuiExecutor {})
    }

    fn run_forever(&self) -> Fallible<()> {
        // We run until we've run out of windows in the Mux.
        // When we're running ssh we have a transient window
        // or two during authentication and we want to de-bounce
        // our decision to quit until we're sure that we have
        // no windows, so we track it here.
        struct State {
            when: Option<Instant>,
        }

        impl State {
            fn mark(&mut self, is_empty: bool) {
                if is_empty {
                    let now = Instant::now();
                    if let Some(start) = self.when.as_ref() {
                        let diff = now - *start;
                        if diff > Duration::new(5, 0) {
                            Connection::get().unwrap().terminate_message_loop();
                        }
                    } else {
                        self.when = Some(now);
                    }
                } else {
                    self.when = None;
                }
            }
        }

        let state = Arc::new(Mutex::new(State { when: None }));

        self.connection
            .schedule_timer(std::time::Duration::from_millis(200), move || {
                let mux = Mux::get().unwrap();
                mux.prune_dead_windows();
                state.lock().unwrap().mark(mux.is_empty());
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
