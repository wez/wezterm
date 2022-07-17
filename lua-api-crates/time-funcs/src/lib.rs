use chrono::prelude::*;
use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods};
use config::lua::{get_or_create_module, get_or_create_sub_module};

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let time_mod = get_or_create_sub_module(lua, "time")?;

    time_mod.set(
        "now",
        lua.create_function(|_, _: ()| Ok(Time { utc: Utc::now() }))?,
    )?;

    time_mod.set(
        "parse_rfc3339",
        lua.create_function(|_, s: String| {
            let time = DateTime::parse_from_rfc3339(&s).map_err(|err| {
                mlua::Error::external(format!("{err:#} while parsing {s} as an RFC3339 time"))
            })?;
            Ok(Time { utc: time.into() })
        })?,
    )?;

    time_mod.set(
        "parse",
        lua.create_function(|_, (s, fmt): (String, String)| {
            let time = DateTime::parse_from_str(&s, &fmt).map_err(|err| {
                mlua::Error::external(format!("{err:#} while parsing {s} using format {fmt}"))
            })?;
            Ok(Time { utc: time.into() })
        })?,
    )?;

    // For backwards compatibility
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("strftime", lua.create_function(strftime)?)?;
    wezterm_mod.set("strftime_utc", lua.create_function(strftime_utc)?)?;
    Ok(())
}

fn strftime_utc<'lua>(_: &'lua Lua, format: String) -> mlua::Result<String> {
    let local: DateTime<Utc> = Utc::now();
    Ok(local.format(&format).to_string())
}

fn strftime<'lua>(_: &'lua Lua, format: String) -> mlua::Result<String> {
    let local: DateTime<Local> = Local::now();
    Ok(local.format(&format).to_string())
}

#[derive(Clone, Debug)]
pub struct Time {
    utc: DateTime<Utc>,
}

impl UserData for Time {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            let utc = this.utc.to_rfc3339();
            Ok(format!("Time(utc: {utc})"))
        });
        methods.add_method("format", |_, this, format: String| {
            let local: DateTime<Local> = this.utc.into();
            Ok(local.format(&format).to_string())
        });
        methods.add_method("format_utc", |_, this, format: String| {
            Ok(this.utc.format(&format).to_string())
        });
        methods.add_method("sun_times", |lua, this, (lat, lon): (f64, f64)| {
            let info = spa::calc_sunrise_and_set(this.utc, lat, lon)
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?;

            let times = match info {
                spa::SunriseAndSet::PolarNight => SunTimes {
                    rise: None,
                    set: None,
                    up: false,
                    progression: 0.,
                },
                spa::SunriseAndSet::PolarDay => SunTimes {
                    rise: None,
                    set: None,
                    up: true,
                    progression: 0.,
                },
                spa::SunriseAndSet::Daylight(rise, set) => {
                    let progression;
                    let up = this.utc >= rise && this.utc <= set;
                    let day_duration = set - rise;
                    let night_duration = chrono::Duration::days(1) - day_duration;
                    if this.utc < rise {
                        // Sun hasn't yet risen
                        progression = (night_duration - (rise - this.utc)).num_minutes() as f64
                            / night_duration.num_minutes() as f64;
                    } else if this.utc >= rise {
                        // Sun is up
                        progression = (this.utc - rise).num_minutes() as f64
                            / day_duration.num_minutes() as f64;
                    } else {
                        // time is after sunset
                        progression = (this.utc - set).num_minutes() as f64
                            / night_duration.num_minutes() as f64;
                    };
                    SunTimes {
                        rise: Some(Time { utc: rise }),
                        set: Some(Time { utc: set }),
                        up,
                        progression,
                    }
                }
            };

            let tbl = lua.create_table()?;
            tbl.set("rise", times.rise)?;
            tbl.set("set", times.set)?;
            tbl.set("up", times.up)?;
            tbl.set("progression", times.progression)?;
            Ok(tbl)
        });
    }
}

struct SunTimes {
    rise: Option<Time>,
    set: Option<Time>,
    up: bool,
    progression: f64,
}
