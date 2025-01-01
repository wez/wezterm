use config::lua::get_or_create_sub_module;
use config::lua::mlua::Lua;
use procinfo::LocalProcessInfo;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let proc_mod = get_or_create_sub_module(lua, "procinfo")?;
    proc_mod.set(
        "pid",
        lua.create_function(|_, _: ()| Ok(unsafe { libc::getpid() }))?,
    )?;
    proc_mod.set(
        "get_info_for_pid",
        lua.create_function(|_, pid: u32| Ok(LocalProcessInfo::with_root_pid(pid)))?,
    )?;
    proc_mod.set(
        "current_working_dir_for_pid",
        lua.create_function(|_, pid: u32| {
            Ok(LocalProcessInfo::current_working_dir(pid)
                .and_then(|p| p.to_str().map(|s| s.to_string())))
        })?,
    )?;
    proc_mod.set(
        "executable_path_for_pid",
        lua.create_function(|_, pid: u32| {
            Ok(LocalProcessInfo::executable_path(pid)
                .and_then(|p| p.to_str().map(|s| s.to_string())))
        })?,
    )?;
    Ok(())
}
