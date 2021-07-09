use crate::cell::{Cell, CellAttributes};
use crate::cellcluster::CellCluster;
use crate::hyperlink::Rule;
use crate::surface::Change;
use bitflags::bitflags;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

bitflags! {
    #[cfg_attr(feature="use_serde", derive(Serialize, Deserialize))]
    struct LineBits : u8 {
        const NONE = 0;
        /// The contents of the Line have changed and cached or
        /// derived data will need to be reassessed.
        const DIRTY = 1;
        /// The line contains 1+ cells with explicit hyperlinks set
        const HAS_HYPERLINK = 1<<1;
        /// true if we have scanned for implicit hyperlinks
        const SCANNED_IMPLICIT_HYPERLINKS = 1<<2;
        /// true if we found implicit hyperlinks in the last scan
        const HAS_IMPLICIT_HYPERLINKS = 1<<3;

        /// true if this line should be displayed with
        /// foreground/background colors reversed
        const REVERSE = 1<<4;

        /// true if this line should be displayed with
        /// in double-width
        const DOUBLE_WIDTH = 1<<5;

        /// true if this line should be displayed
        /// as double-height top-half
        const DOUBLE_HEIGHT_TOP = 1<<6;

        /// true if this line should be displayed
        /// as double-height bottom-half
        const DOUBLE_HEIGHT_BOTTOM = 1<<7;

        const DOUBLE_WIDTH_HEIGHT_MASK =
            Self::DOUBLE_WIDTH.bits |
            Self::DOUBLE_HEIGHT_TOP.bits |
            Self::DOUBLE_HEIGHT_BOTTOM.bits;

    }
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    cells: Vec<Cell>,
    bits: LineBits,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoubleClickRange {
    Range(Range<usize>),
    RangeWithWrap(Range<usize>),
}

impl Line {
    pub fn with_width(width: usize) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize(width, Cell::default());
        let bits = LineBits::DIRTY;
        Self { bits, cells }
    }

    pub fn from_text(s: &str, attrs: &CellAttributes) -> Line {
        let mut cells = Vec::new();

        for sub in s.graphemes(true) {
            let cell = Cell::new_grapheme(sub, attrs.clone());
            let width = cell.width();
            cells.push(cell);
            for _ in 1..width {
                cells.push(Cell::new(' ', attrs.clone()));
            }
        }

        Line {
            cells,
            bits: LineBits::DIRTY,
        }
    }

    pub fn from_text_with_wrapped_last_col(s: &str, attrs: &CellAttributes) -> Line {
        let mut line = Self::from_text(s, attrs);
        line.cells
            .last_mut()
            .map(|cell| cell.attrs_mut().set_wrapped(true));
        line
    }

    pub fn resize_and_clear(&mut self, width: usize) {
        let blank = Cell::default();
        self.cells.clear();
        self.cells.resize(width, blank);
        self.cells.shrink_to_fit();
        self.bits = LineBits::DIRTY;
    }

    pub fn resize(&mut self, width: usize) {
        self.cells.resize(width, Cell::default());
        self.bits |= LineBits::DIRTY;
    }

    /// Wrap the line so that it fits within the provided width.
    /// Returns the list of resultant line(s)
    pub fn wrap(mut self, width: usize) -> Vec<Self> {
        if let Some(end_idx) = self.cells.iter().rposition(|c| c.str() != " ") {
            self.cells.resize(end_idx + 1, Cell::default());

            let mut lines: Vec<_> = self
                .cells
                .chunks_mut(width)
                .map(|chunk| {
                    let mut line = Line {
                        cells: chunk.to_vec(),
                        bits: LineBits::DIRTY,
                    };
                    if line.cells.len() == width {
                        // Ensure that we don't forget that we wrapped
                        line.set_last_cell_was_wrapped(true);
                    }
                    line
                })
                .collect();
            // The last of the chunks wasn't actually wrapped
            if let Some(line) = lines.last_mut() {
                line.set_last_cell_was_wrapped(false);
            }
            lines
        } else {
            vec![self]
        }
    }

    /// Check whether the dirty bit is set.
    /// If it is set, then something about the line has changed since
    /// the dirty bit was last cleared.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        (self.bits & LineBits::DIRTY) == LineBits::DIRTY
    }

    /// Force the dirty bit set.
    /// FIXME: this is abused by term::Screen, want to remove or rethink it.
    #[inline]
    pub fn set_dirty(&mut self) {
        self.bits |= LineBits::DIRTY;
    }

    /// Clear the dirty bit.
    #[inline]
    pub fn clear_dirty(&mut self) {
        self.bits &= !LineBits::DIRTY;
    }

    /// Check whether the reverse video bit is set.  If it is set,
    /// then the line should be displayed with foreground/background
    /// colors reversed.
    #[inline]
    pub fn is_reverse(&self) -> bool {
        self.bits.contains(LineBits::REVERSE)
    }

    /// Force the reverse bit set.  This also implicitly sets dirty.
    #[inline]
    pub fn set_reverse(&mut self, reverse: bool) {
        self.bits.set(LineBits::REVERSE, reverse);
        self.bits.insert(LineBits::DIRTY);
    }

    /// Check whether the line is single-width.
    #[inline]
    pub fn is_single_width(&self) -> bool {
        (self.bits
            & (LineBits::DOUBLE_WIDTH
                | LineBits::DOUBLE_HEIGHT_TOP
                | LineBits::DOUBLE_HEIGHT_BOTTOM))
            == LineBits::NONE
    }

    /// Force single-width.  This also implicitly sets
    /// double-height-(top/bottom) and dirty.
    #[inline]
    pub fn set_single_width(&mut self) {
        self.bits.remove(LineBits::DOUBLE_WIDTH_HEIGHT_MASK);
        self.bits.insert(LineBits::DIRTY);
    }

    /// Check whether the line is double-width and not double-height.
    #[inline]
    pub fn is_double_width(&self) -> bool {
        (self.bits & LineBits::DOUBLE_WIDTH_HEIGHT_MASK) == LineBits::DOUBLE_WIDTH
    }

    /// Force double-width.  This also implicitly sets
    /// double-height-(top/bottom) and dirty.
    #[inline]
    pub fn set_double_width(&mut self) {
        self.bits
            .remove(LineBits::DOUBLE_HEIGHT_TOP | LineBits::DOUBLE_HEIGHT_BOTTOM);
        self.bits.insert(LineBits::DOUBLE_WIDTH | LineBits::DIRTY);
    }

    /// Check whether the line is double-height-top.
    #[inline]
    pub fn is_double_height_top(&self) -> bool {
        (self.bits & LineBits::DOUBLE_WIDTH_HEIGHT_MASK)
            == LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_TOP
    }

    /// Force double-height top-half.  This also implicitly sets
    /// double-width and dirty.
    #[inline]
    pub fn set_double_height_top(&mut self) {
        self.bits.remove(LineBits::DOUBLE_HEIGHT_BOTTOM);
        self.bits
            .insert(LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_TOP | LineBits::DIRTY);
    }

    /// Check whether the line is double-height-bottom.
    #[inline]
    pub fn is_double_height_bottom(&self) -> bool {
        (self.bits & LineBits::DOUBLE_WIDTH_HEIGHT_MASK)
            == LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_BOTTOM
    }

    /// Force double-height bottom-half.  This also implicitly sets
    /// double-width and dirty.
    #[inline]
    pub fn set_double_height_bottom(&mut self) {
        self.bits.remove(LineBits::DOUBLE_HEIGHT_TOP);
        self.bits
            .insert(LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_BOTTOM | LineBits::DIRTY);
    }

    /// If we have any cells with an implicit hyperlink, remove the hyperlink
    /// from the cell attributes but leave the remainder of the attributes alone.
    pub fn invalidate_implicit_hyperlinks(&mut self) {
        if (self.bits & (LineBits::SCANNED_IMPLICIT_HYPERLINKS | LineBits::HAS_IMPLICIT_HYPERLINKS))
            == LineBits::NONE
        {
            return;
        }

        self.bits &= !LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        if (self.bits & LineBits::HAS_IMPLICIT_HYPERLINKS) == LineBits::NONE {
            return;
        }

        for cell in &mut self.cells {
            let replace = match cell.attrs().hyperlink() {
                Some(ref link) if link.is_implicit() => Some(Cell::new_grapheme(
                    cell.str(),
                    cell.attrs().clone().set_hyperlink(None).clone(),
                )),
                _ => None,
            };
            if let Some(replace) = replace {
                *cell = replace;
            }
        }

        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;
        self.bits |= LineBits::DIRTY;
    }

    /// Scan through the line and look for sequences that match the provided
    /// rules.  Matching sequences are considered to be implicit hyperlinks
    /// and will have a hyperlink attribute associated with them.
    /// This function will only make changes if the line has been invalidated
    /// since the last time this function was called.
    /// This function does not remember the values of the `rules` slice, so it
    /// is the responsibility of the caller to call `invalidate_implicit_hyperlinks`
    /// if it wishes to call this function with different `rules`.
    pub fn scan_and_create_hyperlinks(&mut self, rules: &[Rule]) {
        if (self.bits & LineBits::SCANNED_IMPLICIT_HYPERLINKS)
            == LineBits::SCANNED_IMPLICIT_HYPERLINKS
        {
            // Has not changed since last time we scanned
            return;
        }

        // FIXME: let's build a string and a byte-to-cell map here, and
        // use this as an opportunity to rebuild HAS_HYPERLINK, skip matching
        // cells with existing non-implicit hyperlinks, and avoid matching
        // text with zero-width cells.
        let line = self.as_str();
        self.bits |= LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;

        let matches = Rule::match_hyperlinks(&line, rules);
        if matches.is_empty() {
            return;
        }

        // The capture range is measured in bytes but we need to translate
        // that to the index of the column.  This is complicated a bit further
        // because double wide sequences have a blank column cell after them
        // in the cells array, but the string we match against excludes that
        // string.
        let mut cell_idx = 0;
        for (byte_idx, _grapheme) in line.grapheme_indices(true) {
            let cell = &mut self.cells[cell_idx];
            for m in &matches {
                if m.range.contains(&byte_idx) {
                    let attrs = cell.attrs_mut();
                    // Don't replace existing links
                    if attrs.hyperlink().is_none() {
                        attrs.set_hyperlink(Some(Arc::clone(&m.link)));
                        self.bits |= LineBits::HAS_IMPLICIT_HYPERLINKS;
                    }
                }
            }
            cell_idx += cell.width();
        }
    }

    /// Returns true if the line contains a hyperlink
    #[inline]
    pub fn has_hyperlink(&self) -> bool {
        (self.bits & (LineBits::HAS_HYPERLINK | LineBits::HAS_IMPLICIT_HYPERLINKS))
            != LineBits::NONE
    }

    /// Recompose line into the corresponding utf8 string.
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for (_, cell) in self.visible_cells() {
            s.push_str(cell.str());
        }
        s
    }

    pub fn split_off(&mut self, idx: usize) -> Self {
        let cells = self.cells.split_off(idx);
        Self {
            bits: self.bits,
            cells,
        }
    }

    pub fn compute_double_click_range<F: Fn(&str) -> bool>(
        &self,
        click_col: usize,
        is_word: F,
    ) -> DoubleClickRange {
        let len = self.cells.len();

        if click_col >= len {
            return DoubleClickRange::Range(click_col..click_col);
        }

        let mut lower = click_col;
        let mut upper = click_col;

        // TODO: look back and look ahead for cells that are hidden by
        // a preceding multi-wide cell
        for (idx, cell) in self.cells.iter().enumerate().skip(click_col) {
            if !is_word(cell.str()) {
                break;
            }
            upper = idx + 1;
        }
        for (idx, cell) in self.cells.iter().enumerate().rev() {
            if idx > click_col {
                continue;
            }
            if !is_word(cell.str()) {
                break;
            }
            lower = idx;
        }

        if upper > lower && self.cells[upper.min(len) - 1].attrs().wrapped() {
            DoubleClickRange::RangeWithWrap(lower..upper)
        } else {
            DoubleClickRange::Range(lower..upper)
        }
    }

    /// Returns a substring from the line.
    pub fn columns_as_str(&self, range: Range<usize>) -> String {
        let mut s = String::new();
        for (n, c) in self.visible_cells() {
            if n < range.start {
                continue;
            }
            if n >= range.end {
                break;
            }
            s.push_str(c.str());
        }
        s
    }

    /// If we're about to modify a cell obscured by a double-width
    /// character ahead of that cell, we need to nerf that sequence
    /// of cells to avoid partial rendering concerns.
    /// Similarly, when we assign a cell, we need to blank out those
    /// occluded successor cells.
    pub fn set_cell(&mut self, idx: usize, cell: Cell) -> &Cell {
        let width = cell.width();

        // if the line isn't wide enough, pad it out with the default attributes.
        // The .max(1) stuff is here in case we get called with a
        // zero-width cell.  That shouldn't happen: those sequences
        // should get filtered out in the terminal parsing layer,
        // but in case one does sneak through, we need to ensure that
        // we grow the cells array to hold this bogus entry.
        // https://github.com/wez/wezterm/issues/768
        if idx + width.max(1) >= self.cells.len() {
            self.cells.resize(idx + width.max(1), Cell::default());
        }

        self.invalidate_implicit_hyperlinks();
        self.bits |= LineBits::DIRTY;
        if cell.attrs().hyperlink().is_some() {
            self.bits |= LineBits::HAS_HYPERLINK;
        }
        self.invalidate_grapheme_at_or_before(idx);

        // For double-wide or wider chars, ensure that the cells that
        // are overlapped by this one are blanked out.
        for i in 1..=width.saturating_sub(1) {
            self.cells[idx + i] = Cell::new(' ', cell.attrs().clone());
        }

        self.cells[idx] = cell;
        &self.cells[idx]
    }

    /// Place text starting at the specified column index.
    /// Each grapheme of the text run has the same attributes.
    pub fn overlay_text_with_attribute(
        &mut self,
        mut start_idx: usize,
        text: &str,
        attr: CellAttributes,
    ) {
        for (i, c) in text.graphemes(true).enumerate() {
            let cell = Cell::new_grapheme(c, attr.clone());
            let width = cell.width();
            self.set_cell(i + start_idx, cell);

            // Compensate for required spacing/placement of
            // double width characters
            start_idx += width.saturating_sub(1);
        }
    }

    fn invalidate_grapheme_at_or_before(&mut self, idx: usize) {
        // Assumption: that the width of a grapheme is never > 2.
        // This constrains the amount of look-back that we need to do here.
        if idx > 0 {
            let prior = idx - 1;
            let width = self.cells[prior].width();
            if width > 1 {
                let attrs = self.cells[prior].attrs().clone();
                for nerf in prior..prior + width {
                    self.cells[nerf] = Cell::new(' ', attrs.clone());
                }
            }
        }
    }

    pub fn insert_cell(&mut self, x: usize, cell: Cell, right_margin: usize) {
        self.invalidate_implicit_hyperlinks();

        if right_margin <= self.cells.len() {
            self.cells.remove(right_margin - 1);
        }

        if x >= self.cells.len() {
            self.cells.resize(x, Cell::default());
        }

        // If we're inserting a wide cell, we should also insert the overlapped cells.
        // We insert them first so that the grapheme winds up left-most.
        let width = cell.width();
        for _ in 1..=width.saturating_sub(1) {
            self.cells.insert(x, Cell::new(' ', cell.attrs().clone()));
        }

        self.cells.insert(x, cell);
        self.set_dirty();
    }

    pub fn erase_cell(&mut self, x: usize) {
        if x >= self.cells.len() {
            // Already implicitly erased
            return;
        }
        self.invalidate_implicit_hyperlinks();
        self.invalidate_grapheme_at_or_before(x);
        self.cells.remove(x);
        self.cells.push(Cell::default());
        self.set_dirty();
    }

    pub fn erase_cell_with_margin(&mut self, x: usize, right_margin: usize) {
        self.invalidate_implicit_hyperlinks();
        if x < self.cells.len() {
            self.invalidate_grapheme_at_or_before(x);
            self.cells.remove(x);
        }
        if right_margin <= self.cells.len() {
            self.cells.insert(right_margin - 1, Cell::default());
        }
        self.set_dirty();
    }

    pub fn prune_trailing_blanks(&mut self) {
        let def_attr = CellAttributes::default();
        if let Some(end_idx) = self
            .cells
            .iter()
            .rposition(|c| c.str() != " " || c.attrs() != &def_attr)
        {
            self.cells.resize(end_idx + 1, Cell::default());
        }
    }

    pub fn fill_range(&mut self, cols: Range<usize>, cell: &Cell) {
        for x in cols {
            // FIXME: we can skip the look-back for second and subsequent iterations
            self.set_cell(x, cell.clone());
        }
        self.prune_trailing_blanks();
    }

    /// Iterates the visible cells, respecting the width of the cell.
    /// For instance, a double-width cell overlaps the following (blank)
    /// cell, so that blank cell is omitted from the iterator results.
    /// The iterator yields (column_index, Cell).  Column index is the
    /// index into Self::cells, and due to the possibility of skipping
    /// the characters that follow wide characters, the column index may
    /// skip some positions.  It is returned as a convenience to the consumer
    /// as using .enumerate() on this iterator wouldn't be as useful.
    pub fn visible_cells(&self) -> impl Iterator<Item = (usize, &Cell)> {
        let mut skip_width = 0;
        self.cells.iter().enumerate().filter(move |(_idx, cell)| {
            if skip_width > 0 {
                skip_width -= 1;
                false
            } else {
                skip_width = cell.width().saturating_sub(1);
                true
            }
        })
    }

    pub fn cluster(&self) -> Vec<CellCluster> {
        CellCluster::make_cluster(self.cells.len(), self.visible_cells())
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Return true if the line consists solely of whitespace cells
    pub fn is_whitespace(&self) -> bool {
        self.cells.iter().all(|c| c.str() == " ")
    }

    /// Return true if the last cell in the line has the wrapped attribute,
    /// indicating that the following line is logically a part of this one.
    pub fn last_cell_was_wrapped(&self) -> bool {
        self.cells
            .last()
            .map(|c| c.attrs().wrapped())
            .unwrap_or(false)
    }

    /// Adjust the value of the wrapped attribute on the last cell of this
    /// line.
    pub fn set_last_cell_was_wrapped(&mut self, wrapped: bool) {
        if let Some(cell) = self.cells.last_mut() {
            cell.attrs_mut().set_wrapped(wrapped);
            self.set_dirty();
        }
    }

    /// Concatenate the cells from other with this line, appending them
    /// to this line.
    /// This function is used by rewrapping logic when joining wrapped
    /// lines back together.
    pub fn append_line(&mut self, mut other: Line) {
        self.cells.append(&mut other.cells);
        self.set_dirty();
    }

    /// mutable access the cell data, but the caller must take care
    /// to only mutate attributes rather than the cell textual content.
    /// Use set_cell if you need to modify the textual content of the
    /// cell, so that important invariants are upheld.
    pub fn cells_mut_for_attr_changes_only(&mut self) -> &mut [Cell] {
        &mut self.cells
    }

    /// Given a starting attribute value, produce a series of Change
    /// entries to recreate the current line
    pub fn changes(&self, start_attr: &CellAttributes) -> Vec<Change> {
        let mut result = Vec::new();
        let mut attr = start_attr.clone();
        let mut text_run = String::new();

        for (_, cell) in self.visible_cells() {
            if *cell.attrs() == attr {
                text_run.push_str(cell.str());
            } else {
                // flush out the current text run
                if !text_run.is_empty() {
                    result.push(Change::Text(text_run.clone()));
                    text_run.clear();
                }

                attr = cell.attrs().clone();
                result.push(Change::AllAttributes(attr.clone()));
                text_run.push_str(cell.str());
            }
        }

        // flush out any remaining text run
        if !text_run.is_empty() {
            // if this is just spaces then it is likely cheaper
            // to emit ClearToEndOfLine instead.
            if attr
                == CellAttributes::default()
                    .set_background(attr.background)
                    .clone()
            {
                let left = text_run.trim_end_matches(' ').to_string();
                let num_trailing_spaces = text_run.len() - left.len();

                if num_trailing_spaces > 0 {
                    if !left.is_empty() {
                        result.push(Change::Text(left));
                    } else if result.len() == 1 {
                        // if the only queued result prior to clearing
                        // to the end of the line is an attribute change,
                        // we can prune it out and return just the line
                        // clearing operation
                        if let Change::AllAttributes(_) = result[0] {
                            result.clear()
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
                    result.push(Change::Text(text_run));
                }
            } else {
                result.push(Change::Text(text_run));
            }
        }

        result
    }
}

impl<'a> From<&'a str> for Line {
    fn from(s: &str) -> Line {
        Line::from_text(s, &CellAttributes::default())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::hyperlink::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn hyperlinks() {
        let text =
            "❤ 😍🤢 http://example.com \u{1f468}\u{1f3fe}\u{200d}\u{1f9b0} http://example.com";

        let rules = vec![
            Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap(),
            Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
        ];

        let hyperlink = Arc::new(Hyperlink::new_implicit("http://example.com"));
        let hyperlink_attr = CellAttributes::default()
            .set_hyperlink(Some(hyperlink.clone()))
            .clone();

        let mut line: Line = text.into();
        line.scan_and_create_hyperlinks(&rules);
        assert!(line.has_hyperlink());
        assert_eq!(
            line.cells().to_vec(),
            vec![
                Cell::new_grapheme("❤", CellAttributes::default()),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new_grapheme("😍", CellAttributes::default()),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new_grapheme("🤢", CellAttributes::default()),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new(' ', CellAttributes::default()),
                Cell::new('h', hyperlink_attr.clone()),
                Cell::new('t', hyperlink_attr.clone()),
                Cell::new('t', hyperlink_attr.clone()),
                Cell::new('p', hyperlink_attr.clone()),
                Cell::new(':', hyperlink_attr.clone()),
                Cell::new('/', hyperlink_attr.clone()),
                Cell::new('/', hyperlink_attr.clone()),
                Cell::new('e', hyperlink_attr.clone()),
                Cell::new('x', hyperlink_attr.clone()),
                Cell::new('a', hyperlink_attr.clone()),
                Cell::new('m', hyperlink_attr.clone()),
                Cell::new('p', hyperlink_attr.clone()),
                Cell::new('l', hyperlink_attr.clone()),
                Cell::new('e', hyperlink_attr.clone()),
                Cell::new('.', hyperlink_attr.clone()),
                Cell::new('c', hyperlink_attr.clone()),
                Cell::new('o', hyperlink_attr.clone()),
                Cell::new('m', hyperlink_attr.clone()),
                Cell::new(' ', CellAttributes::default()),
                Cell::new_grapheme(
                    // man: dark skin tone, red hair ZWJ emoji grapheme
                    "\u{1f468}\u{1f3fe}\u{200d}\u{1f9b0}",
                    CellAttributes::default()
                ),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new(' ', CellAttributes::default()),
                Cell::new('h', hyperlink_attr.clone()),
                Cell::new('t', hyperlink_attr.clone()),
                Cell::new('t', hyperlink_attr.clone()),
                Cell::new('p', hyperlink_attr.clone()),
                Cell::new(':', hyperlink_attr.clone()),
                Cell::new('/', hyperlink_attr.clone()),
                Cell::new('/', hyperlink_attr.clone()),
                Cell::new('e', hyperlink_attr.clone()),
                Cell::new('x', hyperlink_attr.clone()),
                Cell::new('a', hyperlink_attr.clone()),
                Cell::new('m', hyperlink_attr.clone()),
                Cell::new('p', hyperlink_attr.clone()),
                Cell::new('l', hyperlink_attr.clone()),
                Cell::new('e', hyperlink_attr.clone()),
                Cell::new('.', hyperlink_attr.clone()),
                Cell::new('c', hyperlink_attr.clone()),
                Cell::new('o', hyperlink_attr.clone()),
                Cell::new('m', hyperlink_attr.clone()),
            ]
        );
    }

    #[test]
    fn double_click_range_bounds() {
        let line: Line = "hello".into();
        let r = line.compute_double_click_range(200, |_| true);
        assert_eq!(r, DoubleClickRange::Range(200..200));
    }
}
