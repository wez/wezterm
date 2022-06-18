use config::keyassignment::SpawnTabDomain;
use config::lua::get_or_create_module;
use config::lua::mlua::{self, Lua, UserData, UserDataMethods};
use luahelper::impl_lua_conversion_dynamic;
use mux::domain::SplitSource;
use mux::pane::{Pane, PaneId};
use mux::tab::{SplitDirection, SplitRequest, SplitSize, Tab, TabId};
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

    mux_mod.set(
        "get_pane",
        lua.create_function(|_, pane_id: PaneId| {
            let mux = get_mux()?;
            let pane = MuxPane(pane_id);
            pane.resolve(&mux)?;
            Ok(pane)
        })?,
    )?;

    mux_mod.set(
        "get_tab",
        lua.create_function(|_, tab_id: TabId| {
            let mux = get_mux()?;
            let tab = MuxTab(tab_id);
            tab.resolve(&mux)?;
            Ok(tab)
        })?,
    )?;

    mux_mod.set(
        "spawn_window",
        lua.create_async_function(|_, spawn: SpawnWindow| async move { spawn.spawn().await })?,
    )?;

    wezterm_mod.set("mux", mux_mod)?;
    Ok(())
}

#[derive(Debug, Default, FromDynamic, ToDynamic)]
struct CommandBuilderFrag {
    args: Option<Vec<String>>,
    cwd: Option<String>,
    #[dynamic(default)]
    set_environment_variables: HashMap<String, String>,
}

impl CommandBuilderFrag {
    fn to_command_builder(self) -> (Option<CommandBuilder>, Option<String>) {
        if let Some(args) = self.args {
            let mut builder = CommandBuilder::from_argv(args.iter().map(Into::into).collect());
            for (k, v) in self.set_environment_variables.iter() {
                builder.env(k, v);
            }
            if let Some(cwd) = self.cwd.clone() {
                builder.cwd(cwd);
            }
            (Some(builder), None)
        } else {
            (None, self.cwd)
        }
    }
}

#[derive(Debug, FromDynamic, ToDynamic)]
enum HandySplitDirection {
    Left,
    Right,
    Top,
    Bottom,
}
impl_lua_conversion_dynamic!(HandySplitDirection);

impl Default for HandySplitDirection {
    fn default() -> Self {
        Self::Right
    }
}

#[derive(Debug, Default, FromDynamic, ToDynamic)]
struct SplitPane {
    #[dynamic(flatten)]
    cmd_builder: CommandBuilderFrag,
    #[dynamic(default = "spawn_tab_default_domain")]
    domain: SpawnTabDomain,
    #[dynamic(default)]
    direction: HandySplitDirection,
    #[dynamic(default)]
    top_level: bool,
    #[dynamic(default = "default_split_size")]
    size: f32,
}
impl_lua_conversion_dynamic!(SplitPane);

fn default_split_size() -> f32 {
    0.5
}

impl SplitPane {
    async fn run(self, pane: MuxPane) -> mlua::Result<MuxPane> {
        let (command, command_dir) = self.cmd_builder.to_command_builder();
        let source = SplitSource::Spawn {
            command,
            command_dir,
        };

        let size = if self.size == 0.0 {
            SplitSize::Percent(50)
        } else if self.size < 1.0 {
            SplitSize::Percent((self.size * 100.).floor() as u8)
        } else {
            SplitSize::Cells(self.size as usize)
        };

        let direction = match self.direction {
            HandySplitDirection::Right | HandySplitDirection::Left => SplitDirection::Horizontal,
            HandySplitDirection::Top | HandySplitDirection::Bottom => SplitDirection::Vertical,
        };

        let request = SplitRequest {
            direction,
            target_is_second: match self.direction {
                HandySplitDirection::Top | HandySplitDirection::Left => false,
                HandySplitDirection::Bottom | HandySplitDirection::Right => true,
            },
            top_level: self.top_level,
            size,
        };

        let mux = get_mux()?;
        let (pane, _size) = mux
            .split_pane(pane.0, request, source, self.domain)
            .await
            .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

        Ok(MuxPane(pane.pane_id()))
    }
}

#[derive(Debug, FromDynamic, ToDynamic)]
struct SpawnWindow {
    #[dynamic(default = "spawn_tab_default_domain")]
    domain: SpawnTabDomain,
    width: Option<usize>,
    height: Option<usize>,
    workspace: Option<String>,
    #[dynamic(flatten)]
    cmd_builder: CommandBuilderFrag,
}
impl_lua_conversion_dynamic!(SpawnWindow);

fn spawn_tab_default_domain() -> SpawnTabDomain {
    SpawnTabDomain::DefaultDomain
}

impl SpawnWindow {
    async fn spawn(self) -> mlua::Result<(MuxTab, MuxPane, MuxWindow)> {
        let mux = get_mux()?;

        let size = match (self.width, self.height) {
            (Some(cols), Some(rows)) => TerminalSize {
                rows,
                cols,
                ..Default::default()
            },
            _ => config::configuration().initial_size(0),
        };

        let (cmd_builder, cwd) = self.cmd_builder.to_command_builder();
        let (tab, pane, window_id) = mux
            .spawn_tab_or_window(
                None,
                self.domain,
                cmd_builder,
                cwd,
                size,
                None,
                self.workspace.unwrap_or_else(|| mux.active_workspace()),
            )
            .await
            .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

        Ok((
            MuxTab(tab.tab_id()),
            MuxPane(pane.pane_id()),
            MuxWindow(window_id),
        ))
    }
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
    #[dynamic(default)]
    domain: SpawnTabDomain,
    #[dynamic(flatten)]
    cmd_builder: CommandBuilderFrag,
}
impl_lua_conversion_dynamic!(SpawnTab);

impl SpawnTab {
    async fn spawn(self, window: MuxWindow) -> mlua::Result<(MuxTab, MuxPane, MuxWindow)> {
        let mux = get_mux()?;
        let size;
        let pane;

        {
            let window = window.resolve(&mux)?;
            size = window
                .get_by_idx(0)
                .map(|tab| tab.get_size())
                .unwrap_or_else(|| config::configuration().initial_size(0));

            pane = window
                .get_active()
                .and_then(|tab| tab.get_active_pane().map(|pane| pane.pane_id()));
        };

        let (cmd_builder, cwd) = self.cmd_builder.to_command_builder();

        let (tab, pane, window_id) = mux
            .spawn_tab_or_window(
                Some(window.0),
                self.domain,
                cmd_builder,
                cwd,
                size,
                pane,
                String::new(),
            )
            .await
            .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

        Ok((
            MuxTab(tab.tab_id()),
            MuxPane(pane.pane_id()),
            MuxWindow(window_id),
        ))
    }
}

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
            spawn.spawn(this).await
        });
    }
}

impl MuxPane {
    fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<dyn Pane>> {
        mux.get_pane(self.0)
            .ok_or_else(|| mlua::Error::external(format!("pane id {} not found in mux", self.0)))
    }
}

impl UserData for MuxPane {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("pane_id", |_, this, _: ()| Ok(this.0));
        methods.add_async_method("split", |_, this, args: Option<SplitPane>| async move {
            let args = args.unwrap_or_default();
            args.run(this).await
        });
    }
}

impl MuxTab {
    fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<Tab>> {
        mux.get_tab(self.0)
            .ok_or_else(|| mlua::Error::external(format!("tab id {} not found in mux", self.0)))
    }
}

impl UserData for MuxTab {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("tab_id", |_, this, _: ()| Ok(this.0));
    }
}
