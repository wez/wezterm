use crate::cell::{Cell, CellAttributes, SemanticType, UnicodeVersion};
use crate::cellcluster::CellCluster;
use crate::emoji::Presentation;
use crate::hyperlink::Rule;
use crate::surface::{Change, SequenceNo, SEQ_ZERO};
use bitflags::bitflags;
use fixedbitset::FixedBitSet;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use wezterm_bidi::{Direction, ParagraphDirectionHint};

bitflags! {
    #[cfg_attr(feature="use_serde", derive(Serialize, Deserialize))]
    struct LineBits : u16 {
        const NONE = 0;
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

        /// true if the line should have the bidi algorithm
        /// applied as part of presentation.
        /// This corresponds to the "implicit" bidi modes
        /// described in
        /// <https://terminal-wg.pages.freedesktop.org/bidi/recommendation/basic-modes.html>
        const BIDI_ENABLED = 1<<0;

        /// true if the line base direction is RTL.
        /// When BIDI_ENABLED is also true, this is passed to the bidi algorithm.
        /// When rendering, the line will be rendered from RTL.
        const RTL = 1<<8;

        /// true if the direction for the line should be auto-detected
        /// when BIDI_ENABLED is also true.
        /// If false, the direction is taken from the RTL bit only.
        /// Otherwise, the auto-detect direction is used, falling back
        /// to the direction specified by the RTL bit.
        const AUTO_DETECT_DIRECTION = 1<<9;
    }
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneRange {
    pub semantic_type: SemanticType,
    pub range: Range<u16>,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    cells: CellStorage,
    zones: Vec<ZoneRange>,
    seqno: SequenceNo,
    bits: LineBits,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
enum CellStorage {
    V(VecStorage),
    C(ClusteredLine),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoubleClickRange {
    Range(Range<usize>),
    RangeWithWrap(Range<usize>),
}

impl Line {
    pub fn with_width_and_cell(width: usize, cell: Cell, seqno: SequenceNo) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize(width, cell.clone());
        let bits = LineBits::NONE;
        Self {
            bits,
            cells: CellStorage::V(VecStorage::new(cells)),
            seqno,
            zones: vec![],
        }
    }

    pub fn from_cells(cells: Vec<Cell>, seqno: SequenceNo) -> Self {
        let bits = LineBits::NONE;
        Self {
            bits,
            cells: CellStorage::V(VecStorage::new(cells)),
            seqno,
            zones: vec![],
        }
    }

    /// Create a new line using cluster storage, optimized for appending
    /// and lower memory utilization.
    /// The line will automatically switch to cell storage when necessary
    /// to apply edits.
    pub fn new(seqno: SequenceNo) -> Self {
        Self {
            bits: LineBits::NONE,
            cells: CellStorage::C(ClusteredLine::default()),
            seqno,
            zones: vec![],
        }
    }

    pub fn with_width(width: usize, seqno: SequenceNo) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize_with(width, Cell::blank);
        let bits = LineBits::NONE;
        Self {
            bits,
            cells: CellStorage::V(VecStorage::new(cells)),
            seqno,
            zones: vec![],
        }
    }

    pub fn from_text(
        s: &str,
        attrs: &CellAttributes,
        seqno: SequenceNo,
        unicode_version: Option<UnicodeVersion>,
    ) -> Line {
        let mut cells = Vec::new();

        for sub in s.graphemes(true) {
            let cell = Cell::new_grapheme(sub, attrs.clone(), unicode_version);
            let width = cell.width();
            cells.push(cell);
            for _ in 1..width {
                cells.push(Cell::new(' ', attrs.clone()));
            }
        }

        Line {
            cells: CellStorage::V(VecStorage::new(cells)),
            bits: LineBits::NONE,
            seqno,
            zones: vec![],
        }
    }

    pub fn from_text_with_wrapped_last_col(
        s: &str,
        attrs: &CellAttributes,
        seqno: SequenceNo,
    ) -> Line {
        let mut line = Self::from_text(s, attrs, seqno, None);
        line.cells_mut()
            .last_mut()
            .map(|cell| cell.attrs_mut().set_wrapped(true));
        line
    }

    pub fn resize_and_clear(
        &mut self,
        width: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
    ) {
        {
            let cells = self.coerce_vec_storage();
            for c in cells.iter_mut() {
                *c = Cell::blank_with_attrs(blank_attr.clone());
            }
            cells.resize_with(width, || Cell::blank_with_attrs(blank_attr.clone()));
            cells.shrink_to_fit();
        }
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
        self.bits = LineBits::NONE;
    }

    pub fn resize(&mut self, width: usize, seqno: SequenceNo) {
        self.coerce_vec_storage().resize_with(width, Cell::blank);
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    /// Wrap the line so that it fits within the provided width.
    /// Returns the list of resultant line(s)
    pub fn wrap(mut self, width: usize, seqno: SequenceNo) -> Vec<Self> {
        let cells = self.coerce_vec_storage();
        if let Some(end_idx) = cells.iter().rposition(|c| c.str() != " ") {
            cells.resize_with(end_idx + 1, Cell::blank);

            let mut lines: Vec<_> = cells
                .chunks_mut(width)
                .map(|chunk| {
                    let chunk_len = chunk.len();
                    let mut line = Line {
                        cells: CellStorage::V(VecStorage::new(chunk.to_vec())),
                        bits: LineBits::NONE,
                        seqno: seqno,
                        zones: vec![],
                    };
                    if chunk_len == width {
                        // Ensure that we don't forget that we wrapped
                        line.set_last_cell_was_wrapped(true, seqno);
                    }
                    line
                })
                .collect();
            // The last of the chunks wasn't actually wrapped
            if let Some(line) = lines.last_mut() {
                line.set_last_cell_was_wrapped(false, seqno);
            }
            lines
        } else {
            vec![self]
        }
    }

    /// Returns true if the line's last changed seqno is more recent
    /// than the provided seqno parameter
    pub fn changed_since(&self, seqno: SequenceNo) -> bool {
        self.seqno == SEQ_ZERO || self.seqno > seqno
    }

    pub fn current_seqno(&self) -> SequenceNo {
        self.seqno
    }

    /// Annotate the line with the sequence number of a change.
    /// This can be used together with Line::changed_since to
    /// manage caching and rendering
    #[inline]
    pub fn update_last_change_seqno(&mut self, seqno: SequenceNo) {
        self.seqno = self.seqno.max(seqno);
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
    pub fn set_reverse(&mut self, reverse: bool, seqno: SequenceNo) {
        self.bits.set(LineBits::REVERSE, reverse);
        self.update_last_change_seqno(seqno);
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
    pub fn set_single_width(&mut self, seqno: SequenceNo) {
        self.bits.remove(LineBits::DOUBLE_WIDTH_HEIGHT_MASK);
        self.update_last_change_seqno(seqno);
    }

    /// Check whether the line is double-width and not double-height.
    #[inline]
    pub fn is_double_width(&self) -> bool {
        (self.bits & LineBits::DOUBLE_WIDTH_HEIGHT_MASK) == LineBits::DOUBLE_WIDTH
    }

    /// Force double-width.  This also implicitly sets
    /// double-height-(top/bottom) and dirty.
    #[inline]
    pub fn set_double_width(&mut self, seqno: SequenceNo) {
        self.bits
            .remove(LineBits::DOUBLE_HEIGHT_TOP | LineBits::DOUBLE_HEIGHT_BOTTOM);
        self.bits.insert(LineBits::DOUBLE_WIDTH);
        self.update_last_change_seqno(seqno);
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
    pub fn set_double_height_top(&mut self, seqno: SequenceNo) {
        self.bits.remove(LineBits::DOUBLE_HEIGHT_BOTTOM);
        self.bits
            .insert(LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_TOP);
        self.update_last_change_seqno(seqno);
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
    pub fn set_double_height_bottom(&mut self, seqno: SequenceNo) {
        self.bits.remove(LineBits::DOUBLE_HEIGHT_TOP);
        self.bits
            .insert(LineBits::DOUBLE_WIDTH | LineBits::DOUBLE_HEIGHT_BOTTOM);
        self.update_last_change_seqno(seqno);
    }

    /// Set a flag the indicate whether the line should have the bidi
    /// algorithm applied during rendering
    pub fn set_bidi_enabled(&mut self, enabled: bool, seqno: SequenceNo) {
        self.bits.set(LineBits::BIDI_ENABLED, enabled);
        self.update_last_change_seqno(seqno);
    }

    /// Set the bidi direction for the line.
    /// This affects both the bidi algorithm (if enabled via set_bidi_enabled)
    /// and the layout direction of the line.
    /// `auto_detect` specifies whether the direction should be auto-detected
    /// before falling back to the specified direction.
    pub fn set_direction(&mut self, direction: Direction, auto_detect: bool, seqno: SequenceNo) {
        self.bits
            .set(LineBits::RTL, direction == Direction::LeftToRight);
        self.bits.set(LineBits::AUTO_DETECT_DIRECTION, auto_detect);
        self.update_last_change_seqno(seqno);
    }

    pub fn set_bidi_info(
        &mut self,
        enabled: bool,
        direction: ParagraphDirectionHint,
        seqno: SequenceNo,
    ) {
        self.bits.set(LineBits::BIDI_ENABLED, enabled);
        let (auto, rtl) = match direction {
            ParagraphDirectionHint::AutoRightToLeft => (true, true),
            ParagraphDirectionHint::AutoLeftToRight => (true, false),
            ParagraphDirectionHint::LeftToRight => (false, false),
            ParagraphDirectionHint::RightToLeft => (false, true),
        };
        self.bits.set(LineBits::AUTO_DETECT_DIRECTION, auto);
        self.bits.set(LineBits::RTL, rtl);
        self.update_last_change_seqno(seqno);
    }

    /// Returns a tuple of (BIDI_ENABLED, Direction), indicating whether
    /// the line should have the bidi algorithm applied and its base
    /// direction, respectively.
    pub fn bidi_info(&self) -> (bool, ParagraphDirectionHint) {
        (
            self.bits.contains(LineBits::BIDI_ENABLED),
            match (
                self.bits.contains(LineBits::AUTO_DETECT_DIRECTION),
                self.bits.contains(LineBits::RTL),
            ) {
                (true, true) => ParagraphDirectionHint::AutoRightToLeft,
                (false, true) => ParagraphDirectionHint::RightToLeft,
                (true, false) => ParagraphDirectionHint::AutoLeftToRight,
                (false, false) => ParagraphDirectionHint::LeftToRight,
            },
        )
    }

    fn invalidate_zones(&mut self) {
        self.zones.clear();
    }

    fn compute_zones(&mut self) {
        let blank_cell = Cell::blank();
        let mut last_cell: Option<CellRef> = None;
        let mut current_zone: Option<ZoneRange> = None;
        let mut zones = vec![];

        // Rows may have trailing space+Output cells interleaved
        // with other zones as a result of clear-to-eol and
        // clear-to-end-of-screen sequences.  We don't want
        // those to affect the zones that we compute here
        let mut last_non_blank = self.len();
        for cell in self.visible_cells() {
            if cell.str() != blank_cell.str() || cell.attrs() != blank_cell.attrs() {
                last_non_blank = cell.cell_index();
            }
        }

        for cell in self.visible_cells() {
            if cell.cell_index() > last_non_blank {
                break;
            }
            let grapheme_idx = cell.cell_index() as u16;
            let semantic_type = cell.attrs().semantic_type();
            let new_zone = match last_cell {
                None => true,
                Some(ref c) => c.attrs().semantic_type() != semantic_type,
            };

            if new_zone {
                if let Some(zone) = current_zone.take() {
                    zones.push(zone);
                }

                current_zone.replace(ZoneRange {
                    range: grapheme_idx..grapheme_idx + 1,
                    semantic_type,
                });
            }

            if let Some(zone) = current_zone.as_mut() {
                zone.range.end = grapheme_idx;
            }

            last_cell.replace(cell);
        }

        if let Some(zone) = current_zone.take() {
            zones.push(zone);
        }
        self.zones = zones;
    }

    pub fn semantic_zone_ranges(&mut self) -> &[ZoneRange] {
        if self.zones.is_empty() {
            self.compute_zones();
        }
        &self.zones
    }

    /// If we have any cells with an implicit hyperlink, remove the hyperlink
    /// from the cell attributes but leave the remainder of the attributes alone.
    pub fn invalidate_implicit_hyperlinks(&mut self, seqno: SequenceNo) {
        if (self.bits & (LineBits::SCANNED_IMPLICIT_HYPERLINKS | LineBits::HAS_IMPLICIT_HYPERLINKS))
            == LineBits::NONE
        {
            return;
        }

        self.bits &= !LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        if (self.bits & LineBits::HAS_IMPLICIT_HYPERLINKS) == LineBits::NONE {
            return;
        }

        let cells = self.coerce_vec_storage();
        for cell in cells.iter_mut() {
            let replace = match cell.attrs().hyperlink() {
                Some(ref link) if link.is_implicit() => Some(Cell::new_grapheme(
                    cell.str(),
                    cell.attrs().clone().set_hyperlink(None).clone(),
                    None,
                )),
                _ => None,
            };
            if let Some(replace) = replace {
                *cell = replace;
            }
        }

        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;
        self.update_last_change_seqno(seqno);
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
        self.bits |= LineBits::SCANNED_IMPLICIT_HYPERLINKS;
        self.bits &= !LineBits::HAS_IMPLICIT_HYPERLINKS;
        let line = self.as_str();

        let matches = Rule::match_hyperlinks(&line, rules);
        if matches.is_empty() {
            return;
        }

        let line = line.into_owned();
        let cells = self.coerce_vec_storage();
        if cells.scan_and_create_hyperlinks(&line, matches) {
            self.bits |= LineBits::HAS_IMPLICIT_HYPERLINKS;
        }
    }

    /// Returns true if the line contains a hyperlink
    #[inline]
    pub fn has_hyperlink(&self) -> bool {
        (self.bits & (LineBits::HAS_HYPERLINK | LineBits::HAS_IMPLICIT_HYPERLINKS))
            != LineBits::NONE
    }

    /// Recompose line into the corresponding utf8 string.
    pub fn as_str(&self) -> Cow<str> {
        match &self.cells {
            CellStorage::V(_) => {
                let mut s = String::new();
                for cell in self.visible_cells() {
                    s.push_str(cell.str());
                }
                Cow::Owned(s)
            }
            CellStorage::C(cl) => Cow::Borrowed(&cl.text),
        }
    }

    pub fn split_off(&mut self, idx: usize, seqno: SequenceNo) -> Self {
        let my_cells = self.coerce_vec_storage();
        let cells = my_cells.split_off(idx);
        Self {
            bits: self.bits,
            cells: CellStorage::V(VecStorage::new(cells)),
            seqno,
            zones: vec![],
        }
    }

    pub fn compute_double_click_range<F: Fn(&str) -> bool>(
        &self,
        click_col: usize,
        is_word: F,
    ) -> DoubleClickRange {
        let len = self.len();

        if click_col >= len {
            return DoubleClickRange::Range(click_col..click_col);
        }

        let mut lower = click_col;
        let mut upper = click_col;

        // TODO: look back and look ahead for cells that are hidden by
        // a preceding multi-wide cell
        let cells = self.visible_cells().collect::<Vec<_>>();
        for cell in &cells {
            if cell.cell_index() < click_col {
                continue;
            }
            if !is_word(cell.str()) {
                break;
            }
            upper = cell.cell_index() + 1;
        }
        for cell in cells.iter().rev() {
            if cell.cell_index() > click_col {
                continue;
            }
            if !is_word(cell.str()) {
                break;
            }
            lower = cell.cell_index();
        }

        if upper > lower
            && upper >= len
            && cells
                .last()
                .map(|cell| cell.attrs().wrapped())
                .unwrap_or(false)
        {
            DoubleClickRange::RangeWithWrap(lower..upper)
        } else {
            DoubleClickRange::Range(lower..upper)
        }
    }

    /// Returns a substring from the line.
    pub fn columns_as_str(&self, range: Range<usize>) -> String {
        let mut s = String::new();
        for c in self.visible_cells() {
            if c.cell_index() < range.start {
                continue;
            }
            if c.cell_index() >= range.end {
                break;
            }
            s.push_str(c.str());
        }
        s
    }

    pub fn columns_as_line(&self, range: Range<usize>) -> Self {
        let mut cells = vec![];
        for c in self.visible_cells() {
            if c.cell_index() < range.start {
                continue;
            }
            if c.cell_index() >= range.end {
                break;
            }
            cells.push(c.as_cell());
        }
        Self {
            bits: LineBits::NONE,
            cells: CellStorage::V(VecStorage::new(cells)),
            seqno: self.current_seqno(),
            zones: vec![],
        }
    }

    /// If we're about to modify a cell obscured by a double-width
    /// character ahead of that cell, we need to nerf that sequence
    /// of cells to avoid partial rendering concerns.
    /// Similarly, when we assign a cell, we need to blank out those
    /// occluded successor cells.
    pub fn set_cell(&mut self, idx: usize, cell: Cell, seqno: SequenceNo) {
        self.set_cell_impl(idx, cell, false, seqno);
    }

    pub fn set_cell_clearing_image_placements(
        &mut self,
        idx: usize,
        cell: Cell,
        seqno: SequenceNo,
    ) {
        self.set_cell_impl(idx, cell, true, seqno)
    }

    fn raw_set_cell(&mut self, idx: usize, cell: Cell, clear: bool) {
        let cells = self.coerce_vec_storage();
        cells.set_cell(idx, cell, clear);
    }

    fn set_cell_impl(&mut self, idx: usize, cell: Cell, clear: bool, seqno: SequenceNo) {
        // The .max(1) stuff is here in case we get called with a
        // zero-width cell.  That shouldn't happen: those sequences
        // should get filtered out in the terminal parsing layer,
        // but in case one does sneak through, we need to ensure that
        // we grow the cells array to hold this bogus entry.
        // https://github.com/wez/wezterm/issues/768
        let width = cell.width().max(1);

        self.invalidate_implicit_hyperlinks(seqno);
        self.invalidate_zones();
        self.update_last_change_seqno(seqno);
        if cell.attrs().hyperlink().is_some() {
            self.bits |= LineBits::HAS_HYPERLINK;
        }

        if let CellStorage::C(cl) = &mut self.cells {
            if idx >= cl.len && cell == Cell::blank() {
                // Appending blank beyond end of line; is already
                // implicitly blank
                return;
            }
            while cl.len < idx {
                // Fill out any implied blanks until we can append
                // their intended cell content
                cl.append(Cell::blank());
            }
            if idx == cl.len {
                cl.append(cell);
                return;
            }
            /*
            log::info!(
                "cannot append {cell:?} to {:?} as idx={idx} and cl.len is {}",
                cl,
                cl.len
            );
            */
        }

        // if the line isn't wide enough, pad it out with the default attributes.
        {
            let cells = self.coerce_vec_storage();
            if idx + width > cells.len() {
                cells.resize_with(idx + width, Cell::blank);
            }
        }

        self.invalidate_grapheme_at_or_before(idx);

        // For double-wide or wider chars, ensure that the cells that
        // are overlapped by this one are blanked out.
        for i in 1..=width.saturating_sub(1) {
            self.raw_set_cell(idx + i, Cell::blank_with_attrs(cell.attrs().clone()), clear);
        }

        self.raw_set_cell(idx, cell, clear);
    }

    /// Place text starting at the specified column index.
    /// Each grapheme of the text run has the same attributes.
    pub fn overlay_text_with_attribute(
        &mut self,
        mut start_idx: usize,
        text: &str,
        attr: CellAttributes,
        seqno: SequenceNo,
    ) {
        for (i, c) in text.graphemes(true).enumerate() {
            let cell = Cell::new_grapheme(c, attr.clone(), None);
            let width = cell.width();
            self.set_cell(i + start_idx, cell, seqno);

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
            let cells = self.coerce_vec_storage();
            let width = cells[prior].width();
            if width > 1 {
                let attrs = cells[prior].attrs().clone();
                for nerf in prior..prior + width {
                    cells[nerf] = Cell::blank_with_attrs(attrs.clone());
                }
            }
        }
    }

    pub fn insert_cell(&mut self, x: usize, cell: Cell, right_margin: usize, seqno: SequenceNo) {
        self.invalidate_implicit_hyperlinks(seqno);

        let cells = self.coerce_vec_storage();
        if right_margin <= cells.len() {
            cells.remove(right_margin - 1);
        }

        if x >= cells.len() {
            cells.resize_with(x, Cell::blank);
        }

        // If we're inserting a wide cell, we should also insert the overlapped cells.
        // We insert them first so that the grapheme winds up left-most.
        let width = cell.width();
        for _ in 1..=width.saturating_sub(1) {
            cells.insert(x, Cell::blank_with_attrs(cell.attrs().clone()));
        }

        cells.insert(x, cell);
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    pub fn erase_cell(&mut self, x: usize, seqno: SequenceNo) {
        if x >= self.len() {
            // Already implicitly erased
            return;
        }
        self.invalidate_implicit_hyperlinks(seqno);
        self.invalidate_grapheme_at_or_before(x);
        {
            let cells = self.coerce_vec_storage();
            cells.remove(x);
            cells.push(Cell::default());
        }
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    pub fn remove_cell(&mut self, x: usize, seqno: SequenceNo) {
        if x >= self.len() {
            // Already implicitly removed
            return;
        }
        self.invalidate_implicit_hyperlinks(seqno);
        self.invalidate_grapheme_at_or_before(x);
        self.coerce_vec_storage().remove(x);
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    pub fn erase_cell_with_margin(
        &mut self,
        x: usize,
        right_margin: usize,
        seqno: SequenceNo,
        blank_attr: CellAttributes,
    ) {
        self.invalidate_implicit_hyperlinks(seqno);
        if x < self.len() {
            self.invalidate_grapheme_at_or_before(x);
            self.coerce_vec_storage().remove(x);
        }
        if right_margin <= self.len() + 1
        /* we just removed one */
        {
            self.coerce_vec_storage()
                .insert(right_margin - 1, Cell::blank_with_attrs(blank_attr));
        }
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    pub fn prune_trailing_blanks(&mut self, seqno: SequenceNo) {
        if let CellStorage::C(cl) = &mut self.cells {
            if cl.prune_trailing_blanks() {
                self.update_last_change_seqno(seqno);
                self.invalidate_zones();
            }
            return;
        }

        let def_attr = CellAttributes::blank();
        let cells = self.coerce_vec_storage();
        if let Some(end_idx) = cells
            .iter()
            .rposition(|c| c.str() != " " || c.attrs() != &def_attr)
        {
            cells.resize_with(end_idx + 1, Cell::blank);
            self.update_last_change_seqno(seqno);
            self.invalidate_zones();
        }
    }

    pub fn fill_range(&mut self, cols: Range<usize>, cell: &Cell, seqno: SequenceNo) {
        if self.len() == 0 && *cell == Cell::blank() {
            // We would be filling it with blanks only to prune
            // them all away again before we return; NOP
            return;
        }
        for x in cols {
            // FIXME: we can skip the look-back for second and subsequent iterations
            self.set_cell_impl(x, cell.clone(), true, seqno);
        }
        self.prune_trailing_blanks(seqno);
    }

    pub fn len(&self) -> usize {
        match &self.cells {
            CellStorage::V(cells) => cells.len(),
            CellStorage::C(cl) => cl.len(),
        }
    }

    /// Iterates the visible cells, respecting the width of the cell.
    /// For instance, a double-width cell overlaps the following (blank)
    /// cell, so that blank cell is omitted from the iterator results.
    /// The iterator yields (column_index, Cell).  Column index is the
    /// index into Self::cells, and due to the possibility of skipping
    /// the characters that follow wide characters, the column index may
    /// skip some positions.  It is returned as a convenience to the consumer
    /// as using .enumerate() on this iterator wouldn't be as useful.
    pub fn visible_cells<'a>(&'a self) -> impl Iterator<Item = CellRef<'a>> {
        match &self.cells {
            CellStorage::V(cells) => VisibleCellIter::V(CellSliceIter {
                cells: cells.iter(),
                idx: 0,
                skip_width: 0,
            }),
            CellStorage::C(cl) => VisibleCellIter::C(cl.iter()),
        }
    }

    pub fn get_cell(&self, cell_index: usize) -> Option<CellRef> {
        self.visible_cells()
            .find(|cell| cell.cell_index() == cell_index)
    }

    pub fn cluster(&self, bidi_hint: Option<ParagraphDirectionHint>) -> Vec<CellCluster> {
        CellCluster::make_cluster(self.len(), self.visible_cells(), bidi_hint)
    }

    fn make_cells(&mut self) {
        let cells = match &self.cells {
            CellStorage::V(_) => return,
            CellStorage::C(cl) => cl.to_cell_vec(),
        };
        // log::info!("make_cells\n{:?}", backtrace::Backtrace::new());
        self.cells = CellStorage::V(VecStorage::new(cells));
    }

    fn coerce_vec_storage(&mut self) -> &mut VecStorage {
        self.make_cells();

        match &mut self.cells {
            CellStorage::V(c) => return c,
            CellStorage::C(_) => unreachable!(),
        }
    }

    /// Adjusts the internal storage so that it occupies less
    /// space. Subsequent mutations will incur some overhead to
    /// re-materialize the storage in a form that is suitable
    /// for mutation.
    pub fn compress_for_scrollback(&mut self) {
        let cv = match &self.cells {
            CellStorage::V(v) => ClusteredLine::from_cell_vec(v.len(), self.visible_cells()),
            CellStorage::C(_) => return,
        };
        self.cells = CellStorage::C(cv);
    }

    pub fn cells_mut(&mut self) -> &mut [Cell] {
        self.coerce_vec_storage().as_mut_slice()
    }

    /// Return true if the line consists solely of whitespace cells
    pub fn is_whitespace(&self) -> bool {
        self.visible_cells().all(|c| c.str() == " ")
    }

    /// Return true if the last cell in the line has the wrapped attribute,
    /// indicating that the following line is logically a part of this one.
    pub fn last_cell_was_wrapped(&self) -> bool {
        self.visible_cells()
            .last()
            .map(|c| c.attrs().wrapped())
            .unwrap_or(false)
    }

    /// Adjust the value of the wrapped attribute on the last cell of this
    /// line.
    pub fn set_last_cell_was_wrapped(&mut self, wrapped: bool, seqno: SequenceNo) {
        self.update_last_change_seqno(seqno);
        if let CellStorage::C(cl) = &mut self.cells {
            if cl.len() > 0 {
                cl.set_last_cell_was_wrapped(wrapped);
                return;
            }
        }

        let cells = self.coerce_vec_storage();
        if let Some(cell) = cells.last_mut() {
            cell.attrs_mut().set_wrapped(wrapped);
        }
    }

    /// Concatenate the cells from other with this line, appending them
    /// to this line.
    /// This function is used by rewrapping logic when joining wrapped
    /// lines back together.
    pub fn append_line(&mut self, other: Line, seqno: SequenceNo) {
        match &mut self.cells {
            CellStorage::V(cells) => {
                for cell in other.visible_cells() {
                    cells.push(cell.as_cell());
                }
            }
            CellStorage::C(cl) => {
                for cell in other.visible_cells() {
                    cl.append(cell.as_cell());
                }
            }
        }
        self.update_last_change_seqno(seqno);
        self.invalidate_zones();
    }

    /// mutable access the cell data, but the caller must take care
    /// to only mutate attributes rather than the cell textual content.
    /// Use set_cell if you need to modify the textual content of the
    /// cell, so that important invariants are upheld.
    pub fn cells_mut_for_attr_changes_only(&mut self) -> &mut [Cell] {
        self.coerce_vec_storage().as_mut_slice()
    }

    /// Given a starting attribute value, produce a series of Change
    /// entries to recreate the current line
    pub fn changes(&self, start_attr: &CellAttributes) -> Vec<Change> {
        let mut result = Vec::new();
        let mut attr = start_attr.clone();
        let mut text_run = String::new();

        for cell in self.visible_cells() {
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
                    .set_background(attr.background())
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
                    if attr.background() != Default::default() {
                        result.push(Change::ClearToEndOfLine(attr.background()));
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

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct VecStorage {
    cells: Vec<Cell>,
}

impl VecStorage {
    fn new(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    fn set_cell(&mut self, idx: usize, mut cell: Cell, clear_image_placement: bool) {
        if !clear_image_placement {
            if let Some(images) = self.cells[idx].attrs().images() {
                for image in images {
                    if image.has_placement_id() {
                        cell.attrs_mut().attach_image(Box::new(image));
                    }
                }
            }
        }
        self.cells[idx] = cell;
    }

    fn scan_and_create_hyperlinks(
        &mut self,
        line: &str,
        matches: Vec<crate::hyperlink::RuleMatch>,
    ) -> bool {
        // The capture range is measured in bytes but we need to translate
        // that to the index of the column.  This is complicated a bit further
        // because double wide sequences have a blank column cell after them
        // in the cells array, but the string we match against excludes that
        // string.
        let mut cell_idx = 0;
        let mut has_implicit_hyperlinks = false;
        for (byte_idx, _grapheme) in line.grapheme_indices(true) {
            let cell = &mut self.cells[cell_idx];
            let mut matched = false;
            for m in &matches {
                if m.range.contains(&byte_idx) {
                    let attrs = cell.attrs_mut();
                    // Don't replace existing links
                    if attrs.hyperlink().is_none() {
                        attrs.set_hyperlink(Some(Arc::clone(&m.link)));
                        matched = true;
                    }
                }
            }
            cell_idx += cell.width();
            if matched {
                has_implicit_hyperlinks = true;
            }
        }

        has_implicit_hyperlinks
    }
}

impl std::ops::Deref for VecStorage {
    type Target = Vec<Cell>;

    fn deref(&self) -> &Vec<Cell> {
        &self.cells
    }
}

impl std::ops::DerefMut for VecStorage {
    fn deref_mut(&mut self) -> &mut Vec<Cell> {
        &mut self.cells
    }
}

impl<'a> From<&'a str> for Line {
    fn from(s: &str) -> Line {
        Line::from_text(s, &CellAttributes::default(), SEQ_ZERO, None)
    }
}

/// Iterates over a slice of Cell, yielding only visible cells
struct CellSliceIter<'a> {
    cells: std::slice::Iter<'a, Cell>,
    idx: usize,
    skip_width: usize,
}

impl<'a> Iterator for CellSliceIter<'a> {
    type Item = CellRef<'a>;

    fn next(&mut self) -> Option<CellRef<'a>> {
        while self.skip_width > 0 {
            self.skip_width -= 1;
            let _ = self.cells.next()?;
            self.idx += 1;
        }
        let cell = self.cells.next()?;
        let cell_index = self.idx;
        self.idx += 1;
        self.skip_width = cell.width().saturating_sub(1);
        Some(CellRef::CellRef { cell_index, cell })
    }
}

enum VisibleCellIter<'a> {
    V(CellSliceIter<'a>),
    C(ClusterLineCellIter<'a>),
}

impl<'a> Iterator for VisibleCellIter<'a> {
    type Item = CellRef<'a>;

    fn next(&mut self) -> Option<CellRef<'a>> {
        match self {
            Self::V(iter) => iter.next(),
            Self::C(iter) => iter.next(),
        }
    }
}

#[derive(Debug)]
pub enum CellRef<'a> {
    CellRef {
        cell_index: usize,
        cell: &'a Cell,
    },
    ClusterRef {
        cell_index: usize,
        text: &'a str,
        width: usize,
        attrs: &'a CellAttributes,
    },
}

impl<'a> CellRef<'a> {
    pub fn cell_index(&self) -> usize {
        match self {
            Self::ClusterRef { cell_index, .. } | Self::CellRef { cell_index, .. } => *cell_index,
        }
    }

    pub fn str(&self) -> &str {
        match self {
            Self::CellRef { cell, .. } => cell.str(),
            Self::ClusterRef { text, .. } => text,
        }
    }

    pub fn width(&self) -> usize {
        match self {
            Self::CellRef { cell, .. } => cell.width(),
            Self::ClusterRef { width, .. } => *width,
        }
    }

    pub fn attrs(&self) -> &CellAttributes {
        match self {
            Self::CellRef { cell, .. } => cell.attrs(),
            Self::ClusterRef { attrs, .. } => attrs,
        }
    }

    pub fn presentation(&self) -> Presentation {
        match self {
            Self::CellRef { cell, .. } => cell.presentation(),
            Self::ClusterRef { text, .. } => match Presentation::for_grapheme(text) {
                (_, Some(variation)) => variation,
                (presentation, None) => presentation,
            },
        }
    }

    pub fn as_cell(&self) -> Cell {
        match self {
            Self::CellRef { cell, .. } => (*cell).clone(),
            Self::ClusterRef {
                text, width, attrs, ..
            } => Cell::new_grapheme_with_width(text, *width, (*attrs).clone()),
        }
    }

    pub fn same_contents(&self, other: &Self) -> bool {
        self.str() == other.str() && self.width() == other.width() && self.attrs() == other.attrs()
    }
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct Cluster {
    cell_width: usize,
    attrs: CellAttributes,
}

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Default, Debug, Clone, PartialEq)]
struct ClusteredLine {
    text: String,
    #[cfg_attr(
        feature = "use_serde",
        serde(
            deserialize_with = "deserialize_bitset",
            serialize_with = "serialize_bitset"
        )
    )]
    is_double_wide: Option<FixedBitSet>,
    clusters: Vec<Cluster>,
    len: usize,
}

#[cfg(feature = "use_serde")]
fn deserialize_bitset<'de, D>(deserializer: D) -> Result<Option<FixedBitSet>, D::Error>
where
    D: Deserializer<'de>,
{
    let wide_indices = <Vec<usize>>::deserialize(deserializer)?;
    if wide_indices.is_empty() {
        Ok(None)
    } else {
        let max_idx = wide_indices.iter().max().unwrap_or(&1);
        let mut bitset = FixedBitSet::with_capacity(max_idx + 1);
        for idx in wide_indices {
            bitset.set(idx, true);
        }
        Ok(Some(bitset))
    }
}

/// Serialize the bitset as a vector of the indices of just the 1 bits;
/// the thesis is that most of the cells on a given line are single width.
/// That may not be strictly true for users that heavily use asian scripts,
/// but we'll start with this and see if we need to improve it.
#[cfg(feature = "use_serde")]
fn serialize_bitset<S>(value: &Option<FixedBitSet>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut wide_indices: Vec<usize> = vec![];
    if let Some(bits) = value {
        for idx in bits.ones() {
            wide_indices.push(idx);
        }
    }
    wide_indices.serialize(serializer)
}

impl ClusteredLine {
    fn to_cell_vec(&self) -> Vec<Cell> {
        let mut cells = vec![];

        for c in self.iter() {
            cells.push(c.as_cell());
            for _ in 1..c.width() {
                cells.push(Cell::blank_with_attrs(c.attrs().clone()));
            }
        }

        cells
    }

    fn from_cell_vec<'a>(hint: usize, iter: impl Iterator<Item = CellRef<'a>>) -> Self {
        let mut last_cluster: Option<Cluster> = None;
        let mut is_double_wide = FixedBitSet::with_capacity(hint);
        let mut text = String::new();
        let mut clusters = vec![];
        let mut any_double = false;
        let mut len = 0;

        for cell in iter {
            len += cell.width();

            if cell.width() > 1 {
                any_double = true;
                is_double_wide.set(cell.cell_index(), true);
            }

            text.push_str(cell.str());

            last_cluster = match last_cluster.take() {
                None => Some(Cluster {
                    cell_width: cell.width(),
                    attrs: cell.attrs().clone(),
                }),
                Some(cluster) if cluster.attrs != *cell.attrs() => {
                    clusters.push(cluster);
                    Some(Cluster {
                        cell_width: cell.width(),
                        attrs: cell.attrs().clone(),
                    })
                }
                Some(mut cluster) => {
                    cluster.cell_width += cell.width();
                    Some(cluster)
                }
            };
        }

        if let Some(cluster) = last_cluster.take() {
            clusters.push(cluster);
        }

        Self {
            text,
            is_double_wide: if any_double {
                Some(is_double_wide)
            } else {
                None
            },
            clusters,
            len,
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_double_wide(&self, cell_index: usize) -> bool {
        match &self.is_double_wide {
            Some(bitset) => bitset.contains(cell_index),
            None => false,
        }
    }

    fn iter(&self) -> ClusterLineCellIter {
        let mut clusters = self.clusters.iter();
        let cluster = clusters.next();
        ClusterLineCellIter {
            graphemes: self.text.graphemes(true),
            clusters,
            cluster,
            idx: 0,
            cluster_total: 0,
            line: self,
        }
    }

    fn append(&mut self, cell: Cell) {
        let new_cluster = match self.clusters.last() {
            Some(cluster) => cluster.attrs != *cell.attrs(),
            None => true,
        };
        let new_cell_index = self.len;
        let cell_width = cell.width();
        if new_cluster {
            self.clusters.push(Cluster {
                attrs: (*cell.attrs()).clone(),
                cell_width,
            });
        } else if let Some(cluster) = self.clusters.last_mut() {
            cluster.cell_width += cell_width;
        }
        self.text.push_str(cell.str());

        if cell_width > 1 {
            let bitset = match self.is_double_wide.take() {
                Some(mut bitset) => {
                    bitset.grow(new_cell_index + 1);
                    bitset.set(new_cell_index, true);
                    bitset
                }
                None => {
                    let mut bitset = FixedBitSet::with_capacity(new_cell_index + 1);
                    bitset.set(new_cell_index, true);
                    bitset
                }
            };
            self.is_double_wide.replace(bitset);
        }

        self.len += cell_width;
    }

    fn prune_trailing_blanks(&mut self) -> bool {
        let num_spaces = self.text.chars().rev().take_while(|&c| c == ' ').count();
        if num_spaces == 0 {
            return false;
        }

        let blank = CellAttributes::blank();
        let mut pruned = false;
        for _ in 0..num_spaces {
            let mut need_pop = false;
            if let Some(cluster) = self.clusters.last_mut() {
                if cluster.attrs != blank {
                    break;
                }
                cluster.cell_width -= 1;
                self.text.pop();
                self.len -= 1;
                pruned = true;
                if cluster.cell_width == 0 {
                    need_pop = true;
                }
            }
            if need_pop {
                self.clusters.pop();
            }
        }

        pruned
    }

    fn set_last_cell_was_wrapped(&mut self, wrapped: bool) {
        if let Some(last_cell) = self.iter().last() {
            if last_cell.attrs().wrapped() == wrapped {
                // Nothing to change
                //return;
            }
            let mut attrs = last_cell.attrs().clone();
            attrs.set_wrapped(wrapped);
            let width = last_cell.width();

            let last_cluster = self.clusters.last_mut().unwrap();
            if last_cluster.cell_width == width {
                // Re-purpose final cluster
                last_cluster.attrs = attrs;
            } else {
                last_cluster.cell_width -= width;
                self.clusters.push(Cluster {
                    cell_width: width,
                    attrs,
                });
            }
        }
    }
}

pub struct ClusterLineCellIter<'a> {
    graphemes: unicode_segmentation::Graphemes<'a>,
    clusters: std::slice::Iter<'a, Cluster>,
    cluster: Option<&'a Cluster>,
    idx: usize,
    cluster_total: usize,
    line: &'a ClusteredLine,
}

impl<'a> Iterator for ClusterLineCellIter<'a> {
    type Item = CellRef<'a>;

    fn next(&mut self) -> Option<CellRef<'a>> {
        let text = self.graphemes.next()?;

        let cell_index = self.idx;
        let width = if self.line.is_double_wide(cell_index) {
            2
        } else {
            1
        };
        self.idx += width;
        self.cluster_total += width;
        let attrs = &self.cluster.as_ref()?.attrs;

        if self.cluster_total >= self.cluster.as_ref()?.cell_width {
            self.cluster = self.clusters.next();
            self.cluster_total = 0;
        }

        Some(CellRef::ClusterRef {
            cell_index,
            width,
            text,
            attrs,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::hyperlink::*;
    use k9::assert_equal as assert_eq;

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
            line.coerce_vec_storage().to_vec(),
            vec![
                Cell::new_grapheme("❤", CellAttributes::default(), None),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new_grapheme("😍", CellAttributes::default(), None),
                Cell::new(' ', CellAttributes::default()), // double width spacer
                Cell::new_grapheme("🤢", CellAttributes::default(), None),
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
                    CellAttributes::default(),
                    None,
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

    #[test]
    fn cluster_representation_basic() {
        let line: Line = "hello".into();
        let mut compressed = line.clone();
        compressed.compress_for_scrollback();
        k9::snapshot!(
            &compressed.cells,
            r#"
C(
    ClusteredLine {
        text: "hello",
        is_double_wide: None,
        clusters: [
            Cluster {
                cell_width: 5,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 5,
    },
)
"#
        );
        compressed.coerce_vec_storage();
        assert_eq!(line, compressed);
    }

    #[test]
    fn cluster_representation_double_width() {
        let line: Line = "❤ 😍🤢he❤ 😍🤢llo❤ 😍🤢".into();
        let mut compressed = line.clone();
        compressed.compress_for_scrollback();
        k9::snapshot!(
            &compressed.cells,
            r#"
C(
    ClusteredLine {
        text: "❤ 😍🤢he❤ 😍🤢llo❤ 😍🤢",
        is_double_wide: Some(
            FixedBitSet {
                data: [
                    2626580,
                ],
                length: 23,
            },
        ),
        clusters: [
            Cluster {
                cell_width: 23,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 23,
    },
)
"#
        );
        compressed.coerce_vec_storage();
        assert_eq!(line, compressed);
    }

    #[test]
    fn cluster_representation_empty() {
        let line = Line::from_cells(vec![], SEQ_ZERO);

        let mut compressed = line.clone();
        compressed.compress_for_scrollback();
        k9::snapshot!(
            &compressed.cells,
            r#"
C(
    ClusteredLine {
        text: "",
        is_double_wide: None,
        clusters: [],
        len: 0,
    },
)
"#
        );
        compressed.coerce_vec_storage();
        assert_eq!(line, compressed);
    }

    #[test]
    fn cluster_wrap_last() {
        let mut line: Line = "hello".into();
        line.compress_for_scrollback();
        line.set_last_cell_was_wrapped(true, 1);
        k9::snapshot!(
            line,
            r#"
Line {
    cells: C(
        ClusteredLine {
            text: "hello",
            is_double_wide: None,
            clusters: [
                Cluster {
                    cell_width: 4,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 1,
                    attrs: CellAttributes {
                        attributes: 2048,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: true,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
            ],
            len: 5,
        },
    ),
    zones: [],
    seqno: 1,
    bits: NONE,
}
"#
        );
    }

    fn bold() -> CellAttributes {
        use crate::cell::Intensity;
        let mut attr = CellAttributes::default();
        attr.set_intensity(Intensity::Bold);
        attr
    }

    #[test]
    fn cluster_representation_attributes() {
        let line = Line::from_cells(
            vec![
                Cell::new_grapheme("a", CellAttributes::default(), None),
                Cell::new_grapheme("b", bold(), None),
                Cell::new_grapheme("c", CellAttributes::default(), None),
                Cell::new_grapheme("d", bold(), None),
            ],
            SEQ_ZERO,
        );

        let mut compressed = line.clone();
        compressed.compress_for_scrollback();
        k9::snapshot!(
            &compressed.cells,
            r#"
C(
    ClusteredLine {
        text: "abcd",
        is_double_wide: None,
        clusters: [
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 1,
                    intensity: Bold,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 1,
                    intensity: Bold,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 4,
    },
)
"#
        );
        compressed.coerce_vec_storage();
        assert_eq!(line, compressed);
    }

    #[test]
    fn cluster_append() {
        let mut cl = ClusteredLine::default();
        cl.append(Cell::new_grapheme("h", CellAttributes::default(), None));
        cl.append(Cell::new_grapheme("e", CellAttributes::default(), None));
        cl.append(Cell::new_grapheme("l", bold(), None));
        cl.append(Cell::new_grapheme("l", CellAttributes::default(), None));
        cl.append(Cell::new_grapheme("o", CellAttributes::default(), None));
        k9::snapshot!(
            cl,
            r#"
ClusteredLine {
    text: "hello",
    is_double_wide: None,
    clusters: [
        Cluster {
            cell_width: 2,
            attrs: CellAttributes {
                attributes: 0,
                intensity: Normal,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
        Cluster {
            cell_width: 1,
            attrs: CellAttributes {
                attributes: 1,
                intensity: Bold,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
        Cluster {
            cell_width: 2,
            attrs: CellAttributes {
                attributes: 0,
                intensity: Normal,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
    ],
    len: 5,
}
"#
        );
    }

    #[test]
    fn cluster_line_new() {
        let mut line = Line::new(1);
        line.set_cell(
            0,
            Cell::new_grapheme("h", CellAttributes::default(), None),
            1,
        );
        line.set_cell(
            1,
            Cell::new_grapheme("e", CellAttributes::default(), None),
            2,
        );
        line.set_cell(2, Cell::new_grapheme("l", bold(), None), 3);
        line.set_cell(
            3,
            Cell::new_grapheme("l", CellAttributes::default(), None),
            4,
        );
        line.set_cell(
            4,
            Cell::new_grapheme("o", CellAttributes::default(), None),
            5,
        );
        k9::snapshot!(
            line,
            r#"
Line {
    cells: C(
        ClusteredLine {
            text: "hello",
            is_double_wide: None,
            clusters: [
                Cluster {
                    cell_width: 2,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 1,
                    attrs: CellAttributes {
                        attributes: 1,
                        intensity: Bold,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 2,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
            ],
            len: 5,
        },
    ),
    zones: [],
    seqno: 5,
    bits: NONE,
}
"#
        );
    }
}
