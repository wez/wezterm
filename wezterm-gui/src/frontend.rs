use crate::TermWindow;
use ::window::*;
use anyhow::Error;
pub use config::FrontEndSelection;
use mux::{Mux, MuxNotification};
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_term::Alert;
use wezterm_toast_notification::*;

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
                        promise::spawn::spawn(async move {
                            if let Err(err) = TermWindow::new_window(mux_window_id).await {
                                log::error!("Failed to create window: {:#}", err);
                                let mux = Mux::get().expect("subscribe to trigger on main thread");
                                mux.kill_window(mux_window_id);
                            }
                            anyhow::Result::<()>::Ok(())
                        })
                        .detach();
                    }
                    MuxNotification::WindowWorkspaceChanged(_) => {}
                    MuxNotification::WindowRemoved(_) => {}
                    MuxNotification::PaneRemoved(_) => {}
                    MuxNotification::WindowInvalidated(_) => {}
                    MuxNotification::PaneOutput(_) => {}
                    MuxNotification::PaneAdded(_) => {}
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
                        // Handled via TermWindowNotif; NOP it here.
                    }
                    | MuxNotification::Alert {
                        pane_id: _,
                        alert: Alert::PaletteChanged | Alert::TitleMaybeChanged | Alert::SetUserVar{..},
                    } => {}
                    MuxNotification::Empty => {
                        if mux::activity::Activity::count() == 0 {
                            log::trace!("Mux is now empty, terminate gui");
                            Connection::get().unwrap().terminate_message_loop();
                        }
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
        self.connection.run_message_loop()
    }
}

thread_local! {
    static FRONT_END: RefCell<Option<Rc<GuiFrontEnd>>> = RefCell::new(None);
}

pub fn shutdown() {
    FRONT_END.with(|f| drop(f.borrow_mut().take()));
}

pub fn try_new() -> Result<Rc<GuiFrontEnd>, Error> {
    let front_end = GuiFrontEnd::try_new()?;
    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));
    Ok(front_end)
}
