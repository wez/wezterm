use super::*;
use luahelper::dynamic_to_lua_value;

#[derive(Clone, Copy, Debug)]
pub struct MuxPane(pub PaneId);

impl MuxPane {
    pub fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<dyn Pane>> {
        mux.get_pane(self.0)
            .ok_or_else(|| mlua::Error::external(format!("pane id {} not found in mux", self.0)))
    }
}

impl UserData for MuxPane {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("MuxPane(pane_id:{}, pid:{})", this.0, unsafe {
                libc::getpid()
            }))
        });
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

        // An alias of send-paste for backwards compatibility with prior releases when there was a
        // separate Gui-level PaneObject
        methods.add_method("paste", |_, this, text: String| {
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

        // For backwards compatibility with prior releases when there
        // was a separate Gui-level PaneObject
        methods.add_method("mux_pane", |_, this, _: ()| Ok(*this));

        methods.add_method("get_title", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_title())
        });

        methods.add_method("get_current_working_dir", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_current_working_dir().map(|u| u.to_string()))
        });

        methods.add_method("get_metadata", |lua, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            let value = pane.get_metadata();
            dynamic_to_lua_value(lua, value)
        });

        methods.add_method("get_foreground_process_name", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_foreground_process_name())
        });

        methods.add_method("get_foreground_process_info", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_foreground_process_info())
        });

        methods.add_method("get_cursor_position", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_cursor_position())
        });

        methods.add_method("get_dimensions", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_dimensions())
        });

        methods.add_method("get_user_vars", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.copy_user_vars())
        });

        methods.add_method("has_unseen_output", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.has_unseen_output())
        });

        methods.add_method("is_alt_screen_active", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.is_alt_screen_active())
        });

        // When called with no arguments, returns the lines from the
        // viewport as plain text (no escape sequences).
        // When called with an optional integer argument, returns the
        // last nlines lines of the terminal output.
        // The returned string will have trailing whitespace trimmed.
        methods.add_method("get_lines_as_text", |_, this, nlines: Option<usize>| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
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
                let mux = get_mux()?;
                let pane = this.resolve(&mux)?;
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
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
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

        methods.add_method("inject_output", |_, this, text: String| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;

            let mut parser = termwiz::escape::parser::Parser::new();
            let mut actions = vec![];
            parser.parse(text.as_bytes(), |action| actions.push(action));

            pane.perform_actions(actions);
            Ok(())
        });
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
