use crate::mux::window::Window as MuxWindow;
use std::cell::Ref;
use term::Line;
use termwiz::cell::unicode_column_width;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::color::{ColorSpec, RgbColor};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq)]
pub struct TabBarState {
    line: Line,
    widths: Vec<usize>,
}

impl TabBarState {
    pub fn default() -> Self {
        Self {
            line: Line::with_width(1),
            widths: vec![],
        }
    }

    pub fn line(&self) -> &Line {
        &self.line
    }

    /// Build a new tab bar from the current state
    /// mouse_x is some if the mouse is on the same row as the tab bar.
    /// title_width is the total number of cell columns in the window.
    /// window allows access to the tabs associated with the window.
    pub fn new(title_width: usize, mouse_x: Option<usize>, window: &Ref<MuxWindow>) -> Self {
        // We ultimately want to produce a line looking like this:
        // ` | tab1-title x | tab2-title x |  +      . - X `
        // Where the `+` sign will spawn a new tab (or show a context
        // menu with tab creation options) and the other three chars
        // are symbols representing minimize, maximize and close.
        let per_tab_overhead = 2;
        let system_overhead = 0;

        let tab_titles: Vec<_> = window.iter().map(|w| w.get_title()).collect();
        let titles_len: usize = tab_titles.iter().map(|s| unicode_column_width(s)).sum();
        let number_of_tabs = tab_titles.len();

        let available_cells = title_width - ((number_of_tabs * per_tab_overhead) + system_overhead);
        let tab_width_max = if available_cells >= titles_len {
            // We can render each title with its full width
            usize::max_value()
        } else {
            // We need to clamp the length to balance them out
            available_cells / number_of_tabs
        };

        let black = RgbColor::new(0x05, 0x05, 0x05);
        let white = RgbColor::new(0xe7, 0xe7, 0xe7);
        let hover_gray = RgbColor::new(0x20, 0x20, 0x20);
        let active_gray = RgbColor::new(0x30, 0x30, 0x30);
        let black_cell = Cell::new(
            ' ',
            CellAttributes::default()
                .set_background(ColorSpec::TrueColor(black))
                .set_foreground(ColorSpec::TrueColor(white))
                .clone(),
        );
        let hover_cell = Cell::new(
            ' ',
            CellAttributes::default()
                .set_background(ColorSpec::TrueColor(hover_gray))
                .set_foreground(ColorSpec::TrueColor(white))
                .clone(),
        );
        let active_cell = Cell::new(
            ' ',
            CellAttributes::default()
                .set_background(ColorSpec::TrueColor(active_gray))
                .set_foreground(ColorSpec::TrueColor(white))
                .clone(),
        );

        let mut line = Line::with_width(title_width);

        let active_tab_no = window.get_active_idx();
        let mut x = 0;
        let mut widths = vec![];

        for (tab_idx, tab_title) in tab_titles.iter().enumerate() {
            let tab_title_len = unicode_column_width(tab_title).min(tab_width_max);

            let hover = mouse_x
                .map(|mouse_x| mouse_x >= x && mouse_x < x + tab_title_len + per_tab_overhead)
                .unwrap_or(false);
            let active = tab_idx == active_tab_no;

            let bg_cell = if active {
                active_cell.clone()
            } else if hover {
                hover_cell.clone()
            } else {
                black_cell.clone()
            };

            let tab_start_idx = x;

            line.set_cell(x, bg_cell.clone());
            x += 1;

            for (idx, sub) in tab_title.graphemes(true).enumerate() {
                if idx >= tab_width_max {
                    break;
                }

                line.set_cell(x, Cell::new_grapheme(sub, bg_cell.attrs().clone()));
                x += 1;
            }

            widths.push(x - tab_start_idx);

            line.set_cell(x, bg_cell.clone());
            x += 1;
        }

        for idx in x..title_width {
            line.set_cell(idx, black_cell.clone());
        }

        Self { line, widths }
    }

    /// Test whether a mouse coordinate is within one of the tab headers.
    /// If so, returns `Some(tab_idx)`
    pub fn hit_test(&self, mouse_x: usize) -> Option<usize> {
        let mut x = 0;
        for (tab_idx, width) in self.widths.iter().enumerate() {
            if mouse_x >= x && mouse_x < x + width {
                return Some(tab_idx);
            }
            x += width;
        }
        None
    }
}
