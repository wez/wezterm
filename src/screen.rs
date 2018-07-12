use cell::{AttributeChange, Cell, CellAttributes};
use std::borrow::Cow;
use std::cmp::min;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Position {
    NoChange,
    /// Negative values move up, positive values down
    Relative(isize),
    Absolute(usize),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Change {
    Attribute(AttributeChange),
    AllAttributes(CellAttributes),
    Text(String),
    //   ClearScreen,
    //   ClearToStartOfLine,
    //   ClearToEndOfLine,
    //   ClearToEndOfScreen,
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
            }
        }

        // flush out any remaining text run
        if text_run.len() > 0 {
            // TODO: if this is just spaces then it may be cheaper
            // to emit ClearToEndOfLine here instead.
            result.push(Change::Text(text_run.clone()));
            text_run.clear();
        }

        result
    }
}

pub type SequenceNo = usize;

#[derive(Default)]
pub struct Screen {
    width: usize,
    height: usize,
    lines: Vec<Line>,
    attributes: CellAttributes,
    xpos: usize,
    ypos: usize,
    seqno: SequenceNo,
    changes: Vec<Change>,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Self {
        let mut scr = Screen {
            width,
            height,
            ..Default::default()
        };
        scr.resize(width, height);
        scr
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.lines.resize(height, Line::with_width(width));
        for line in &mut self.lines {
            line.resize(width);
        }
        self.width = width;
        self.height = height;

        // FIXME: cursor position is now undefined
    }

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
        }
    }

    fn scroll_screen_up(&mut self) {
        self.lines.remove(0);
        self.lines.push(Line::with_width(self.width));
    }

    fn print_text(&mut self, text: &str) {
        for c in text.chars() {
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
            Intensity(value) => self.attributes.set_intensity(*value),
            Underline(value) => self.attributes.set_underline(*value),
            Italic(value) => self.attributes.set_italic(*value),
            Blink(value) => self.attributes.set_blink(*value),
            Reverse(value) => self.attributes.set_reverse(*value),
            StrikeThrough(value) => self.attributes.set_strikethrough(*value),
            Invisible(value) => self.attributes.set_invisible(*value),
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
    /// to match the model.  The input seq argument should be 0
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

    /// Without allocating resources, estimate how many Change entries
    /// we would produce in repaint_all for the current state.
    fn estimate_full_paint_cost(&self) -> usize {
        // assume 1 per cell with 20% overhead for attribute changes
        3 + (((self.width * self.height) as f64) * 1.2) as usize
    }

    fn repaint_all(&self) -> Vec<Change> {
        let mut result = Vec::new();

        // Home the cursor
        result.push(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        });
        // Reset attributes back to defaults
        result.push(Change::AllAttributes(CellAttributes::default()));

        let mut attr = CellAttributes::default();

        for (idx, line) in self.lines.iter().enumerate() {
            let mut changes = line.changes(&attr);

            let result_len = result.len();
            if result[result_len - 1].is_text() && changes[0].is_text() {
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
                result.push(Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(1),
                });
            }

            result.append(&mut changes);
            attr = line.cells[self.width - 1].attrs().clone();
        }

        // Place the cursor at its intended position
        result.push(Change::CursorPosition {
            x: Position::Absolute(self.xpos),
            y: Position::Absolute(self.ypos),
        });

        result
    }
}

/// Applies a Position update to either the x or y position.
/// The value is clamped to be in the range: 0..limit
fn compute_position_change(current: usize, pos: &Position, limit: usize) -> usize {
    use self::Position::*;
    match pos {
        NoChange => current,
        Relative(delta) => {
            if *delta > 0 {
                min(current.saturating_add(*delta as usize), limit - 1)
            } else {
                current.saturating_sub((*delta).abs() as usize)
            }
        }
        Absolute(abs) => min(*abs, limit - 1),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // The \x20's look a little awkward, but we can't use a plain
    // space in the first chararcter of a multi-line continuation;
    // it gets eaten up and ignored.

    #[test]
    fn test_basic_print() {
        let mut s = Screen::new(4, 3);
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
    fn test_cursor_movement() {
        let mut s = Screen::new(4, 3);
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
    fn test_attribute_setting() {
        use cell::Intensity;

        let mut s = Screen::new(3, 1);
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
    fn test_empty_changes() {
        let s = Screen::new(4, 3);

        let empty = &[
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::AllAttributes(CellAttributes::default()),
            Change::Text("            ".to_string()),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
        ];

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
    fn test_delta_change() {
        let mut s = Screen::new(4, 3);

        let initial = &[
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::AllAttributes(CellAttributes::default()),
            Change::Text("a           ".to_string()),
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

        {
            s.add_change(Change::Attribute(AttributeChange::Intensity(
                Intensity::Bold,
            )));
            s.add_change("c");
            s.add_change(Change::Attribute(AttributeChange::Intensity(
                Intensity::Normal,
            )));
            s.add_change("d");
            let (_seq, changes) = s.get_changes(seq_pos);

            use cell::Intensity;

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
    }
}
