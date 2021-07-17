use crate::termwindow::{PaneInformation, TabInformation};
use config::lua::{format_as_escapes, FormatItem};
use config::{ConfigHandle, TabBarColors};
use mlua::FromLua;
use termwiz::cell::unicode_column_width;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::ColorSpec;
use termwiz::escape::csi::Sgr;
use termwiz::escape::parser::Parser;
use termwiz::escape::{Action, ControlCode, CSI};
use wezterm_term::Line;

#[derive(Clone, Debug, PartialEq)]
pub struct TabBarState {
    line: Line,
    items: Vec<TabEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TabBarItem {
    None,
    Tab(usize),
    NewTabButton,
}

#[derive(Clone, Debug, PartialEq)]
struct TabEntry {
    item: TabBarItem,
    x: usize,
    width: usize,
}

#[derive(Clone, Debug)]
struct TitleText {
    items: Vec<FormatItem>,
    len: usize,
}

fn call_format_tab_title(
    tab: &TabInformation,
    tab_info: &[TabInformation],
    pane_info: &[PaneInformation],
    config: &ConfigHandle,
    hover: bool,
    tab_max_width: usize,
) -> Option<TitleText> {
    match config::run_immediate_with_lua_config(|lua| {
        if let Some(lua) = lua {
            let tabs = lua.create_sequence_from(tab_info.iter().cloned())?;
            let panes = lua.create_sequence_from(pane_info.iter().cloned())?;

            let v = config::lua::emit_sync_callback(
                &*lua,
                (
                    "format-tab-title".to_string(),
                    (
                        tab.clone(),
                        tabs,
                        panes,
                        (**config).clone(),
                        hover,
                        tab_max_width,
                    ),
                ),
            )?;
            match &v {
                mlua::Value::Nil => Ok(None),
                mlua::Value::Table(_) => {
                    let items = <Vec<FormatItem>>::from_lua(v, &*lua)?;

                    let esc = format_as_escapes(items.clone())?;
                    let cells = parse_status_text(&esc, CellAttributes::default());

                    Ok(Some(TitleText {
                        items,
                        len: cells.len(),
                    }))
                }
                _ => {
                    let s = String::from_lua(v, &*lua)?;
                    Ok(Some(TitleText {
                        len: unicode_column_width(&s),
                        items: vec![FormatItem::Text(s)],
                    }))
                }
            }
        } else {
            Ok(None)
        }
    }) {
        Ok(s) => s,
        Err(err) => {
            log::warn!("format-tab-title: {}", err);
            None
        }
    }
}

fn compute_tab_title(
    tab: &TabInformation,
    tab_info: &[TabInformation],
    pane_info: &[PaneInformation],
    config: &ConfigHandle,
    hover: bool,
    tab_max_width: usize,
) -> TitleText {
    let title = call_format_tab_title(tab, tab_info, pane_info, config, hover, tab_max_width);

    match title {
        Some(title) => title,
        None => {
            let title = if let Some(pane) = &tab.active_pane {
                let mut title = pane.title.clone();
                if config.show_tab_index_in_tab_bar {
                    title = format!(
                        " {}: {} ",
                        tab.tab_index
                            + if config.tab_and_split_indices_are_zero_based {
                                0
                            } else {
                                1
                            },
                        pane.title
                    );
                }
                // We have a preferred soft minimum on tab width to make it
                // easier to click on tab titles, but we'll still go below
                // this if there are too many tabs to fit the window at
                // this width.
                while unicode_column_width(&title) < 5 {
                    title.push(' ');
                }
                title
            } else {
                " no pane ".to_string()
            };

            TitleText {
                len: unicode_column_width(&title),
                items: vec![FormatItem::Text(title)],
            }
        }
    }
}

fn is_tab_hover(mouse_x: Option<usize>, x: usize, tab_title_len: usize) -> bool {
    return mouse_x
        .map(|mouse_x| mouse_x >= x && mouse_x < x + tab_title_len)
        .unwrap_or(false);
}

impl TabBarState {
    pub fn default() -> Self {
        Self {
            line: Line::with_width(1),
            items: vec![],
        }
    }

    pub fn line(&self) -> &Line {
        &self.line
    }

    /// Build a new tab bar from the current state
    /// mouse_x is some if the mouse is on the same row as the tab bar.
    /// title_width is the total number of cell columns in the window.
    /// window allows access to the tabs associated with the window.
    pub fn new(
        title_width: usize,
        mouse_x: Option<usize>,
        tab_info: &[TabInformation],
        pane_info: &[PaneInformation],
        colors: Option<&TabBarColors>,
        config: &ConfigHandle,
        right_status: &str,
    ) -> Self {
        let colors = colors.cloned().unwrap_or_else(TabBarColors::default);

        let active_cell_attrs = colors.active_tab.as_cell_attributes();
        let inactive_hover_attrs = colors.inactive_tab_hover.as_cell_attributes();
        let inactive_cell_attrs = colors.inactive_tab.as_cell_attributes();
        let new_tab_hover_attrs = colors.new_tab_hover.as_cell_attributes();
        let new_tab_attrs = colors.new_tab.as_cell_attributes();

        let new_tab = parse_status_text(&config.tab_bar_style.new_tab, new_tab_attrs.clone());
        let new_tab_hover = parse_status_text(
            &config.tab_bar_style.new_tab_hover,
            new_tab_hover_attrs.clone(),
        );

        // We ultimately want to produce a line looking like this:
        // ` | tab1-title x | tab2-title x |  +      . - X `
        // Where the `+` sign will spawn a new tab (or show a context
        // menu with tab creation options) and the other three chars
        // are symbols representing minimize, maximize and close.

        let mut active_tab_no = 0;

        let tab_titles: Vec<TitleText> = tab_info
            .iter()
            .map(|tab| {
                if tab.is_active {
                    active_tab_no = tab.tab_index;
                }
                compute_tab_title(
                    tab,
                    tab_info,
                    pane_info,
                    config,
                    false,
                    config.tab_max_width,
                )
            })
            .collect();
        let titles_len: usize = tab_titles.iter().map(|s| s.len).sum();
        let number_of_tabs = tab_titles.len();

        let available_cells =
            title_width.saturating_sub((number_of_tabs.saturating_sub(1)) + (new_tab.len()));
        let tab_width_max = if available_cells >= titles_len {
            // We can render each title with its full width
            usize::max_value()
        } else {
            // We need to clamp the length to balance them out
            available_cells / number_of_tabs
        }
        .min(config.tab_max_width);

        let mut line = Line::with_width(title_width);

        let mut x = 0;
        let mut items = vec![];

        for (tab_idx, tab_title) in tab_titles.iter().enumerate() {
            let tab_title_len = tab_title.len.min(tab_width_max);
            let active = tab_idx == active_tab_no;
            let hover = !active && is_tab_hover(mouse_x, x, tab_title_len);

            // Recompute the title so that it factors in both the hover state
            // and the adjusted maximum tab width based on available space.
            let tab_title = compute_tab_title(
                &tab_info[tab_idx],
                tab_info,
                pane_info,
                config,
                hover,
                tab_title_len,
            );

            let cell_attrs = if active {
                &active_cell_attrs
            } else if hover {
                &inactive_hover_attrs
            } else {
                &inactive_cell_attrs
            };

            let tab_start_idx = x;

            let esc = format_as_escapes(tab_title.items.clone()).expect("already parsed ok above");
            let cells = parse_status_text(&esc, cell_attrs.clone());
            let mut n = 0;
            for cell in cells {
                let len = cell.width();
                if n + len > tab_width_max {
                    break;
                }
                line.set_cell(x, cell);
                x += len;
                n += len;
            }

            items.push(TabEntry {
                item: TabBarItem::Tab(tab_idx),
                x: tab_start_idx,
                width: x - tab_start_idx,
            });
        }

        // New tab button
        {
            let hover = is_tab_hover(mouse_x, x, new_tab_hover.len());

            let cells = if hover { &new_tab_hover } else { &new_tab };

            let button_start = x;

            for c in cells {
                let len = c.width();
                line.set_cell(x, c.clone());
                x += len;
            }

            items.push(TabEntry {
                item: TabBarItem::NewTabButton,
                x: button_start,
                width: x - button_start,
            });
        }

        let black_cell = Cell::new(
            ' ',
            CellAttributes::default()
                .set_background(ColorSpec::TrueColor(colors.background))
                .clone(),
        );

        for idx in x..title_width {
            line.set_cell(idx, black_cell.clone());
        }

        let rhs_cells = parse_status_text(right_status, black_cell.attrs().clone());
        let rhs_len = rhs_cells.len().min(title_width.saturating_sub(x));
        let skip = rhs_cells.len() - rhs_len;

        for (idx, cell) in rhs_cells.into_iter().skip(skip).rev().enumerate() {
            line.set_cell(title_width - (1 + idx), cell);
        }

        Self { line, items }
    }

    /// Determine which component the mouse is over
    pub fn hit_test(&self, mouse_x: usize) -> TabBarItem {
        for entry in self.items.iter() {
            if mouse_x >= entry.x && mouse_x < entry.x + entry.width {
                return entry.item;
            }
        }
        TabBarItem::None
    }
}

fn parse_status_text(text: &str, default_cell: CellAttributes) -> Vec<Cell> {
    let mut pen = default_cell.clone();
    let mut cells = vec![];
    let mut ignoring = false;
    let mut print_buffer = String::new();

    fn flush_print(buf: &mut String, cells: &mut Vec<Cell>, pen: &CellAttributes) {
        for g in unicode_segmentation::UnicodeSegmentation::graphemes(buf.as_str(), true) {
            cells.push(Cell::new_grapheme(g, pen.clone()));
        }
        buf.clear();
    }

    let mut parser = Parser::new();
    parser.parse(text.as_bytes(), |action| {
        if ignoring {
            return;
        }
        match action {
            Action::Print(c) => print_buffer.push(c),
            Action::Control(c) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
                match c {
                    ControlCode::CarriageReturn | ControlCode::LineFeed => {
                        ignoring = true;
                    }
                    _ => {}
                }
            }
            Action::CSI(csi) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
                match csi {
                    CSI::Sgr(sgr) => match sgr {
                        Sgr::Reset => pen = default_cell.clone(),
                        Sgr::Intensity(i) => {
                            pen.set_intensity(i);
                        }
                        Sgr::Underline(u) => {
                            pen.set_underline(u);
                        }
                        Sgr::Overline(o) => {
                            pen.set_overline(o);
                        }
                        Sgr::Blink(b) => {
                            pen.set_blink(b);
                        }
                        Sgr::Italic(i) => {
                            pen.set_italic(i);
                        }
                        Sgr::Inverse(inverse) => {
                            pen.set_reverse(inverse);
                        }
                        Sgr::Invisible(invis) => {
                            pen.set_invisible(invis);
                        }
                        Sgr::StrikeThrough(strike) => {
                            pen.set_strikethrough(strike);
                        }
                        Sgr::Foreground(col) => {
                            if let ColorSpec::Default = col {
                                pen.set_foreground(default_cell.foreground());
                            } else {
                                pen.set_foreground(col);
                            }
                        }
                        Sgr::Background(col) => {
                            if let ColorSpec::Default = col {
                                pen.set_background(default_cell.background());
                            } else {
                                pen.set_background(col);
                            }
                        }
                        Sgr::UnderlineColor(col) => {
                            pen.set_underline_color(col);
                        }
                        Sgr::Font(_) => {}
                    },
                    _ => {}
                }
            }
            Action::Esc(_) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
            }
            Action::Sixel(_) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
            }
            Action::DeviceControl(_) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
            }
            Action::OperatingSystemCommand(_) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
            }
        }
    });
    flush_print(&mut print_buffer, &mut cells, &pen);
    cells
}
