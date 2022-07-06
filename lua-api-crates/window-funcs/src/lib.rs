use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua};
use luahelper::impl_lua_conversion_dynamic;
use std::collections::HashMap;
use std::rc::Rc;
use wezterm_dynamic::{FromDynamic, ToDynamic};
use window::{Connection, ConnectionOps};

fn get_conn() -> mlua::Result<Rc<Connection>> {
    Connection::get().ok_or_else(|| {
        mlua::Error::external("cannot get window Connection: not running on the gui thread?")
    })
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ScreenInfo {
    pub name: String,
    pub x: isize,
    pub y: isize,
    pub width: isize,
    pub height: isize,
}
impl_lua_conversion_dynamic!(ScreenInfo);

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct Screens {
    pub main: ScreenInfo,
    pub by_name: HashMap<String, ScreenInfo>,
    pub origin_x: isize,
    pub origin_y: isize,
    pub virtual_width: isize,
    pub virtual_height: isize,
}
impl_lua_conversion_dynamic!(Screens);

impl From<window::screen::ScreenInfo> for ScreenInfo {
    fn from(info: window::screen::ScreenInfo) -> Self {
        Self {
            name: info.name,
            x: info.rect.min_x(),
            y: info.rect.min_y(),
            width: info.rect.width(),
            height: info.rect.height(),
        }
    }
}

impl From<window::screen::Screens> for Screens {
    fn from(screens: window::screen::Screens) -> Self {
        let origin_x = screens.virtual_rect.min_x();
        let origin_y = screens.virtual_rect.min_y();
        let virtual_width = screens.virtual_rect.width();
        let virtual_height = screens.virtual_rect.height();
        Self {
            main: screens.main.into(),
            by_name: screens
                .by_name
                .into_iter()
                .map(|(k, info)| (k, info.into()))
                .collect(),
            origin_x,
            origin_y,
            virtual_width,
            virtual_height,
        }
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    let window_mod = lua.create_table()?;

    window_mod.set(
        "screens",
        lua.create_function(|_, _: ()| {
            let conn = get_conn()?;
            let screens: Screens = conn
                .screens()
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?
                .into();
            Ok(screens)
        })?,
    )?;

    wezterm_mod.set("window", window_mod)?;
    Ok(())
}
