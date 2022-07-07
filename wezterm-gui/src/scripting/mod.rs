use crate::frontend::try_front_end;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua};
use mux::window::WindowId as MuxWindowId;

pub mod guiwin;
pub mod pane;

fn luaerr(err: anyhow::Error) -> mlua::Error {
    mlua::Error::external(err)
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let window_mod = get_or_create_sub_module(lua, "gui")?;

    window_mod.set(
        "gui_window_for_mux_window",
        lua.create_async_function(|_, mux_window_id: MuxWindowId| async move {
            let fe =
                try_front_end().ok_or_else(|| mlua::Error::external("not called on gui thread"))?;
            let _ = fe.reconcile_workspace().await;
            let win = fe.gui_window_for_mux_window(mux_window_id).ok_or_else(|| {
                mlua::Error::external(format!(
                    "mux window id {mux_window_id} is not currently associated with a gui window"
                ))
            })?;
            Ok(win)
        })?,
    )?;

    Ok(())
}
