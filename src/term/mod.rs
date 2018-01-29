//! Terminal model

use failure::Error;
use std;
use std::ops::{Deref, DerefMut};
use unicode_segmentation;
use vte;

pub mod color;
mod csi;
use self::csi::*;

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
}

impl Line {
    /// Create a new line with the specified number of columns.
    /// Each cell has the default attributes.
    pub fn new(cols: usize) -> Line {
        let mut cells = Vec::with_capacity(cols);
        cells.resize(cols, Default::default());
        Line { cells }
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

        Line { cells }
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
    pub lines: Vec<Line>,

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
        let cells = &mut self.lines[line_idx].cells;
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
        let line = &mut self.lines[line_idx];
        let max_col = line.cells.len();
        for x in cols {
            if x >= max_col {
                break;
            }
            line.cells[x] = blank;
        }
    }

    fn scroll_up(&mut self, num_rows: usize) {
        let max_allowed = self.physical_rows + self.scrollback_size;
        if self.lines.len() + num_rows >= max_allowed {
            let lines_to_pop = (self.lines.len() + num_rows) - max_allowed;
            for _ in 0..lines_to_pop {
                self.lines.remove(0);
            }
        }
        for _ in 0..num_rows {
            self.lines.push(Line::new(self.physical_cols));
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

    /// If true then the terminal state has changed
    state_changed: bool,

    /// Some parsing operations may yield responses that need
    /// to be returned to the client.  They are collected here
    /// and this is used as the result of the advance_bytes()
    /// method.
    answerback: Option<Vec<u8>>,
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
            state_changed: true,
            answerback: None,
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

    /// Return true if the state has changed; the implication is that the terminal
    /// needs to be redrawn in some fashion.
    /// TODO: should probably build up a damage list instead
    pub fn get_state_changed(&self) -> bool {
        self.state_changed
    }

    /// Clear the state changed flag; the intent is that the consumer of this
    /// class will clear the state after each paint.
    pub fn clear_state_changed(&mut self) {
        self.state_changed = false;
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.screen.resize(physical_rows, physical_cols);
        self.alt_screen.resize(physical_rows, physical_cols);
    }

    /// Returns the width of the screen and a slice over the visible rows
    /// TODO: should allow an arbitrary view for scrollback
    pub fn visible_cells(&self) -> (usize, &[Line]) {
        let screen = self.screen();
        let width = screen.physical_cols;
        let height = screen.physical_rows;
        let len = screen.lines.len();
        (width, &screen.lines[len - height..len])
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
        self.cursor_x = x;
        self.cursor_y = y.min(rows-1);
        self.state_changed = true;
    }

    fn scroll_up(&mut self, num_rows: usize) {
        self.screen_mut().scroll_up(num_rows)
    }

    fn new_line(&mut self, move_to_first_column: bool) {
        let x = if move_to_first_column {
            0
        } else {
            self.cursor_x
        };
        let y = self.cursor_y;
        let num_rows = self.screen().physical_rows;
        let y = if y + 1 == num_rows {
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
        let x = self.cursor_x;
        let y = self.cursor_y;

        let pen = self.pen;
        self.screen_mut().set_cell(x, y, c, &pen);

        self.cursor_x += 1;
        // TODO: wrap at the end of the screen
        self.state_changed = true;
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b /* VT */ | 0x0c /* FF */=> {
                self.new_line(true /* TODO: depend on terminal mode */)
            }
            b'\r' => {
                self.cursor_x = 0;
                self.state_changed = true;
            }
            0x08 /* BS */ => {
                self.cursor_x -= 1;
                self.state_changed = true;
            }
            _ => println!("unhandled vte execute {}", byte),
        }
    }
    fn hook(&mut self, _: &[i64], _: &[u8], _: bool) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn csi_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: char) {
        for act in CSIParser::new(params, intermediates, ignore, byte) {
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
            }
        }
    }
    fn esc_dispatch(&mut self, _: &[i64], _: &[u8], _: bool, _: u8) {}
}
