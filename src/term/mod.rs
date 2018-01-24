//! Terminal model

use std;
use unicode_segmentation;

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
}


impl CellAttributes {
    bitfield!(bold, set_bold, 0);
    bitfield!(underline, set_underline, 1);
    bitfield!(italic, set_italic, 2);
    bitfield!(blink, set_blink, 3);
    bitfield!(reverse, set_reverse, 4);
    bitfield!(strikethrough, set_strikethrough, 5);
    // Allow up to 8 different font values
    bitfield!(font, set_font, 0b111000000, 6);
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

#[derive(Debug, Clone, Default)]
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
        for i in 0..physical_rows {
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
}
