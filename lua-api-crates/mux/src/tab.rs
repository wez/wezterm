use super::*;

#[derive(Clone, Copy, Debug)]
pub struct MuxTab(pub TabId);

impl MuxTab {
    pub fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<Tab>> {
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
                .iter_panes_ignoring_zoom()
                .into_iter()
                .map(|info| MuxPane(info.pane.pane_id()))
                .collect::<Vec<MuxPane>>())
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
    }
}
