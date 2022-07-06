use config::keyassignment::SpawnTabDomain;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, UserData, UserDataMethods, Value as LuaValue};
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
    let mux_mod = get_or_create_sub_module(lua, "mux")?;

    mux_mod.set(
        "get_active_workspace",
        lua.create_function(|_, _: ()| {
            let mux = get_mux()?;
            Ok(mux.active_workspace())
        })?,
    )?;

    mux_mod.set(
        "get_workspace_names",
        lua.create_function(|_, _: ()| {
            let mux = get_mux()?;
            Ok(mux.iter_workspaces())
        })?,
    )?;

    mux_mod.set(
        "set_active_workspace",
        lua.create_function(|_, workspace: String| {
            let mux = get_mux()?;
            let workspaces = mux.iter_workspaces();
            if workspaces.contains(&workspace) {
                Ok(mux.set_active_workspace(&workspace))
            } else {
                Err(mlua::Error::external(format!(
                    "{:?} is not an existing workspace",
                    workspace
                )))
            }
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

    mux_mod.set(
        "all_windows",
        lua.create_function(|_, _: ()| {
            let mux = get_mux()?;
            Ok(mux
                .iter_windows()
                .into_iter()
                .map(|id| MuxWindow(id))
                .collect::<Vec<MuxWindow>>())
        })?,
    )?;

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
pub struct MuxWindow(pub WindowId);
#[derive(Clone, Copy, Debug)]
pub struct MuxTab(pub TabId);
#[derive(Clone, Copy, Debug)]
pub struct MuxPane(pub PaneId);

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

#[derive(Clone, FromDynamic, ToDynamic)]
struct MuxTabInfo {
    pub index: usize,
    pub is_active: bool,
}
impl_lua_conversion_dynamic!(MuxTabInfo);

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
        methods.add_method("get_title", |_, this, _: ()| {
            let mux = get_mux()?;
            let window = this.resolve(&mux)?;
            Ok(window.get_title().to_string())
        });
        methods.add_method("set_title", |_, this, title: String| {
            let mux = get_mux()?;
            let mut window = this.resolve_mut(&mux)?;
            Ok(window.set_title(&title))
        });
        methods.add_method("tabs", |_, this, _: ()| {
            let mux = get_mux()?;
            let window = this.resolve(&mux)?;
            Ok(window
                .iter()
                .map(|tab| MuxTab(tab.tab_id()))
                .collect::<Vec<MuxTab>>())
        });
        methods.add_method("tabs_with_info", |lua, this, _: ()| {
            let mux = get_mux()?;
            let window = this.resolve(&mux)?;
            let result = lua.create_table()?;
            let active_idx = window.get_active_idx();
            for (index, tab) in window.iter().enumerate() {
                let info = MuxTabInfo {
                    index,
                    is_active: index == active_idx,
                };
                let info = luahelper::dynamic_to_lua_value(lua, info.to_dynamic())?;
                match &info {
                    LuaValue::Table(t) => {
                        t.set("tab", MuxTab(tab.tab_id()))?;
                    }
                    _ => {}
                }
                result.set(index + 1, info)?;
            }
            Ok(result)
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
        methods.add_method("send_paste", |_, this, text: String| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            pane.send_paste(&text)
                .map_err(|e| mlua::Error::external(format!("{:#}", e)))?;
            Ok(())
        });
        methods.add_method("send_text", |_, this, text: String| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            pane.writer()
                .write_all(text.as_bytes())
                .map_err(|e| mlua::Error::external(format!("{:#}", e)))?;
            Ok(())
        });
        methods.add_method("window", |_, this, _: ()| {
            let mux = get_mux()?;
            Ok(mux
                .resolve_pane_id(this.0)
                .map(|(_domain_id, window_id, _tab_id)| MuxWindow(window_id)))
        });
        methods.add_method("tab", |_, this, _: ()| {
            let mux = get_mux()?;
            Ok(mux
                .resolve_pane_id(this.0)
                .map(|(_domain_id, _window_id, tab_id)| MuxTab(tab_id)))
        });
    }
}

impl MuxTab {
    fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<Tab>> {
        mux.get_tab(self.0)
            .ok_or_else(|| mlua::Error::external(format!("tab id {} not found in mux", self.0)))
    }
}

#[derive(Clone, FromDynamic, ToDynamic)]
struct MuxPaneInfo {
    /// The topological pane index that can be used to reference this pane
    pub index: usize,
    /// true if this is the active pane at the time the position was computed
    pub is_active: bool,
    /// true if this pane is zoomed
    pub is_zoomed: bool,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub left: usize,
    /// The offset from the top left corner of the containing tab to the top
    /// left corner of this pane, in cells.
    pub top: usize,
    /// The width of this pane in cells
    pub width: usize,
    pub pixel_width: usize,
    /// The height of this pane in cells
    pub height: usize,
    pub pixel_height: usize,
}
impl_lua_conversion_dynamic!(MuxPaneInfo);

impl UserData for MuxTab {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("tab_id", |_, this, _: ()| Ok(this.0));
        methods.add_method("window", |_, this, _: ()| {
            let mux = get_mux()?;
            for window_id in mux.iter_windows() {
                if let Some(window) = mux.get_window(window_id) {
                    for tab in window.iter() {
                        if tab.tab_id() == this.0 {
                            return Ok(Some(MuxWindow(window_id)));
                        }
                    }
                }
            }
            Ok(None)
        });
        methods.add_method("get_title", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab.get_title().to_string())
        });
        methods.add_method("set_title", |_, this, title: String| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab.set_title(&title))
        });
        methods.add_method("panes", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab
                .iter_panes()
                .into_iter()
                .map(|info| MuxPane(info.pane.pane_id()))
                .collect::<Vec<MuxPane>>())
        });
        methods.add_method("panes_with_info", |lua, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;

            let result = lua.create_table()?;
            for (idx, pos) in tab.iter_panes().into_iter().enumerate() {
                let info = MuxPaneInfo {
                    index: pos.index,
                    is_active: pos.is_active,
                    is_zoomed: pos.is_zoomed,
                    left: pos.left,
                    top: pos.top,
                    width: pos.width,
                    pixel_width: pos.pixel_width,
                    height: pos.height,
                    pixel_height: pos.pixel_height,
                };
                let info = luahelper::dynamic_to_lua_value(lua, info.to_dynamic())?;
                match &info {
                    LuaValue::Table(t) => {
                        t.set("pane", MuxPane(pos.pane.pane_id()))?;
                    }
                    _ => {}
                }
                result.set(idx + 1, info)?;
            }

            Ok(result)
        });
    }
}
