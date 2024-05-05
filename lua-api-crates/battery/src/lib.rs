use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua};
use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("battery_info", lua.create_function(battery_info)?)?;
    Ok(())
}

#[derive(FromDynamic, ToDynamic, Debug)]
struct BatteryInfo {
    state_of_charge: f32,
    vendor: String,
    model: String,
    state: String,
    serial: String,
    time_to_full: Option<f32>,
    time_to_empty: Option<f32>,
}
impl_lua_conversion_dynamic!(BatteryInfo);

fn battery_info<'lua>(_: &'lua Lua, _: ()) -> mlua::Result<Vec<BatteryInfo>> {
    use starship_battery::{Manager, State};
    let manager = Manager::new().map_err(mlua::Error::external)?;
    let mut result = vec![];
    for b in manager.batteries().map_err(mlua::Error::external)? {
        let bat = b.map_err(mlua::Error::external)?;
        result.push(BatteryInfo {
            state_of_charge: bat.state_of_charge().value,
            vendor: opt_string(bat.vendor()),
            model: opt_string(bat.model()),
            serial: opt_string(bat.serial_number()),
            state: match bat.state() {
                State::Charging => "Charging",
                State::Discharging => "Discharging",
                State::Empty => "Empty",
                State::Full => "Full",
                State::Unknown => "Unknown",
            }
            .to_string(),
            time_to_full: bat.time_to_full().map(|q| q.value),
            time_to_empty: bat.time_to_empty().map(|q| q.value),
        })
    }
    Ok(result)
}

fn opt_string(s: Option<&str>) -> String {
    match s {
        Some(s) => s,
        None => "unknown",
    }
    .to_string()
}
