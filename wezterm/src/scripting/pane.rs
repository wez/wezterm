//! PaneObject represents a Mux Pane instance in lua code
use super::luaerr;
use anyhow::anyhow;
use mlua::{UserData, UserDataMethods};
use mux::pane::{Pane, PaneId};
use mux::Mux;
use std::rc::Rc;

#[derive(Clone)]
pub struct PaneObject {
    pane: PaneId,
}

impl PaneObject {
    pub fn new(pane: &Rc<dyn Pane>) -> Self {
        Self {
            pane: pane.pane_id(),
        }
    }

    pub fn pane(&self) -> mlua::Result<Rc<dyn Pane>> {
        let mux = Mux::get()
            .ok_or_else(|| anyhow!("must be called on main thread"))
            .map_err(luaerr)?;
        mux.get_pane(self.pane)
            .ok_or_else(|| anyhow!("pane id {} is not valid", self.pane))
            .map_err(luaerr)
    }
}

impl UserData for PaneObject {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("pane_id", |_, this, _: ()| Ok(this.pane()?.pane_id()));
        methods.add_method("get_title", |_, this, _: ()| Ok(this.pane()?.get_title()));
        methods.add_method("get_current_working_dir", |_, this, _: ()| {
            Ok(this
                .pane()?
                .get_current_working_dir()
                .map(|u| u.to_string()))
        });
        methods.add_method("paste", |_, this, text: String| {
            this.pane()?.send_paste(&text).map_err(luaerr)?;
            Ok(())
        });
        methods.add_method("get_cursor_position", |_, this, _: ()| {
            Ok(this.pane()?.renderer().get_cursor_position())
        });
        methods.add_method("get_dimensions", |_, this, _: ()| {
            Ok(this.pane()?.renderer().get_dimensions())
        });
    }
}
