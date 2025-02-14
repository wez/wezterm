use super::*;
use luahelper::mlua::LuaSerdeExt;
use luahelper::{dynamic_to_lua_value, from_lua, to_lua};
use mlua::Value;
use mux::pane::CachePolicy;
use std::cmp::Ordering;
use std::sync::Arc;
use termwiz::cell::SemanticType;
use termwiz_funcs::lines_to_escapes;
use url_funcs::Url;
use wezterm_term::{SemanticZone, StableRowIndex};

#[derive(Clone, Copy, Debug)]
pub struct MuxPane(pub PaneId);

impl MuxPane {
    pub fn resolve<'a>(&self, mux: &'a Arc<Mux>) -> mlua::Result<Arc<dyn Pane>> {
        mux.get_pane(self.0)
            .ok_or_else(|| mlua::Error::external(format!("pane id {} not found in mux", self.0)))
    }

    fn get_text_from_semantic_zone(&self, zone: SemanticZone) -> mlua::Result<String> {
        let mux = get_mux()?;
        let pane = self.resolve(&mux)?;

        let mut last_was_wrapped = false;
        let first_row = zone.start_y;
        let last_row = zone.end_y;

        fn cols_for_row(zone: &SemanticZone, row: StableRowIndex) -> std::ops::Range<usize> {
            if row < zone.start_y || row > zone.end_y {
                0..0
            } else if zone.start_y == zone.end_y {
                // A single line zone
                if zone.start_x <= zone.end_x {
                    zone.start_x..zone.end_x.saturating_add(1)
                } else {
                    zone.end_x..zone.start_x.saturating_add(1)
                }
            } else if row == zone.end_y {
                // last line of multi-line
                0..zone.end_x.saturating_add(1)
            } else if row == zone.start_y {
                // first line of multi-line
                zone.start_x..usize::max_value()
            } else {
                // some "middle" line of multi-line
                0..usize::max_value()
            }
        }

        let mut s = String::new();
        for line in pane.get_logical_lines(zone.start_y..zone.end_y + 1) {
            if !s.is_empty() && !last_was_wrapped {
                s.push('\n');
            }
            let last_idx = line.physical_lines.len().saturating_sub(1);
            for (idx, phys) in line.physical_lines.iter().enumerate() {
                let this_row = line.first_row + idx as StableRowIndex;
                if this_row >= first_row && this_row <= last_row {
                    let last_phys_idx = phys.len().saturating_sub(1);

                    let cols = cols_for_row(&zone, this_row);
                    let last_col_idx = cols.end.saturating_sub(1).min(last_phys_idx);
                    let col_span = phys.columns_as_str(cols);
                    // Only trim trailing whitespace if we are the last line
                    // in a wrapped sequence
                    if idx == last_idx {
                        s.push_str(col_span.trim_end());
                    } else {
                        s.push_str(&col_span);
                    }

                    last_was_wrapped = last_col_idx == last_phys_idx
                        && phys
                            .get_cell(last_col_idx)
                            .map(|c| c.attrs().wrapped())
                            .unwrap_or(false);
                }
            }
        }

        Ok(s)
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
            args.unwrap_or_default().run(this).await
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

        methods.add_method("get_progress", |lua, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            let progress = pane.get_progress();
            lua.to_value(&progress)
        });

        methods.add_method("get_current_working_dir", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane
                .get_current_working_dir(CachePolicy::FetchImmediate)
                .map(|url| Url { url }))
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
            Ok(pane.get_foreground_process_name(CachePolicy::FetchImmediate))
        });

        methods.add_method("get_foreground_process_info", |_, this, _: ()| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            Ok(pane.get_foreground_process_info(CachePolicy::AllowStale))
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

        methods.add_method("get_lines_as_escapes", |_, this, nlines: Option<usize>| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            let dims = pane.get_dimensions();
            let nlines = nlines.unwrap_or(dims.viewport_rows);
            let bottom_row = dims.physical_top + dims.viewport_rows as isize;
            let top_row = bottom_row.saturating_sub(nlines as isize);
            let (_first_row, lines) = pane.get_lines(top_row..bottom_row);
            let text = lines_to_escapes(lines).map_err(mlua::Error::external)?;
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
            if let Some(mux) = Mux::try_get() {
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

        methods.add_method("get_semantic_zones", |lua, this, of_type: Value| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;

            let of_type: Option<SemanticType> = from_lua(of_type)?;

            let mut zones = pane
                .get_semantic_zones()
                .map_err(|e| mlua::Error::external(format!("{:#}", e)))?;

            if let Some(of_type) = of_type {
                zones.retain(|zone| zone.semantic_type == of_type);
            }

            let zones = to_lua(lua, zones)?;
            Ok(zones)
        });

        methods.add_method(
            "get_semantic_zone_at",
            |lua, this, (x, y): (usize, StableRowIndex)| {
                let mux = get_mux()?;
                let pane = this.resolve(&mux)?;

                let zones = pane.get_semantic_zones().unwrap_or_else(|_| vec![]);

                fn find_zone(x: usize, y: StableRowIndex, zone: &SemanticZone) -> Ordering {
                    match zone.start_y.cmp(&y) {
                        Ordering::Greater => return Ordering::Greater,
                        // If the zone starts on the same line then check that the
                        // x position is within bounds
                        Ordering::Equal => match zone.start_x.cmp(&x) {
                            Ordering::Greater => return Ordering::Greater,
                            Ordering::Equal | Ordering::Less => {}
                        },
                        Ordering::Less => {}
                    }
                    match zone.end_y.cmp(&y) {
                        Ordering::Less => Ordering::Less,
                        // If the zone ends on the same line then check that the
                        // x position is within bounds
                        Ordering::Equal => match zone.end_x.cmp(&x) {
                            Ordering::Less => Ordering::Less,
                            Ordering::Equal | Ordering::Greater => Ordering::Equal,
                        },
                        Ordering::Greater => Ordering::Equal,
                    }
                }

                match zones.binary_search_by(|zone| find_zone(x, y, zone)) {
                    Ok(idx) => {
                        let zone = to_lua(lua, zones[idx])?;
                        Ok(Some(zone))
                    }
                    Err(_) => Ok(None),
                }
            },
        );

        methods.add_method("get_text_from_semantic_zone", |_lua, this, zone: Value| {
            let zone: SemanticZone = from_lua(zone)?;
            this.get_text_from_semantic_zone(zone)
        });

        methods.add_method("get_text_from_region", |_lua, this, (start_x, start_y, end_x, end_y): (usize, StableRowIndex, usize, StableRowIndex)| {
            let zone = SemanticZone {
                start_x,
                start_y,
                end_x,
                end_y,
                // semantic_type is not used by get_text_from_semantic_zone
                semantic_type: SemanticType::Output,
            };
            this.get_text_from_semantic_zone(zone)
        });

        methods.add_async_method("move_to_new_tab", |_lua, this, ()| async move {
            let mux = Mux::get();
            let (_domain, window_id, _tab) = mux
                .resolve_pane_id(this.0)
                .ok_or_else(|| mlua::Error::external(format!("pane {} not found", this.0)))?;
            let (tab, window) = mux
                .move_pane_to_new_tab(this.0, Some(window_id), None)
                .await
                .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

            Ok((MuxTab(tab.tab_id()), MuxWindow(window)))
        });

        methods.add_async_method(
            "move_to_new_window",
            |_lua, this, workspace: Option<String>| async move {
                let mux = Mux::get();
                let (tab, window) = mux
                    .move_pane_to_new_tab(this.0, None, workspace)
                    .await
                    .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

                Ok((MuxTab(tab.tab_id()), MuxWindow(window)))
            },
        );

        methods.add_method("activate", move |_lua, this, ()| {
            let mux = Mux::get();
            let pane = this.resolve(&mux)?;
            let (_domain_id, window_id, tab_id) = mux
                .resolve_pane_id(this.0)
                .ok_or_else(|| mlua::Error::external(format!("pane {} not found", this.0)))?;
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
            let tab = mux
                .get_tab(tab_id)
                .ok_or_else(|| mlua::Error::external(format!("tab {tab_id} not found")))?;
            tab.set_active_pane(&pane);
            Ok(())
        });

        methods.add_method("get_tty_name", move |_lua, this, ()| {
            let mux = Mux::get();
            let pane = this.resolve(&mux)?;
            Ok(pane.tty_name())
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
    async fn run(&self, pane: &MuxPane) -> mlua::Result<MuxPane> {
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
            .split_pane(pane.0, request, source, self.domain.clone())
            .await
            .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

        Ok(MuxPane(pane.pane_id()))
    }
}
