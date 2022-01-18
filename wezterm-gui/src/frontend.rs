use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use ::window::*;
use anyhow::Error;
pub use config::FrontEndSelection;
use mux::client::ClientId;
use mux::window::WindowId as MuxWindowId;
use mux::{Mux, MuxNotification};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use wezterm_term::Alert;
use wezterm_toast_notification::*;

pub struct GuiFrontEnd {
    connection: Rc<Connection>,
    switching_workspaces: RefCell<bool>,
    known_windows: RefCell<BTreeMap<Window, MuxWindowId>>,
    client_id: Arc<ClientId>,
}

impl Drop for GuiFrontEnd {
    fn drop(&mut self) {
        ::window::shutdown();
    }
}

impl GuiFrontEnd {
    pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
        let connection = Connection::init()?;

        let mux = Mux::get().expect("mux started and running on main thread");
        let client_id = mux.active_identity().expect("to have set my own id");

        let front_end = Rc::new(GuiFrontEnd {
            connection,
            switching_workspaces: RefCell::new(false),
            known_windows: RefCell::new(BTreeMap::new()),
            client_id: client_id.clone(),
        });
        let fe = Rc::downgrade(&front_end);
        mux.subscribe(move |n| {
            if let Some(_fe) = fe.upgrade() {
                match n {
                    MuxNotification::WindowWorkspaceChanged(_)
                    | MuxNotification::ActiveWorkspaceChanged(_) => {}
                    MuxNotification::WindowCreated(_) | MuxNotification::WindowRemoved(_) => {
                        promise::spawn::spawn(async move {
                            let fe = crate::frontend::front_end();
                            if !fe.is_switching_workspace() {
                                fe.reconcile_workspace();
                            }
                        })
                        .detach();
                    }
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
                    MuxNotification::Alert {
                        pane_id: _,
                        alert:
                            Alert::OutputSinceFocusLost
                            | Alert::PaletteChanged
                            | Alert::TitleMaybeChanged
                            | Alert::SetUserVar { .. },
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

    pub fn reconcile_workspace(&self) {
        let mux = Mux::get().expect("mux started and running on main thread");
        let workspace = mux.active_workspace_for_client(&self.client_id);

        if mux.is_workspace_empty(&workspace) {
            // We don't want to silently kill off things that might
            // be running in other workspaces, so let's pick one
            // and activate it
            if self.is_switching_workspace() {
                return;
            }
            for workspace in mux.iter_workspaces() {
                if !mux.is_workspace_empty(&workspace) {
                    mux.set_active_workspace_for_client(&self.client_id, &workspace);
                    log::debug!("using {} instead, as it is not empty", workspace);
                    break;
                }
            }
        }

        let workspace = mux.active_workspace_for_client(&self.client_id);
        log::debug!("workspace is {}, fixup windows", workspace);

        let mut mux_windows = mux.iter_windows_in_workspace(&workspace);

        // First, repurpose existing windows.
        // Note that both iter_windows_in_workspace and self.known_windows have a
        // deterministic iteration order, so switching back and forth should result
        // in a consistent mux <-> gui window mapping.
        let known_windows = std::mem::take(&mut *self.known_windows.borrow_mut());
        let mut windows = BTreeMap::new();
        let mut unused = BTreeMap::new();

        for (window, window_id) in known_windows.into_iter() {
            if let Some(idx) = mux_windows.iter().position(|&id| id == window_id) {
                // it already points to the desired mux window
                windows.insert(window, window_id);
                mux_windows.remove(idx);
            } else {
                unused.insert(window, window_id);
            }
        }

        let mut mux_windows = mux_windows.into_iter();

        for (window, _old_id) in unused.into_iter() {
            if let Some(mux_window_id) = mux_windows.next() {
                window.notify(TermWindowNotif::SwitchToMuxWindow(mux_window_id));
                windows.insert(window, mux_window_id);
            } else {
                // We have more windows than are in the new workspace;
                // we no longer need this one!
                window.close();
            }
        }

        *self.known_windows.borrow_mut() = windows;

        // then spawn any new windows that are needed
        promise::spawn::spawn(async move {
            while let Some(mux_window_id) = mux_windows.next() {
                if let Err(err) = TermWindow::new_window(mux_window_id).await {
                    log::error!("Failed to create window: {:#}", err);
                    let mux = Mux::get().expect("switching_workspaces to trigger on main thread");
                    mux.kill_window(mux_window_id);
                }
            }
            *front_end().switching_workspaces.borrow_mut() = false;
        })
        .detach();
    }

    pub fn switch_workspace(&self, workspace: &str) {
        let mux = Mux::get().expect("mux started and running on main thread");
        mux.set_active_workspace_for_client(&self.client_id, workspace);
        *self.switching_workspaces.borrow_mut() = false;
        self.reconcile_workspace();
    }

    pub fn record_known_window(&self, window: Window, mux_window_id: MuxWindowId) {
        self.known_windows
            .borrow_mut()
            .insert(window, mux_window_id);
        if !self.is_switching_workspace() {
            self.reconcile_workspace();
        }
    }

    pub fn forget_known_window(&self, window: &Window) {
        self.known_windows.borrow_mut().remove(window);
        if !self.is_switching_workspace() {
            self.reconcile_workspace();
        }
    }

    pub fn is_switching_workspace(&self) -> bool {
        *self.switching_workspaces.borrow()
    }
}

thread_local! {
    static FRONT_END: RefCell<Option<Rc<GuiFrontEnd>>> = RefCell::new(None);
}

pub fn front_end() -> Rc<GuiFrontEnd> {
    FRONT_END
        .with(|f| f.borrow().as_ref().map(Rc::clone))
        .expect("to be called on gui thread")
}

pub struct WorkspaceSwitcher {
    new_name: String,
}

impl WorkspaceSwitcher {
    pub fn new(new_name: &str) -> Self {
        *front_end().switching_workspaces.borrow_mut() = true;
        Self {
            new_name: new_name.to_string(),
        }
    }

    pub fn do_switch(self) {
        // Drop is invoked, which will complete the switch
    }
}

impl Drop for WorkspaceSwitcher {
    fn drop(&mut self) {
        front_end().switch_workspace(&self.new_name);
    }
}

pub fn shutdown() {
    FRONT_END.with(|f| drop(f.borrow_mut().take()));
}

pub fn try_new() -> Result<Rc<GuiFrontEnd>, Error> {
    let front_end = GuiFrontEnd::try_new()?;
    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));
    Ok(front_end)
}
