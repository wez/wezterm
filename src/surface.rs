use cell::{AttributeChange, Cell, CellAttributes};
use color::ColorAttribute;
use std::borrow::Cow;
use std::cmp::min;

/// Position holds 0-based positioning information, where
/// Absolute(0) is the start of the line or column,
/// Resltive(0) is the current position in the line or
/// column and EndRelative(0) is the end position in the
/// line or column.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Position {
    NoChange,
    /// Negative values move up, positive values down
    Relative(isize),
    /// Relative to the start of the line or top of the screen
    Absolute(usize),
    /// Relative to the end of line or bottom of screen
    EndRelative(usize),
}

/// `Change` describes an update operation to be applied to a `Surface`.
/// Changes to the active attributes (color, style), moving the cursor
/// and outputting text are examples of some of the values.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Change {
    /// Change a single attribute
    Attribute(AttributeChange),
    /// Change all possible attributes to the given set of values
    AllAttributes(CellAttributes),
    /// Add printable text.
    /// Control characters are rendered inert by transforming them
    /// to space.  CR and LF characters are interpreted by moving
    /// the cursor position.  CR moves the cursor to the start of
    /// the line and LF moves the cursor down to the next line.
    /// You typically want to use both together when sending in
    /// a line break.
    Text(String),
    /// Clear the screen to the specified color.
    /// Implicitly clears all attributes prior to clearing the screen.
    /// Moves the cursor to the home position (top left).
    ClearScreen(ColorAttribute),
    /// Clear from the current cursor X position to the rightmost
    /// edge of the screen.  The background color is set to the
    /// provided color.  The cursor position remains unchanged.
    ClearToEndOfLine(ColorAttribute),
    /// Clear from the current cursor X position to the rightmost
    /// edge of the screen on the current line.  Clear all of the
    /// lines below the current cursor Y position.  The background
    /// color is set ot the provided color.  The cursor position
    /// remains unchanged.
    ClearToEndOfScreen(ColorAttribute),
    /// Move the cursor to the specified `Position`.
    CursorPosition { x: Position, y: Position },
    /*   CursorVisibility(bool),
     *   ChangeScrollRegion{top: usize, bottom: usize}, */
}

impl Change {
    fn is_text(&self) -> bool {
        match self {
            Change::Text(_) => true,
            _ => false,
        }
    }

    fn text(&self) -> &str {
        match self {
            Change::Text(text) => text,
            _ => panic!("you must use Change::is_text() to guard calls to Change::text()"),
        }
    }
}

impl<S: Into<String>> From<S> for Change {
    fn from(s: S) -> Self {
        Change::Text(s.into())
    }
}

impl From<AttributeChange> for Change {
    fn from(c: AttributeChange) -> Self {
        Change::Attribute(c)
    }
}

#[derive(Debug, Clone)]
struct Line {
    cells: Vec<Cell>,
}

impl Line {
    fn with_width(width: usize) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize(width, Cell::default());
        Self { cells }
    }

    fn resize(&mut self, width: usize) {
        self.cells.resize(width, Cell::default());
    }

    /// Given a starting attribute value, produce a series of Change
    /// entries to recreate the current line
    fn changes(&self, start_attr: &CellAttributes) -> Vec<Change> {
        let mut result = Vec::new();
        let mut attr = start_attr.clone();
        let mut text_run = String::new();

        for cell in &self.cells {
            if *cell.attrs() == attr {
                text_run.push(cell.char());
            } else {
                // flush out the current text run
                if text_run.len() > 0 {
                    result.push(Change::Text(text_run.clone()));
                    text_run.clear();
                }

                attr = cell.attrs().clone();
                result.push(Change::AllAttributes(attr.clone()));
                text_run.push(cell.char());
            }
        }

        // flush out any remaining text run
        if text_run.len() > 0 {
            // if this is just spaces then it is likely cheaper
            // to emit ClearToEndOfLine instead.
            if attr == CellAttributes::default()
                .set_background(attr.background)
                .clone()
            {
                let left = text_run.trim_right_matches(' ').to_string();
                let num_trailing_spaces = text_run.len() - left.len();

                if num_trailing_spaces > 0 {
                    if left.len() > 0 {
                        result.push(Change::Text(left.to_string()));
                    } else if result.len() == 1 {
                        // if the only queued result prior to clearing
                        // to the end of the line is an attribute change,
                        // we can prune it out and return just the line
                        // clearing operation
                        match result[0] {
                            Change::AllAttributes(_) => result.clear(),
                            _ => {}
                        }
                    }

                    // Since this function is only called in the full repaint
                    // case, and we always emit a clear screen with the default
                    // background color, we don't need to emit an instruction
                    // to clear the remainder of the line unless it has a different
                    // background color.
                    if attr.background != Default::default() {
                        result.push(Change::ClearToEndOfLine(attr.background));
                    }
                } else {
                    result.push(Change::Text(text_run.clone()));
                }
            } else {
                result.push(Change::Text(text_run.clone()));
            }
        }

        result
    }
}

/// SequenceNo indicates a logical position within a stream of changes.
/// The sequence is only meaningful within a given `Surface` instance.
pub type SequenceNo = usize;

/// The `Surface` type represents the contents of a terminal screen.
/// It is not directly connected to a terminal device.
/// It consists of a buffer and a log of changes.  You can accumulate
/// updates to the screen by adding instances of the `Change` enum
/// that describe the updates.
///
/// When ready to render the `Surface` to a `Terminal`, you can use
/// the `get_changes` method to return an optimized stream of `Change`s
/// since the last render and then pass it to an instance of `Renderer`.
///
/// `Surface`s can also be composited together; this is useful when
/// building up a UI with layers or widgets: each widget can be its
/// own `Surface` instance and have its content maintained independently
/// from the other widgets on the screen and can then be copied into
/// the target `Surface` buffer for rendering.
///
/// To support more efficient updates in the composite use case, a
/// `draw_from_screen` method is available; the intent is to have one
/// `Surface` be hold the data that was last rendered, and a second `Surface`
/// of the same size that is repeatedly redrawn from the composite
/// of the widgets.  `draw_from_screen` is used to extract the smallest
/// difference between the updated screen and apply those changes to
/// the render target, and then use `get_changes` to render those without
/// repainting the world on each update.
#[derive(Default)]
pub struct Surface {
    width: usize,
    height: usize,
    lines: Vec<Line>,
    attributes: CellAttributes,
    xpos: usize,
    ypos: usize,
    seqno: SequenceNo,
    changes: Vec<Change>,
}

impl Surface {
    /// Create a new Surface with the specified width and height.
    pub fn new(width: usize, height: usize) -> Self {
        let mut scr = Surface {
            width,
            height,
            ..Default::default()
        };
        scr.resize(width, height);
        scr
    }

    /// Returns the (width, height) of the surface
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Resize the Surface to the specified width and height.
    /// If the width and/or height are smaller than previously, the rows and/or
    /// columns are truncated.  If the width and/or height are larger than
    /// previously then an appropriate number of cells are added to the
    /// buffer and filled with default attributes.
    /// The resize event invalidates the change stream, discarding it and
    /// causing a subsequent `get_changes` call to yield a full repaint.
    /// If the cursor position would be outside the bounds of the newly resized
    /// screen, it will be moved to be within the new bounds.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.lines.resize(height, Line::with_width(width));
        for line in &mut self.lines {
            line.resize(width);
        }
        self.width = width;
        self.height = height;

        // We need to invalidate the change stream prior to this
        // event, so we nominally generate an entry for the resize
        // here.  Since rendering a resize doesn't make sense, we
        // don't record a Change entry.  Instead what we do is
        // increment the sequence number and then flush the whole
        // stream.  The next call to get_changes() will perform a
        // full repaint, and that is what we want.
        // We only do this if we have any changes buffered.
        if !self.changes.is_empty() {
            self.seqno += 1;
            self.changes.clear();
        }

        // Ensure that the cursor position is well-defined
        self.xpos = compute_position_change(self.xpos, &Position::NoChange, self.width);
        self.ypos = compute_position_change(self.ypos, &Position::NoChange, self.height);
    }

    /// Efficiently apply a series of changes
    /// Returns the sequence number at the end of the change.
    pub fn add_changes(&mut self, mut changes: Vec<Change>) -> SequenceNo {
        let seq = self.seqno.saturating_sub(1) + changes.len();

        for change in &changes {
            self.apply_change(&change);
        }

        self.seqno += changes.len();
        self.changes.append(&mut changes);

        seq
    }

    /// Apply a change and return the sequence number at the end of the change.
    pub fn add_change<C: Into<Change>>(&mut self, change: C) -> SequenceNo {
        let seq = self.seqno;
        self.seqno += 1;
        let change = change.into();
        self.apply_change(&change);
        self.changes.push(change);
        seq
    }

    fn apply_change(&mut self, change: &Change) {
        match change {
            Change::AllAttributes(attr) => self.attributes = attr.clone(),
            Change::Text(text) => self.print_text(text),
            Change::Attribute(change) => self.change_attribute(change),
            Change::CursorPosition { x, y } => self.set_cursor_pos(x, y),
            Change::ClearScreen(color) => self.clear_screen(color),
            Change::ClearToEndOfLine(color) => self.clear_eol(color),
            Change::ClearToEndOfScreen(color) => self.clear_eos(color),
        }
    }

    fn clear_screen(&mut self, color: &ColorAttribute) {
        self.attributes = CellAttributes::default()
            .set_background(color.clone())
            .clone();
        let cleared = Cell::new(' ', self.attributes.clone());
        for line in &mut self.lines {
            for cell in &mut line.cells {
                *cell = cleared.clone();
            }
        }
    }

    fn clear_eos(&mut self, color: &ColorAttribute) {
        self.attributes = CellAttributes::default()
            .set_background(color.clone())
            .clone();
        let cleared = Cell::new(' ', self.attributes.clone());
        for cell in self.lines[self.ypos].cells.iter_mut().skip(self.xpos) {
            *cell = cleared.clone();
        }
        for line in &mut self.lines.iter_mut().skip(self.ypos + 1) {
            for cell in &mut line.cells {
                *cell = cleared.clone();
            }
        }
    }

    fn clear_eol(&mut self, color: &ColorAttribute) {
        self.attributes = CellAttributes::default()
            .set_background(color.clone())
            .clone();
        let cleared = Cell::new(' ', self.attributes.clone());
        for cell in self.lines[self.ypos].cells.iter_mut().skip(self.xpos) {
            *cell = cleared.clone();
        }
    }

    fn scroll_screen_up(&mut self) {
        self.lines.remove(0);
        self.lines.push(Line::with_width(self.width));
    }

    fn print_text(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\r' {
                self.xpos = 0;
                continue;
            }

            if c == '\n' {
                let new_y = self.ypos + 1;
                if new_y >= self.height {
                    self.scroll_screen_up();
                } else {
                    self.ypos = new_y;
                }
                continue;
            }

            if self.xpos >= self.width {
                let new_y = self.ypos + 1;
                if new_y >= self.height {
                    self.scroll_screen_up();
                } else {
                    self.ypos = new_y;
                }
                self.xpos = 0;
            }

            self.lines[self.ypos].cells[self.xpos] = Cell::new(c, self.attributes.clone());

            // Increment the position now; we'll defer processing
            // wrapping until the next printed character, otherwise
            // we'll eagerly scroll when we reach the right margin.
            self.xpos += 1;
        }
    }

    fn change_attribute(&mut self, change: &AttributeChange) {
        use cell::AttributeChange::*;
        match change {
            Intensity(value) => {
                self.attributes.set_intensity(*value);
            }
            Underline(value) => {
                self.attributes.set_underline(*value);
            }
            Italic(value) => {
                self.attributes.set_italic(*value);
            }
            Blink(value) => {
                self.attributes.set_blink(*value);
            }
            Reverse(value) => {
                self.attributes.set_reverse(*value);
            }
            StrikeThrough(value) => {
                self.attributes.set_strikethrough(*value);
            }
            Invisible(value) => {
                self.attributes.set_invisible(*value);
            }
            Foreground(value) => self.attributes.foreground = *value,
            Background(value) => self.attributes.background = *value,
            Hyperlink(value) => self.attributes.hyperlink = value.clone(),
        }
    }

    fn set_cursor_pos(&mut self, x: &Position, y: &Position) {
        self.xpos = compute_position_change(self.xpos, x, self.width);
        self.ypos = compute_position_change(self.ypos, y, self.height);
    }

    /// Returns the entire contents of the screen as a string.
    /// Only the character data is returned.  The end of each line is
    /// returned as a \n character.
    /// This function exists primarily for testing purposes.
    pub fn screen_chars_to_string(&self) -> String {
        let mut s = String::new();

        for line in &self.lines {
            for cell in &line.cells {
                s.push(cell.char());
            }
            s.push('\n');
        }

        s
    }

    /// Returns the cell data for the screen.
    /// This is intended to be used for testing purposes.
    pub fn screen_cells(&self) -> Vec<&[Cell]> {
        let mut lines = Vec::new();
        for line in &self.lines {
            lines.push(line.cells.as_slice());
        }
        lines
    }

    /// Returns a stream of changes suitable to update the screen
    /// to match the model.  The input `seq` argument should be 0
    /// on the first call, or in any situation where the screen
    /// contents may have been invalidated, otherwise it should
    /// be set to the `SequenceNo` returned by the most recent call
    /// to `get_changes`.
    /// `get_changes` will use a heuristic to decide on the lower
    /// cost approach to updating the screen and return some sequence
    /// of `Change` entries that will update the display accordingly.
    /// The worst case is that this function will fabricate a sequence
    /// of Change entries to paint the screen from scratch.
    pub fn get_changes(&self, seq: SequenceNo) -> (SequenceNo, Cow<[Change]>) {
        // Do we have continuity in the sequence numbering?
        let first = self.seqno.saturating_sub(self.changes.len());
        if seq == 0 || first > seq || self.seqno == 0 {
            // No, we have folded away some data, we'll need a full paint
            return (self.seqno, Cow::Owned(self.repaint_all()));
        }

        // Approximate cost to render the change screen
        let delta_cost = self.seqno - seq;
        // Approximate cost to repaint from scratch
        let full_cost = self.estimate_full_paint_cost();

        if delta_cost > full_cost {
            (self.seqno, Cow::Owned(self.repaint_all()))
        } else {
            (self.seqno, Cow::Borrowed(&self.changes[seq - first..]))
        }
    }

    /// After having called `get_changes` and processed the resultant
    /// change stream, the caller can then pass the returned `SequenceNo`
    /// value to this call to prune the list of changes and free up
    /// resources from the change log.
    pub fn flush_changes_older_than(&mut self, seq: SequenceNo) {
        let first = self.seqno.saturating_sub(self.changes.len());
        let idx = seq - first;
        if idx > self.changes.len() {
            return;
        }
        self.changes = self.changes.split_off(seq - first);
    }

    /// Without allocating resources, estimate how many Change entries
    /// we would produce in repaint_all for the current state.
    fn estimate_full_paint_cost(&self) -> usize {
        // assume 1 per cell with 20% overhead for attribute changes
        3 + (((self.width * self.height) as f64) * 1.2) as usize
    }

    fn repaint_all(&self) -> Vec<Change> {
        let mut result = Vec::new();

        // Home the cursor and clear the screen to defaults
        result.push(Change::ClearScreen(Default::default()));

        let mut attr = CellAttributes::default();

        let crlf = Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Relative(1),
        };

        // Walk backwards through the lines; the goal is to determine
        // if the screen ends with a number of clear lines that we
        // can coalesce together as a ClearToEndOfScreen op.
        // We track the index (from the end) of the last matching
        // run, together with the color of that run.
        let mut trailing_color = None;
        let mut trailing_idx = None;

        for (idx, line) in self.lines.iter().rev().enumerate() {
            let mut changes = line.changes(&attr);
            if changes.len() == 0 {
                // The line recorded no changes; this means that the line
                // consists of spaces and the default background color
                match trailing_color {
                    Some(other) if other != Default::default() => {
                        // Color doesn't match up, so we have to stop
                        // looking for the ClearToEndOfScreen run here
                        break;
                    }
                    // Color does match
                    Some(_) => continue,
                    // we don't have a run, we should start one
                    None => {
                        trailing_color = Some(Default::default());
                        trailing_idx = Some(idx);
                        continue;
                    }
                }
            } else {
                let last_change = changes.len() - 1;
                match (&changes[last_change], trailing_color) {
                    (&Change::ClearToEndOfLine(ref color), None) => {
                        trailing_color = Some(color.clone());
                        trailing_idx = Some(idx);
                    }
                    (&Change::ClearToEndOfLine(ref color), Some(other)) => {
                        if other == *color {
                            trailing_idx = Some(idx);
                            continue;
                        } else {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }

        for (idx, line) in self.lines.iter().enumerate() {
            match trailing_idx {
                Some(t) if self.height - t == idx => {
                    let color =
                        trailing_color.expect("didn't set trailing_color along with trailing_idx");

                    // The first in the sequence of the ClearToEndOfLine may
                    // be batched up here; let's remove it if that is the case.
                    let last_result = result.len() - 1;
                    match result[last_result] {
                        Change::ClearToEndOfLine(col) if col == color => {
                            result.remove(last_result);
                        }
                        _ => {}
                    }

                    result.push(Change::ClearToEndOfScreen(color));
                    break;
                }
                _ => {}
            }

            let mut changes = line.changes(&attr);

            let result_len = result.len();
            if changes.len() > 0 && result[result_len - 1].is_text() && changes[0].is_text() {
                // Assumption: that the output has working automatic margins.
                // We can skip the cursor position change and just join the
                // text items together
                if let Change::Text(mut prefix) = result.remove(result_len - 1) {
                    prefix.push_str(changes[0].text());
                    changes[0] = Change::Text(prefix);
                }
            } else if idx != 0 {
                // We emit a relative move at the end of each
                // line with the theory that this will translate
                // to a short \r\n sequence rather than the longer
                // absolute cursor positioning sequence
                result.push(crlf.clone());
            }

            result.append(&mut changes);
            attr = line.cells[self.width - 1].attrs().clone();
        }

        // Remove any trailing sequence of cursor movements, as we're
        // going to just finish up with an absolute move anyway.
        loop {
            let result_len = result.len();
            if result_len == 0 {
                break;
            }
            match result[result_len - 1] {
                Change::CursorPosition { .. } => {
                    result.remove(result_len - 1);
                }
                _ => break,
            }
        }

        // Place the cursor at its intended position, but only if we moved the
        // cursor.  We don't explicitly track movement but can infer it from the
        // size of the results: results will have an initial ClearScreen entry
        // that homes the cursor.  If the screen is otherwise blank there will
        // be no further entries and we don't need to emit cursor movement.
        // However, in the optimization passes above, we may have removed
        // some number of movement entries, so let's be sure to check the
        // cursor position to make sure that we don't fail to emit movement.

        let moved_cursor = result.len() != 1;
        if moved_cursor || self.xpos != 0 || self.ypos != 0 {
            result.push(Change::CursorPosition {
                x: Position::Absolute(self.xpos),
                y: Position::Absolute(self.ypos),
            });
        }

        result
    }

    /// Computes the change stream required to make the region within `self`
    /// at coordinates `x`, `y` and size `width`, `height` look like the
    /// same sized region within `other` at coordinates `other_x`, `other_y`.
    ///
    /// `other` and `self` may be the same, causing regions within the same
    /// `Surface` to be differenced; this is used by the `copy_region` method.
    ///
    /// The returned list of `Change`s can be passed to the `add_changes` method
    /// to make the region within self match the region within other.
    /// # Panics
    /// Will panic if the regions of interest are not within the bounds of
    /// their respective `Surface`.
    pub fn diff_region(
        &self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        other: &Surface,
        other_x: usize,
        other_y: usize,
    ) -> Vec<Change> {
        assert!(x + width <= self.width);
        assert!(y + height <= self.height);
        assert!(other_x + width <= other.width);
        assert!(other_y + height <= other.height);

        let mut result = Vec::new();
        // Keep track of the cursor position that the change stream
        // selects for updates so that we can avoid emitting redundant
        // position changes.
        let mut cursor = None;
        // Similarly, we keep track of the cell attributes that we have
        // activated for change stream to avoid over-emitting.
        // Tracking the cursor and attributes in this way helps to coalesce
        // lines of text into simpler strings.
        let mut attr: Option<CellAttributes> = None;

        for ((row_num, line), other_line) in self.lines
            .iter()
            .enumerate()
            .skip(y)
            .take_while(|(row_num, _)| *row_num < y + height)
            .zip(other.lines.iter().skip(other_y))
        {
            for ((col_num, cell), other_cell) in line.cells
                .iter()
                .enumerate()
                .skip(x)
                .take_while(|(col_num, _)| *col_num < x + width)
                .zip(other_line.cells.iter().skip(other_x))
            {
                if cell != other_cell {
                    cursor = match cursor.take() {
                        Some((cursor_row, cursor_col))
                            if cursor_row == row_num && cursor_col == col_num - 1 =>
                        {
                            // It is on the column prior, so we don't need
                            // to explicitly move it.  Record the effective
                            // position for next time.
                            Some((row_num, col_num))
                        }
                        _ => {
                            // Need to explicitly move the cursor
                            result.push(Change::CursorPosition {
                                y: Position::Absolute(row_num),
                                x: Position::Absolute(col_num),
                            });
                            // and remember the position for next time
                            Some((row_num, col_num))
                        }
                    };

                    // we could get fancy and try to minimize the update traffic
                    // by computing a series of AttributeChange values here.
                    // For now, let's just record the new value
                    attr = match attr.take() {
                        Some(ref attr) if attr == other_cell.attrs() => {
                            // Active attributes match, so we don't need
                            // to emit a change for them
                            Some(attr.clone())
                        }
                        _ => {
                            // Attributes are different
                            result.push(Change::AllAttributes(other_cell.attrs().clone()));
                            Some(other_cell.attrs().clone())
                        }
                    };
                    if cell.char() != other_cell.char() {
                        // A little bit of bloat in the code to avoid runs of single
                        // character Text entries; just append to the string.
                        let result_len = result.len();
                        if result_len > 0 && result[result_len - 1].is_text() {
                            if let Some(Change::Text(ref mut prefix)) =
                                result.get_mut(result_len - 1)
                            {
                                prefix.push(other_cell.char());
                            }
                        } else {
                            result.push(Change::Text(other_cell.char().to_string()));
                        }
                    }
                }
            }
        }

        result
    }

    /// Computes the change stream required to make `self` have the same
    /// screen contents as `other`.
    pub fn diff_screens(&self, other: &Surface) -> Vec<Change> {
        self.diff_region(0, 0, self.width, self.height, other, 0, 0)
    }

    /// Draw the contents of `other` into self at the specified coordinates.
    /// The required updates are recorded as Change entries as well as stored
    /// in the screen line/cell data.
    pub fn draw_from_screen(&mut self, other: &Surface, x: usize, y: usize) -> SequenceNo {
        let changes = self.diff_region(x, y, other.width, other.height, other, 0, 0);
        self.add_changes(changes)
    }

    /// Copy the contents of the specified region to the same sized
    /// region elsewhere in the screen display.
    /// The regions may overlap.
    /// # Panics
    /// The destination region must be the same size as the source
    /// (which is implied by the function parameters) and must fit
    /// within the width and height of the Surface or this operation
    /// will panic.
    pub fn copy_region(
        &mut self,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        dest_x: usize,
        dest_y: usize,
    ) -> SequenceNo {
        let changes = self.diff_region(dest_x, dest_y, width, height, self, src_x, src_y);
        self.add_changes(changes)
    }
}

/// Applies a Position update to either the x or y position.
/// The value is clamped to be in the range: 0..limit
fn compute_position_change(current: usize, pos: &Position, limit: usize) -> usize {
    use self::Position::*;
    match pos {
        NoChange => min(current, limit - 1),
        Relative(delta) => {
            if *delta > 0 {
                min(current.saturating_add(*delta as usize), limit - 1)
            } else {
                current.saturating_sub((*delta).abs() as usize)
            }
        }
        Absolute(abs) => min(*abs, limit - 1),
        EndRelative(delta) => limit.saturating_sub(*delta),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cell::Intensity;
    use color::AnsiColor;

    // The \x20's look a little awkward, but we can't use a plain
    // space in the first chararcter of a multi-line continuation;
    // it gets eaten up and ignored.

    #[test]
    fn basic_print() {
        let mut s = Surface::new(4, 3);
        assert_eq!(
            s.screen_chars_to_string(),
            "\x20\x20\x20\x20\n\
             \x20\x20\x20\x20\n\
             \x20\x20\x20\x20\n"
        );

        s.add_change("w00t");
        assert_eq!(
            s.screen_chars_to_string(),
            "w00t\n\
             \x20\x20\x20\x20\n\
             \x20\x20\x20\x20\n"
        );

        s.add_change("foo");
        assert_eq!(
            s.screen_chars_to_string(),
            "w00t\n\
             foo\x20\n\
             \x20\x20\x20\x20\n"
        );

        s.add_change("baar");
        assert_eq!(
            s.screen_chars_to_string(),
            "w00t\n\
             foob\n\
             aar\x20\n"
        );

        s.add_change("baz");
        assert_eq!(
            s.screen_chars_to_string(),
            "foob\n\
             aarb\n\
             az\x20\x20\n"
        );
    }

    #[test]
    fn newline() {
        let mut s = Surface::new(4, 4);
        s.add_change("bloo\rwat\n hey\r\nho");
        assert_eq!(
            s.screen_chars_to_string(),
            "wato\n\
             \x20\x20\x20\x20\n\
             hey \n\
             ho  \n"
        );
    }

    #[test]
    fn clear_screen() {
        let mut s = Surface::new(2, 2);
        s.add_change("hello");
        s.add_change(Change::ClearScreen(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "  \n  \n");
    }

    #[test]
    fn clear_eol() {
        let mut s = Surface::new(3, 3);
        s.add_change("helwowfoo");
        s.add_change(Change::ClearToEndOfLine(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "hel\nwow\nfoo\n");
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        });
        s.add_change(Change::ClearToEndOfLine(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "   \nwow\nfoo\n");
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(1),
            y: Position::Absolute(1),
        });
        s.add_change(Change::ClearToEndOfLine(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "   \nw  \nfoo\n");
    }

    #[test]
    fn clear_eos() {
        let mut s = Surface::new(3, 3);
        s.add_change("helwowfoo");
        s.add_change(Change::ClearToEndOfScreen(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "hel\nwow\nfoo\n");
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(1),
            y: Position::Absolute(1),
        });
        s.add_change(Change::ClearToEndOfScreen(Default::default()));
        assert_eq!(s.screen_chars_to_string(), "hel\nw  \n   \n");

        let (_seq, changes) = s.get_changes(0);
        assert_eq!(
            &[
                Change::ClearScreen(Default::default()),
                Change::Text("helw".into()),
                Change::CursorPosition {
                    x: Position::Absolute(1),
                    y: Position::Absolute(1),
                },
            ],
            &*changes
        );
    }

    #[test]
    fn clear_eos_back_color() {
        let mut s = Surface::new(3, 3);
        s.add_change(Change::ClearScreen(AnsiColor::Red.into()));
        s.add_change("helwowfoo");
        assert_eq!(s.screen_chars_to_string(), "hel\nwow\nfoo\n");
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(1),
            y: Position::Absolute(1),
        });
        s.add_change(Change::ClearToEndOfScreen(AnsiColor::Red.into()));
        assert_eq!(s.screen_chars_to_string(), "hel\nw  \n   \n");

        let (_seq, changes) = s.get_changes(0);
        assert_eq!(
            &[
                Change::ClearScreen(Default::default()),
                Change::AllAttributes(
                    CellAttributes::default()
                        .set_background(AnsiColor::Red)
                        .clone()
                ),
                Change::Text("helw".into()),
                Change::ClearToEndOfScreen(AnsiColor::Red.into()),
                Change::CursorPosition {
                    x: Position::Absolute(1),
                    y: Position::Absolute(1),
                },
            ],
            &*changes
        );
    }

    #[test]
    fn clear_eol_opt() {
        let mut s = Surface::new(3, 3);
        s.add_change(Change::Attribute(AttributeChange::Background(
            AnsiColor::Red.into(),
        )));
        s.add_change("111   333");
        let (_seq, changes) = s.get_changes(0);
        assert_eq!(
            &[
                Change::ClearScreen(Default::default()),
                Change::AllAttributes(
                    CellAttributes::default()
                        .set_background(AnsiColor::Red)
                        .clone()
                ),
                Change::Text("111".into()),
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(1),
                },
                Change::ClearToEndOfLine(AnsiColor::Red.into()),
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(1),
                },
                Change::Text("333".into()),
                Change::CursorPosition {
                    x: Position::Absolute(3),
                    y: Position::Absolute(2),
                },
            ],
            &*changes
        );
    }

    #[test]
    fn clear_and_move_cursor() {
        let mut s = Surface::new(4, 3);
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(3),
            y: Position::Absolute(2),
        });
        let (_seq, changes) = s.get_changes(0);
        assert_eq!(
            &[
                Change::ClearScreen(Default::default()),
                Change::CursorPosition {
                    x: Position::Absolute(3),
                    y: Position::Absolute(2),
                },
            ],
            &*changes
        );
    }

    #[test]
    fn cursor_movement() {
        let mut s = Surface::new(4, 3);
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(3),
            y: Position::Absolute(2),
        });
        s.add_change("X");
        assert_eq!(
            s.screen_chars_to_string(),
            "\x20\x20\x20\x20\n\
             \x20\x20\x20\x20\n\
             \x20\x20\x20X\n"
        );

        s.add_change(Change::CursorPosition {
            x: Position::Relative(-2),
            y: Position::Relative(-1),
        });
        s.add_change("-");
        assert_eq!(
            s.screen_chars_to_string(),
            "\x20\x20\x20\x20\n\
             \x20\x20-\x20\n\
             \x20\x20\x20X\n"
        );

        s.add_change(Change::CursorPosition {
            x: Position::Relative(1),
            y: Position::Relative(-1),
        });
        s.add_change("-");
        assert_eq!(
            s.screen_chars_to_string(),
            "\x20\x20\x20-\n\
             \x20\x20-\x20\n\
             \x20\x20\x20X\n"
        );
    }

    #[test]
    fn attribute_setting() {
        use cell::Intensity;

        let mut s = Surface::new(3, 1);
        s.add_change("n");
        s.add_change(AttributeChange::Intensity(Intensity::Bold));
        s.add_change("b");

        let mut bold = CellAttributes::default();
        bold.set_intensity(Intensity::Bold);

        assert_eq!(
            s.screen_cells(),
            [[
                Cell::new('n', CellAttributes::default()),
                Cell::new('b', bold),
                Cell::default(),
            ]]
        );
    }

    #[test]
    fn empty_changes() {
        let s = Surface::new(4, 3);

        let empty = &[Change::ClearScreen(Default::default())];

        let (seq, changes) = s.get_changes(0);
        assert_eq!(seq, 0);
        assert_eq!(empty, &*changes);

        // Using an invalid sequence number should get us the full
        // repaint also.
        let (seq, changes) = s.get_changes(1);
        assert_eq!(seq, 0);
        assert_eq!(empty, &*changes);
    }

    #[test]
    fn add_changes_empty() {
        let mut s = Surface::new(2, 2);
        let last_seq = s.add_change("foo");
        assert_eq!(0, last_seq);
        assert_eq!(last_seq, s.add_changes(vec![]));
        assert_eq!(last_seq + 1, s.add_changes(vec![Change::Text("a".into())]));
    }

    #[test]
    fn resize_delta_flush() {
        let mut s = Surface::new(4, 3);
        s.add_change("a");
        let (seq, _) = s.get_changes(0);
        s.resize(2, 2);

        let full = &[
            Change::ClearScreen(Default::default()),
            Change::Text("a".to_string()),
            Change::CursorPosition {
                x: Position::Absolute(1),
                y: Position::Absolute(0),
            },
        ];

        let (_seq, changes) = s.get_changes(seq);
        // The resize causes get_changes to return a full repaint
        assert_eq!(full, &*changes);
    }

    #[test]
    fn dont_lose_first_char_on_attr_change() {
        let mut s = Surface::new(2, 2);
        s.add_change(Change::Attribute(AttributeChange::Foreground(
            AnsiColor::Maroon.into(),
        )));
        s.add_change("ab");
        let (_seq, changes) = s.get_changes(0);
        assert_eq!(
            &[
                Change::ClearScreen(Default::default()),
                Change::AllAttributes(
                    CellAttributes::default()
                        .set_foreground(AnsiColor::Maroon)
                        .clone()
                ),
                Change::Text("ab".into()),
                Change::CursorPosition {
                    x: Position::Absolute(2),
                    y: Position::Absolute(0),
                },
            ],
            &*changes
        );
    }

    #[test]
    fn resize_cursor_position() {
        let mut s = Surface::new(4, 4);

        s.add_change(" a");
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(3),
            y: Position::Absolute(3),
        });

        assert_eq!(s.xpos, 3);
        assert_eq!(s.ypos, 3);
        s.resize(2, 2);
        assert_eq!(s.xpos, 1);
        assert_eq!(s.ypos, 1);

        let full = &[
            Change::ClearScreen(Default::default()),
            Change::Text(" a".to_string()),
            Change::CursorPosition {
                x: Position::Absolute(1),
                y: Position::Absolute(1),
            },
        ];

        let (_seq, changes) = s.get_changes(0);
        assert_eq!(full, &*changes);
    }

    #[test]
    fn delta_change() {
        let mut s = Surface::new(4, 3);
        // flushing nothing should be a NOP
        s.flush_changes_older_than(0);

        // check that using an invalid index doesn't panic
        s.flush_changes_older_than(1);

        let initial = &[
            Change::ClearScreen(Default::default()),
            Change::Text("a".to_string()),
            Change::CursorPosition {
                x: Position::Absolute(1),
                y: Position::Absolute(0),
            },
        ];

        let seq_pos = {
            let next_seq = s.add_change("a");
            let (seq, changes) = s.get_changes(0);
            assert_eq!(seq, next_seq + 1);
            assert_eq!(initial, &*changes);
            seq
        };

        let seq_pos = {
            let next_seq = s.add_change("b");
            let (seq, changes) = s.get_changes(seq_pos);
            assert_eq!(seq, next_seq + 1);
            assert_eq!(&[Change::Text("b".to_string())], &*changes);
            seq
        };

        // prep some deltas for the loop to test below
        {
            s.add_change(Change::Attribute(AttributeChange::Intensity(
                Intensity::Bold,
            )));
            s.add_change("c");
            s.add_change(Change::Attribute(AttributeChange::Intensity(
                Intensity::Normal,
            )));
            s.add_change("d");
        }

        // Do this three times to ennsure that the behavior is consistent
        // across multiple flush calls
        for _ in 0..3 {
            {
                let (_seq, changes) = s.get_changes(seq_pos);

                assert_eq!(
                    &[
                        Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                        Change::Text("c".to_string()),
                        Change::Attribute(AttributeChange::Intensity(Intensity::Normal)),
                        Change::Text("d".to_string()),
                    ],
                    &*changes
                );
            }

            // Flush the changes so that the next iteration is run on a pruned
            // set of changes.  It should not change the outcome of the body
            // of the loop.
            s.flush_changes_older_than(seq_pos);
        }
    }

    #[test]
    fn diff_screens() {
        let mut s = Surface::new(4, 3);
        s.add_change("w00t");
        s.add_change("foo");
        s.add_change("baar");
        s.add_change("baz");
        assert_eq!(
            s.screen_chars_to_string(),
            "foob\n\
             aarb\n\
             az  \n"
        );

        let s2 = Surface::new(2, 2);

        {
            // We want to sample the top left corner
            let changes = s2.diff_region(0, 0, 2, 2, &s, 0, 0);
            assert_eq!(
                vec![
                    Change::CursorPosition {
                        x: Position::Absolute(0),
                        y: Position::Absolute(0),
                    },
                    Change::AllAttributes(CellAttributes::default()),
                    Change::Text("fo".into()),
                    Change::CursorPosition {
                        x: Position::Absolute(0),
                        y: Position::Absolute(1),
                    },
                    Change::Text("aa".into()),
                ],
                changes
            );
        }

        // Throw in some attribute changes too
        s.add_change(Change::CursorPosition {
            x: Position::Absolute(1),
            y: Position::Absolute(1),
        });
        s.add_change(Change::Attribute(AttributeChange::Intensity(
            Intensity::Bold,
        )));
        s.add_change("XO");

        {
            let changes = s2.diff_region(0, 0, 2, 2, &s, 1, 1);
            assert_eq!(
                vec![
                    Change::CursorPosition {
                        x: Position::Absolute(0),
                        y: Position::Absolute(0),
                    },
                    Change::AllAttributes(
                        CellAttributes::default()
                            .set_intensity(Intensity::Bold)
                            .clone(),
                    ),
                    Change::Text("XO".into()),
                    Change::CursorPosition {
                        x: Position::Absolute(0),
                        y: Position::Absolute(1),
                    },
                    Change::AllAttributes(CellAttributes::default()),
                    Change::Text("z".into()),
                    /* There's no change for the final character
                     * position because it is a space in both regions. */
                ],
                changes
            );
        }
    }

    #[test]
    fn draw_screens() {
        let mut s = Surface::new(4, 4);

        let mut s1 = Surface::new(2, 2);
        s1.add_change("1234");

        let mut s2 = Surface::new(2, 2);
        s2.add_change("XYZA");

        s.draw_from_screen(&s1, 0, 0);
        s.draw_from_screen(&s2, 2, 2);

        assert_eq!(
            s.screen_chars_to_string(),
            "12  \n\
             34  \n\
             \x20\x20XY\n\
             \x20\x20ZA\n"
        );
    }

    #[test]
    fn copy_region() {
        let mut s = Surface::new(4, 3);
        s.add_change("w00t");
        s.add_change("foo");
        s.add_change("baar");
        s.add_change("baz");
        assert_eq!(
            s.screen_chars_to_string(),
            "foob\n\
             aarb\n\
             az  \n"
        );

        // Copy top left to bottom left
        s.copy_region(0, 0, 2, 2, 2, 1);
        assert_eq!(
            s.screen_chars_to_string(),
            "foob\n\
             aafo\n\
             azaa\n"
        );
    }
}
