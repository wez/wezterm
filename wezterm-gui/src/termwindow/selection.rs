use crate::selection::{Selection, SelectionCoordinate, SelectionMode, SelectionRange, SelectionX};
use ::window::WindowOps;
use mux::pane::{Pane, PaneId};
use std::cell::RefMut;
use std::sync::Arc;
use termwiz::surface::Line;
use wezterm_term::StableRowIndex;

impl super::TermWindow {
    pub fn selection(&self, pane_id: PaneId) -> RefMut<Selection> {
        RefMut::map(self.pane_state(pane_id), |state| &mut state.selection)
    }

    /// Returns the selection region as a series of Line
    pub fn selection_lines(&self, pane: &Arc<dyn Pane>) -> Vec<Line> {
        let mut result = vec![];

        let rectangular = self.selection(pane.pane_id()).rectangular;
        if let Some(sel) = self
            .selection(pane.pane_id())
            .range
            .as_ref()
            .map(|r| r.normalize())
        {
            let mut last_was_wrapped = false;
            let first_row = sel.rows().start;
            let last_row = sel.rows().end;

            for line in pane.get_logical_lines(sel.rows()) {
                if result.is_empty() || !last_was_wrapped {
                    result.push(Line::with_width(0, line.physical_lines[0].current_seqno()));
                }
                let last_idx = line.physical_lines.len().saturating_sub(1);
                for (idx, phys) in line.physical_lines.iter().enumerate() {
                    let this_row = line.first_row + idx as StableRowIndex;
                    if this_row >= first_row && this_row < last_row {
                        let last_phys_idx = phys.len().saturating_sub(1);
                        let cols = sel.cols_for_row(this_row, rectangular);
                        let last_col_idx = cols.end.saturating_sub(1).min(last_phys_idx);
                        let mut col_span = phys.columns_as_line(cols);
                        let seqno = col_span.current_seqno();
                        // Only trim trailing whitespace if we are the last line
                        // in a wrapped sequence
                        if idx == last_idx {
                            col_span.prune_trailing_blanks(seqno);
                        }

                        result
                            .last_mut()
                            .map(|line| line.append_line(col_span, seqno));

                        last_was_wrapped = last_col_idx == last_phys_idx
                            && phys
                                .get_cell(last_col_idx)
                                .map(|c| c.attrs().wrapped())
                                .unwrap_or(false);
                    }
                }
            }
        }

        result
    }

    /// Returns the selection text only
    pub fn selection_text(&self, pane: &Arc<dyn Pane>) -> String {
        let mut s = String::new();
        let rectangular = self.selection(pane.pane_id()).rectangular;
        if let Some(sel) = self
            .selection(pane.pane_id())
            .range
            .as_ref()
            .map(|r| r.normalize())
        {
            let mut last_was_wrapped = false;
            let first_row = sel.rows().start;
            let last_row = sel.rows().end;

            for line in pane.get_logical_lines(sel.rows()) {
                if !s.is_empty() && !last_was_wrapped {
                    s.push('\n');
                }
                let last_idx = line.physical_lines.len().saturating_sub(1);
                for (idx, phys) in line.physical_lines.iter().enumerate() {
                    let this_row = line.first_row + idx as StableRowIndex;
                    if this_row >= first_row && this_row < last_row {
                        let last_phys_idx = phys.len().saturating_sub(1);
                        let cols = sel.cols_for_row(this_row, rectangular);
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
        }

        s
    }

    pub fn clear_selection(&mut self, pane: &Arc<dyn Pane>) {
        let mut selection = self.selection(pane.pane_id());
        selection.clear();
        selection.seqno = pane.get_current_seqno();
        self.window.as_ref().unwrap().invalidate();
    }

    pub fn extend_selection_at_mouse_cursor(&mut self, mode: SelectionMode, pane: &Arc<dyn Pane>) {
        self.selection(pane.pane_id()).seqno = pane.get_current_seqno();
        let (position, y) = match self.pane_state(pane.pane_id()).mouse_terminal_coords {
            Some(coords) => coords,
            None => return,
        };
        let x = position.column;
        match mode {
            SelectionMode::Cell | SelectionMode::Block => {
                // Origin is the cell in which the selection action started. E.g. the cell
                // that had the mouse over it when the left mouse button was pressed
                let origin = self
                    .selection(pane.pane_id())
                    .origin
                    .unwrap_or(SelectionCoordinate::x_y(x, y));
                self.selection(pane.pane_id()).origin = Some(origin);
                self.selection(pane.pane_id()).rectangular = mode == SelectionMode::Block;

                // Compute the start and end horizontall cell of the selection.
                // The selection extent depends on the mouse cursor position in relation
                // to the origin.
                let (start_x, end_x) = if mode == SelectionMode::Block {
                    if x >= origin.x {
                        // If the selection is extending forwards from the origin,
                        // it includes the origin
                        (origin.x, SelectionX::Cell(x).saturating_sub(1))
                    } else {
                        // If the selection is extending backwards from the origin,
                        // it doesn't include the origin
                        (origin.x.saturating_sub(1), SelectionX::Cell(x))
                    }
                } else {
                    if (x >= origin.x && y == origin.y) || y > origin.y {
                        // If the selection is extending forwards from the origin, it includes the
                        // origin and doesn't include the cell under the cursor. Note that the
                        // reported cell here is offset by -50% from the real cell you see on the
                        // screen, so this causes a visual cell on the screen to be selected when
                        // the mouse moves over 50% of its width, which effectively means the next
                        // cell is being reported here, hence it's excluded
                        (origin.x, SelectionX::Cell(x).saturating_sub(1))
                    } else {
                        // If the selection is extending backwards from the origin, it doesn't
                        // include the origin and includes the cell under the cursor, which has
                        // the same effect as described above when going backwards
                        (origin.x.saturating_sub(1), SelectionX::Cell(x))
                    }
                };

                self.selection(pane.pane_id()).range =
                    if mode == SelectionMode::Block && origin.x == x {
                        // Ignore rectangle selections with a width of zero
                        None
                    } else if origin.x != x || origin.y != y {
                        // Only considers a selection if the cursor moved from the origin point
                        Some(
                            SelectionRange::start(SelectionCoordinate {
                                x: start_x,
                                y: origin.y,
                            })
                            .extend(SelectionCoordinate { x: end_x, y }),
                        )
                    } else {
                        None
                    };
            }
            SelectionMode::Word => {
                let end_word = SelectionRange::word_around(SelectionCoordinate::x_y(x, y), &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .origin
                    .clone()
                    .unwrap_or(end_word.start);
                let start_word = SelectionRange::word_around(start_coord, &**pane);

                let selection_range = start_word.extend_with(end_word);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
            SelectionMode::Line => {
                let end_line = SelectionRange::line_around(SelectionCoordinate::x_y(x, y), &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .origin
                    .clone()
                    .unwrap_or(end_line.start);
                let start_line = SelectionRange::line_around(start_coord, &**pane);

                let selection_range = start_line.extend_with(end_line);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
            SelectionMode::SemanticZone => {
                let end_word = SelectionRange::zone_around(SelectionCoordinate::x_y(x, y), &**pane);

                let start_coord = self
                    .selection(pane.pane_id())
                    .origin
                    .clone()
                    .unwrap_or(end_word.start);
                let start_word = SelectionRange::zone_around(start_coord, &**pane);

                let selection_range = start_word.extend_with(end_word);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
        }

        let dims = pane.get_dimensions();

        // Scroll viewport when mouse mouves out of its vertical bounds
        if position.row == 0 && position.y_pixel_offset < 0 {
            self.set_viewport(pane.pane_id(), Some(y.saturating_sub(1)), dims);
        } else if position.row >= dims.viewport_rows as i64 {
            let top = self
                .get_viewport(pane.pane_id())
                .unwrap_or(dims.physical_top);
            self.set_viewport(pane.pane_id(), Some(top + 1), dims);
        }

        self.window.as_ref().unwrap().invalidate();
    }

    pub fn select_text_at_mouse_cursor(&mut self, mode: SelectionMode, pane: &Arc<dyn Pane>) {
        let (x, y) = match self.pane_state(pane.pane_id()).mouse_terminal_coords {
            Some(coords) => (coords.0.column, coords.1),
            None => return,
        };
        match mode {
            SelectionMode::Line => {
                let start = SelectionCoordinate::x_y(0, y);
                let selection_range = SelectionRange::line_around(start, &**pane);

                self.selection(pane.pane_id()).origin = Some(start);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
            SelectionMode::Word => {
                let selection_range =
                    SelectionRange::word_around(SelectionCoordinate::x_y(x, y), &**pane);

                self.selection(pane.pane_id()).origin = Some(selection_range.start);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
            SelectionMode::SemanticZone => {
                let selection_range =
                    SelectionRange::zone_around(SelectionCoordinate::x_y(x, y), &**pane);

                self.selection(pane.pane_id()).origin = Some(selection_range.start);
                self.selection(pane.pane_id()).range = Some(selection_range);
                self.selection(pane.pane_id()).rectangular = false;
            }
            SelectionMode::Cell | SelectionMode::Block => {
                self.selection(pane.pane_id())
                    .begin(SelectionCoordinate::x_y(x, y));
                self.selection(pane.pane_id()).rectangular = mode == SelectionMode::Block;
            }
        }

        self.selection(pane.pane_id()).seqno = pane.get_current_seqno();
        self.window.as_ref().unwrap().invalidate();
    }
}
