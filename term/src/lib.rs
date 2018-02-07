//! Terminal model
#![feature(slice_patterns)]

#[allow(unused_imports)]
#[macro_use]
extern crate failure;
#[macro_use]
extern crate bitflags;
extern crate unicode_width;
extern crate unicode_segmentation;
extern crate vte;

use failure::Error;
use std::ops::{Deref, DerefMut, Range};

#[macro_use]
mod debug;

/// Represents the index into screen.lines.  Index 0 is the top of
/// the scrollback (if any).  The index of the top of the visible screen
/// depends on the terminal dimensions and the scrollback size.
pub type PhysRowIndex = usize;

/// Represents an index into the visible portion of the screen.
/// Value 0 is the first visible row.  VisibleRowIndex needs to be
/// resolved into a PhysRowIndex to obtain an actual row.  It is not
/// valid to have a negative VisibleRowIndex value so this type logically
/// should be unsigned, however, having a different sign is helpful to
/// have the compiler catch accidental arithmetic performed between
/// PhysRowIndex and VisibleRowIndex.  We could define our own type with
/// its own Add and Sub operators, but then we'd not be able to iterate
/// over Ranges of these types without also laboriously implementing an
/// iterator Skip trait that is currently only in unstable rust.
pub type VisibleRowIndex = i64;

/// range.contains(), but that is unstable
fn in_range<T: PartialOrd>(value: T, range: &Range<T>) -> bool {
    value >= range.start && value < range.end
}

/// Position allows referring to an absolute visible row number
/// or a position relative to some existing row number (typically
/// where the cursor is located).  Both of the cases are represented
/// as signed numbers so that the math and error checking for out
/// of range values can be deferred to the point where we execute
/// the request.
#[derive(Debug)]
pub enum Position {
    Absolute(VisibleRowIndex),
    Relative(i64),
}

/// Describes the location of the cursor in the visible portion
/// of the screen.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct CursorPosition {
    pub x: usize,
    pub y: VisibleRowIndex,
}

pub mod color;
mod csi;
use self::csi::*;

#[cfg(test)]
mod test;

/// The response we given when queries for device attributes.
/// This particular string says "we are a VT102".
/// TODO: Consider VT220 extended response which can advertise
/// certain feature sets.
pub const DEVICE_IDENT: &[u8] = b"\x1b[?6c";

#[allow(dead_code)]
pub const CSI: &[u8] = b"\x1b[";
#[allow(dead_code)]
pub const OSC: &[u8] = b"\x1b]";
#[allow(dead_code)]
pub const ST: &[u8] = b"\x1b\\";
#[allow(dead_code)]
pub const DCS: &[u8] = b"\x1bP";

bitflags! {
    #[derive(Default)]
    pub struct KeyModifiers :u8{
        const CTRL = 1;
        const ALT = 2;
        const META = 4;
        const SUPER = 8;
        const SHIFT = 16;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Unknown,
    Control,
    Alt,
    Meta,
    Super,
    Hyper,
    Shift,
    Left,
    Up,
    Right,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub x: usize,
    pub y: VisibleRowIndex,
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CellAttributes {
    attributes: u16,
    pub foreground: color::ColorAttribute,
    pub background: color::ColorAttribute,
}

/// Define getter and setter for the attributes bitfield.
/// The first form is for a simple boolean value stored in
/// a single bit.  The $bitnum parameter specifies which bit.
/// The second form is for an integer value that occupies a range
/// of bits.  The $bitmask and $bitshift parameters define how
/// to transform from the stored bit value to the consumable
/// value.
macro_rules! bitfield {
    ($getter:ident, $setter:ident, $bitnum:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> bool {
            (self.attributes & (1 << $bitnum)) == (1 << $bitnum)
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: bool) {
            let attr_value = if value { 1 << $bitnum } else { 0 };
            self.attributes = (self.attributes & !(1 << $bitnum)) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> u16 {
            (self.attributes >> $bitshift) & $bitmask
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: u16) {
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
        }
    };

    ($getter:ident, $setter:ident, $enum:ident, $bitmask:expr, $bitshift:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub fn $getter(&self) -> $enum {
            unsafe { std::mem::transmute(((self.attributes >> $bitshift) & $bitmask) as u16)}
        }

        #[inline]
        #[allow(dead_code)]
        pub fn $setter(&mut self, value: $enum) {
            let value = value as u16;
            let clear = !($bitmask << $bitshift);
            let attr_value = (value & $bitmask) << $bitshift;
            self.attributes = (self.attributes & clear) | attr_value;
        }
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Intensity {
    Normal = 0,
    Bold = 1,
    Half = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Underline {
    None = 0,
    Single = 1,
    Double = 2,
}

impl CellAttributes {
    bitfield!(intensity, set_intensity, Intensity, 0b11, 0);
    bitfield!(underline, set_underline, Underline, 0b1100, 2);
    bitfield!(italic, set_italic, 4);
    bitfield!(blink, set_blink, 5);
    bitfield!(reverse, set_reverse, 6);
    bitfield!(strikethrough, set_strikethrough, 7);
    bitfield!(halfbright, set_halfbright, 8);
    bitfield!(invisible, set_invisible, 9);
    // Allow up to 8 different font values
    //bitfield!(font, set_font, 0b111000000, 6);
}

impl Default for CellAttributes {
    fn default() -> CellAttributes {
        CellAttributes {
            attributes: 0,
            foreground: color::ColorAttribute::Foreground,
            background: color::ColorAttribute::Background,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Cell {
    bytes: [u8; 8],
    pub attrs: CellAttributes,
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::from_char(' ', &CellAttributes::default())
    }
}

impl Cell {
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        if let Some(len) = self.bytes.iter().position(|&c| c == 0) {
            &self.bytes[0..len]
        } else {
            &self.bytes
        }
    }

    pub fn from_char(c: char, attr: &CellAttributes) -> Cell {
        let mut bytes = [0u8; 8];
        c.encode_utf8(&mut bytes);
        Cell {
            bytes,
            attrs: *attr,
        }
    }

    pub fn width(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        std::str::from_utf8(self.bytes()).unwrap_or("").width()
    }
}

impl From<char> for Cell {
    fn from(c: char) -> Cell {
        Cell::from_char(c, &CellAttributes::default())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Line {
    pub cells: Vec<Cell>,
    dirty: bool,
}

/// A CellCluster is another representation of a Line.
/// A Vec<CellCluster> is produced by walking through the Cells in
/// a line and collecting succesive Cells with the same attributes
/// together into a CellCluster instance.  Additional metadata to
/// aid in font rendering is also collected.
#[derive(Debug, Clone)]
pub struct CellCluster {
    pub attrs: CellAttributes,
    pub text: String,
    pub byte_to_cell_idx: Vec<usize>,
}

impl CellCluster {
    /// Start off a new cluster with some initial data
    fn new(attrs: CellAttributes, text: &str, cell_idx: usize) -> CellCluster {
        let mut idx = Vec::new();
        for _ in 0..text.len() {
            idx.push(cell_idx);
        }
        CellCluster {
            attrs,
            text: text.into(),
            byte_to_cell_idx: idx,
        }
    }

    /// Add to this cluster
    fn add(&mut self, text: &str, cell_idx: usize) {
        for _ in 0..text.len() {
            self.byte_to_cell_idx.push(cell_idx);
        }
        self.text.push_str(text);
    }
}

impl Line {
    /// Create a new line with the specified number of columns.
    /// Each cell has the default attributes.
    pub fn new(cols: usize) -> Line {
        let mut cells = Vec::with_capacity(cols);
        cells.resize(cols, Default::default());
        Line { cells, dirty: true }
    }

    /// Recompose line into the corresponding utf8 string.
    /// In the future, we'll want to decompose into clusters of Cells that share
    /// the same render attributes
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for c in self.cells.iter() {
            s.push_str(std::str::from_utf8(c.bytes()).unwrap_or("?"));
        }
        s
    }

    /// Compute the list of CellClusters for this line
    pub fn cluster(&self) -> Vec<CellCluster> {
        let mut last_cluster = None;
        let mut clusters = Vec::new();

        for (cell_idx, c) in self.cells.iter().enumerate() {
            let cell_str = std::str::from_utf8(c.bytes()).unwrap_or("?");

            last_cluster = match last_cluster.take() {
                None => {
                    // Start new cluster
                    Some(CellCluster::new(c.attrs, cell_str, cell_idx))
                }
                Some(mut last) => {
                    if last.attrs != c.attrs {
                        // Flush pending cluster and start a new one
                        clusters.push(last);
                        Some(CellCluster::new(c.attrs, cell_str, cell_idx))
                    } else {
                        // Add to current cluster
                        last.add(cell_str, cell_idx);
                        Some(last)
                    }
                }
            };
        }

        if let Some(cluster) = last_cluster {
            // Don't forget to include any pending cluster on the final step!
            clusters.push(cluster);
        }

        clusters
    }

    #[allow(dead_code)]
    pub fn from_text(s: &str, attrs: &CellAttributes) -> Line {
        let mut cells = Vec::new();

        for (_, sub) in unicode_segmentation::UnicodeSegmentation::grapheme_indices(s, true) {
            let mut bytes = [0u8; 8];
            let len = sub.len().min(8);
            bytes[0..len].copy_from_slice(sub.as_bytes());

            cells.push(Cell {
                bytes,
                attrs: *attrs,
            });
        }

        Line { cells, dirty: true }
    }

    #[inline]
    fn set_dirty(&mut self) {
        self.dirty = true;
    }

    #[inline]
    fn set_clean(&mut self) {
        self.dirty = false;
    }
}

impl<'a> From<&'a str> for Line {
    fn from(s: &str) -> Line {
        Line::from_text(s, &CellAttributes::default())
    }
}

/// Holds the model of a screen.  This can either be the primary screen
/// which includes lines of scrollback text, or the alternate screen
/// which holds no scrollback.  The intent is to have one instance of
/// Screen for each of these things.
#[derive(Debug, Clone)]
pub struct Screen {
    /// Holds the line data that comprises the screen contents.
    /// This is allocated with capacity for the entire scrollback.
    /// The last N lines are the visible lines, with those prior being
    /// the lines that have scrolled off the top of the screen.
    /// Index 0 is the topmost line of the screen/scrollback (depending
    /// on the current window size) and will be the first line to be
    /// popped off the front of the screen when a new line is added that
    /// would otherwise have exceeded the line capacity
    lines: Vec<Line>,

    /// Maximum number of lines of scrollback
    scrollback_size: usize,

    /// Physical, visible height of the screen (not including scrollback)
    physical_rows: usize,
    /// Physical, visible width of the screen
    physical_cols: usize,
}

impl Screen {
    /// Create a new Screen with the specified dimensions.
    /// The Cells in the viewable portion of the screen are set to the
    /// default cell attributes.
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Screen {
        let mut lines = Vec::with_capacity(physical_rows + scrollback_size);
        for _ in 0..physical_rows {
            lines.push(Line::new(physical_cols));
        }

        Screen {
            lines,
            scrollback_size,
            physical_rows,
            physical_cols,
        }
    }

    /// Resize the physical, viewable portion of the screen
    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        let capacity = physical_rows + self.scrollback_size;
        let current_capacity = self.lines.capacity();
        if capacity > current_capacity {
            self.lines.reserve(capacity - current_capacity);
        }

        if physical_rows > self.physical_rows {
            // Enlarging the viewable portion?  Add more lines at the bottom
            for _ in self.physical_rows..physical_rows {
                self.lines.push(Line::new(physical_cols));
            }
        }
        self.physical_rows = physical_rows;
        self.physical_cols = physical_cols;
    }

    /// Get mutable reference to a line, relative to start of scrollback.
    /// Sets the line dirty.
    fn line_mut(&mut self, idx: PhysRowIndex) -> &mut Line {
        let line = &mut self.lines[idx];
        line.set_dirty();
        line
    }

    /// Sets a line dirty.  The line is relative to the visible origin.
    #[inline]
    fn dirty_line(&mut self, idx: VisibleRowIndex) {
        let line_idx = self.phys_row(idx);
        if line_idx < self.lines.len() {
            self.lines[line_idx].set_dirty();
        }
    }

    /// Clears the dirty flag for a line.  The line is relative to the visible origin.
    #[inline]
    #[allow(dead_code)]
    fn clean_line(&mut self, idx: VisibleRowIndex) {
        let line_idx = self.phys_row(idx);
        self.lines[line_idx].dirty = false;
    }

    /// Returns a slice over the visible lines in the screen (no scrollback)
    #[cfg(test)]
    fn visible_lines(&self) -> &[Line] {
        let line_idx = self.lines.len() - self.physical_rows;
        &self.lines[line_idx..line_idx + self.physical_rows]
    }

    /// Set a cell.  the x and y coordinates are relative to the visible screeen
    /// origin.  0,0 is the top left.
    pub fn set_cell(
        &mut self,
        x: usize,
        y: VisibleRowIndex,
        c: char,
        attr: &CellAttributes,
    ) -> &Cell {
        let line_idx = self.phys_row(y);
        debug!(
            "set_cell x,y {},{}, line_idx = {} {} {:?}",
            x,
            y,
            line_idx,
            c,
            attr
        );

        let cells = &mut self.line_mut(line_idx).cells;
        let width = cells.len();
        // if the line isn't wide enough, pad it out with the default attributes
        if x >= width {
            cells.resize(x + 1, Cell::default());
        }
        cells[x] = Cell::from_char(c, attr);
        &cells[x]
    }

    pub fn clear_line(&mut self, y: VisibleRowIndex, cols: std::ops::Range<usize>) {
        let blank = Cell::default();
        let line_idx = self.phys_row(y);
        let line = self.line_mut(line_idx);
        let max_col = line.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            line.cells[x] = blank;
        }
    }

    /// Translate a VisibleRowIndex into a PhysRowIndex.  The resultant index
    /// will be invalidated by inserting or removing rows!
    #[inline]
    fn phys_row(&self, row: VisibleRowIndex) -> PhysRowIndex {
        assert!(row >= 0);
        (self.lines.len() - self.physical_rows) + row as usize
    }

    /// Translate a range of VisibleRowIndex to a range of PhysRowIndex.
    /// The resultant range will be invalidated by inserting or removing rows!
    #[inline]
    fn phys_range(&self, range: &Range<VisibleRowIndex>) -> Range<PhysRowIndex> {
        self.phys_row(range.start)..self.phys_row(range.end)
    }

    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    ///
    /// scroll the region up by num_rows.  Any rows that would be scrolled
    /// beyond the top get removed from the screen.
    /// In other words, we remove (top..top+num_rows) and then insert num_rows
    /// at bottom.
    /// If the top of the region is the top of the visible display, rather than
    /// removing the lines we let them go into the scrollback.
    fn scroll_up(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        let phys_scroll = self.phys_range(&scroll_region);
        assert!(num_rows <= phys_scroll.end - phys_scroll.start);

        // Invalidate the lines that will move before they move so that
        // the indices of the lines are stable (we may remove lines below)
        for y in phys_scroll.clone() {
            self.line_mut(y).set_dirty();
        }

        // if we're going to remove lines due to lack of scrollback capacity,
        // remember how many so that we can adjust our insertion point later.
        let lines_removed = if scroll_region.start > 0 {
            // No scrollback available for these;
            // Remove the scrolled lines
            num_rows
        } else {
            let max_allowed = self.physical_rows + self.scrollback_size;
            if self.lines.len() + num_rows >= max_allowed {
                (self.lines.len() + num_rows) - max_allowed
            } else {
                0
            }
        };

        // Perform the removal
        for _ in 0..lines_removed {
            self.lines.remove(phys_scroll.start);
        }

        if scroll_region.start == 0 {
            // All of the lines above the top are now effectively dirty because
            // they were moved into scrollback by the scroll operation.
            for y in 0..phys_scroll.start {
                self.line_mut(y).set_dirty();
            }
        }

        for _ in 0..num_rows {
            self.lines.insert(
                phys_scroll.end - lines_removed,
                Line::new(self.physical_cols),
            );
        }
    }

    /// ---------
    /// |
    /// |--- top
    /// |
    /// |--- bottom
    ///
    /// scroll the region down by num_rows.  Any rows that would be scrolled
    /// beyond the bottom get removed from the screen.
    /// In other words, we remove (bottom-num_rows..bottom) and then insert num_rows
    /// at scroll_top.
    fn scroll_down(&mut self, scroll_region: &Range<VisibleRowIndex>, num_rows: usize) {
        let phys_scroll = self.phys_range(&scroll_region);
        assert!(num_rows <= phys_scroll.end - phys_scroll.start);

        let middle = phys_scroll.end - num_rows;

        // dirty the rows in the region
        for y in phys_scroll.start..middle {
            self.line_mut(y).set_dirty();
        }

        for _ in 0..num_rows {
            self.lines.remove(middle);
        }

        for _ in 0..num_rows {
            self.lines.insert(
                phys_scroll.start,
                Line::new(self.physical_cols),
            );
        }
    }
}

pub struct TerminalState {
    /// The primary screen + scrollback
    screen: Screen,
    /// The alternate screen; no scrollback
    alt_screen: Screen,
    /// Tells us which screen is active
    alt_screen_is_active: bool,
    /// The current set of attributes in effect for the next
    /// attempt to print to the display
    pen: CellAttributes,
    /// The current cursor position, relative to the top left
    /// of the screen.  0-based index.
    cursor: CursorPosition,
    saved_cursor: CursorPosition,

    /// if true, implicitly move to the next line on the next
    /// printed character
    wrap_next: bool,

    /// Some parsing operations may yield responses that need
    /// to be returned to the client.  They are collected here
    /// and this is used as the result of the advance_bytes()
    /// method.
    answerback: Vec<AnswerBack>,

    /// The scroll region
    scroll_region: Range<VisibleRowIndex>,

    /// When set, modifies the sequence of bytes sent for keys
    /// designated as cursor keys.  This includes various navigation
    /// keys.  The code in key_down() is responsible for interpreting this.
    application_cursor_keys: bool,

    /// When set, modifies the sequence of bytes sent for keys
    /// in the numeric keypad portion of the keyboard.
    application_keypad: bool,

    /// When set, pasting the clipboard should bracket the data with
    /// designated marker characters.
    bracketed_paste: bool,

    sgr_mouse: bool,
    button_event_mouse: bool,
    current_mouse_button: MouseButton,
    cursor_visible: bool,

    /// Used to compute the offset to the top of the viewport.
    /// This is used to display the scrollback of the terminal.
    /// It is distinct from the scroll_region in that the scroll region
    /// afects how the terminal output is scrolled as data is output,
    /// and the viewport_offset is used to index into the scrollback
    /// purely for display purposes.
    /// The offset is measured from the top of the physical viewable
    /// screen with larger numbers going backwards.
    viewport_offset: VisibleRowIndex,
}

impl TerminalState {
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        scrollback_size: usize,
    ) -> TerminalState {
        let screen = Screen::new(physical_rows, physical_cols, scrollback_size);
        let alt_screen = Screen::new(physical_rows, physical_cols, 0);

        TerminalState {
            screen,
            alt_screen,
            alt_screen_is_active: false,
            pen: CellAttributes::default(),
            cursor: CursorPosition::default(),
            saved_cursor: CursorPosition::default(),
            answerback: Vec::new(),
            scroll_region: 0..physical_rows as VisibleRowIndex,
            wrap_next: false,
            application_cursor_keys: false,
            application_keypad: false,
            bracketed_paste: false,
            sgr_mouse: false,
            button_event_mouse: false,
            cursor_visible: true,
            current_mouse_button: MouseButton::None,
            viewport_offset: 0,
        }
    }

    fn screen(&self) -> &Screen {
        if self.alt_screen_is_active {
            &self.alt_screen
        } else {
            &self.screen
        }
    }

    fn screen_mut(&mut self) -> &mut Screen {
        if self.alt_screen_is_active {
            &mut self.alt_screen
        } else {
            &mut self.screen
        }
    }

    pub fn mouse_event(&mut self, event: MouseEvent, host: &mut TerminalHost) -> Result<(), Error> {
        match event {
            MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelUp,
                ..
            } |
            MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelDown,
                ..
            } => {
                let (report_button, scroll_delta, key) = if event.button == MouseButton::WheelUp {
                    (64, -1, KeyCode::Up)
                } else {
                    (65, 1, KeyCode::Down)
                };

                if self.sgr_mouse {
                    write!(
                        host.writer(),
                        "\x1b[<{};{};{}M",
                        report_button,
                        event.x + 1,
                        event.y + 1
                    )?;
                } else if self.alt_screen_is_active {
                    // Send cursor keys instead (equivalent to xterm's alternateScroll mode)
                    self.key_down(key, KeyModifiers::default(), host)?;
                } else {
                    self.scroll_viewport(scroll_delta)
                }
            }
            MouseEvent { kind: MouseEventKind::Press, .. } => {
                self.current_mouse_button = event.button;
                if let Some(button) = match event.button {
                    MouseButton::Left => Some(0),
                    MouseButton::Middle => Some(1),
                    MouseButton::Right => Some(2),
                    _ => None,
                }
                {
                    if self.sgr_mouse {
                        write!(
                            host.writer(),
                            "\x1b[<{};{};{}M",
                            button,
                            event.x + 1,
                            event.y + 1
                        )?;
                    }
                }
            }
            MouseEvent { kind: MouseEventKind::Release, .. } => {
                self.current_mouse_button = MouseButton::None;
                if self.sgr_mouse {
                    write!(host.writer(), "\x1b[<3;{};{}m", event.x + 1, event.y + 1)?;
                }
            }
            MouseEvent { kind: MouseEventKind::Move, .. } => {
                if let Some(button) = match (self.current_mouse_button, self.button_event_mouse) {
                    (_, false) => None,
                    (MouseButton::Left, true) => Some(32),
                    (MouseButton::Middle, true) => Some(33),
                    (MouseButton::Right, true) => Some(34),
                    (..) => None,
                }
                {
                    if self.sgr_mouse {
                        write!(
                            host.writer(),
                            "\x1b[<{};{};{}M",
                            button,
                            event.x + 1,
                            event.y + 1
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Processes a key_down event generated by the gui/render layer
    /// that is embedding the Terminal.  This method translates the
    /// keycode into a sequence of bytes to send to the slave end
    /// of the pty via the `Write`-able object provided by the caller.
    pub fn key_down(
        &mut self,
        key: KeyCode,
        mods: KeyModifiers,
        host: &mut TerminalHost,
    ) -> Result<(), Error> {
        const CTRL: KeyModifiers = KeyModifiers::CTRL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const APPCURSOR: bool = true;
        use KeyCode::*;

        let ctrl = mods & CTRL;
        let shift = mods & SHIFT;
        let alt = mods & ALT;

        let mut buf = String::new();

        // TODO: also respect self.application_keypad

        let to_send = match (key, ctrl, alt, shift, self.application_cursor_keys) {
            (Char(c), CTRL, _, SHIFT, _) if c <= 0xff as char => {
                // If shift is held we have C == 0x43 and want to translate
                // that into 0x03
                buf.push((c as u8 - 0x40) as char);
                buf.as_str()
            }
            (Char(c), CTRL, ..) if c <= 0xff as char => {
                // If shift is not held we have C == 0x63 and want to translate
                // that into 0x03
                buf.push((c as u8 - 0x60) as char);
                buf.as_str()
            }
            (Char(c), _, ALT, ..) if c <= 0xff as char => {
                buf.push((c as u8 | 0x80) as char);
                buf.as_str()
            }
            (Char(c), ..) => {
                buf.push(c);
                buf.as_str()
            }
            (Insert, _, _, SHIFT, _) => {
                let clip = host.get_clipboard()?;
                if self.bracketed_paste {
                    use std::fmt::Write;
                    write!(buf, "\x1b[200~{}\x1b[201~", clip)?;
                } else {
                    buf = clip;
                }
                buf.as_str()
            }

            (Up, _, _, _, APPCURSOR) => "\x1bOA",
            (Down, _, _, _, APPCURSOR) => "\x1bOB",
            (Right, _, _, _, APPCURSOR) => "\x1bOC",
            (Left, _, _, _, APPCURSOR) => "\x1bOD",
            (Home, _, _, _, APPCURSOR) => "\x1bOH",
            (End, _, _, _, APPCURSOR) => "\x1bOF",

            (Up, ..) => "\x1b[A",
            (Down, ..) => "\x1b[B",
            (Right, ..) => "\x1b[C",
            (Left, ..) => "\x1b[D",
            (PageUp, _, _, SHIFT, _) => {
                let rows = self.screen().physical_rows as i64;
                self.scroll_viewport(-rows);
                ""
            }
            (PageDown, _, _, SHIFT, _) => {
                let rows = self.screen().physical_rows as i64;
                self.scroll_viewport(rows);
                ""
            }
            (PageUp, ..) => "\x1b[5~",
            (PageDown, ..) => "\x1b[6~",
            (Home, ..) => "\x1b[H",
            (End, ..) => "\x1b[F",
            (Insert, ..) => "\x1b[2~",

            // Modifier keys pressed on their own and unmappable keys don't expand to anything
            (Control, ..) | (Alt, ..) | (Meta, ..) | (Super, ..) | (Hyper, ..) | (Shift, ..) |
            (Unknown, ..) => "",
        };

        host.writer().write(&to_send.as_bytes())?;

        // Reset the viewport if we sent data to the parser
        if to_send.len() > 0 && self.viewport_offset != 0 {
            // TODO: some folks like to configure this behavior.
            self.set_scroll_viewport(0);
        }

        Ok(())
    }

    pub fn key_up(
        &mut self,
        _: KeyCode,
        _: KeyModifiers,
        _: &mut TerminalHost,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.screen.resize(physical_rows, physical_cols);
        self.alt_screen.resize(physical_rows, physical_cols);
        self.scroll_region = 0..physical_rows as i64;
        self.set_scroll_viewport(0);
    }

    /// Returns true if any of the visible lines are marked dirty
    pub fn has_dirty_lines(&self) -> bool {
        let screen = self.screen();
        let height = screen.physical_rows;
        let len = screen.lines.len() - self.viewport_offset as usize;

        for line in screen.lines.iter().skip(len - height) {
            if line.dirty {
                return true;
            }
        }

        false
    }

    /// Returns the set of visible lines that are dirty.
    /// The return value is a Vec<(line_idx, line)>, where
    /// line_idx is relative to the top of the viewport
    pub fn get_dirty_lines(&self) -> Vec<(usize, &Line)> {
        let mut res = Vec::new();

        let screen = self.screen();
        let height = screen.physical_rows;
        let len = screen.lines.len() - self.viewport_offset as usize;

        for (i, mut line) in screen.lines.iter().skip(len - height).enumerate() {
            if line.dirty {
                res.push((i, &*line));
            }
        }

        res
    }

    /// Clear the dirty flag for all dirty lines
    pub fn clean_dirty_lines(&mut self) {
        let screen = self.screen_mut();
        for line in screen.lines.iter_mut() {
            line.set_clean();
        }
    }

    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    pub fn cursor_pos(&self) -> CursorPosition {
        // TODO: figure out how to expose cursor visibility; Option<CursorPosition>?
        CursorPosition {
            x: self.cursor.x,
            y: self.cursor.y + self.viewport_offset,
        }
    }

    /// Sets the cursor position. x and y are 0-based and relative to the
    /// top left of the visible screen.
    /// TODO: DEC origin mode impacts the interpreation of these
    fn set_cursor_pos(&mut self, x: &Position, y: &Position) {
        let x = match x {
            &Position::Relative(x) => (self.cursor.x as i64 + x).max(0),
            &Position::Absolute(x) => x,
        };
        let y = match y {
            &Position::Relative(y) => (self.cursor.y + y).max(0),
            &Position::Absolute(y) => y,
        };

        let rows = self.screen().physical_rows;
        let cols = self.screen().physical_cols;
        let old_y = self.cursor.y;
        let new_y = y.min(rows as i64 - 1);

        self.cursor.x = x.min(cols as i64 - 1) as usize;
        self.cursor.y = new_y;
        self.wrap_next = false;

        let screen = self.screen_mut();
        screen.dirty_line(old_y);
        screen.dirty_line(new_y);
    }

    fn set_scroll_viewport(&mut self, position: VisibleRowIndex) {
        let position = position.max(0);

        let rows = self.screen().physical_rows;
        let avail_scrollback = self.screen().lines.len() - rows;

        let position = position.min(avail_scrollback as i64);

        self.viewport_offset = position;
        let top = self.screen().lines.len() - (rows + position as usize);
        let screen = self.screen_mut();
        for y in top..top + rows {
            screen.line_mut(y).set_dirty();
        }
    }

    /// Adjust the scroll position of the viewport by delta.
    /// Dirties the lines that are now in view.
    pub fn scroll_viewport(&mut self, delta: VisibleRowIndex) {
        // TODO: ignore/reset this when we switch to the alt screen

        let position = self.viewport_offset - delta;
        self.set_scroll_viewport(position);
    }

    fn scroll_up(&mut self, num_rows: usize) {
        let scroll_region = self.scroll_region.clone();
        self.screen_mut().scroll_up(&scroll_region, num_rows)
    }

    fn scroll_down(&mut self, num_rows: usize) {
        let scroll_region = self.scroll_region.clone();
        self.screen_mut().scroll_down(&scroll_region, num_rows)
    }

    fn new_line(&mut self, move_to_first_column: bool) {
        let x = if move_to_first_column {
            0
        } else {
            self.cursor.x
        };
        let y = self.cursor.y;
        let y = if y == self.scroll_region.end - 1 {
            self.scroll_up(1);
            y
        } else {
            y + 1
        };
        self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y as i64));
    }

    fn push_answerback(&mut self, buf: &[u8]) {
        self.answerback.push(AnswerBack::WriteToPty(buf.to_vec()));
    }

    /// Move the cursor up 1 line.  If the position is at the top scroll margin,
    /// scroll the region down.
    fn reverse_index(&mut self) {
        let y = self.cursor.y;
        let y = if y == self.scroll_region.start {
            self.scroll_down(1);
            y
        } else {
            y - 1
        };
        self.set_cursor_pos(&Position::Relative(0), &Position::Absolute(y as i64));
    }

    fn perform_csi(&mut self, act: CSIAction) {
        debug!("{:?}", act);
        match act {
            CSIAction::SetPen(pen) => {
                self.pen = pen;
            }
            CSIAction::SetForegroundColor(color) => {
                self.pen.foreground = color;
            }
            CSIAction::SetBackgroundColor(color) => {
                self.pen.background = color;
            }
            CSIAction::SetIntensity(level) => {
                self.pen.set_intensity(level);
            }
            CSIAction::SetUnderline(level) => {
                self.pen.set_underline(level);
            }
            CSIAction::SetItalic(on) => {
                self.pen.set_italic(on);
            }
            CSIAction::SetBlink(on) => {
                self.pen.set_blink(on);
            }
            CSIAction::SetReverse(on) => {
                self.pen.set_reverse(on);
            }
            CSIAction::SetStrikethrough(on) => {
                self.pen.set_strikethrough(on);
            }
            CSIAction::SetInvisible(on) => {
                self.pen.set_invisible(on);
            }
            CSIAction::SetCursorXY { x, y } => {
                self.set_cursor_pos(&x, &y);
            }
            CSIAction::EraseInLine(erase) => {
                let cx = self.cursor.x;
                let cy = self.cursor.y;
                let mut screen = self.screen_mut();
                let cols = screen.physical_cols;
                match erase {
                    LineErase::ToRight => {
                        screen.clear_line(cy, cx..cols);
                    }
                    LineErase::ToLeft => {
                        screen.clear_line(cy, 0..cx);
                    }
                    LineErase::All => {
                        screen.clear_line(cy, 0..cols);
                    }
                }
            }
            CSIAction::EraseInDisplay(erase) => {
                let cy = self.cursor.y;
                let mut screen = self.screen_mut();
                let cols = screen.physical_cols;
                let rows = screen.physical_rows as VisibleRowIndex;
                match erase {
                    DisplayErase::Below => {
                        for y in cy..rows {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::Above => {
                        for y in 0..cy {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::All => {
                        for y in 0..rows {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::SavedLines => {
                        println!("ed: no support for xterm Erase Saved Lines yet");
                    }
                }
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::StartBlinkingCursor, _) => {
                // ignored
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ShowCursor, on) => {
                self.cursor_visible = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ButtonEventMouse, on) => {
                self.button_event_mouse = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::SGRMouse, on) => {
                self.sgr_mouse = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ClearAndEnableAlternateScreen, on) => {
                // TODO: some folks like to disable alt screen
                match (on, self.alt_screen_is_active) {
                    (true, false) => {
                        self.perform_csi(CSIAction::SaveCursor);
                        self.alt_screen_is_active = true;
                        self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                        self.perform_csi(CSIAction::EraseInDisplay(DisplayErase::All));
                    }
                    (false, true) => {
                        self.alt_screen_is_active = false;
                        self.perform_csi(CSIAction::RestoreCursor);
                        self.set_scroll_viewport(0);
                    }
                    _ => {}
                }
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ApplicationCursorKeys, on) => {
                self.application_cursor_keys = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::BrackedPaste, on) => {
                self.bracketed_paste = on;
            }
            CSIAction::DeviceStatusReport => {
                // "OK"
                self.push_answerback(b"\x1b[0n");
            }
            CSIAction::ReportCursorPosition => {
                let row = self.cursor.y + 1;
                let col = self.cursor.x + 1;
                self.push_answerback(format!("\x1b[{};{}R", row, col).as_bytes());
            }
            CSIAction::SetScrollingRegion { top, bottom } => {
                let rows = self.screen().physical_rows;
                let mut top = top.min(rows as i64 - 1);
                let mut bottom = bottom.min(rows as i64 - 1);
                if top > bottom {
                    std::mem::swap(&mut top, &mut bottom);
                }
                self.scroll_region = top..bottom + 1;
            }
            CSIAction::RequestDeviceAttributes => {
                self.push_answerback(DEVICE_IDENT);
            }
            CSIAction::DeleteLines(n) => {
                if in_range(self.cursor.y, &self.scroll_region) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_up(&scroll_region, n as usize);
                }
            }
            CSIAction::InsertLines(n) => {
                if in_range(self.cursor.y, &self.scroll_region) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_down(&scroll_region, n as usize);
                }
            }
            CSIAction::SaveCursor => {
                self.saved_cursor = self.cursor;
            }
            CSIAction::RestoreCursor => {
                let x = self.saved_cursor.x;
                let y = self.saved_cursor.y;
                self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y));
            }
            CSIAction::LinePosition(row) => {
                self.set_cursor_pos(&Position::Relative(0), &row);
            }
            CSIAction::ScrollLines(amount) => {
                if amount > 0 {
                    self.scroll_down(amount as usize);
                } else {
                    self.scroll_up((-amount) as usize);
                }
            }
        }
    }
}

pub struct Terminal {
    /// The terminal model/state
    state: TerminalState,
    /// Baseline terminal escape sequence parser
    parser: vte::Parser,
}

impl Deref for Terminal {
    type Target = TerminalState;

    fn deref(&self) -> &TerminalState {
        &self.state
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }
}

/// When the terminal parser needs to convey a response
/// back to the caller, this enum holds that response
#[derive(Debug, Clone)]
pub enum AnswerBack {
    /// Some data to send back to the application on
    /// the slave end of the pty.
    WriteToPty(Vec<u8>),
    /// The application has requested that we change
    /// the terminal title, and here it is.
    TitleChanged(String),
}

impl Terminal {
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Terminal {
        Terminal {
            state: TerminalState::new(physical_rows, physical_cols, scrollback_size),
            parser: vte::Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes of input.
    /// The return value is a (likely empty most of the time)
    /// sequence of AnswerBack objects that may need to be rendered
    /// in the UI or sent back to the client on the slave side of
    /// the pty.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) -> Vec<AnswerBack> {
        let bytes = bytes.as_ref();
        for b in bytes.iter() {
            self.parser.advance(&mut self.state, *b);
        }
        self.answerback.drain(0..).collect()
    }
}

/// Represents the host of the terminal.
/// Provides a means for sending data to the connected pty,
/// and for operating on the clipboard
pub trait TerminalHost {
    /// Returns an object that can be used to send data to the
    /// slave end of the associated pty.
    fn writer(&mut self) -> &mut std::io::Write;

    /// Returns the current clipboard contents
    fn get_clipboard(&mut self) -> Result<String, Error>;

    /// Adjust the contents of the clipboard
    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error>;
}

impl vte::Perform for TerminalState {
    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        if self.wrap_next {
            self.new_line(true);
        }

        let x = self.cursor.x;
        let y = self.cursor.y;
        let width = self.screen().physical_cols;

        let pen = self.pen;

        // Assign the cell and extract its printable width
        let print_width = {
            let cell = self.screen_mut().set_cell(x, y, c, &pen);
            cell.width()
        };

        // for double- or triple-wide cells, the client of the terminal
        // expects the cursor to move by the visible width, which means that
        // we need to generate non-printing cells to pad out the gap.  They
        // need to be non-printing rather than space so that that renderer
        // doesn't render an actual space between the glyphs.
        for non_print_x in 1..print_width {
            self.screen_mut().set_cell(
                x + non_print_x,
                y,
                0 as char, // non-printable
                &pen,
            );
        }

        if x + print_width < width {
            self.set_cursor_pos(
                &Position::Relative(print_width as i64),
                &Position::Relative(0),
            );
        } else {
            self.wrap_next = true;
        }
    }

    fn execute(&mut self, byte: u8) {
        debug!("execute {:02x}", byte);
        match byte {
            b'\n' | 0x0b /* VT */ | 0x0c /* FF */ => {
                self.new_line(true /* TODO: depend on terminal mode */)
            }
            b'\r' => /* CR */ {
                self.set_cursor_pos(&Position::Absolute(0), &Position::Relative(0));
            }
            0x08 /* BS */ => {
                self.set_cursor_pos(&Position::Relative(-1), &Position::Relative(0));
            }
            _ => println!("unhandled vte execute {}", byte),
        }
    }
    fn hook(&mut self, _: &[i64], _: &[u8], _: bool) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, osc: &[&[u8]]) {
        match osc {
            &[b"0", title] => {
                use std::str;
                if let Ok(title) = str::from_utf8(title) {
                    self.answerback.push(
                        AnswerBack::TitleChanged(title.to_string()),
                    );
                } else {
                    println!("OSC: failed to decode utf for {:?}", title);
                }
            }
            _ => {
                println!("OSC unhandled: {:?}", osc);
            }
        }
    }
    fn csi_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: char) {
        for act in CSIParser::new(params, intermediates, ignore, byte) {
            self.perform_csi(act);
        }
    }

    fn esc_dispatch(&mut self, params: &[i64], intermediates: &[u8], _ignore: bool, byte: u8) {
        debug!(
            "ESC params={:?}, intermediates={:?} b={:02x} {}",
            params,
            intermediates,
            byte,
            byte as char
        );
        // Sequences from both of these sections show up in this handler:
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-C1-_8-Bit_-Control-Characters
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Controls-beginning-with-ESC
        match (byte, intermediates, params) {
            // String Terminator (ST); explicitly has nothing to do here, as its purpose is
            // handled by vte::Parser
            (b'\\', &[], &[]) => {}
            // Application Keypad (DECKPAM)
            (b'=', &[], &[]) => {
                self.application_keypad = true;
            }
            // Normal Keypad (DECKPAM)
            (b'>', &[], &[]) => {
                self.application_keypad = false;
            }
            // Reverse Index (RI)
            (b'M', &[], &[]) => self.reverse_index(),

            // Enable alternate character set mode (smacs)
            (b'0', &[b'('], &[]) => {}
            // Exit alternate character set mode (rmacs)
            (b'B', &[b'('], &[]) => {}

            // DECSC - Save Cursor
            (b'7', &[], &[]) => self.perform_csi(CSIAction::SaveCursor),
            // DECRC - Restore Cursor
            (b'8', &[], &[]) => self.perform_csi(CSIAction::RestoreCursor),

            (..) => {
                println!(
                    "ESC unhandled params={:?}, intermediates={:?} b={:02x} {}",
                    params,
                    intermediates,
                    byte,
                    byte as char
                );
            }
        }
    }
}
