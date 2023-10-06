use crate::termwindow::{PaneInformation, TabInformation, UIItem, UIItemType};
use config::{ConfigHandle, TabBarColors};
use finl_unicode::grapheme_clusters::Graphemes;
use mlua::FromLua;
use termwiz::cell::{unicode_column_width, Cell, CellAttributes};
use termwiz::color::ColorSpec;
use termwiz::escape::csi::Sgr;
use termwiz::escape::parser::Parser;
use termwiz::escape::{Action, ControlCode, CSI};
use termwiz::surface::SEQ_ZERO;
use termwiz_funcs::{format_as_escapes, FormatItem};
use wezterm_term::Line;
use window::{IntegratedTitleButton, IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct TabBarState {
    line: Line,
    items: Vec<TabEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabBarItem {
    None,
    LeftStatus,
    RightStatus,
    Tab { tab_idx: usize, active: bool },
    NewTabButton,
    WindowButton(IntegratedTitleButton),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabEntry {
    pub item: TabBarItem,
    pub title: Line,
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
                    let line = parse_status_text(&esc, CellAttributes::default());

                    Ok(Some(TitleText {
                        items,
                        len: line.len(),
                    }))
                }
                _ => {
                    let s = String::from_lua(v, &*lua)?;
                    let line = parse_status_text(&s, CellAttributes::default());
                    Ok(Some(TitleText {
                        len: line.len(),
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
                let mut title = if tab.tab_title.is_empty() {
                    pane.title.clone()
                } else {
                    tab.tab_title.clone()
                };
                let classic_spacing = if config.use_fancy_tab_bar { "" } else { " " };
                if config.show_tab_index_in_tab_bar {
                    title = format!(
                        "{}{}: {}{}",
                        classic_spacing,
                        tab.tab_index
                            + if config.tab_and_split_indices_are_zero_based {
                                0
                            } else {
                                1
                            },
                        title,
                        classic_spacing,
                    );
                }
                // We have a preferred soft minimum on tab width to make it
                // easier to click on tab titles, but we'll still go below
                // this if there are too many tabs to fit the window at
                // this width.
                if !config.use_fancy_tab_bar {
                    while unicode_column_width(&title, None) < 5 {
                        title.push(' ');
                    }
                }
                title
            } else {
                " no pane ".to_string()
            };

            TitleText {
                len: unicode_column_width(&title, None),
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
            line: Line::with_width(1, SEQ_ZERO),
            items: vec![TabEntry {
                item: TabBarItem::None,
                title: Line::from_text(" ", &CellAttributes::blank(), 1, None),
                x: 1,
                width: 1,
            }],
        }
    }

    pub fn line(&self) -> &Line {
        &self.line
    }

    pub fn items(&self) -> &[TabEntry] {
        &self.items
    }

    fn integrated_title_buttons(
        mouse_x: Option<usize>,
        x: &mut usize,
        config: &ConfigHandle,
        items: &mut Vec<TabEntry>,
        line: &mut Line,
        colors: &TabBarColors,
    ) {
        let default_cell = if config.use_fancy_tab_bar {
            CellAttributes::default()
        } else {
            colors.new_tab().as_cell_attributes()
        };

        let default_cell_hover = if config.use_fancy_tab_bar {
            CellAttributes::default()
        } else {
            colors.new_tab_hover().as_cell_attributes()
        };

        let window_hide =
            parse_status_text(&config.tab_bar_style.window_hide, default_cell.clone());
        let window_hide_hover = parse_status_text(
            &config.tab_bar_style.window_hide_hover,
            default_cell_hover.clone(),
        );

        let window_maximize =
            parse_status_text(&config.tab_bar_style.window_maximize, default_cell.clone());
        let window_maximize_hover = parse_status_text(
            &config.tab_bar_style.window_maximize_hover,
            default_cell_hover.clone(),
        );

        let window_close =
            parse_status_text(&config.tab_bar_style.window_close, default_cell.clone());
        let window_close_hover = parse_status_text(
            &config.tab_bar_style.window_close_hover,
            default_cell_hover.clone(),
        );

        for button in &config.integrated_title_buttons {
            use IntegratedTitleButton as Button;
            let title = match button {
                Button::Hide => {
                    let hover = is_tab_hover(mouse_x, *x, window_hide_hover.len());

                    if hover {
                        &window_hide_hover
                    } else {
                        &window_hide
                    }
                }
                Button::Maximize => {
                    let hover = is_tab_hover(mouse_x, *x, window_maximize_hover.len());

                    if hover {
                        &window_maximize_hover
                    } else {
                        &window_maximize
                    }
                }
                Button::Close => {
                    let hover = is_tab_hover(mouse_x, *x, window_close_hover.len());

                    if hover {
                        &window_close_hover
                    } else {
                        &window_close
                    }
                }
            };

            line.append_line(title.to_owned(), SEQ_ZERO);

            let width = title.len();
            items.push(TabEntry {
                item: TabBarItem::WindowButton(*button),
                title: title.to_owned(),
                x: *x,
                width,
            });

            *x += width;
        }
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
        left_status: &str,
        right_status: &str,
    ) -> Self {
        let colors = colors.cloned().unwrap_or_else(TabBarColors::default);

        let active_cell_attrs = colors.active_tab().as_cell_attributes();
        let inactive_hover_attrs = colors.inactive_tab_hover().as_cell_attributes();
        let inactive_cell_attrs = colors.inactive_tab().as_cell_attributes();
        let new_tab_hover_attrs = colors.new_tab_hover().as_cell_attributes();
        let new_tab_attrs = colors.new_tab().as_cell_attributes();

        let new_tab = parse_status_text(
            &config.tab_bar_style.new_tab,
            if config.use_fancy_tab_bar {
                CellAttributes::default()
            } else {
                new_tab_attrs.clone()
            },
        );
        let new_tab_hover = parse_status_text(
            &config.tab_bar_style.new_tab_hover,
            if config.use_fancy_tab_bar {
                CellAttributes::default()
            } else {
                new_tab_hover_attrs.clone()
            },
        );

        let use_integrated_title_buttons = config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS);

        // We ultimately want to produce a line looking like this:
        // ` | tab1-title x | tab2-title x |  +      . - X `
        // Where the `+` sign will spawn a new tab (or show a context
        // menu with tab creation options) and the other three chars
        // are symbols representing minimize, maximize and close.

        let mut active_tab_no = 0;

        let config_tab_max_width = if config.tab_bar_fill {
            // We have no layout, so this is a rough estimate
            // The tab bar consists of the tab titles, the new tab button, and some padding
            title_width.saturating_sub(new_tab.len() + 2 + tab_info.len()) / (tab_info.len())
        } else {
            config.tab_max_width
        };

        let tab_titles: Vec<TitleText> = if config.show_tabs_in_tab_bar {
            tab_info
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
                        config_tab_max_width,
                    )
                })
                .collect()
        } else {
            vec![]
        };
        let titles_len: usize = tab_titles.iter().map(|s| s.len).sum();
        let number_of_tabs = tab_titles.len();
        let available_cells =
            title_width.saturating_sub(number_of_tabs.saturating_sub(1) + new_tab.len());
        let tab_width_max = if config.use_fancy_tab_bar || available_cells >= titles_len {
            // We can render each title with its full width
            usize::MAX
        } else {
            // We need to clamp the length to balance them out
            available_cells / number_of_tabs
        }
        .min(config_tab_max_width);
        let mut line = Line::with_width(0, SEQ_ZERO);

        let mut x = 0;
        let mut items = vec![];

        let black_cell = Cell::blank_with_attrs(
            CellAttributes::default()
                .set_background(ColorSpec::TrueColor(*colors.background()))
                .clone(),
        );

        if use_integrated_title_buttons
            && config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            && config.use_fancy_tab_bar == false
            && config.tab_bar_at_bottom == false
        {
            for _ in 0..10 as usize {
                line.insert_cell(0, black_cell.clone(), title_width, SEQ_ZERO);
                x += 1;
            }
        }

        if use_integrated_title_buttons
            && config.integrated_title_button_style != IntegratedTitleButtonStyle::MacOsNative
            && config.integrated_title_button_alignment == IntegratedTitleButtonAlignment::Left
        {
            Self::integrated_title_buttons(mouse_x, &mut x, config, &mut items, &mut line, &colors);
        }

        let left_status_line = parse_status_text(left_status, black_cell.attrs().clone());
        if left_status_line.len() > 0 {
            items.push(TabEntry {
                item: TabBarItem::LeftStatus,
                title: left_status_line.clone(),
                x,
                width: left_status_line.len(),
            });
            x += left_status_line.len();
            line.append_line(left_status_line, SEQ_ZERO);
        }

        for (tab_idx, tab_title) in tab_titles.iter().enumerate() {
            // The title is allowed to grow to the max size of the computed tab width
            let tab_title_max_len = tab_title.len.max(tab_width_max).min(tab_width_max);
            let active = tab_idx == active_tab_no;
            let hover = !active && is_tab_hover(mouse_x, x, tab_title_max_len);

            // Recompute the title so that it factors in both the hover state
            // and the adjusted maximum tab width based on available space.
            let tab_title = compute_tab_title(
                &tab_info[tab_idx],
                tab_info,
                pane_info,
                config,
                hover,
                tab_title_max_len,
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
            let mut tab_line = parse_status_text(
                &esc,
                if config.use_fancy_tab_bar {
                    CellAttributes::default()
                } else {
                    cell_attrs.clone()
                },
            );

            let title = tab_line.clone();
            if tab_line.len() > tab_width_max {
                tab_line.resize(tab_width_max, SEQ_ZERO);
            }

            let width = tab_line.len();

            items.push(TabEntry {
                item: TabBarItem::Tab { tab_idx, active },
                title,
                x: tab_start_idx,
                width,
            });

            line.append_line(tab_line, SEQ_ZERO);
            x += width;
        }

        // New tab button
        if config.show_new_tab_button_in_tab_bar {
            let hover = is_tab_hover(mouse_x, x, new_tab_hover.len());

            let new_tab_button = if hover { &new_tab_hover } else { &new_tab };

            let button_start = x;
            let width = new_tab_button.len();

            line.append_line(new_tab_button.clone(), SEQ_ZERO);

            items.push(TabEntry {
                item: TabBarItem::NewTabButton,
                title: new_tab_button.clone(),
                x: button_start,
                width,
            });

            x += width;
        }

        // Reserve place for integrated title buttons
        let title_width = if use_integrated_title_buttons
            && config.integrated_title_button_style != IntegratedTitleButtonStyle::MacOsNative
            && config.integrated_title_button_alignment == IntegratedTitleButtonAlignment::Right
        {
            let window_hide =
                parse_status_text(&config.tab_bar_style.window_hide, CellAttributes::default());
            let window_hide_hover = parse_status_text(
                &config.tab_bar_style.window_hide_hover,
                CellAttributes::default(),
            );

            let window_maximize = parse_status_text(
                &config.tab_bar_style.window_maximize,
                CellAttributes::default(),
            );
            let window_maximize_hover = parse_status_text(
                &config.tab_bar_style.window_maximize_hover,
                CellAttributes::default(),
            );
            let window_close = parse_status_text(
                &config.tab_bar_style.window_close,
                CellAttributes::default(),
            );
            let window_close_hover = parse_status_text(
                &config.tab_bar_style.window_close_hover,
                CellAttributes::default(),
            );

            let hide_len = window_hide.len().max(window_hide_hover.len());
            let maximize_len = window_maximize.len().max(window_maximize_hover.len());
            let close_len = window_close.len().max(window_close_hover.len());

            let mut width_to_reserve = 0;
            for button in &config.integrated_title_buttons {
                use IntegratedTitleButton as Button;
                let button_len = match button {
                    Button::Hide => hide_len,
                    Button::Maximize => maximize_len,
                    Button::Close => close_len,
                };
                width_to_reserve += button_len;
            }

            title_width.saturating_sub(width_to_reserve)
        } else {
            title_width
        };

        let status_space_available = title_width.saturating_sub(x);

        let mut right_status_line = parse_status_text(right_status, black_cell.attrs().clone());
        items.push(TabEntry {
            item: TabBarItem::RightStatus,
            title: right_status_line.clone(),
            x,
            width: status_space_available,
        });

        while right_status_line.len() > status_space_available {
            right_status_line.remove_cell(0, SEQ_ZERO);
        }

        line.append_line(right_status_line, SEQ_ZERO);
        while line.len() < title_width {
            line.insert_cell(x, black_cell.clone(), title_width, SEQ_ZERO);
        }

        if use_integrated_title_buttons
            && config.integrated_title_button_style != IntegratedTitleButtonStyle::MacOsNative
            && config.integrated_title_button_alignment == IntegratedTitleButtonAlignment::Right
        {
            x = title_width;
            Self::integrated_title_buttons(mouse_x, &mut x, config, &mut items, &mut line, &colors);
        }

        Self { line, items }
    }

    pub fn compute_ui_items(&self, y: usize, cell_height: usize, cell_width: usize) -> Vec<UIItem> {
        let mut items = vec![];

        for entry in self.items.iter() {
            items.push(UIItem {
                x: entry.x * cell_width,
                width: entry.width * cell_width,
                y,
                height: cell_height,
                item_type: UIItemType::TabBar(entry.item),
            });
        }

        items
    }
}

pub fn parse_status_text(text: &str, default_cell: CellAttributes) -> Line {
    let mut pen = default_cell.clone();
    let mut cells = vec![];
    let mut ignoring = false;
    let mut print_buffer = String::new();

    fn flush_print(buf: &mut String, cells: &mut Vec<Cell>, pen: &CellAttributes) {
        for g in Graphemes::new(buf.as_str()) {
            let cell = Cell::new_grapheme(g, pen.clone(), None);
            let width = cell.width();
            cells.push(cell);
            for _ in 1..width {
                // Line/Screen expect double wide graphemes to be followed by a blank in
                // the next column position, otherwise we'll render incorrectly
                cells.push(Cell::blank_with_attrs(pen.clone()));
            }
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
            Action::PrintString(s) => print_buffer.push_str(&s),
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
                        Sgr::VerticalAlign(o) => {
                            pen.set_vertical_align(o);
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
            Action::OperatingSystemCommand(_)
            | Action::DeviceControl(_)
            | Action::Esc(_)
            | Action::KittyImage(_)
            | Action::XtGetTcap(_)
            | Action::Sixel(_) => {
                flush_print(&mut print_buffer, &mut cells, &pen);
            }
        }
    });
    flush_print(&mut print_buffer, &mut cells, &pen);
    Line::from_cells(cells, SEQ_ZERO)
}
