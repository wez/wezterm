use crate::commands::{CommandDef, ExpandedCommand};
use crate::overlay::selector::{matcher_pattern, matcher_score};
use crate::termwindow::box_model::*;
use crate::termwindow::modal::Modal;
use crate::termwindow::render::corners::{
    BOTTOM_LEFT_ROUNDED_CORNER, BOTTOM_RIGHT_ROUNDED_CORNER, TOP_LEFT_ROUNDED_CORNER,
    TOP_RIGHT_ROUNDED_CORNER,
};
use crate::termwindow::{DimensionContext, GuiWin, TermWindow};
use crate::utilsprites::RenderMetrics;
use config::keyassignment::KeyAssignment;
use config::Dimension;
use frecency::Frecency;
use luahelper::{from_lua_value_dynamic, impl_lua_conversion_dynamic};
use mux_lua::MuxPane;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use termwiz::nerdfonts::NERD_FONTS;
use wezterm_dynamic::{FromDynamic, ToDynamic};
use wezterm_term::{KeyCode, KeyModifiers, MouseEvent};
use window::color::LinearRgba;
use window::Modifiers;

struct MatchResults {
    selection: String,
    matches: Vec<usize>,
}

pub struct CommandPalette {
    element: RefCell<Option<Vec<ComputedElement>>>,
    selection: RefCell<String>,
    matches: RefCell<Option<MatchResults>>,
    selected_row: RefCell<usize>,
    top_row: RefCell<usize>,
    max_rows_on_screen: RefCell<usize>,
    commands: Vec<ExpandedCommand>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Recent {
    brief: String,
    frecency: Frecency,
}

fn recent_file_name() -> PathBuf {
    config::DATA_DIR.join("recent-commands.json")
}

fn load_recents() -> anyhow::Result<Vec<Recent>> {
    let file_name = recent_file_name();
    let f = std::fs::File::open(&file_name)?;
    let mut recents: Vec<Recent> = serde_json::from_reader(f)?;
    recents.sort_by(|a, b| b.frecency.score().partial_cmp(&a.frecency.score()).unwrap());
    Ok(recents)
}

fn save_recent(command: &ExpandedCommand) -> anyhow::Result<()> {
    let mut recents = load_recents().unwrap_or_else(|_| vec![]);
    if let Some(recent_idx) = recents.iter().position(|r| r.brief == command.brief) {
        let recent = recents.get_mut(recent_idx).unwrap();
        recent.frecency.register_access();
    } else {
        let mut frecency = Frecency::new();
        frecency.register_access();
        recents.push(Recent {
            brief: command.brief.to_string(),
            frecency,
        });
    }

    let json = serde_json::to_string(&recents)?;
    let file_name = recent_file_name();
    std::fs::write(&file_name, json)?;
    Ok(())
}

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct UserPaletteEntry {
    pub brief: String,
    pub doc: Option<String>,
    pub action: KeyAssignment,
    pub icon: Option<String>,
}
impl_lua_conversion_dynamic!(UserPaletteEntry);

fn build_commands(
    gui_window: GuiWin,
    pane: Option<MuxPane>,
    filter_copy_mode: bool,
) -> Vec<ExpandedCommand> {
    let mut commands = CommandDef::actions_for_palette_and_menubar(&config::configuration());

    match config::run_immediate_with_lua_config(|lua| {
        let mut entries: Vec<UserPaletteEntry> = vec![];

        if let Some(lua) = lua {
            let result = config::lua::emit_sync_callback(
                &*lua,
                ("augment-command-palette".to_string(), (gui_window, pane)),
            )?;

            if !matches!(&result, mlua::Value::Nil) {
                entries = from_lua_value_dynamic(result)?;
            }
        }

        Ok(entries)
    }) {
        Ok(entries) => {
            for entry in entries {
                commands.push(ExpandedCommand {
                    brief: entry.brief.into(),
                    doc: match entry.doc {
                        Some(doc) => doc.into(),
                        None => "".into(),
                    },
                    action: entry.action,
                    keys: vec![],
                    menubar: &[],
                    icon: entry.icon.map(Cow::Owned),
                });
            }
        }
        Err(err) => {
            log::warn!("augment-command-palette: {err:#}");
        }
    }

    commands.retain(|cmd| {
        if filter_copy_mode {
            !matches!(cmd.action, KeyAssignment::CopyMode(_))
        } else {
            true
        }
    });

    let mut scores: HashMap<&str, f64> = HashMap::new();
    let recents = load_recents();
    if let Ok(recents) = &recents {
        for r in recents {
            scores.insert(&r.brief, r.frecency.score());
        }
    }

    commands.sort_by(|a, b| {
        match (scores.get(&*a.brief), scores.get(&*b.brief)) {
            // Want descending frecency score, so swap a<->b
            // for the compare here
            (Some(a), Some(b)) => match b.partial_cmp(a) {
                Some(Ordering::Equal) | None => {}
                Some(ordering) => return ordering,
            },
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }

        match a.menubar.cmp(&b.menubar) {
            Ordering::Equal => a.brief.cmp(&b.brief),
            ordering => ordering,
        }
    });

    commands
}

#[derive(Debug)]
struct MatchResult {
    row_idx: usize,
    score: u32,
}

impl MatchResult {
    fn new(row_idx: usize, score: u32, selection: &str, commands: &[ExpandedCommand]) -> Self {
        Self {
            row_idx,
            score: if commands[row_idx].brief == selection {
                // Pump up the score for an exact match, otherwise
                // the order may be undesirable if there are a lot
                // of candidates with the same score
                u32::max_value()
            } else {
                score
            },
        }
    }
}

fn compute_matches(selection: &str, commands: &[ExpandedCommand]) -> Vec<usize> {
    if selection.is_empty() {
        commands.iter().enumerate().map(|(idx, _)| idx).collect()
    } else {
        let pattern = matcher_pattern(selection);

        let start = std::time::Instant::now();
        let mut scores: Vec<MatchResult> = commands
            .par_iter()
            .enumerate()
            .filter_map(|(row_idx, entry)| {
                let group = entry.menubar.join(" ");
                let text = format!("{group}: {}. {} {:?}", entry.brief, entry.doc, entry.action);
                matcher_score(&pattern, &text)
                    .map(|score| MatchResult::new(row_idx, score, selection, commands))
            })
            .collect();
        scores.sort_by(|a, b| a.score.cmp(&b.score).reverse());
        log::trace!("matching took {:?}", start.elapsed());

        scores.iter().map(|result| result.row_idx).collect()
    }
}

impl CommandPalette {
    pub fn new(term_window: &mut TermWindow) -> Self {
        // Showing the CopyMode actions in the palette is useless
        // if the CopyOverlay isn't active, so figure out if that
        // is the case so that we can filter them out in build_commands.
        let filter_copy_mode = term_window
            .get_active_pane_or_overlay()
            .map(|pane| {
                pane.downcast_ref::<crate::termwindow::CopyOverlay>()
                    .is_none()
            })
            .unwrap_or(true);

        let mux_pane = term_window
            .get_active_pane_or_overlay()
            .map(|pane| MuxPane(pane.pane_id()));

        let commands = build_commands(GuiWin::new(term_window), mux_pane, filter_copy_mode);

        Self {
            element: RefCell::new(None),
            selection: RefCell::new(String::new()),
            commands,
            matches: RefCell::new(None),
            selected_row: RefCell::new(0),
            top_row: RefCell::new(0),
            max_rows_on_screen: RefCell::new(0),
        }
    }

    fn compute(
        term_window: &mut TermWindow,
        selection: &str,
        commands: &[ExpandedCommand],
        matches: &MatchResults,
        max_rows_on_screen: usize,
        selected_row: usize,
        top_row: usize,
    ) -> anyhow::Result<Vec<ComputedElement>> {
        let font = term_window
            .fonts
            .command_palette_font()
            .expect("to resolve command palette font");
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());

        let top_bar_height = if term_window.show_tab_bar && !term_window.config.tab_bar_at_bottom {
            term_window.tab_bar_pixel_height().unwrap()
        } else {
            0.
        };
        let (padding_left, padding_top) = term_window.padding_left_top();
        let border = term_window.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        let mut elements =
            vec![
                Element::new(&font, ElementContent::Text(format!("> {selection}_")))
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg: LinearRgba::TRANSPARENT.into(),
                        text: term_window
                            .config
                            .command_palette_fg_color
                            .to_linear()
                            .into(),
                    })
                    .display(DisplayType::Block),
            ];

        for (display_idx, command) in matches
            .matches
            .iter()
            .map(|&idx| &commands[idx])
            .enumerate()
            .skip(top_row)
            .take(max_rows_on_screen)
        {
            let group = if command.menubar.is_empty() {
                String::new()
            } else {
                format!("{}: ", command.menubar.join(" | "))
            };

            let icon = match &command.icon {
                Some(nf) => NERD_FONTS.get(nf.as_ref()).unwrap_or_else(|| {
                    log::error!("nerdfont {nf} not found in NERD_FONTS");
                    &'?'
                }),
                None => &' ',
            };

            let solid_bg_color: InheritableColor = term_window
                .config
                .command_palette_bg_color
                .to_linear()
                .into();
            let solid_fg_color: InheritableColor = term_window
                .config
                .command_palette_fg_color
                .to_linear()
                .into();

            let (bg, text) = if display_idx == selected_row {
                (solid_fg_color.clone(), solid_bg_color.clone())
            } else {
                (LinearRgba::TRANSPARENT.into(), solid_fg_color.clone())
            };

            let (label_bg, label_text) = if display_idx == selected_row {
                (solid_fg_color.clone(), solid_bg_color.clone())
            } else {
                (solid_bg_color.clone(), solid_fg_color.clone())
            };

            // DRY if the brief and doc are the same
            let label = if command.doc.is_empty()
                || command.brief.to_ascii_lowercase() == command.doc.to_ascii_lowercase()
            {
                format!("{group}{}", command.brief)
            } else {
                format!("{group}{}. {}", command.brief, command.doc)
            };

            let mut row = vec![
                Element::new(&font, ElementContent::Text(icon.to_string()))
                    .min_width(Some(Dimension::Cells(2.))),
                Element::new(&font, ElementContent::Text(label)),
            ];

            if !command.keys.is_empty() {
                let mut keys = command.keys.clone();

                keys.sort_by(|(a_mods, a_key), (b_mods, b_key)| {
                    fn score_mods(mods: &Modifiers) -> usize {
                        let mut score: usize = mods.bits() as usize;
                        // Prefer keys with CMD on macOS, but not on other systems,
                        // where CMD tends to be reserved by the desktop environment
                        if cfg!(target_os = "macos") && mods.contains(Modifiers::SUPER) {
                            score += 1000;
                        } else if !cfg!(target_os = "macos") && !mods.contains(Modifiers::SUPER) {
                            score += 1000;
                        }
                        score
                    }

                    let a_mods = score_mods(a_mods);
                    let b_mods = score_mods(b_mods);

                    match b_mods.cmp(&a_mods) {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    }

                    a_key.cmp(&b_key)
                });

                let separator = if term_window.config.ui_key_cap_rendering
                    == ::window::UIKeyCapRendering::AppleSymbols
                {
                    " "
                } else {
                    "-"
                };

                let mut keys = keys
                    .into_iter()
                    .map(|(mods, keycode)| {
                        let mut mod_string =
                            mods.to_string_with_separator(::window::ModifierToStringArgs {
                                separator,
                                want_none: false,
                                ui_key_cap_rendering: Some(term_window.config.ui_key_cap_rendering),
                            });
                        if !mod_string.is_empty() {
                            mod_string.push_str(separator);
                        }
                        let keycode = crate::inputmap::ui_key(
                            &keycode,
                            term_window.config.ui_key_cap_rendering,
                        );
                        format!("{mod_string}{keycode}")
                    })
                    .collect::<Vec<_>>();

                keys.dedup();
                keys.truncate(term_window.config.palette_max_key_assigments_for_action);

                let key_label = keys.join(", ");

                row.push(
                    Element::new(&font, ElementContent::Text(key_label))
                        .float(Float::Right)
                        .padding(BoxDimension {
                            left: Dimension::Cells(1.25),
                            right: Dimension::Cells(0.5),
                            top: Dimension::Cells(0.),
                            bottom: Dimension::Cells(0.),
                        })
                        .zindex(10)
                        .colors(ElementColors {
                            border: BorderColor::default(),
                            bg: label_bg.clone(),
                            text: label_text.clone(),
                        }),
                );
            }

            elements.push(
                Element::new(&font, ElementContent::Children(row))
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg,
                        text,
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.25),
                        right: Dimension::Cells(0.25),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .min_width(Some(Dimension::Percent(1.)))
                    .display(DisplayType::Block),
            );
        }

        let dimensions = term_window.dimensions;
        let size = term_window.terminal_size;

        // Avoid covering the entire width
        let desired_width = (size.cols / 3).max(120).min(size.cols);

        // Center it
        let avail_pixel_width =
            size.cols as f32 * term_window.render_metrics.cell_size.width as f32;
        let desired_pixel_width =
            desired_width as f32 * term_window.render_metrics.cell_size.width as f32;

        let element = Element::new(&font, ElementContent::Children(elements))
            .colors(ElementColors {
                border: BorderColor::new(
                    term_window
                        .config
                        .command_palette_bg_color
                        .to_linear()
                        .into(),
                ),
                bg: term_window
                    .config
                    .command_palette_bg_color
                    .to_linear()
                    .into(),
                text: term_window
                    .config
                    .command_palette_fg_color
                    .to_linear()
                    .into(),
            })
            .margin(BoxDimension {
                left: Dimension::Cells(0.25),
                right: Dimension::Cells(0.25),
                top: Dimension::Cells(0.25),
                bottom: Dimension::Cells(0.25),
            })
            .padding(BoxDimension {
                left: Dimension::Cells(0.25),
                right: Dimension::Cells(0.25),
                top: Dimension::Cells(0.25),
                bottom: Dimension::Cells(0.25),
            })
            .border(BoxDimension::new(Dimension::Pixels(1.)))
            .border_corners(Some(Corners {
                top_left: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: TOP_LEFT_ROUNDED_CORNER,
                },
                top_right: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: TOP_RIGHT_ROUNDED_CORNER,
                },
                bottom_left: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: BOTTOM_LEFT_ROUNDED_CORNER,
                },
                bottom_right: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: BOTTOM_RIGHT_ROUNDED_CORNER,
                },
            }))
            .min_width(Some(Dimension::Pixels(desired_pixel_width)));

        let x_adjust = ((avail_pixel_width - padding_left) - desired_pixel_width) / 2.;

        let computed = term_window.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    padding_left + x_adjust,
                    top_pixel_y,
                    desired_pixel_width,
                    size.rows as f32 * term_window.render_metrics.cell_size.height as f32,
                ),
                metrics: &metrics,
                gl_state: term_window.render_state.as_ref().unwrap(),
                zindex: 100,
            },
            &element,
        )?;

        Ok(vec![computed])
    }

    fn updated_input(&self) {
        *self.selected_row.borrow_mut() = 0;
        *self.top_row.borrow_mut() = 0;
    }

    fn move_up(&self) {
        let mut row = self.selected_row.borrow_mut();
        *row = row.saturating_sub(1);

        let mut top_row = self.top_row.borrow_mut();
        if *row < *top_row {
            *top_row = *row;
        }
    }

    fn move_down(&self) {
        let max_rows_on_screen = *self.max_rows_on_screen.borrow();
        let limit = self
            .matches
            .borrow()
            .as_ref()
            .map(|m| m.matches.len())
            .unwrap_or_else(|| self.commands.len())
            .saturating_sub(1);
        let mut row = self.selected_row.borrow_mut();
        *row = row.saturating_add(1).min(limit);
        let mut top_row = self.top_row.borrow_mut();
        if *row > *top_row + max_rows_on_screen - 1 {
            *top_row = row.saturating_sub(max_rows_on_screen - 1);
        }
    }
}

impl Modal for CommandPalette {
    fn perform_assignment(
        &self,
        _assignment: &KeyAssignment,
        _term_window: &mut TermWindow,
    ) -> bool {
        false
    }

    fn mouse_event(&self, _event: MouseEvent, _term_window: &mut TermWindow) -> anyhow::Result<()> {
        Ok(())
    }

    fn key_down(
        &self,
        key: KeyCode,
        mods: KeyModifiers,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<bool> {
        match (key, mods) {
            (KeyCode::Escape, KeyModifiers::NONE) | (KeyCode::Char('g'), KeyModifiers::CTRL) => {
                term_window.cancel_modal();
            }
            (KeyCode::UpArrow, KeyModifiers::NONE) | (KeyCode::Char('p'), KeyModifiers::CTRL) => {
                self.move_up();
            }
            (KeyCode::DownArrow, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CTRL) => {
                self.move_down();
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                // Type to add to the selection
                let mut selection = self.selection.borrow_mut();
                selection.push(c);
                self.updated_input();
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                // Backspace to edit the selection
                let mut selection = self.selection.borrow_mut();
                selection.pop();
                self.updated_input();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                // CTRL-u to clear the selection
                let mut selection = self.selection.borrow_mut();
                selection.clear();
                self.updated_input();
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                // Enter the selected character to the current pane
                let selected_idx = *self.selected_row.borrow();
                let alias_idx = match self.matches.borrow().as_ref() {
                    None => return Ok(true),
                    Some(results) => match results.matches.get(selected_idx) {
                        Some(i) => *i,
                        None => return Ok(true),
                    },
                };
                let item = &self.commands[alias_idx];
                if let Err(err) = save_recent(item) {
                    log::error!("Error while saving recents: {err:#}");
                }
                term_window.cancel_modal();

                if let Some(pane) = term_window.get_active_pane_or_overlay() {
                    if let Err(err) = term_window.perform_key_assignment(&pane, &item.action) {
                        log::error!("Error while performing {item:?}: {err:#}");
                    }
                }
                return Ok(true);
            }
            _ => return Ok(false),
        }
        term_window.invalidate_modal();
        Ok(true)
    }

    fn computed_element(
        &self,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<Ref<[ComputedElement]>> {
        let selection = self.selection.borrow();
        let selection = selection.as_str();

        let mut results = self.matches.borrow_mut();

        let font = term_window
            .fonts
            .command_palette_font()
            .expect("to resolve char selection font");
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());

        let mut max_rows_on_screen = ((term_window.dimensions.pixel_height * 8 / 10)
            / metrics.cell_size.height as usize)
            - 2;
        if let Some(size) = term_window.config.command_palette_rows {
            max_rows_on_screen = max_rows_on_screen.min(size);
        }
        *self.max_rows_on_screen.borrow_mut() = max_rows_on_screen;

        let rebuild_matches = results
            .as_ref()
            .map(|m| m.selection != selection)
            .unwrap_or(true);
        if rebuild_matches {
            results.replace(MatchResults {
                selection: selection.to_string(),
                matches: compute_matches(selection, &self.commands),
            });
        };
        let matches = results.as_ref().unwrap();

        if self.element.borrow().is_none() {
            let element = Self::compute(
                term_window,
                selection,
                &self.commands,
                matches,
                max_rows_on_screen,
                *self.selected_row.borrow(),
                *self.top_row.borrow(),
            )?;
            self.element.borrow_mut().replace(element);
        }
        Ok(Ref::map(self.element.borrow(), |v| {
            v.as_ref().unwrap().as_slice()
        }))
    }

    fn reconfigure(&self, _term_window: &mut TermWindow) {
        self.element.borrow_mut().take();
    }
}
