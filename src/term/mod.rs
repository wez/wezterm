//! Terminal model

use std;
use unicode_segmentation;
use vte;

pub mod color;

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
        // TODO: if the line isn't wide enough, we should pad it out with
        // the default attributes
        debug!(
            "set_cell x,y {},{}, line_idx = {} {} {:?}",
            x,
            y,
            line_idx,
            c,
            attr
        );
        self.lines[line_idx].cells[x] = Cell::from_char(c, attr);
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
        }
    }

    fn screen(&mut self) -> &Screen {
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
}

pub struct Terminal {
    /// The terminal model/state
    state: TerminalState,
    /// Baseline terminal escape sequence parser
    parser: vte::Parser,
}

impl Terminal {
    pub fn new(physical_rows: usize, physical_cols: usize, scrollback_size: usize) -> Terminal {
        Terminal {
            state: TerminalState::new(physical_rows, physical_cols, scrollback_size),
            parser: vte::Parser::new(),
        }
    }

    /// Feed the terminal parser a single byte of input
    pub fn advance(&mut self, byte: u8) {
        self.parser.advance(&mut self.state, byte);
    }

    /// Feed the terminal parser a slice of bytes of input
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) {
        let bytes = bytes.as_ref();
        for b in bytes.iter() {
            self.parser.advance(&mut self.state, *b);
        }
    }

    /// Return true if the state has changed; the implication is that the terminal
    /// needs to be redrawn in some fashion.
    /// TODO: should probably build up a damage list instead
    pub fn get_state_changed(&self) -> bool {
        self.state.state_changed
    }

    /// Clear the state changed flag; the intent is that the consumer of this
    /// class will clear the state after each paint.
    pub fn clear_state_changed(&mut self) {
        self.state.state_changed = false;
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.state.screen.resize(physical_rows, physical_cols);
        self.state.alt_screen.resize(physical_rows, physical_cols);
    }


    /// Returns the width of the screen and a slice over the visible rows
    /// TODO: should allow an arbitrary view for scrollback
    pub fn visible_cells(&self) -> (usize, &[Line]) {
        let screen = if self.state.alt_screen_is_active {
            &self.state.alt_screen
        } else {
            &self.state.screen
        };
        let width = screen.physical_cols;
        let height = screen.physical_rows;
        let len = screen.lines.len();
        (width, &screen.lines[len - height..len])
    }
}

#[derive(Debug)]
enum CSIAction {
    SetPen(CellAttributes),
    SetForegroundColor(color::ColorAttribute),
    SetBackgroundColor(color::ColorAttribute),
    SetIntensity(Intensity),
    SetUnderline(Underline),
    SetItalic(bool),
    SetBlink(bool),
    SetReverse(bool),
    SetStrikethrough(bool),
    SetInvisible(bool),
}

impl CSIAction {
    /// Parses out a "Set Graphics Rendition" action.
    /// Returns the decoded action plus the unparsed remainder of the
    /// parameter stream.  Returns None if we couldn't decode one of
    /// the parameter elements.
    fn parse_sgr(params: &[i64]) -> Option<(CSIAction, &[i64])> {
        if params.len() > 5 {
            // ISO-8613-6 foreground and background color specification
            // using full RGB color codes.
            if params[0] == 38 && params[1] == 2 {
                return Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::Rgb(color::RgbColor {
                            red: params[3] as u8,
                            green: params[4] as u8,
                            blue: params[5] as u8,
                        }),
                    ),
                    &params[6..],
                ));
            }
            if params[0] == 48 && params[1] == 2 {
                return Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::Rgb(color::RgbColor {
                            red: params[3] as u8,
                            green: params[4] as u8,
                            blue: params[5] as u8,
                        }),
                    ),
                    &params[6..],
                ));
            }
        }
        if params.len() > 2 {
            // Some special look-ahead cases for 88 and 256 color support
            if params[0] == 38 && params[1] == 5 {
                // 38;5;IDX -> foreground color
                let color = color::ColorAttribute::PaletteIndex(params[2] as u8);
                return Some((CSIAction::SetForegroundColor(color), &params[3..]));
            }

            if params[0] == 48 && params[1] == 5 {
                // 48;5;IDX -> background color
                let color = color::ColorAttribute::PaletteIndex(params[2] as u8);
                return Some((CSIAction::SetBackgroundColor(color), &params[3..]));
            }
        }

        let p = params[0];
        match p {
            0 => Some((CSIAction::SetPen(CellAttributes::default()), &params[1..])),
            1 => Some((CSIAction::SetIntensity(Intensity::Bold), &params[1..])),
            2 => Some((CSIAction::SetIntensity(Intensity::Half), &params[1..])),
            3 => Some((CSIAction::SetItalic(true), &params[1..])),
            4 => Some((CSIAction::SetUnderline(Underline::Single), &params[1..])),
            5 => Some((CSIAction::SetBlink(true), &params[1..])),
            7 => Some((CSIAction::SetReverse(true), &params[1..])),
            8 => Some((CSIAction::SetInvisible(true), &params[1..])),
            9 => Some((CSIAction::SetStrikethrough(true), &params[1..])),
            21 => Some((CSIAction::SetUnderline(Underline::Double), &params[1..])),
            22 => Some((CSIAction::SetIntensity(Intensity::Normal), &params[1..])),
            23 => Some((CSIAction::SetItalic(false), &params[1..])),
            24 => Some((CSIAction::SetUnderline(Underline::None), &params[1..])),
            25 => Some((CSIAction::SetBlink(false), &params[1..])),
            27 => Some((CSIAction::SetReverse(false), &params[1..])),
            28 => Some((CSIAction::SetInvisible(false), &params[1..])),
            29 => Some((CSIAction::SetStrikethrough(false), &params[1..])),
            30...37 => {
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 30),
                    ),
                    &params[1..],
                ))
            }
            39 => {
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::Foreground,
                    ),
                    &params[1..],
                ))
            }
            90...97 => {
                // Bright foreground colors
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 90 + 8),
                    ),
                    &params[1..],
                ))
            }
            40...47 => {
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 40),
                    ),
                    &params[1..],
                ))
            }
            49 => {
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::Background,
                    ),
                    &params[1..],
                ))
            }
            100...107 => {
                // Bright background colors
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 100 + 8),
                    ),
                    &params[1..],
                ))
            }
            _ => None,
        }
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
            b'\n' => {
                self.cursor_y += 1;
                self.state_changed = true;
            }
            b'\r' => {
                self.cursor_x = 0;
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
        match byte {
            'm' => {
                let mut params = params;

                if params.len() == 0 {
                    // Empty parameter list means to reset the attributes to default
                    self.pen = CellAttributes::default();
                }

                while params.len() > 0 {
                    match CSIAction::parse_sgr(params) {
                        Some((CSIAction::SetPen(pen), remainder)) => {
                            self.pen = pen;
                            params = remainder;
                        }
                        Some((CSIAction::SetForegroundColor(color), remainder)) => {
                            self.pen.foreground = color;
                            params = remainder;
                        }
                        Some((CSIAction::SetBackgroundColor(color), remainder)) => {
                            self.pen.background = color;
                            params = remainder;
                        }
                        Some((CSIAction::SetIntensity(level), remainder)) => {
                            self.pen.set_intensity(level);
                            params = remainder;
                        }
                        Some((CSIAction::SetUnderline(level), remainder)) => {
                            self.pen.set_underline(level);
                            params = remainder;
                        }
                        Some((CSIAction::SetItalic(on), remainder)) => {
                            self.pen.set_italic(on);
                            params = remainder;
                        }
                        Some((CSIAction::SetBlink(on), remainder)) => {
                            self.pen.set_blink(on);
                            params = remainder;
                        }
                        Some((CSIAction::SetReverse(on), remainder)) => {
                            self.pen.set_reverse(on);
                            params = remainder;
                        }
                        Some((CSIAction::SetStrikethrough(on), remainder)) => {
                            self.pen.set_strikethrough(on);
                            params = remainder;
                        }
                        Some((CSIAction::SetInvisible(on), remainder)) => {
                            self.pen.set_invisible(on);
                            params = remainder;
                        }
                        None => {
                            println!("parse_sgr: unhandled sequence {:?}", params);
                            break;
                        }
                    }
                }
            }
            'H' => {
                // Cursor Position (CUP)
                if params.len() == 2 {
                    // coordinates are 1-based; convert to 0-based
                    self.cursor_y = (params[0] - 1) as usize;
                    self.cursor_x = (params[1] - 1) as usize;
                } else {
                    // no parameters -> home the cursor
                    self.cursor_x = 0;
                    self.cursor_y = 0;
                }
            }
            'K' => {
                // Erase in line (EL)
                #[derive(Debug)]
                enum Erase {
                    ToRight,
                    ToLeft,
                    All,
                    Unknown,
                }
                let what = if params.len() == 0 {
                    Erase::ToRight
                } else {
                    match params[0] {
                        0 => Erase::ToRight,
                        1 => Erase::ToLeft,
                        2 => Erase::All,
                        _ => Erase::Unknown,
                    }
                };

                let cx = self.cursor_x;
                let cy = self.cursor_y;
                let mut screen = self.screen_mut();
                let cols = screen.physical_cols;
                match what {
                    Erase::ToRight => {
                        screen.clear_line(cy, cx..cols);
                    }
                    Erase::ToLeft => {
                        screen.clear_line(cy, 0..cx);
                    }
                    Erase::All => {
                        screen.clear_line(cy, 0..cols);
                    }
                    Erase::Unknown => {}
                }

            }
            _ => {
                println!(
                    "CSI: unhandled params {:?} intermediates {:?} ignore={} byte={}",
                    params,
                    intermediates,
                    ignore,
                    byte
                );
            }
        }
    }
    fn esc_dispatch(&mut self, _: &[i64], _: &[u8], _: bool, _: u8) {}
}
