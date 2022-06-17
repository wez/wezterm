use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua};
use mux::Mux;
use std::rc::Rc;

fn get_mux() -> mlua::Result<Rc<Mux>> {
    Mux::get()
        .ok_or_else(|| mlua::Error::external("cannot get Mux: not running on the mux thread?"))
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    let mux_mod = lua.create_table()?;

    mux_mod.set(
        "active_workspace",
        lua.create_function(|_, _: ()| {
            let mux = get_mux()?;
            Ok(mux.active_workspace())
        })?,
    )?;

    wezterm_mod.set("mux", mux_mod)?;
    Ok(())
}
