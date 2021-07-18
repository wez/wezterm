//! GuiWin represents a Gui TermWindow (as opposed to a Mux window) in lua code
use super::luaerr;
use super::pane::PaneObject;
use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use config::keyassignment::KeyAssignment;
use luahelper::*;
use mlua::{UserData, UserDataMethods};
use mux::window::WindowId as MuxWindowId;
use serde::*;
use wezterm_toast_notification::ToastNotification;
use window::{Connection, ConnectionOps, WindowOps};

#[derive(Clone)]
pub struct GuiWin {
    mux_window_id: MuxWindowId,
    pub window: ::window::Window,
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
}

impl UserData for GuiWin {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("window_id", |_, this, _: ()| Ok(this.mux_window_id));
        methods.add_method(
            "toast_notification",
            |_, _, (title, message, url, timeout): (String, String, Option<String>, Option<u64>)| {
                wezterm_toast_notification::show(ToastNotification {
                    title,
                    message,
                    url,
                    timeout: timeout.map(std::time::Duration::from_millis)
                });
                Ok(())
            },
        );
        methods.add_method("get_appearance", |_, _, _: ()| {
            Ok(Connection::get().unwrap().get_appearance().to_string())
        });
        methods.add_method("set_right_status", |_, this, status: String| {
            this.window.notify(TermWindowNotif::SetRightStatus(status));
            Ok(())
        });
        methods.add_async_method("get_dimensions", |_, this, _: ()| async move {
            let (tx, rx) = smol::channel::bounded(1);
            this.window.notify(TermWindowNotif::GetDimensions(tx));
            let (dims, is_full_screen) = rx
                .recv()
                .await
                .map_err(|e| anyhow::anyhow!("{:#}", e))
                .map_err(luaerr)?;

            #[derive(Serialize, Deserialize)]
            struct Dims {
                pixel_width: usize,
                pixel_height: usize,
                dpi: usize,
                is_full_screen: bool,
            }
            impl_lua_conversion!(Dims);

            let dims = Dims {
                pixel_width: dims.pixel_width,
                pixel_height: dims.pixel_height,
                dpi: dims.dpi,
                is_full_screen,
            };
            Ok(dims)
        });
        methods.add_async_method(
            "get_selection_text_for_pane",
            |_, this, pane: PaneObject| async move {
                let (tx, rx) = smol::channel::bounded(1);
                this.window.notify(TermWindowNotif::GetSelectionForPane {
                    pane_id: pane.pane,
                    tx,
                });
                let text = rx
                    .recv()
                    .await
                    .map_err(|e| anyhow::anyhow!("{:#}", e))
                    .map_err(luaerr)?;

                Ok(text)
            },
        );
        methods.add_method(
            "perform_action",
            |_, this, (assignment, pane): (KeyAssignment, PaneObject)| {
                this.window.notify(TermWindowNotif::PerformAssignment {
                    pane_id: pane.pane,
                    assignment,
                });
                Ok(())
            },
        );
        methods.add_async_method("effective_config", |_, this, _: ()| async move {
            let (tx, rx) = smol::channel::bounded(1);
            this.window.notify(TermWindowNotif::GetEffectiveConfig(tx));
            let config = rx
                .recv()
                .await
                .map_err(|e| anyhow::anyhow!("{:#}", e))
                .map_err(luaerr)?;

            Ok((*config).clone())
        });
        methods.add_async_method("get_config_overrides", |_, this, _: ()| async move {
            let (tx, rx) = smol::channel::bounded(1);
            this.window.notify(TermWindowNotif::GetConfigOverrides(tx));
            let overrides = rx
                .recv()
                .await
                .map_err(|e| anyhow::anyhow!("{:#}", e))
                .map_err(luaerr)?;

            let wrap = JsonLua(overrides);
            Ok(wrap)
        });
        methods.add_method("set_config_overrides", |_, this, value: JsonLua| {
            this.window
                .notify(TermWindowNotif::SetConfigOverrides(value.0));
            Ok(())
        });
    }
}
