//! Terminal model

use std;
use unicode_segmentation;

pub mod color;

#[derive(Debug, Clone, Copy)]
pub struct CellAttributes {
    pub bold: bool,
    pub underline: bool,
    pub italic: bool,
    pub blink: bool,
    pub reverse: bool,
    pub strikethrough: bool,
    pub font: u8,
    pub foreground: color::ColorAttribute,
    pub background: color::ColorAttribute,
}

impl Default for CellAttributes {
    fn default() -> CellAttributes {
        CellAttributes {
            bold: false,
            underline: false,
            italic: false,
            blink: false,
            reverse: false,
            strikethrough: false,
            font: 0,
            foreground: color::ColorAttribute::Foreground,
            background: color::ColorAttribute::Background,
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct Line {
    pub cells: Vec<Cell>,
}

impl Line {
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
