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
            Ok(this.pane()?.get_cursor_position())
        });
        methods.add_method("get_dimensions", |_, this, _: ()| {
            Ok(this.pane()?.get_dimensions())
        });
        methods.add_method("get_user_vars", |_, this, _: ()| {
            Ok(this.pane()?.copy_user_vars())
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
                for (_, cell) in line.visible_cells() {
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
    }
}
