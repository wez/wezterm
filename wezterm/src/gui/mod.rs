use ::window::*;
use anyhow::Error;
pub use config::FrontEndSelection;
use mux::{Mux, MuxNotification};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

mod glyphcache;
mod overlay;
mod quad;
mod renderstate;
mod scrollbar;
mod selection;
mod tabbar;
mod termwindow;
mod utilsprites;

pub use selection::SelectionMode;
pub use termwindow::TermWindow;

pub struct GuiFrontEnd {
    connection: Rc<Connection>,
}

impl Drop for GuiFrontEnd {
    fn drop(&mut self) {
        ::window::shutdown();
    }
}

static USE_OPENGL: AtomicBool = AtomicBool::new(true);

pub fn is_opengl_enabled() -> bool {
    USE_OPENGL.load(Ordering::Acquire)
}

impl GuiFrontEnd {
    pub fn try_new_no_opengl() -> anyhow::Result<Rc<GuiFrontEnd>> {
        USE_OPENGL.store(false, Ordering::Release);
        Self::try_new()
    }

    pub fn try_new_swrast() -> anyhow::Result<Rc<GuiFrontEnd>> {
        ::window::prefer_swrast();
        Self::try_new()
    }

    pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
        let config = config::configuration();

        prefer_egl(config.prefer_egl);

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if !config.enable_wayland {
                Connection::disable_wayland();
            }
        }
        #[cfg(windows)]
        {
            if is_running_in_rdp_session() {
                // Using OpenGL in RDP has problematic behavior upon
                // disconnect, so we force the use of software rendering.
                log::trace!("Running in an RDP session, use SWRAST");
                prefer_swrast();
            }
        }

        let connection = Connection::init()?;
        let front_end = Rc::new(GuiFrontEnd { connection });
        let mux = Mux::get().unwrap();
        let fe = Rc::downgrade(&front_end);
        mux.subscribe(move |n| {
            if let Some(_fe) = fe.upgrade() {
                match n {
                    MuxNotification::WindowCreated(mux_window_id) => {
                        termwindow::TermWindow::new_window(mux_window_id).ok();
                    }
                    MuxNotification::PaneOutput(_) => {}
                }
                true
            } else {
                false
            }
        });
        Ok(front_end)
    }

    pub fn run_forever(&self) -> anyhow::Result<()> {
        self.connection
            .schedule_timer(std::time::Duration::from_millis(200), move || {
                if mux::activity::Activity::count() == 0 {
                    let mux = Mux::get().unwrap();
                    mux.prune_dead_windows();
                    if mux.is_empty() {
                        Connection::get().unwrap().terminate_message_loop();
                    }
                }
            });

        self.connection.run_message_loop()
    }
}

thread_local! {
    static FRONT_END: RefCell<Option<Rc<GuiFrontEnd>>> = RefCell::new(None);
}

pub fn front_end() -> Option<Rc<GuiFrontEnd>> {
    let mut res = None;
    FRONT_END.with(|f| {
        if let Some(me) = &*f.borrow() {
            res = Some(Rc::clone(me));
        }
    });
    res
}

pub fn shutdown() {
    FRONT_END.with(|f| drop(f.borrow_mut().take()));
}

pub fn try_new(sel: FrontEndSelection) -> Result<Rc<GuiFrontEnd>, Error> {
    let front_end = match sel {
        FrontEndSelection::Software => GuiFrontEnd::try_new_swrast(),
        FrontEndSelection::OldSoftware => GuiFrontEnd::try_new_no_opengl(),
        FrontEndSelection::OpenGL => GuiFrontEnd::try_new(),
    };

    let front_end = front_end?;

    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));

    Ok(front_end)
}
