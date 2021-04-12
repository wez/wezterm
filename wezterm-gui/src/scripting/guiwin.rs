//! GuiWin represents a Gui TermWindow (as opposed to a Mux window) in lua code
use super::luaerr;
use super::pane::PaneObject;
use crate::TermWindow;
use anyhow::anyhow;
use config::keyassignment::KeyAssignment;
use luahelper::*;
use mlua::{UserData, UserDataMethods};
use mux::window::WindowId as MuxWindowId;
use serde::*;
use wezterm_toast_notification::{
    persistent_toast_notification, persistent_toast_notification_with_click_to_open_url,
};
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

    pub async fn with_term_window<F, T>(&self, mut f: F) -> mlua::Result<T>
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
        methods.add_method(
            "toast_notification",
            |_, _, (title, message, url): (String, String, Option<String>)| {
                match url {
                    Some(url) => {
                        persistent_toast_notification_with_click_to_open_url(&title, &message, &url)
                    }
                    None => persistent_toast_notification(&title, &message),
                };
                Ok(())
            },
        );
        methods.add_async_method("set_right_status", |_, this, status: String| async move {
            this.with_term_window(move |term_window, _ops| {
                if status != term_window.right_status {
                    term_window.right_status = status.clone();
                    term_window.update_title_post_status();
                }
                Ok(())
            })
            .await
        });
        methods.add_async_method("get_dimensions", |_, this, _: ()| async move {
            this.with_term_window(move |term_window, _ops| {
                #[derive(Serialize, Deserialize)]
                struct Dims {
                    pixel_width: usize,
                    pixel_height: usize,
                    dpi: usize,
                    is_full_screen: bool,
                }
                impl_lua_conversion!(Dims);

                let dims = Dims {
                    pixel_width: term_window.dimensions.pixel_width,
                    pixel_height: term_window.dimensions.pixel_height,
                    dpi: term_window.dimensions.dpi,
                    is_full_screen: term_window.is_full_screen,
                };
                Ok(dims)
            })
            .await
        });
        methods.add_async_method(
            "get_selection_text_for_pane",
            |_, this, pane: PaneObject| async move {
                this.with_term_window(move |term_window, _ops| {
                    Ok(term_window.selection_text(&pane.pane()?))
                })
                .await
            },
        );
        methods.add_async_method(
            "perform_action",
            |_, this, (assignment, pane): (KeyAssignment, PaneObject)| async move {
                this.with_term_window(move |term_window, _ops| {
                    term_window.perform_key_assignment(&pane.pane()?, &assignment)
                })
                .await
            },
        );
        methods.add_async_method("effective_config", |_, this, _: ()| async move {
            this.with_term_window(move |term_window, _ops| Ok((*term_window.config).clone()))
                .await
        });
        methods.add_async_method("get_config_overrides", |_, this, _: ()| async move {
            this.with_term_window(move |term_window, _ops| {
                let wrap = JsonLua(term_window.config_overrides.clone());
                Ok(wrap)
            })
            .await
        });
        methods.add_async_method(
            "set_config_overrides",
            |_, this, value: JsonLua| async move {
                this.with_term_window(move |term_window, _ops| {
                    term_window.config_overrides = value.0.clone();
                    term_window.config_was_reloaded();
                    Ok(())
                })
                .await
            },
        );
    }
}
