use ::window::*;
use anyhow::Error;
pub use config::FrontEndSelection;
use mux::{Mux, MuxNotification};
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_term::Alert;
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
    pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
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
                    MuxNotification::Alert {
                        pane_id: _,
                        alert:
                            Alert::ToastNotification {
                                title,
                                body,
                                focus: _,
                            },
                    } => {
                        let message = if title.is_none() { "" } else { &body };
                        let title = title.as_ref().unwrap_or(&body);
                        // FIXME: if notification.focus is true, we should do
                        // something here to arrange to focus pane_id when the
                        // notification is clicked
                        persistent_toast_notification(title, message);
                    }
                    MuxNotification::Alert {
                        pane_id: _,
                        alert: Alert::Bell,
                    } => {
                        // persistent_toast_notification("Ding!", "This is the bell");
                        log::info!("Ding! (this is the bell)");
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

pub fn try_new() -> Result<Rc<GuiFrontEnd>, Error> {
    let front_end = GuiFrontEnd::try_new()?;
    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));
    Ok(front_end)
}
