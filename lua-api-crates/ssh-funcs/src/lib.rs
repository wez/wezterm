use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua, Variadic};
use std::collections::HashMap;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set(
        "enumerate_ssh_hosts",
        lua.create_function(enumerate_ssh_hosts)?,
    )?;
    wezterm_mod.set(
        "default_ssh_domains",
        lua.create_function(|_, ()| Ok(config::SshDomain::default_domains()))?,
    )?;
    Ok(())
}

fn enumerate_ssh_hosts<'lua>(
    lua: &'lua Lua,
    config_files: Variadic<String>,
) -> mlua::Result<HashMap<String, wezterm_ssh::ConfigMap>> {
    let mut config = wezterm_ssh::Config::new();
    for file in config_files {
        config.add_config_file(file);
    }
    config.add_default_config_files();

    // Trigger a config reload if any of the parsed ssh config files change
    let files: Variadic<String> = config
        .loaded_config_files()
        .into_iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();
    config::lua::add_to_config_reload_watch_list(lua, files)?;

    let mut map = HashMap::new();
    for host in config.enumerate_hosts() {
        let host_config = config.for_host(&host);
        map.insert(host, host_config);
    }

    Ok(map)
}
