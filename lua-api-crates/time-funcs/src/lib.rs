use chrono::prelude::*;
use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods, UserDataRef};
use config::lua::{
    emit_event, get_or_create_module, get_or_create_sub_module, is_event_emission, wrap_callback,
};
use config::ConfigSubscription;
use std::rc::Rc;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref CONFIG_SUBSCRIPTION: Mutex<Option<ConfigSubscription>> = Mutex::new(None);
}

/// We contrive to call this from the main thread in response to the
/// config being reloaded.
/// It spawns a task for each of the timers that have been configured
/// by the user via `wezterm.time.call_after`.
fn schedule_all(lua: Option<Rc<mlua::Lua>>) -> mlua::Result<()> {
    if let Some(lua) = lua {
        let scheduled_events: Vec<UserDataRef<ScheduledEvent>> =
            lua.named_registry_value(SCHEDULED_EVENTS)?;
        lua.set_named_registry_value(SCHEDULED_EVENTS, Vec::<ScheduledEvent>::new())?;
        let generation = config::configuration().generation();
        for event in scheduled_events {
            event.clone().schedule(generation);
        }
    }
    Ok(())
}

/// Helper to schedule !Send futures to run with access to the lua
/// config on the main thread
fn schedule_trampoline() {
    promise::spawn::spawn(async move {
        config::with_lua_config_on_main_thread(|lua| async move {
            schedule_all(lua)?;
            Ok(())
        })
        .await
    })
    .detach();
}

/// Called by the config subsystem when the config is reloaded.
/// We use it to schedule our setup function that will schedule
/// the call_after functions from the main thread.
pub fn config_was_reloaded() -> bool {
    if promise::spawn::is_scheduler_configured() {
        promise::spawn::spawn_into_main_thread(async move {
            schedule_trampoline();
        })
        .detach();
    }

    true
}

/// Keeps track of `call_after` state
#[derive(Debug, Clone)]
struct ScheduledEvent {
    /// The name of the registry entry that will resolve to
    /// their callback function
    user_event_id: String,
    /// The delay after which to run their callback
    interval_seconds: f64,
}

impl ScheduledEvent {
    /// Schedule a task with the scheduler runtime.
    /// Note that this will extend the lifetime of the lua context
    /// until their timeout completes and their function is called.
    /// That can lead to exponential growth in callbacks on each
    /// config reload, which is undesirable!
    /// To address that, we pass in the current configuration generation
    /// at the time that we called schedule.
    /// Later, after our interval has elapsed, if the generation
    /// doesn't match the then-current generation we skip performing
    /// the actual callback.
    /// That means that for large intervals we may keep more memory
    /// occupied, but we won't run the callback twice for the first
    /// reload, or 4 times for the second and so on.
    fn schedule(self, generation: usize) {
        let event = self;
        promise::spawn::spawn(async move {
            config::with_lua_config_on_main_thread(move |lua| async move {
                if let Some(lua) = lua {
                    event.run(&lua, generation).await?;
                }
                Ok(())
            })
            .await
        })
        .detach();
    }

    async fn run(self, lua: &Lua, generation: usize) -> mlua::Result<()> {
        let duration = std::time::Duration::from_secs_f64(self.interval_seconds);
        smol::Timer::after(duration).await;
        // Skip doing anything of consequence if the generation has
        // changed.
        if config::configuration().generation() == generation {
            let args = lua.pack_multi(())?;
            emit_event(&lua, (self.user_event_id, args)).await?;
        }
        Ok(())
    }
}

impl UserData for ScheduledEvent {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(_methods: &mut M) {}
}

const SCHEDULED_EVENTS: &str = "wezterm-scheduled-events";

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    {
        let mut sub = CONFIG_SUBSCRIPTION.lock().unwrap();
        if sub.is_none() {
            sub.replace(config::subscribe_to_config_reload(config_was_reloaded));
        }
    }
    lua.set_named_registry_value(SCHEDULED_EVENTS, Vec::<ScheduledEvent>::new())?;
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

    time_mod.set(
        "call_after",
        lua.create_function(|lua, (interval_seconds, func): (f64, mlua::Function)| {
            let user_event_id = wrap_callback(lua, func)?;

            let event = ScheduledEvent {
                user_event_id,
                interval_seconds,
            };

            if is_event_emission(lua)? {
                let generation = config::configuration().generation();
                event.schedule(generation);
            } else {
                let scheduled_events: Vec<UserDataRef<ScheduledEvent>> =
                    lua.named_registry_value(SCHEDULED_EVENTS)?;
                let mut scheduled_events: Vec<ScheduledEvent> =
                    scheduled_events.into_iter().map(|e| e.clone()).collect();
                scheduled_events.push(event);
                lua.set_named_registry_value(SCHEDULED_EVENTS, scheduled_events)?;
            }
            Ok(())
        })?,
    )?;

    // For backwards compatibility
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("sleep_ms", lua.create_async_function(sleep_ms)?)?;
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

async fn sleep_ms<'lua>(_: &'lua Lua, milliseconds: u64) -> mlua::Result<()> {
    let duration = std::time::Duration::from_millis(milliseconds);
    smol::Timer::after(duration).await;
    Ok(())
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
                    } else if up {
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
