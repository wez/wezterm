use crate::config::configuration;
use crate::font::FontConfiguration;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::*;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

mod glyphcache;
mod quad;
mod renderstate;
mod scrollbar;
mod selection;
mod tabbar;
mod termwindow;
mod utilsprites;

pub struct GuiFrontEnd {
    connection: Rc<Connection>,
}

static USE_OPENGL: AtomicBool = AtomicBool::new(true);

pub fn is_opengl_enabled() -> bool {
    USE_OPENGL.load(Ordering::Acquire)
}

impl GuiFrontEnd {
    pub fn try_new_no_opengl() -> anyhow::Result<Rc<dyn FrontEnd>> {
        USE_OPENGL.store(false, Ordering::Release);
        Self::try_new()
    }

    pub fn try_new() -> anyhow::Result<Rc<dyn FrontEnd>> {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if !configuration().enable_wayland {
                Connection::disable_wayland();
            }
        }
        let connection = Connection::init()?;
        let front_end = Rc::new(GuiFrontEnd { connection });
        Ok(front_end)
    }
}

impl FrontEnd for GuiFrontEnd {
    fn run_forever(&self) -> anyhow::Result<()> {
        self.connection
            .schedule_timer(std::time::Duration::from_millis(200), move || {
                if crate::frontend::activity::Activity::count() == 0 {
                    let mux = Mux::get().unwrap();
                    mux.prune_dead_windows();
                    if mux.is_empty() {
                        Connection::get().unwrap().terminate_message_loop();
                    }
                }
            });

        self.connection.run_message_loop()
    }

    fn spawn_new_window(
        &self,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        window_id: MuxWindowId,
    ) -> anyhow::Result<()> {
        termwindow::TermWindow::new_window(&configuration(), fontconfig, tab, window_id)
    }
}
