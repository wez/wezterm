//! GuiWin represents a Gui TermWindow (as opposed to a Mux window) in lua code
use super::luaerr;
use super::pane::PaneObject;
use crate::gui::TermWindow;
use anyhow::anyhow;
use config::keyassignment::KeyAssignment;
use mlua::{UserData, UserDataMethods};
use mux::window::WindowId as MuxWindowId;
use window::WindowOps;

#[derive(Clone)]
pub struct GuiWin {
    mux_window_id: MuxWindowId,
    window: ::window::Window,
}

impl GuiWin {
    pub fn new(term_window: &TermWindow) -> Self {
        let window = term_window.window.clone().unwrap();
        let mux_window_id = term_window.mux_window_id;
        Self {
            window,
            mux_window_id,
        }
    }

    async fn with_term_window<F, T>(&self, mut f: F) -> mlua::Result<T>
    where
        F: FnMut(&mut TermWindow, &dyn WindowOps) -> anyhow::Result<T>,
        F: Send + 'static,
        T: Send + 'static,
    {
        self.window
            .apply(move |tw, ops| {
                if let Some(term_window) = tw.downcast_mut::<TermWindow>() {
                    f(term_window, ops)
                } else {
                    Err(anyhow!("Window is not TermWindow!?"))
                }
            })
            .await
            .map_err(luaerr)
    }
}

impl UserData for GuiWin {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("window_id", |_, this, _: ()| Ok(this.mux_window_id));
        methods.add_async_method(
            "perform_action",
            |_, this, (assignment, pane): (KeyAssignment, PaneObject)| async move {
                this.with_term_window(move |term_window, _ops| {
                    term_window.perform_key_assignment(&pane.pane()?, &assignment)
                })
                .await
            },
        );
    }
}
