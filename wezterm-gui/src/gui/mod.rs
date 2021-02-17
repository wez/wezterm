use ::window::*;
use anyhow::Error;
pub use config::FrontEndSelection;
use mux::{Mux, MuxNotification};
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_toast_notification::*;

mod glyphcache;
mod overlay;
mod quad;
mod renderstate;
mod scrollbar;
mod selection;
mod shapecache;
mod tabbar;
mod termwindow;
mod utilsprites;

pub use selection::SelectionMode;
pub use termwindow::set_window_class;
pub use termwindow::TermWindow;
pub use termwindow::ICON_DATA;

pub struct GuiFrontEnd {
    connection: Rc<Connection>,
}

impl Drop for GuiFrontEnd {
    fn drop(&mut self) {
        ::window::shutdown();
    }
}

impl GuiFrontEnd {
    pub fn try_new_swrast() -> anyhow::Result<Rc<GuiFrontEnd>> {
        ::window::prefer_swrast();
        Self::try_new()
    }

    pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
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
        let mux = Mux::get().expect("mux started and running on main thread");
        let fe = Rc::downgrade(&front_end);
        mux.subscribe(move |n| {
            if let Some(_fe) = fe.upgrade() {
                match n {
                    MuxNotification::WindowCreated(mux_window_id) => {
                        if let Err(err) = termwindow::TermWindow::new_window(mux_window_id) {
                            log::error!("Failed to create window: {:#}", err);
                            let mux = Mux::get().expect("subscribe to trigger on main thread");
                            mux.kill_window(mux_window_id);
                        }
                    }
                    MuxNotification::PaneOutput(_) => {}
                    MuxNotification::ToastNotification {
                        pane_id: _,
                        notification,
                    } => {
                        let title = notification.title.as_ref().unwrap_or(&notification.body);
                        let message = if notification.title.is_none() {
                            ""
                        } else {
                            &notification.body
                        };
                        // FIXME: if notification.focus is true, we should do
                        // something here to arrange to focus pane_id when the
                        // notification is clicked
                        persistent_toast_notification(title, message);
                    }
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
        FrontEndSelection::OpenGL => GuiFrontEnd::try_new(),
    };

    let front_end = front_end?;

    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));

    Ok(front_end)
}
