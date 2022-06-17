use config::keyassignment::SpawnTabDomain;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua, UserData, UserDataMethods};
use luahelper::impl_lua_conversion_dynamic;
use mux::pane::PaneId;
use mux::tab::TabId;
use mux::window::{Window, WindowId};
use mux::Mux;
use portable_pty::CommandBuilder;
use std::cell::{Ref, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use wezterm_dynamic::{FromDynamic, ToDynamic};
use wezterm_term::TerminalSize;

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

    mux_mod.set(
        "get_window",
        lua.create_function(|_, window_id: WindowId| {
            let mux = get_mux()?;
            let window = MuxWindow(window_id);
            window.resolve(&mux)?;
            Ok(window)
        })?,
    )?;

    wezterm_mod.set("mux", mux_mod)?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct MuxWindow(WindowId);
#[derive(Clone, Copy, Debug)]
struct MuxTab(TabId);
#[derive(Clone, Copy, Debug)]
struct MuxPane(PaneId);

impl MuxWindow {
    fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Ref<'a, Window>> {
        mux.get_window(self.0)
            .ok_or_else(|| mlua::Error::external(format!("window id {} not found in mux", self.0)))
    }

    fn resolve_mut<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<RefMut<'a, Window>> {
        mux.get_window_mut(self.0)
            .ok_or_else(|| mlua::Error::external(format!("window id {} not found in mux", self.0)))
    }
}

#[derive(Debug, FromDynamic, ToDynamic)]
struct SpawnTab {
    args: Option<Vec<String>>,
    cwd: Option<String>,
    #[dynamic(default)]
    domain: SpawnTabDomain,
    width: Option<usize>,
    height: Option<usize>,
    #[dynamic(default)]
    set_environment_variables: HashMap<String, String>,
}
impl_lua_conversion_dynamic!(SpawnTab);

impl UserData for MuxWindow {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("window_id", |_, this, _: ()| Ok(this.0));
        methods.add_method("get_workspace", |_, this, _: ()| {
            let mux = get_mux()?;
            let window = this.resolve(&mux)?;
            Ok(window.get_workspace().to_string())
        });
        methods.add_method("set_workspace", |_, this, new_name: String| {
            let mux = get_mux()?;
            let mut window = this.resolve_mut(&mux)?;
            Ok(window.set_workspace(&new_name))
        });
        methods.add_async_method("spawn_tab", |_, this, spawn: SpawnTab| async move {
            let mux = get_mux()?;
            let size;
            let pane;

            {
                let window = this.resolve(&mux)?;
                size = window
                    .get_by_idx(0)
                    .map(|tab| tab.get_size())
                    .unwrap_or_else(|| match (spawn.width, spawn.height) {
                        (Some(cols), Some(rows)) => TerminalSize {
                            rows,
                            cols,
                            ..Default::default()
                        },
                        _ => config::configuration().initial_size(0),
                    });

                pane = window
                    .get_active()
                    .and_then(|tab| tab.get_active_pane().map(|pane| pane.pane_id()));
            };

            let cmd_builder = if let Some(args) = spawn.args {
                let mut builder = CommandBuilder::from_argv(args.iter().map(Into::into).collect());
                for (k, v) in spawn.set_environment_variables.iter() {
                    builder.env(k, v);
                }
                if let Some(cwd) = spawn.cwd.clone() {
                    builder.cwd(cwd);
                }
                Some(builder)
            } else {
                None
            };

            let (tab, pane, window_id) = mux
                .spawn_tab_or_window(
                    Some(this.0),
                    spawn.domain,
                    cmd_builder,
                    spawn.cwd,
                    size,
                    pane,
                    String::new(),
                )
                .await
                .map_err(|e| mlua::Error::external(e.to_string()))?;

            Ok((
                MuxTab(tab.tab_id()),
                MuxPane(pane.pane_id()),
                MuxWindow(window_id),
            ))
        });
    }
}

impl UserData for MuxPane {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {}
}

impl UserData for MuxTab {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {}
}
