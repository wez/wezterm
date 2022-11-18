use crate::frontend::try_front_end;
use crate::inputmap::InputMap;
use config::keyassignment::KeyTable;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua};
use config::{DeferredKeyCode, GpuInfo, Key, KeyNoAction};
use luahelper::dynamic_to_lua_value;
use mux::window::WindowId as MuxWindowId;
use std::collections::HashMap;
use wezterm_dynamic::ToDynamic;

pub mod guiwin;

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

    fn key_table_to_lua(table: &KeyTable) -> Vec<Key> {
        let mut keys = vec![];
        for ((key, mods), entry) in table {
            keys.push(Key {
                key: KeyNoAction {
                    key: DeferredKeyCode::KeyCode(key.clone()),
                    mods: *mods,
                },
                action: entry.action.clone(),
            });
        }
        keys
    }

    window_mod.set(
        "default_keys",
        lua.create_function(|lua, _: ()| {
            let map = InputMap::default_input_map();
            let keys = key_table_to_lua(&map.keys.default);
            dynamic_to_lua_value(lua, keys.to_dynamic())
        })?,
    )?;

    window_mod.set(
        "default_key_tables",
        lua.create_function(|lua, _: ()| {
            let inputmap = InputMap::default_input_map();
            let mut tables: HashMap<String, Vec<Key>> = HashMap::new();
            for (k, table) in &inputmap.keys.by_name {
                let keys = key_table_to_lua(table);
                tables.insert(k.to_string(), keys);
            }
            dynamic_to_lua_value(lua, tables.to_dynamic())
        })?,
    )?;

    window_mod.set(
        "enumerate_gpus",
        lua.create_function(|_, _: ()| {
            let backends = wgpu::Backends::all();
            let instance = wgpu::Instance::new(backends);
            let gpus: Vec<GpuInfo> = instance
                .enumerate_adapters(backends)
                .map(|adapter| {
                    let info = adapter.get_info();
                    crate::termwindow::webgpu::adapter_info_to_gpu_info(info)
                })
                .collect();
            Ok(gpus)
        })?,
    )?;

    Ok(())
}
