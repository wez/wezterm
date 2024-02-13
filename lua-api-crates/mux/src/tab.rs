use config::keyassignment::{PaneDirection, RotationDirection};

use super::*;
use luahelper::mlua::Value;
use luahelper::{from_lua, to_lua};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct MuxTab(pub TabId);

impl MuxTab {
    pub fn resolve<'a>(&self, mux: &'a Arc<Mux>) -> mlua::Result<Arc<Tab>> {
        mux.get_tab(self.0)
            .ok_or_else(|| mlua::Error::external(format!("tab id {} not found in mux", self.0)))
    }
}

impl UserData for MuxTab {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("MuxTab(tab_id:{}, pid:{})", this.0, unsafe {
                libc::getpid()
            }))
        });
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
            Ok(tab.get_title())
        });
        methods.add_method("set_title", |_, this, title: String| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab.set_title(&title))
        });
        methods.add_method("active_pane", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab.get_active_pane().map(|pane| MuxPane(pane.pane_id())))
        });
        methods.add_method("panes", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            Ok(tab
                .iter_panes_ignoring_zoom()
                .into_iter()
                .map(|info| MuxPane(info.pane.pane_id()))
                .collect::<Vec<MuxPane>>())
        });

        methods.add_method("get_pane_direction", |_, this, direction: Value| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            let panes = tab.iter_panes_ignoring_zoom();

            let dir: PaneDirection = from_lua(direction)?;
            let pane = tab
                .get_pane_direction(dir, true)
                .map(|pane_index| MuxPane(panes[pane_index].pane.pane_id()));
            Ok(pane)
        });

        methods.add_method("set_zoomed", |_, this, zoomed: bool| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            let was_zoomed = tab.set_zoomed(zoomed);
            Ok(was_zoomed)
        });

        methods.add_method("panes_with_info", |lua, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;

            let result = lua.create_table()?;
            for (idx, pos) in tab.iter_panes_ignoring_zoom().into_iter().enumerate() {
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

        methods.add_method("rotate_counter_clockwise", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;

            let tab_id = tab.tab_id();
            let direction = RotationDirection::CounterClockwise;
            promise::spawn::spawn(async move {
                let mux = Mux::get();
                if let Err(err) = mux.rotate_panes(tab_id, direction).await {
                    log::error!("Unable to rotate panes: {:#}", err);
                }
            })
            .detach();

            Ok(())
        });

        methods.add_method("rotate_clockwise", |_, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;

            let tab_id = tab.tab_id();
            let direction = RotationDirection::CounterClockwise;
            promise::spawn::spawn(async move {
                let mux = Mux::get();
                if let Err(err) = mux.rotate_panes(tab_id, direction).await {
                    log::error!("Unable to rotate panes: {:#}", err);
                }
            })
            .detach();

            Ok(())
        });

        methods.add_method("get_size", |lua, this, _: ()| {
            let mux = get_mux()?;
            let tab = this.resolve(&mux)?;
            to_lua(lua, tab.get_size())
        });

        methods.add_method("activate", move |_lua, this, ()| {
            let mux = Mux::get();
            let tab = this.resolve(&mux)?;

            let pane = tab.get_active_pane().ok_or_else(|| {
                mlua::Error::external(format!("tab {} has no active pane!?", this.0))
            })?;

            let (_domain_id, window_id, tab_id) =
                mux.resolve_pane_id(pane.pane_id()).ok_or_else(|| {
                    mlua::Error::external(format!("pane {} not found", pane.pane_id()))
                })?;
            {
                let mut window = mux.get_window_mut(window_id).ok_or_else(|| {
                    mlua::Error::external(format!("window {window_id} not found"))
                })?;
                let tab_idx = window.idx_by_id(tab_id).ok_or_else(|| {
                    mlua::Error::external(format!(
                        "tab {tab_id} isn't really in window {window_id}!?"
                    ))
                })?;
                window.save_and_then_set_active(tab_idx);
            }
            Ok(())
        });
    }
}
