//! Terminal model

use failure::Error;
use std;
use std::ops::{Deref, DerefMut};
use unicode_segmentation;
use vte;

pub mod color;
mod csi;
use self::csi::*;

/// The response we given when queries for device attributes.
/// This particular string says "we are a VT102".
/// TODO: Consider VT220 extended response which can advertise
/// certain feature sets.
const DEVICE_IDENT: &[u8] = b"\x1b[?6c";

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
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy, Default)]
pub struct Cell {
    chars: [u8; 8],
    pub attrs: CellAttributes,
}

impl Cell {
    #[inline]
    pub fn chars(&self) -> &[u8] {
        if let Some(len) = self.chars.iter().position(|&c| c == 0) {
            &self.chars[0..len]
        } else {
            &self.chars
        }
    }

    pub fn from_char(c: char, attr: &CellAttributes) -> Cell {
        let mut chars = [0u8; 8];
        c.encode_utf8(&mut chars);
        Cell {
            chars,
            attrs: *attr,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub cells: Vec<Cell>,
    dirty: bool,
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
            s.push_str(std::str::from_utf8(c.chars()).unwrap_or("?"));
        }
        s
    }

    #[allow(dead_code)]
    pub fn from_text(s: &str, attrs: &CellAttributes) -> Line {
        let mut cells = Vec::new();

        for (_, sub) in unicode_segmentation::UnicodeSegmentation::grapheme_indices(s, true) {
            let mut chars = [0u8; 8];
            let len = sub.len().min(8);
            chars[0..len].copy_from_slice(sub.as_bytes());

            cells.push(Cell {
                chars,
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
    fn line_mut(&mut self, idx: usize) -> &mut Line {
        let line = &mut self.lines[idx];
        line.set_dirty();
        line
    }

    /// Sets a line dirty.  The line is relative to the visible origin.
    #[inline]
    fn dirty_line(&mut self, idx: usize) {
        let line_idx = (self.lines.len() - self.physical_rows) + idx;
        self.lines[line_idx].set_dirty();
    }

    /// Clears the dirty flag for a line.  The line is relative to the visible origin.
    #[inline]
    #[allow(dead_code)]
    fn clean_line(&mut self, idx: usize) {
        let line_idx = (self.lines.len() - self.physical_rows) + idx;
        self.lines[line_idx].dirty = false;
    }

    /// Set a cell.  the x and y coordinates are relative to the visible screeen
    /// origin.  0,0 is the top left.
    pub fn set_cell(&mut self, x: usize, y: usize, c: char, attr: &CellAttributes) {
        let line_idx = (self.lines.len() - self.physical_rows) + y;
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
            cells.resize(x + 1, Cell::from_char(' ', &CellAttributes::default()));
        }
        cells[x] = Cell::from_char(c, attr);
    }

    pub fn clear_line(&mut self, y: usize, cols: std::ops::Range<usize>) {
        let blank = Cell::from_char(' ', &CellAttributes::default());
        let line_idx = (self.lines.len() - self.physical_rows) + y;
        let line = self.line_mut(line_idx);
        let max_col = line.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            line.cells[x] = blank;
        }
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
    fn scroll_up(&mut self, scroll_top: usize, scroll_bottom: usize, num_rows: usize) {
        let origin_row = self.lines.len() - self.physical_rows;
        let top_idx = origin_row + scroll_top;
        let bot_idx = origin_row + scroll_bottom;

        // Invalidate the lines that will move before they move so that
        // the indices of the lines are stable (we may remove lines below)
        for y in top_idx + num_rows..bot_idx {
            self.line_mut(y).set_dirty();
        }

        if scroll_top > 0 {
            // No scrollback available for these;
            // Remove the scrolled lines
            for _ in 0..num_rows {
                self.lines.remove(top_idx);
            }
        } else {
            // The lines at the top will move into the scrollback.
            // Let's check to make sure that we don't exceed the capacity

            let max_allowed = self.physical_rows + self.scrollback_size;
            if self.lines.len() + num_rows >= max_allowed {
                // Any rows that get pushed out of scrollback get removed
                let lines_to_pop = (self.lines.len() + num_rows) - max_allowed;
                for _ in 0..lines_to_pop {
                    self.lines.remove(0);
                }
            }

            // All of the lines above the top are now effectively dirty because
            // they were moved by the scroll operation.
            for y in 0..top_idx {
                self.line_mut(y).set_dirty();
            }
        }

        let insertion = if scroll_bottom == self.physical_rows - 1 {
            // Insert AFTER the bottom, otherwise we'll push down the last row!
            bot_idx + 1
        } else {
            // if we're scrolling within the screen, the bottom is the bottom
            bot_idx
        };
        for _ in 0..num_rows {
            self.lines.insert(insertion, Line::new(self.physical_cols));
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
    fn scroll_down(&mut self, scroll_top: usize, scroll_bottom: usize, num_rows: usize) {
        let top_idx = (self.lines.len() - self.physical_rows) + scroll_top;
        let bot_idx = (self.lines.len() - self.physical_rows) + scroll_bottom;

        let bottom = bot_idx - num_rows;
        for _ in bottom..bot_idx {
            self.lines.remove(bottom);
        }

        for y in top_idx..bot_idx {
            self.line_mut(y).set_dirty();
        }

        for _ in 0..num_rows {
            self.lines.insert(top_idx, Line::new(self.physical_cols));
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
    cursor_x: usize,
    cursor_y: usize,
    /// if true, implicitly move to the next line on the next
    /// printed character
    wrap_next: bool,

    /// Some parsing operations may yield responses that need
    /// to be returned to the client.  They are collected here
    /// and this is used as the result of the advance_bytes()
    /// method.
    answerback: Option<Vec<u8>>,

    /// The scroll region
    scroll_top: usize,
    scroll_bottom: usize,
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
            cursor_x: 0,
            cursor_y: 0,
            answerback: None,
            scroll_top: 0,
            scroll_bottom: physical_rows - 1,
            wrap_next: false,
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

    pub fn key_down<W: std::io::Write>(
        &mut self,
        key: KeyCode,
        mods: KeyModifiers,
        write: &mut W,
    ) -> Result<(), Error> {
        match key {
            KeyCode::Char(c) => {
                let adjusted = if mods.contains(KeyModifiers::CTRL) && c <= 0xff as char {
                    if mods.contains(KeyModifiers::SHIFT) {
                        // If shift is held we have C == 0x43 and want to translate
                        // that into 0x03
                        (c as u8 - 0x40) as char
                    } else {
                        // If shift is not held we have c == 0x63 and want to translate
                        // that into 0x03
                        (c as u8 - 0x60) as char
                    }
                } else if mods.contains(KeyModifiers::ALT) && c <= 0xff as char {
                    (c as u8 | 0x80) as char
                } else {
                    c
                };

                let mut buf = [0; 8];
                let encoded = adjusted.encode_utf8(&mut buf);
                write.write(encoded.as_bytes())?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn key_up<W: std::io::Write>(
        &mut self,
        _: KeyCode,
        _: KeyModifiers,
        _: &mut W,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.screen.resize(physical_rows, physical_cols);
        self.alt_screen.resize(physical_rows, physical_cols);
    }

    /// Returns true if any of the visible lines are marked dirty
    pub fn has_dirty_lines(&self) -> bool {
        let screen = self.screen();
        let height = screen.physical_rows;
        let len = screen.lines.len();

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
        let len = screen.lines.len();

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
    pub fn cursor_pos(&self) -> (usize, usize) {
        (self.cursor_x, self.cursor_y)
    }

    /// Sets the cursor position. x and y are 0-based and relative to the
    /// top left of the visible screen.
    /// TODO: DEC origin mode impacts the interpreation of these
    fn set_cursor_pos(&mut self, x: usize, y: usize) {
        let rows = self.screen().physical_rows;
        let old_y = self.cursor_y;
        let new_y = y.min(rows - 1);

        self.cursor_x = x;
        self.cursor_y = new_y;
        self.wrap_next = false;

        let screen = self.screen_mut();
        screen.dirty_line(old_y);
        screen.dirty_line(new_y);
    }

    fn delta_cursor_pos(&mut self, x: i64, y: i64) {
        let x = self.cursor_x as i64 + x;
        let y = self.cursor_y as i64 + y;
        self.set_cursor_pos(x as usize, y as usize)
    }

    fn scroll_up(&mut self, num_rows: usize) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        self.screen_mut().scroll_up(top, bottom, num_rows)
    }

    fn scroll_down(&mut self, num_rows: usize) {
        let top = self.scroll_top;
        let bottom = self.scroll_bottom;
        self.screen_mut().scroll_down(top, bottom, num_rows)
    }

    fn new_line(&mut self, move_to_first_column: bool) {
        let x = if move_to_first_column {
            0
        } else {
            self.cursor_x
        };
        let y = self.cursor_y;
        let y = if y == self.scroll_bottom {
            self.scroll_up(1);
            y
        } else {
            y + 1
        };
        self.set_cursor_pos(x, y);
    }

    fn push_answerback(&mut self, buf: &[u8]) {
        let mut result = self.answerback.take().unwrap_or_else(Vec::new);
        result.extend_from_slice(buf);
        self.answerback = Some(result)
    }

    /// Move the cursor up 1 line.  If the position is at the top scroll margin,
    /// scroll the region down.
    fn reverse_index(&mut self) {
        let y = self.cursor_y;
        let y = if y == self.scroll_top {
            self.scroll_down(1);
            y
        } else {
            y - 1
        };
        let x = self.cursor_x;
        self.set_cursor_pos(x, y);
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

impl Terminal {
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Terminal {
        Terminal {
            state: TerminalState::new(physical_rows, physical_cols, scrollback_size),
            parser: vte::Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes of input.
    /// The return value is an optional sequence of bytes which should
    /// we sent back to the client.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) -> Option<Vec<u8>> {
        let bytes = bytes.as_ref();
        for b in bytes.iter() {
            self.parser.advance(&mut self.state, *b);
        }
        self.answerback.take()
    }
}


impl vte::Perform for TerminalState {
    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        if self.wrap_next {
            // TODO: remember that this was a wrapped line in the attributes?
            self.new_line(true);
        }

        let x = self.cursor_x;
        let y = self.cursor_y;
        let width = self.screen().physical_cols;

        let pen = self.pen;
        self.screen_mut().set_cell(x, y, c, &pen);

        if x + 1 < width {
            // TODO: the 1 here should be based on the glyph width
            self.set_cursor_pos(x + 1, y);
        } else {
            self.wrap_next = true;
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b /* VT */ | 0x0c /* FF */ => {
                self.new_line(true /* TODO: depend on terminal mode */)
            }
            b'\r' => {
                let row = self.cursor_y;
                self.screen_mut().dirty_line(row);
                self.cursor_x = 0;
            }
            0x08 /* BS */ => {
                let row = self.cursor_y;
                self.screen_mut().dirty_line(row);
                self.cursor_x -= 1;
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
                    println!("OSC: set title {}", title);
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
                CSIAction::SetCursorXY(x, y) => {
                    self.set_cursor_pos(x, y);
                }
                CSIAction::DeltaCursorXY { x, y } => {
                    self.delta_cursor_pos(x, y);
                }
                CSIAction::EraseInLine(erase) => {
                    let cx = self.cursor_x;
                    let cy = self.cursor_y;
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
                    let cy = self.cursor_y;
                    let mut screen = self.screen_mut();
                    let cols = screen.physical_cols;
                    let rows = screen.physical_rows;
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
                CSIAction::SetDecPrivateMode(DecPrivateMode::ApplicationCursorKeys, _on) => {}
                CSIAction::SetDecPrivateMode(DecPrivateMode::BrackedPaste, _on) => {}
                CSIAction::DeviceStatusReport => {
                    // "OK"
                    self.push_answerback(b"\x1b[0n");
                }
                CSIAction::ReportCursorPosition => {
                    let row = self.cursor_y + 1;
                    let col = self.cursor_x + 1;
                    self.push_answerback(format!("\x1b[{};{}R", row, col).as_bytes());
                }
                CSIAction::SetScrollingRegion { top, bottom } => {
                    // TODO: this isn't respected fully yet
                    let rows = self.screen().physical_rows;
                    self.scroll_top = top.min(rows - 1);
                    self.scroll_bottom = bottom.min(rows - 1);
                    if self.scroll_top > self.scroll_bottom {
                        std::mem::swap(&mut self.scroll_top, &mut self.scroll_bottom);
                    }
                    println!(
                        "SetScrollingRegion {} - {}",
                        self.scroll_top,
                        self.scroll_bottom
                    );
                }
                CSIAction::RequestDeviceAttributes => {
                    self.push_answerback(DEVICE_IDENT);
                }
                CSIAction::DeleteLines(n) => {
                    let top = self.cursor_y;
                    println!("execute delete {} lines with scroll up {} {}", n, top, top+n);
                    self.screen_mut().scroll_up(top, top+n, n);
                }
                CSIAction::InsertLines(n) => {
                    let top = self.cursor_y;
                    println!("execute insert {} lines with scroll down {} {}", n, top, top+n);
                    self.screen_mut().scroll_down(top, top+n, n);
                }
            }
        }
    }

    fn esc_dispatch(&mut self, params: &[i64], intermediates: &[u8], _ignore: bool, byte: u8) {
        // Sequences from both of these sections show up in this handler:
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-C1-_8-Bit_-Control-Characters
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Controls-beginning-with-ESC
        match (byte, intermediates, params) {
            // String Terminator (ST); explicitly has nothing to do here, as its purpose is
            // handled by vte::Parser
            (b'\\', &[], &[]) => {}
            // Application Keypad (DECKPAM)
            (b'=', &[], &[]) => {}
            // Normal Keypad (DECKPAM)
            (b'>', &[], &[]) => {}
            // Reverse Index (RI)
            (b'M', &[], &[]) => self.reverse_index(),

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
