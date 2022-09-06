//! PaneObject represents a Mux Pane instance in lua code
use super::luaerr;
use anyhow::anyhow;
use luahelper::dynamic_to_lua_value;
use mlua::{UserData, UserDataMethods};
use mux::pane::{Pane, PaneId};
use mux::Mux;
use std::rc::Rc;

#[derive(Clone)]
pub struct PaneObject {
    pub pane: PaneId,
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
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!(
                "GuiPane(pane_id:{}, pid:{})",
                this.pane()?.pane_id(),
                unsafe { libc::getpid() }
            ))
        });
        methods.add_method("pane_id", |_, this, _: ()| Ok(this.pane()?.pane_id()));
        methods.add_method("mux_pane", |_, this, _: ()| {
            Ok(mux_lua::MuxPane(this.pane()?.pane_id()))
        });
        methods.add_method("get_title", |_, this, _: ()| Ok(this.pane()?.get_title()));
        methods.add_method("get_current_working_dir", |_, this, _: ()| {
            Ok(this
                .pane()?
                .get_current_working_dir()
                .map(|u| u.to_string()))
        });
        methods.add_method("get_metadata", |lua, this, _: ()| {
            let value = this.pane()?.get_metadata();
            dynamic_to_lua_value(lua, value)
        });
        methods.add_method("get_foreground_process_name", |_, this, _: ()| {
            Ok(this.pane()?.get_foreground_process_name())
        });
        methods.add_method("get_foreground_process_info", |_, this, _: ()| {
            Ok(this.pane()?.get_foreground_process_info())
        });
        methods.add_method("paste", |_, this, text: String| {
            this.pane()?.send_paste(&text).map_err(luaerr)?;
            Ok(())
        });
        methods.add_method("get_cursor_position", |_, this, _: ()| {
            Ok(this.pane()?.get_cursor_position())
        });
        methods.add_method("get_dimensions", |_, this, _: ()| {
            Ok(this.pane()?.get_dimensions())
        });
        methods.add_method("get_user_vars", |_, this, _: ()| {
            Ok(this.pane()?.copy_user_vars())
        });
        methods.add_method("has_unseen_output", |_, this, _: ()| {
            Ok(this.pane()?.has_unseen_output())
        });
        methods.add_method("is_alt_screen_active", |_, this, _: ()| {
            Ok(this.pane()?.is_alt_screen_active())
        });

        // When called with no arguments, returns the lines from the
        // viewport as plain text (no escape sequences).
        // When called with an optional integer argument, returns the
        // last nlines lines of the terminal output.
        // The returned string will have trailing whitespace trimmed.
        methods.add_method("get_lines_as_text", |_, this, nlines: Option<usize>| {
            let pane = this.pane()?;
            let dims = pane.get_dimensions();
            let nlines = nlines.unwrap_or(dims.viewport_rows);
            let bottom_row = dims.physical_top + dims.viewport_rows as isize;
            let top_row = bottom_row.saturating_sub(nlines as isize);
            let (_first_row, lines) = pane.get_lines(top_row..bottom_row);
            let mut text = String::new();
            for line in lines {
                for cell in line.visible_cells() {
                    text.push_str(cell.str());
                }
                let trimmed = text.trim_end().len();
                text.truncate(trimmed);
                text.push('\n');
            }
            let trimmed = text.trim_end().len();
            text.truncate(trimmed);
            Ok(text)
        });

        methods.add_method(
            "get_logical_lines_as_text",
            |_, this, nlines: Option<usize>| {
                let pane = this.pane()?;
                let dims = pane.get_dimensions();
                let nlines = nlines.unwrap_or(dims.viewport_rows);
                let bottom_row = dims.physical_top + dims.viewport_rows as isize;
                let top_row = bottom_row.saturating_sub(nlines as isize);
                let lines = pane.get_logical_lines(top_row..bottom_row);
                let mut text = String::new();
                for line in lines {
                    for cell in line.logical.visible_cells() {
                        text.push_str(cell.str());
                    }
                    let trimmed = text.trim_end().len();
                    text.truncate(trimmed);
                    text.push('\n');
                }
                let trimmed = text.trim_end().len();
                text.truncate(trimmed);
                Ok(text)
            },
        );

        methods.add_method("get_domain_name", |_, this, _: ()| {
            let pane = this.pane()?;
            let mut name = None;
            if let Some(mux) = Mux::get() {
                let domain_id = pane.domain_id();
                name = mux
                    .get_domain(domain_id)
                    .map(|dom| dom.domain_name().to_string());
            }
            match name {
                Some(name) => Ok(name),
                None => Ok("".to_string()),
            }
        });
    }
}
