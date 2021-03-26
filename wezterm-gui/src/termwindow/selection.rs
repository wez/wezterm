use crate::selection::{SelectionCoordinate, SelectionMode, SelectionRange};
use ::window::WindowOps;
use mux::pane::Pane;
use std::rc::Rc;
use wezterm_term::StableRowIndex;

impl super::TermWindow {
    pub fn selection_text(&self, pane: &Rc<dyn Pane>) -> String {
        let mut s = String::new();
        if let Some(sel) = self
            .selection(pane.pane_id())
            .range
            .as_ref()
            .map(|r| r.normalize())
        {
            let mut last_was_wrapped = false;
            let (first_row, lines) =
                pane.get_lines_with_hyperlinks_applied(sel.rows(), &self.config.hyperlink_rules);
            for (idx, line) in lines.iter().enumerate() {
                let cols = sel.cols_for_row(first_row + idx as StableRowIndex);
                let last_col_idx = cols.end.min(line.cells().len()).saturating_sub(1);
                if !s.is_empty() && !last_was_wrapped {
                    s.push('\n');
                }
                s.push_str(line.columns_as_str(cols).trim_end());

                let last_cell = &line.cells()[last_col_idx];
                // TODO: should really test for any unicode whitespace
                last_was_wrapped = last_cell.attrs().wrapped() && last_cell.str() != " ";
            }
        }

        s
    }

    pub fn extend_selection_at_mouse_cursor(
        &mut self,
        mode: Option<SelectionMode>,
        pane: &Rc<dyn Pane>,
    ) {
        let mode = mode.unwrap_or(SelectionMode::Cell);
        let (x, y) = self.last_mouse_terminal_coords;
        match mode {
            SelectionMode::Cell => {
                let end = SelectionCoordinate { x, y };
                let selection_range = self.selection(pane.pane_id()).range.take();
                let sel = match selection_range {
                    None => {
                        SelectionRange::start(self.selection(pane.pane_id()).start.unwrap_or(end))
                            .extend(end)
                    }
                    Some(sel) => sel.extend(end),
                };
                self.selection(pane.pane_id()).range = Some(sel);
            }
            SelectionMode::Word => {
                let end_word = SelectionRange::word_around(SelectionCoordinate { x, y }, &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .start
                    .clone()
                    .unwrap_or(end_word.start);
                let start_word = SelectionRange::word_around(start_coord, &**pane);

                let selection_range = start_word.extend_with(end_word);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Line => {
                let end_line = SelectionRange::line_around(SelectionCoordinate { x, y }, &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .start
                    .clone()
                    .unwrap_or(end_line.start);
                let start_line = SelectionRange::line_around(start_coord, &**pane);

                let selection_range = start_line.extend_with(end_line);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::SemanticZone => {
                let end_word = SelectionRange::zone_around(SelectionCoordinate { x, y }, &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .start
                    .clone()
                    .unwrap_or(end_word.start);
                let start_word = SelectionRange::zone_around(start_coord, &**pane);

                let selection_range = start_word.extend_with(end_word);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
        }

        // When the mouse gets close enough to the top or bottom then scroll
        // the viewport so that we can see more in that direction and are able
        // to select more than fits in the viewport.

        // This is similar to the logic in the copy mode overlay, but the gap
        // is smaller because it feels more natural for mouse selection to have
        // a smaller gap.
        const VERTICAL_GAP: isize = 1;
        let dims = pane.get_dimensions();
        let top = self
            .get_viewport(pane.pane_id())
            .unwrap_or(dims.physical_top);
        let vertical_gap = if dims.physical_top <= VERTICAL_GAP {
            1
        } else {
            VERTICAL_GAP
        };
        let top_gap = y - top;
        if top_gap < vertical_gap {
            // Increase the gap so we can "look ahead"
            self.set_viewport(pane.pane_id(), Some(y.saturating_sub(vertical_gap)), dims);
        } else {
            let bottom_gap = (dims.viewport_rows as isize).saturating_sub(top_gap);
            if bottom_gap < vertical_gap {
                self.set_viewport(pane.pane_id(), Some(top + vertical_gap - bottom_gap), dims);
            }
        }

        self.window.as_ref().unwrap().invalidate();
    }

    pub fn select_text_at_mouse_cursor(&mut self, mode: SelectionMode, pane: &Rc<dyn Pane>) {
        let (x, y) = self.last_mouse_terminal_coords;
        match mode {
            SelectionMode::Line => {
                let start = SelectionCoordinate { x, y };
                let selection_range = SelectionRange::line_around(start, &**pane);

                self.selection(pane.pane_id()).start = Some(start);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Word => {
                let selection_range =
                    SelectionRange::word_around(SelectionCoordinate { x, y }, &**pane);

                self.selection(pane.pane_id()).start = Some(selection_range.start);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::SemanticZone => {
                let selection_range =
                    SelectionRange::zone_around(SelectionCoordinate { x, y }, &**pane);

                self.selection(pane.pane_id()).start = Some(selection_range.start);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Cell => {
                self.selection(pane.pane_id())
                    .begin(SelectionCoordinate { x, y });
            }
        }

        self.window.as_ref().unwrap().invalidate();
    }
}
