use config::{ConfigHandle, TabBarColors};
use mux::window::Window as MuxWindow;
use std::cell::Ref;
use termwiz::cell::unicode_column_width;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::ColorSpec;
use termwiz::escape::csi::Sgr;
use termwiz::escape::parser::Parser;
use termwiz::escape::{Action, ControlCode, CSI};
use unicode_segmentation::UnicodeSegmentation;
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
        window: &Ref<MuxWindow>,
        colors: Option<&TabBarColors>,
        config: &ConfigHandle,
        right_status: &str,
    ) -> Self {
        let colors = colors.cloned().unwrap_or_else(TabBarColors::default);

        let active_cell_attrs = colors.active_tab.as_cell_attributes();
        let inactive_hover_attrs = colors.inactive_tab_hover.as_cell_attributes();
        let inactive_cell_attrs = colors.inactive_tab.as_cell_attributes();

        let active_tab_left = parse_status_text(
            &config.tab_bar_style.active_tab_left,
            active_cell_attrs.clone(),
        );
        let active_tab_right = parse_status_text(
            &config.tab_bar_style.active_tab_right,
            active_cell_attrs.clone(),
        );
        let inactive_tab_left = parse_status_text(
            &config.tab_bar_style.inactive_tab_left,
            inactive_cell_attrs.clone(),
        );
        let inactive_tab_right = parse_status_text(
            &config.tab_bar_style.inactive_tab_right,
            inactive_cell_attrs.clone(),
        );
        let inactive_tab_hover_left = parse_status_text(
            &config.tab_bar_style.inactive_tab_hover_left,
            inactive_hover_attrs.clone(),
        );
        let inactive_tab_hover_right = parse_status_text(
            &config.tab_bar_style.inactive_tab_hover_right,
            inactive_hover_attrs.clone(),
        );

        let new_tab_left = parse_status_text(
            &config.tab_bar_style.new_tab_left,
            inactive_cell_attrs.clone(),
        );
        let new_tab_right = parse_status_text(
            &config.tab_bar_style.new_tab_right,
            inactive_cell_attrs.clone(),
        );
        let new_tab_hover_left = parse_status_text(
            &config.tab_bar_style.new_tab_hover_left,
            inactive_hover_attrs.clone(),
        );
        let new_tab_hover_right = parse_status_text(
            &config.tab_bar_style.new_tab_hover_right,
            inactive_hover_attrs.clone(),
        );

        // We ultimately want to produce a line looking like this:
        // ` | tab1-title x | tab2-title x |  +      . - X `
        // Where the `+` sign will spawn a new tab (or show a context
        // menu with tab creation options) and the other three chars
        // are symbols representing minimize, maximize and close.

        let tab_titles: Vec<String> = window
            .iter()
            .enumerate()
            .map(|(idx, tab)| {
                if let Some(pane) = tab.get_active_pane() {
                    let mut title = pane.get_title();
                    if config.show_tab_index_in_tab_bar {
                        title = format!(
                            "{}: {}",
                            idx + if config.tab_and_split_indices_are_zero_based {
                                0
                            } else {
                                1
                            },
                            title
                        );
                    }
                    // We have a preferred soft minimum on tab width to make it
                    // easier to click on tab titles, but we'll still go below
                    // this if there are too many tabs to fit the window at
                    // this width.
                    while title.len() < 5 {
                        title.push(' ');
                    }
                    title
                } else {
                    "no pane".to_string()
                }
            })
            .collect();
        let titles_len: usize = tab_titles.iter().map(|s| unicode_column_width(s)).sum();
        let number_of_tabs = tab_titles.len();

        let available_cells = title_width.saturating_sub(
            (number_of_tabs.saturating_sub(1)
                * (inactive_tab_left.len() + inactive_tab_right.len()))
                + (new_tab_left.len() + new_tab_right.len() + 1),
        );
        let tab_width_max = if available_cells >= titles_len {
            // We can render each title with its full width
            usize::max_value()
        } else {
            // We need to clamp the length to balance them out
            available_cells / number_of_tabs
        }
        .min(config.tab_max_width);

        let mut line = Line::with_width(title_width);

        let active_tab_no = window.get_active_idx();
        let mut x = 0;
        let mut items = vec![];

        for (tab_idx, tab_title) in tab_titles.iter().enumerate() {
            let tab_title_len = unicode_column_width(tab_title).min(tab_width_max);

            let active = tab_idx == active_tab_no;
            let hover = !active
                && mouse_x
                    .map(|mouse_x| {
                        mouse_x >= x
                            && mouse_x
                                < x + tab_title_len
                                    + (inactive_tab_left.len() + inactive_tab_right.len())
                    })
                    .unwrap_or(false);

            let (cell_attrs, left, right) = if active {
                (&active_cell_attrs, &active_tab_left, &active_tab_right)
            } else if hover {
                (
                    &inactive_hover_attrs,
                    &inactive_tab_hover_left,
                    &inactive_tab_hover_right,
                )
            } else {
                (
                    &inactive_cell_attrs,
                    &inactive_tab_left,
                    &inactive_tab_right,
                )
            };

            let tab_start_idx = x;

            for c in left {
                line.set_cell(x, c.clone());
                x += 1;
            }

            for (idx, sub) in tab_title.graphemes(true).enumerate() {
                if idx >= tab_width_max {
                    break;
                }

                line.set_cell(x, Cell::new_grapheme(sub, cell_attrs.clone()));
                x += 1;
            }

            for c in right {
                line.set_cell(x, c.clone());
                x += 1;
            }

            items.push(TabEntry {
                item: TabBarItem::Tab(tab_idx),
                x: tab_start_idx,
                width: x - tab_start_idx,
            });
        }

        // New tab button
        {
            let hover = mouse_x
                .map(|mouse_x| mouse_x >= x && mouse_x < x + 3)
                .unwrap_or(false);

            let (cell_attrs, left, right) = if hover {
                (
                    &inactive_hover_attrs,
                    &new_tab_hover_left,
                    &new_tab_hover_right,
                )
            } else {
                (&inactive_cell_attrs, &new_tab_left, &new_tab_right)
            };

            let button_start = x;

            for c in left {
                line.set_cell(x, c.clone());
                x += 1;
            }
            line.set_cell(x, Cell::new('+', cell_attrs.clone()));
            x += 1;

            for c in right {
                line.set_cell(x, c.clone());
                x += 1;
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
                                pen.set_foreground(default_cell.foreground);
                            } else {
                                pen.set_foreground(col);
                            }
                        }
                        Sgr::Background(col) => {
                            if let ColorSpec::Default = col {
                                pen.set_background(default_cell.background);
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
