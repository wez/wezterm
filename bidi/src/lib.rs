use level::MAX_DEPTH;
use level_stack::{LevelStack, Override};
use log::trace;
use std::borrow::Cow;
use std::ops::Range;
use wezterm_dynamic::{FromDynamic, ToDynamic};

mod bidi_brackets;
mod bidi_class;
mod direction;
mod level;
mod level_stack;

use bidi_brackets::BracketType;
pub use bidi_class::BidiClass;
pub use direction::Direction;
pub use level::Level;

/// Placeholder codepoint index that corresponds to NO_LEVEL
const DELETED: usize = usize::max_value();

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromDynamic, ToDynamic)]
pub enum ParagraphDirectionHint {
    LeftToRight,
    RightToLeft,
    /// Attempt to auto-detect but fall back to LTR
    AutoLeftToRight,
    /// Attempt to auto-detect but fall back to RTL
    AutoRightToLeft,
}

impl Default for ParagraphDirectionHint {
    fn default() -> Self {
        Self::LeftToRight
    }
}

impl ParagraphDirectionHint {
    /// Returns just the direction portion of the hint, independent
    /// of the auto-detection state.
    pub fn direction(self) -> Direction {
        match self {
            ParagraphDirectionHint::AutoLeftToRight | ParagraphDirectionHint::LeftToRight => {
                Direction::LeftToRight
            }
            ParagraphDirectionHint::AutoRightToLeft | ParagraphDirectionHint::RightToLeft => {
                Direction::RightToLeft
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct BidiContext {
    orig_char_types: Vec<BidiClass>,
    char_types: Vec<BidiClass>,
    levels: Vec<Level>,
    base_level: Level,
    runs: Vec<Run>,
    reorder_nsm: bool,
}

/// Represents a formatting character that has been removed by the X9 rule
pub const NO_LEVEL: i8 = -1;

/// A `BidiRun` represents a run which is a contiguous sequence of codepoints
/// from the original paragraph that have been resolved to the same embedding
/// level, and that thus all have the same direction.
///
/// The `range` field encapsulates the starting and ending codepoint indices
/// into the original paragraph.
///
/// Note: while the run sequence has the same level throughout, the X9 portion
/// of the bidi algorithm can logically delete some control characters.
/// I haven't been able to prove to myself that those control characters
/// never manifest in the middle of a run, so it is recommended that you use the `indices`
/// method to skip over any such elements if your shaper doesn't want them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidiRun {
    /// The direction for this run.  Derived from the level.
    pub direction: Direction,

    /// Embedding level of this run.
    pub level: Level,

    /// The starting and ending codepoint indices for this run
    pub range: Range<usize>,

    /// the list of control codepoint indices that were removed from the text
    /// by the X9 portion of the bidi algorithm.
    // Expected to have low cardinality and be generally empty, so we're
    // using a simple vec for this.
    pub removed_by_x9: Vec<usize>,
}

impl BidiRun {
    pub fn indices<'a>(&'a self) -> impl Iterator<Item = usize> + 'a {
        struct Iter<'a> {
            range: Range<usize>,
            removed_by_x9: &'a [usize],
        }

        impl<'a> Iterator for Iter<'a> {
            type Item = usize;
            fn next(&mut self) -> Option<usize> {
                for idx in self.range.by_ref() {
                    if self.removed_by_x9.iter().any(|&i| i == idx) {
                        // Skip it
                        continue;
                    }
                    return Some(idx);
                }
                None
            }
        }

        Iter {
            range: self.range.clone(),
            removed_by_x9: &self.removed_by_x9,
        }
    }
}

struct RunIter<'a> {
    pos: usize,
    levels: Cow<'a, [Level]>,
    line_range: Range<usize>,
}

impl<'a> Iterator for RunIter<'a> {
    type Item = BidiRun;

    fn next(&mut self) -> Option<BidiRun> {
        loop {
            if self.pos >= self.levels.len() {
                return None;
            }

            let start = self.pos;
            let len = span_len(start, &self.levels);
            self.pos += len;

            let level = self.levels[start];
            if !level.removed_by_x9() {
                let range = start..start + len;

                let mut removed_by_x9 = vec![];
                for idx in range.clone() {
                    if self.levels[idx].removed_by_x9() {
                        removed_by_x9.push(idx + self.line_range.start);
                    }
                }

                return Some(BidiRun {
                    direction: level.direction(),
                    level,
                    range: self.line_range.start + range.start..self.line_range.start + range.end,
                    removed_by_x9,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorderedRun {
    /// The direction for this run.  Derived from the level.
    pub direction: Direction,

    /// Embedding level of this run.
    pub level: Level,

    /// The starting and ending codepoint indices for this run
    pub range: Range<usize>,

    /// The indices in their adjusted order
    pub indices: Vec<usize>,
}

fn span_len(start: usize, levels: &[Level]) -> usize {
    let starting_level = levels[start];
    levels
        .iter()
        .skip(start + 1)
        .position(|&l| l != starting_level)
        .unwrap_or(levels.len() - (start + 1))
        + 1
}

impl BidiContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_level(&self) -> Level {
        self.base_level
    }

    /// When `reorder` is set to true, reordering will apply rule L3 to
    /// non-spacing marks.  This is likely more desirable for terminal
    /// based applications than it is for more modern GUI applications
    /// that feed into eg: harfbuzz.
    pub fn set_reorder_non_spacing_marks(&mut self, reorder: bool) {
        self.reorder_nsm = reorder;
    }

    /// Produces a sequence of `BidiRun` structs that represent runs of
    /// text and their direction (and level) across the entire paragraph.
    pub fn runs<'a>(&'a self) -> impl Iterator<Item = BidiRun> + 'a {
        RunIter {
            pos: 0,
            levels: Cow::Borrowed(&self.levels),
            line_range: 0..self.levels.len(),
        }
    }

    /// Given a line_range (a subslice of the current paragraph that represents
    /// a single wrapped line), this method resets whitespace levels for the line
    /// boundaries, and then returns the set of runs for that line.
    pub fn line_runs(&self, line_range: Range<usize>) -> impl Iterator<Item = BidiRun> {
        let levels = self.reset_whitespace_levels(line_range.clone());

        RunIter {
            pos: 0,
            levels: levels.into(),
            line_range,
        }
    }

    pub fn reordered_runs(&self, line_range: Range<usize>) -> Vec<ReorderedRun> {
        // reorder_line's `level` result includes entries that were
        // removed_by_x9() but `reordered` does NOT (for compatibility with
        // the UCD test suite).
        // We need to account for that when we reorder the levels here!
        let (levels, reordered) = self.reorder_line(line_range);
        let mut reordered_levels = vec![Level(NO_LEVEL); reordered.len()];

        for (vis_idx, &log_idx) in reordered.iter().enumerate() {
            reordered_levels[vis_idx] = levels[log_idx];
        }

        reordered_levels.retain(|l| !l.removed_by_x9());

        let mut runs = vec![];

        let mut idx = 0;
        while idx < reordered_levels.len() {
            let len = span_len(idx, &reordered_levels);
            let level = reordered_levels[idx];
            if !level.removed_by_x9() {
                let idx_range = idx..idx + len;
                let start = reordered[idx_range.clone()].iter().min().unwrap();
                let end = reordered[idx_range.clone()].iter().max().unwrap();
                runs.push(ReorderedRun {
                    direction: level.direction(),
                    level,
                    range: *start..*end + 1,
                    indices: reordered[idx_range].to_vec(),
                });
            }
            idx += len;
        }

        runs
    }

    /// `line_range` indicates a contiguous range of character indices
    /// in the paragraph set via `resolve_paragraph`.
    /// This method returns the reordered set of indices for display
    /// purposes.
    pub fn reorder_line(&self, line_range: Range<usize>) -> (Vec<Level>, Vec<usize>) {
        self.dump_state("before L1");
        let mut levels = self.reset_whitespace_levels(line_range.clone());
        assert_eq!(levels.len(), line_range.end - line_range.start);
        let reordered = self.reverse_levels(line_range.start, &mut levels);

        (levels, reordered)
    }

    /// Performs Rule L3.
    /// This rule is optional and must be enabled by calling the
    /// set_reorder_non_spacing_marks method
    fn reorder_non_spacing_marks(&self, levels: &mut [Level], visual: &mut [usize]) {
        let mut idx = levels.len() - 1;
        loop {
            if idx > 0
                && !levels[idx].removed_by_x9()
                && levels[idx].direction() == Direction::RightToLeft
                && self.orig_char_types[visual[idx]] == BidiClass::NonspacingMark
            {
                // Keep scanning backwards within this level
                let level = levels[idx];
                let seq_end = idx;

                idx -= 1;
                while idx > 0 && levels[idx].removed_by_x9()
                    || (levels[idx] == level
                        && matches!(
                            self.orig_char_types[visual[idx]],
                            BidiClass::LeftToRightEmbedding
                                | BidiClass::RightToLeftEmbedding
                                | BidiClass::LeftToRightOverride
                                | BidiClass::RightToLeftOverride
                                | BidiClass::PopDirectionalFormat
                                | BidiClass::BoundaryNeutral
                                | BidiClass::NonspacingMark
                        ))
                {
                    idx -= 1;
                }

                if levels[idx] != level {
                    idx += 1;
                }

                if seq_end > idx {
                    visual[idx..=seq_end].reverse();
                    levels[idx..=seq_end].reverse();
                }
            }

            if idx == 0 {
                return;
            }
            idx -= 1;
        }
    }

    /// This function runs Rule L2.
    ///
    /// Find the highest level among the resolved levels.
    /// Then from that highest level down to the lowest odd
    /// level, reverse any contiguous runs at that level or higher.
    fn reverse_levels(&self, first_cidx: usize, levels: &mut [Level]) -> Vec<usize> {
        // Not typed as Level because the Step trait required by the loop
        // below is nightly only
        let mut highest_level = 0;
        let mut lowest_odd_level = MAX_DEPTH as i8 + 1;
        let mut no_levels = true;

        for &level in levels.iter() {
            if level.removed_by_x9() {
                continue;
            }

            // Found something other than NO_LEVEL
            no_levels = false;
            highest_level = highest_level.max(level.0);
            if level.0 % 2 == 1 && level.0 < lowest_odd_level {
                lowest_odd_level = level.0;
            }
        }

        if no_levels {
            return vec![];
        }

        // Initial visual order
        let mut visual = vec![];
        for i in 0..levels.len() {
            if levels[i].removed_by_x9() {
                visual.push(DELETED);
            } else {
                visual.push(i + first_cidx);
            }
        }

        // Apply L3. UAX9 has this occur after L2, but we do it
        // before that for consistency with FriBidi's implementation.
        if self.reorder_nsm {
            self.reorder_non_spacing_marks(levels, &mut visual);
        }

        // Apply L2.
        for level in (lowest_odd_level..=highest_level).rev() {
            let level = Level(level);
            let mut i = 0;
            let mut in_range = false;
            let mut significant_range = false;
            let mut first_pos = None;
            let mut last_pos = None;

            while i < levels.len() {
                if levels[i] >= level {
                    if !in_range {
                        in_range = true;
                        first_pos.replace(i);
                    } else {
                        // Hit a second explicit level
                        significant_range = true;
                        last_pos.replace(i);
                    }
                } else if levels[i].removed_by_x9() {
                    // Don't break ranges for deleted controls
                    if in_range {
                        last_pos.replace(i);
                    }
                } else {
                    // End of a range.  Reset the range flag
                    // and rever the range.
                    in_range = false;
                    match (last_pos, first_pos, significant_range) {
                        (Some(last_pos), Some(first_pos), true) if last_pos > first_pos => {
                            visual[first_pos..=last_pos].reverse();
                        }
                        _ => {}
                    }
                    first_pos = None;
                    last_pos = None;
                }
                i += 1;
            }

            if in_range && significant_range {
                match (last_pos, first_pos) {
                    (Some(last_pos), Some(first_pos)) if last_pos > first_pos => {
                        visual[first_pos..=last_pos].reverse();
                    }
                    _ => {}
                }
            }
        }

        visual.retain(|&i| i != DELETED);
        visual
    }

    /// <http://unicode.org/reports/tr9/>
    pub fn resolve_paragraph(&mut self, paragraph: &[char], hint: ParagraphDirectionHint) {
        self.populate_char_types(paragraph);
        self.resolve(hint, paragraph);
    }

    /// BD1: The bidirectional character types are values assigned to each
    /// Unicode character, including unassigned characters
    fn populate_char_types(&mut self, paragraph: &[char]) {
        self.orig_char_types.clear();
        self.orig_char_types.reserve(paragraph.len());
        self.orig_char_types
            .extend(paragraph.iter().map(|&c| bidi_class_for_char(c)));
    }

    pub fn set_char_types(&mut self, char_types: &[BidiClass], hint: ParagraphDirectionHint) {
        self.orig_char_types.clear();
        self.orig_char_types.extend(char_types);
        self.resolve(hint, &[]);
    }

    fn resolve(&mut self, hint: ParagraphDirectionHint, paragraph: &[char]) {
        trace!("\n**** resolve \n");
        self.char_types.clear();
        self.char_types.extend(self.orig_char_types.iter());

        self.base_level = match hint {
            ParagraphDirectionHint::LeftToRight => Level(0),
            ParagraphDirectionHint::RightToLeft => Level(1),
            ParagraphDirectionHint::AutoLeftToRight => {
                paragraph_level(&self.char_types, false, Direction::LeftToRight)
            }
            ParagraphDirectionHint::AutoRightToLeft => {
                paragraph_level(&self.char_types, false, Direction::RightToLeft)
            }
        };

        self.dump_state("before X1-X8");
        self.explicit_embedding_levels();
        self.dump_state("before X9");
        self.delete_format_characters();
        self.dump_state("after X9");
        self.identify_runs();
        let iso_runs = self.identify_isolating_run_sequences();

        self.dump_state("before W1");
        self.resolve_combining_marks(&iso_runs); // W1
        self.dump_state("before W2");
        self.resolve_european_numbers(&iso_runs); // W2
        self.dump_state("before W3");
        self.resolve_arabic_letters(&iso_runs); // W3
        self.dump_state("before W4");
        self.resolve_separators(&iso_runs); // W4
        self.dump_state("before W5");
        self.resolve_terminators(&iso_runs); // W5
        self.dump_state("before W6");
        self.resolve_es_cs_et(&iso_runs); // W6
        self.dump_state("before W7");
        self.resolve_en(&iso_runs); // W7

        self.dump_state("before N0");
        self.resolve_paired_brackets(&iso_runs, paragraph); // N0

        self.dump_state("before N1");
        self.resolve_neutrals_by_context(&iso_runs); // N1
        self.dump_state("before N2");
        self.resolve_neutrals_by_level(&iso_runs); // N2

        self.dump_state("before I1, I2");
        self.resolve_implicit_levels();
    }

    fn dump_state(&self, label: &str) {
        trace!("State: {}", label);
        trace!("BidiClass: {:?}", self.char_types);
        trace!("Levels: {:?}", self.levels);
        trace!("");
    }

    /// This is the method for Rule W1.
    ///
    /// Resolve combining marks for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For characters of bc=NSM, change the Bidi_Class
    /// value to that of the preceding character. Formatting characters
    /// (Bidi_Class RLE, LRE, RLO, LRO, PDF) and boundary neutral (Bidi_Class BN)
    /// are skipped over in this calculation, because they have been
    /// "deleted" by Rule X9.
    ///
    /// If a bc=NSM character occurs at the start of a text chain, it is given
    /// the Bidi_Class of sot (either R or L).
    fn resolve_combining_marks(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            let mut prior_bc = iso_run.sos;
            for &idx in &iso_run.indices {
                if self.char_types[idx] == BidiClass::NonspacingMark {
                    self.char_types[idx] = prior_bc;
                } else if !self.levels[idx].removed_by_x9() {
                    prior_bc = self.char_types[idx];
                }
            }
        }
    }

    /// This is the method for Rule W2.
    ///
    /// Resolve European numbers for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For characters of bc=EN, scan back to find the first
    /// character of strong type (or sot). If the strong type is bc=AL,
    /// change the Bidi_Class EN to AN. Formatting characters
    /// (Bidi_Class RLE, LRE, RLO, LRO, PDF) and boundary neutral (Bidi_Class BN)
    /// are skipped over in this calculation, because they have been
    /// "deleted" by Rule X9.
    fn resolve_european_numbers(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for (ridx, &cidx) in iso_run.indices.iter().enumerate() {
                if self.char_types[cidx] == BidiClass::EuropeanNumber {
                    // Scan backwards to find the first strong type
                    let mut first_strong_bc = iso_run.sos;

                    if ridx > 0 {
                        for &pidx in iso_run.indices.get(0..ridx).unwrap().iter().rev() {
                            match self.char_types[pidx] {
                                bc @ BidiClass::LeftToRight
                                | bc @ BidiClass::RightToLeft
                                | bc @ BidiClass::ArabicLetter => {
                                    first_strong_bc = bc;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Check if the first strong type is AL. If so
                    // reset this EN to AN.
                    if first_strong_bc == BidiClass::ArabicLetter {
                        self.char_types[cidx] = BidiClass::ArabicNumber;
                    }
                }
            }
        }
    }

    /// This is the method for Rule W3.
    ///
    /// Resolve Bidi_Class=AL for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For characters of bc=AL, change the Bidi_Class
    /// value to R.
    fn resolve_arabic_letters(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for &idx in &iso_run.indices {
                if self.char_types[idx] == BidiClass::ArabicLetter {
                    self.char_types[idx] = BidiClass::RightToLeft;
                }
            }
        }
    }

    /// Look back ahead of `index_idx` and return true if the
    /// bidi class == bc.  However, skip backwards over entries
    /// that were removed by X9; they will have NO_LEVEL.
    /// Returns the char index of the match.
    fn is_prior_context(
        &self,
        index_idx: usize,
        indices: &[usize],
        bc: BidiClass,
    ) -> Option<usize> {
        if index_idx == 0 {
            return None;
        }
        for &idx in indices[0..index_idx].iter().rev() {
            if self.char_types[idx] == bc {
                return Some(idx);
            }
            if !self.levels[idx].removed_by_x9() {
                break;
            }
        }
        None
    }

    /// Look ahead of `index_idx` and return true if the
    /// bidi class == bc.  However, skip over entries
    /// that were removed by X9; they will have NO_LEVEL.
    /// Returns the char index of the match.
    fn is_following_context(
        &self,
        index_idx: usize,
        indices: &[usize],
        bc: BidiClass,
    ) -> Option<usize> {
        for &idx in &indices[index_idx + 1..] {
            if self.char_types[idx] == bc {
                return Some(idx);
            }
            if !self.levels[idx].removed_by_x9() {
                break;
            }
        }
        None
    }

    fn is_in_context(&self, index_idx: usize, indices: &[usize], bc: BidiClass) -> bool {
        self.is_prior_context(index_idx, indices, bc).is_some()
            && self.is_following_context(index_idx, indices, bc).is_some()
    }

    /// This is the method for Rule W4.
    ///
    /// Resolve Bidi_Class=ES and CS for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class.
    ///
    /// For characters of bc=ES, check if they are *between* EN.
    /// If so, change their Bidi_Class to EN.
    ///
    /// For characters of bc=CS, check if they are *between* EN
    /// or between AN. If so, change their Bidi_Class to match.
    ///
    fn resolve_separators(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for (index_idx, &idx) in iso_run.indices.iter().enumerate() {
                if self.char_types[idx] == BidiClass::EuropeanSeparator {
                    if self.is_in_context(index_idx, &iso_run.indices, BidiClass::EuropeanNumber) {
                        self.char_types[idx] = BidiClass::EuropeanNumber;
                    }
                } else if self.char_types[idx] == BidiClass::CommonSeparator {
                    if self.is_in_context(index_idx, &iso_run.indices, BidiClass::EuropeanNumber) {
                        self.char_types[idx] = BidiClass::EuropeanNumber;
                    } else if self.is_in_context(
                        index_idx,
                        &iso_run.indices,
                        BidiClass::ArabicNumber,
                    ) {
                        self.char_types[idx] = BidiClass::ArabicNumber;
                    }
                }
            }
        }
    }

    /// This is the method for Rule W5.
    ///
    /// Resolve Bidi_Class=ET for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class.
    ///
    /// For characters of bc=ET, check if they are *next to* EN.
    /// If so, change their Bidi_Class to EN. This includes
    /// ET on either side of EN, so the context on both sides
    /// needs to be checked.
    ///
    /// Because this rule applies to indefinite sequences of ET,
    /// and because the context which triggers any change is
    /// adjacency to EN, the strategy taken here is to seek for
    /// EN first. If found, scan backwards, changing any eligible
    /// ET to EN. Then scan forwards, changing any eligible ET
    /// to EN. Then continue the search from the point of the
    /// last ET changed (if any).
    ///
    fn resolve_terminators(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for (index_idx, &idx) in iso_run.indices.iter().enumerate() {
                if self.char_types[idx] == BidiClass::EuropeanNumber {
                    for &prior_idx in iso_run.indices[0..index_idx].iter().rev() {
                        if self.char_types[prior_idx] == BidiClass::EuropeanTerminator {
                            self.char_types[prior_idx] = BidiClass::EuropeanNumber;
                        } else if !self.levels[prior_idx].removed_by_x9() {
                            break;
                        }
                    }
                    for &next_idx in &iso_run.indices[index_idx + 1..] {
                        if self.char_types[next_idx] == BidiClass::EuropeanTerminator {
                            self.char_types[next_idx] = BidiClass::EuropeanNumber;
                        } else if !self.levels[next_idx].removed_by_x9() {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// This is the method for Rule W6.
    ///
    /// Resolve remaining Bidi_Class=ES, CS, or ET for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For characters of bc=ES, bc=CS, or bc=ET, change
    /// the Bidi_Class value to ON. This resolves any remaining
    /// separators or terminators which were not already processed
    /// by Rules W4 and W5.
    fn resolve_es_cs_et(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for &idx in &iso_run.indices {
                match self.char_types[idx] {
                    BidiClass::EuropeanSeparator
                    | BidiClass::CommonSeparator
                    | BidiClass::EuropeanTerminator => {
                        self.char_types[idx] = BidiClass::OtherNeutral;
                    }
                    _ => {}
                }
            }
        }
    }

    /// This is the method for Rule W7.
    ///
    /// Resolve Bidi_Class=EN for a single level text chain.
    ///
    /// Process the text chain in reverse order. For each character in the text chain, examine its
    /// Bidi_Class. For characters of bc=EN, scan back to find the first strong
    /// directional type. If that type is L, change the Bidi_Class
    /// value of the number to L.
    fn resolve_en(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for (ridx, &cidx) in iso_run.indices.iter().enumerate().rev() {
                if self.char_types[cidx] == BidiClass::EuropeanNumber {
                    // Scan backwards to find the first strong type
                    let mut first_strong_bc = iso_run.sos;

                    if ridx > 0 {
                        for &pidx in iso_run.indices.get(0..ridx).unwrap().iter().rev() {
                            match self.char_types[pidx] {
                                bc @ BidiClass::LeftToRight | bc @ BidiClass::RightToLeft => {
                                    first_strong_bc = bc;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }

                    if first_strong_bc == BidiClass::LeftToRight {
                        self.char_types[cidx] = BidiClass::LeftToRight;
                    }
                }
            }
        }
    }

    /// This is the method for Rule N0. (New in UBA63)
    /// Resolve paired brackets for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For any character with the bpt value open or close,
    /// scan its context seeking a matching paired bracket. If found,
    /// resolve the type of both brackets to match the embedding
    /// direction.
    ///
    /// For UBA63 (and unchanged in UBA70), the error handling for
    /// a stack overflow was unspecified for this rule.
    ///
    /// Starting with UBA80, the exact stack size is specified (63),
    /// and the specification declares that if a stack overflow
    /// condition is encountered, the BD16 processing for this
    /// particular isolating run ceases immediately. This condition
    /// does not treated as a fatal error, however, so the rule
    /// should not return an error code here, which would stop
    /// all processing for *all* runs of the input string.
    fn resolve_paired_brackets(&mut self, iso_runs: &[IsolatingRunSequence], paragraph: &[char]) {
        if paragraph.is_empty() {
            // BidiTest cases don't populate the paragraph, but they
            // also don't contain any bracket related tests either,
            // so we have nothing to do here.
            return;
        }

        let mut stack = BracketStack::new();
        for iso_run in iso_runs {
            stack.clear();
            for (ridx, &cidx) in iso_run.indices.iter().enumerate() {
                if let Some((closing_bracket, bpt)) = lookup_closing(paragraph[cidx]) {
                    trace!("ridx={} cidx={} {:?} bracket", ridx, cidx, paragraph[cidx]);
                    if self.char_types[cidx] == BidiClass::OtherNeutral {
                        if bpt == BracketType::Open {
                            trace!("push open ridx={}", ridx);
                            if !stack.push(closing_bracket, ridx) {
                                // Stack overflow: halt processing
                                return;
                            }
                        } else {
                            // a closing bracket
                            trace!("close at ridx={}, search for opener", ridx);
                            stack.seek_matching_open_bracket(paragraph[cidx], ridx);
                        }
                    }
                }
            }

            if stack.pairs.is_empty() {
                // The pairList pointer will still be NULL if no paired brackets
                // were found. In this case, no further processing is necessary.
                continue;
            }

            // Because of the way the stack
            // processing works, the pairs may not be in the best order
            // in the pair list for further processing. Sort them
            // by position order of the opening bracket.
            stack.pairs.sort_unstable_by_key(|p| p.opening_pos);
            trace!("\nPairs: {:?}", stack.pairs);

            for pair in &stack.pairs {
                // Now for each pair, we have the first and last position
                // of the substring in this isolating run sequence
                // enclosed by those brackets (inclusive
                // of the brackets). Resolve that individual pair.
                self.resolve_one_pair(pair, &iso_run);
            }
        }
    }

    /// Set the Bidi_Class of a bracket pair, based on the
    /// direction determined by the N0 rule processing in
    /// br_ResolveOnePair().
    ///
    /// The direction passed in will either be BIDI_R or BIDI_L.
    ///
    /// This setting is abstracted in a function here, rather than
    /// simply being done inline, because of
    /// an edge case added to rule N0 as of UBA80. For UBA63 (and
    /// UBA70), no special handling of combining marks following
    /// either of the brackets is done. However, starting with UBA80,
    /// there is an edge case fix-up done which echoes the processing
    /// of rule W1. The text run needs to be scanned to find any
    /// combining marks (orig_bc=NSM) following a bracket which has
    /// its Bidi_Class changed by N0. Then those combining marks
    /// can again be adjusted to match the Bidi_Class of the
    /// bracket they apply to. This is an odd edge case, as combining
    /// marks do not typically occur with brackets, but the UBA80
    /// specification is now explicit about requiring this fix-up
    /// to be done.
    fn set_bracket_pair_bc(
        pair: &Pair,
        indices: &[usize],
        direction: Direction,
        char_types: &mut [BidiClass],
        orig_char_types: &[BidiClass],
        levels: &[Level],
    ) {
        let opening_pos = indices[pair.opening_pos];
        let closing_pos = indices[pair.closing_pos];
        let bc = match direction {
            Direction::LeftToRight => BidiClass::LeftToRight,
            Direction::RightToLeft => BidiClass::RightToLeft,
        };
        trace!(
            "set_bracket_pair_bc index={} from {:?} -> {:?}",
            opening_pos,
            char_types[opening_pos],
            bc
        );
        trace!(
            "set_bracket_pair_bc index={} from {:?} -> {:?}",
            closing_pos,
            char_types[closing_pos],
            bc
        );
        char_types[opening_pos] = bc;
        char_types[closing_pos] = bc;

        // Here is the tricky part.
        //
        // First scan from the opening bracket for any subsequent
        // character whose *original* Bidi_Class was NSM, and set
        // the current bc for it to direction also, to match the bracket.
        // Break out of the loop at the first character with any other
        // original Bidi_Class, so that this change only impacts
        // actual combining mark sequences.
        //
        // 2020-03-27 note: This scanning for original combining marks
        // must also scan past any intervening NO_LEVEL characters,
        // typically bc=BN characters removed earlier by rule X9.
        // Such sequences may, for example involve a ZWJ or ZWNJ,
        // or in bizarre edge cases involve other bc=BN characters
        // such as ZWSP. The latter would be defective combining character
        // sequences, but also need to be handled here.
        //
        // Then repeat the process for the matching closing bracket.
        //
        // The processing for the opening bracket is bounded to the
        // right by the position of the matching closing bracket.
        // The processing for the closing bracket is bounded to the
        // right by the end of the text run.
        for &cidx in &indices[pair.opening_pos + 1..pair.closing_pos] {
            if orig_char_types[cidx] == BidiClass::NonspacingMark {
                char_types[cidx] = bc;
            } else if !levels[cidx].removed_by_x9() {
                break;
            }
        }
        for &cidx in &indices[pair.closing_pos + 1..] {
            if orig_char_types[cidx] == BidiClass::NonspacingMark {
                char_types[cidx] = bc;
            } else if !levels[cidx].removed_by_x9() {
                break;
            }
        }
    }

    /// Resolve the embedding levels of one pair of matched brackets.
    ///
    /// This determination is based on the embedding direction.
    /// See BD3 in the UBA specification.
    ///
    /// If embedding level is even, embedding direction = L.
    /// If embedding level is odd,  embedding direction = R.
    fn resolve_one_pair(&mut self, pair: &Pair, iso_run: &IsolatingRunSequence) {
        let embedding_direction = iso_run.level.direction();
        let opposite_direction = embedding_direction.opposite();

        let mut strong_type_found = false;
        // Next check for a strong type (R or L)
        // between the matched brackets. If a strong type is found
        // which matches the embedding direction, then set the type of both
        // brackets to match the embedding direction, too.
        if pair.opening_pos < pair.closing_pos.saturating_sub(1) {
            trace!("pair: {:?}", pair);
            for &cidx in &iso_run.indices[pair.opening_pos + 1..pair.closing_pos] {
                let direction = match self.char_types[cidx] {
                    BidiClass::RightToLeft
                    | BidiClass::EuropeanNumber
                    | BidiClass::ArabicNumber => Some(Direction::RightToLeft),
                    BidiClass::LeftToRight => Some(Direction::LeftToRight),
                    _ => None,
                };

                if direction == Some(embedding_direction) {
                    // N0 step b
                    trace!("Strong direction e between brackets");
                    Self::set_bracket_pair_bc(
                        pair,
                        &iso_run.indices,
                        embedding_direction,
                        &mut self.char_types,
                        &self.orig_char_types,
                        &self.levels,
                    );
                    return;
                } else if direction == Some(opposite_direction) {
                    strong_type_found = true;
                }
            }
        }

        if strong_type_found {
            // First attempt to resolve direction by checking the prior context for
            // a strong type matching the opposite direction. N0 Step c1.
            if (opposite_direction == Direction::LeftToRight
                && self.is_prior_context_left(pair.opening_pos, &iso_run.indices, iso_run.sos))
                || (opposite_direction == Direction::RightToLeft
                    && self.is_prior_context_right(pair.opening_pos, &iso_run.indices, iso_run.sos))
            {
                Self::set_bracket_pair_bc(
                    pair,
                    &iso_run.indices,
                    opposite_direction,
                    &mut self.char_types,
                    &self.orig_char_types,
                    &self.levels,
                );
                return;
            } else {
                // No strong type matching the oppositedirection was found either
                // before or after these brackets in this text chain. Resolve the
                // brackets based on the embedding direction. N0 Step c2.
                Self::set_bracket_pair_bc(
                    pair,
                    &iso_run.indices,
                    embedding_direction,
                    &mut self.char_types,
                    &self.orig_char_types,
                    &self.levels,
                );
                return;
            }
        } else {
            // No strong type was found between the brackets. Leave
            // the brackets with unresolved direction.
        }
    }

    /// This is the method for Rule N1.
    ///
    /// Resolve neutrals by context for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For any character of neutral type, examine its
    /// context.
    ///
    /// L N L --> L L L
    /// R N R --> R R R [note that AN and EN count as R for this rule]
    ///
    /// Here "N" stands for "any sequence of neutrals", so the neutral
    /// does not have to be immediately adjacent to a strong type
    /// to be resolved this way.
    fn resolve_neutrals_by_context(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for (ridx, &cidx) in iso_run.indices.iter().enumerate().rev() {
                if !self.char_types[cidx].is_neutral() {
                    continue;
                }

                if self.is_prior_context_left(ridx, &iso_run.indices, iso_run.sos)
                    && self.is_following_context_left(ridx, &iso_run.indices, iso_run.eos)
                {
                    trace!(
                        "ridx={} cidx={} was {:?}, setting to LeftToRight",
                        ridx,
                        cidx,
                        self.char_types[cidx]
                    );
                    self.char_types[cidx] = BidiClass::LeftToRight;
                } else if self.is_prior_context_right(ridx, &iso_run.indices, iso_run.sos)
                    && self.is_following_context_right(ridx, &iso_run.indices, iso_run.eos)
                {
                    trace!(
                        "ridx={} cidx={} was {:?}, setting to RightToLeft",
                        ridx,
                        cidx,
                        self.char_types[cidx]
                    );
                    self.char_types[cidx] = BidiClass::RightToLeft;
                }
            }
        }
    }

    /// Scan backwards in a text chain, checking if the first non-neutral character
    /// is an "L" type.  Skip over any "deleted" controls, which have NO_LEVEL,
    /// as well as any neutral types.
    fn is_prior_context_left(&self, index_idx: usize, indices: &[usize], sot: BidiClass) -> bool {
        if index_idx == 0 {
            trace!(
                "is_prior_context_left: short circuit because index_idx=0. sot is {:?}",
                sot
            );
            return sot == BidiClass::LeftToRight;
        }
        for &idx in indices[0..index_idx].iter().rev() {
            trace!(
                "is_prior_context_left considering idx={} {:?}",
                idx,
                self.char_types[idx]
            );
            if self.char_types[idx] == BidiClass::LeftToRight {
                return true;
            }
            if self.levels[idx].removed_by_x9() {
                continue;
            }
            if self.char_types[idx].is_neutral() {
                continue;
            }
            return false;
        }
        sot == BidiClass::LeftToRight
    }

    /// Scan forwards in a text chain, checking if the first non-neutral character is an "L" type.
    /// Skip over any "deleted" controls, which have NO_LEVEL, as well as any neutral types.
    fn is_following_context_left(
        &self,
        index_idx: usize,
        indices: &[usize],
        eot: BidiClass,
    ) -> bool {
        trace!(
            "is_following_context_left index_idx={} vs. len {}",
            index_idx,
            indices.len()
        );
        for &idx in &indices[index_idx + 1..] {
            if self.char_types[idx] == BidiClass::LeftToRight {
                trace!("is_following_context_left true because idx={} is left", idx);
                return true;
            }
            if self.levels[idx].removed_by_x9() {
                continue;
            }
            if self.char_types[idx].is_neutral() {
                continue;
            }
            return false;
        }
        trace!(
            "is_following_context_left fall through to bottom, check against eot={:?}",
            eot
        );
        eot == BidiClass::LeftToRight
    }

    /// Used by Rule N1.
    ///
    /// Scan backwards in a text chain, checking if the first non-neutral character is an "R" type.
    /// (BIDI_R, BIDI_AN, BIDI_EN) Skip over any "deleted" controls, which
    /// have NO_LEVEL, as well as any neutral types.
    fn is_prior_context_right(&self, index_idx: usize, indices: &[usize], sot: BidiClass) -> bool {
        if index_idx == 0 {
            return sot == BidiClass::RightToLeft;
        }
        for &idx in indices[0..index_idx].iter().rev() {
            match self.char_types[idx] {
                BidiClass::RightToLeft | BidiClass::ArabicNumber | BidiClass::EuropeanNumber => {
                    return true;
                }
                _ => {}
            }
            if self.levels[idx].removed_by_x9() {
                continue;
            }
            if self.char_types[idx].is_neutral() {
                continue;
            }
            return false;
        }
        sot == BidiClass::RightToLeft
    }

    fn is_following_context_right(
        &self,
        index_idx: usize,
        indices: &[usize],
        eot: BidiClass,
    ) -> bool {
        for &idx in &indices[index_idx + 1..] {
            match self.char_types[idx] {
                BidiClass::RightToLeft | BidiClass::ArabicNumber | BidiClass::EuropeanNumber => {
                    return true;
                }
                _ => {}
            }
            if self.levels[idx].removed_by_x9() {
                continue;
            }
            if self.char_types[idx].is_neutral() {
                continue;
            }
            return false;
        }
        eot == BidiClass::RightToLeft
    }

    /// This is the method for Rule N2.
    ///
    /// Resolve neutrals by level for a single text chain.
    ///
    /// For each character in the text chain, examine its
    /// Bidi_Class. For any character of neutral type, examine its
    /// embedding level and resolve accordingly.
    ///
    /// N --> e
    /// where e = L for an even level, R for an odd level
    fn resolve_neutrals_by_level(&mut self, iso_runs: &[IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for &cidx in iso_run.indices.iter().rev() {
                if self.char_types[cidx].is_neutral() {
                    self.char_types[cidx] = self.levels[cidx].as_bidi_class();
                }
            }
        }
    }

    /// This function runs Rules I1 and I2 together.
    fn resolve_implicit_levels(&mut self) {
        for (idx, level) in self.levels.iter_mut().enumerate() {
            if level.removed_by_x9() {
                continue;
            }

            match level.direction() {
                Direction::LeftToRight => {
                    // I1
                    match self.char_types[idx] {
                        BidiClass::RightToLeft => {
                            level.0 += 1;
                        }
                        BidiClass::ArabicNumber | BidiClass::EuropeanNumber => {
                            level.0 += 2;
                        }
                        _ => {}
                    }
                }
                Direction::RightToLeft => {
                    // I2
                    match self.char_types[idx] {
                        BidiClass::LeftToRight
                        | BidiClass::ArabicNumber
                        | BidiClass::EuropeanNumber => {
                            level.0 += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// This function runs Rule L1.
    ///
    /// The strategy here for Rule L1 is to scan forward through
    /// the text searching for segment separators or paragraph
    /// separators. If a segment separator or paragraph
    /// separator is found, it is reset to the paragraph embedding
    /// level. Then scan backwards from the separator to
    /// find any contiguous stretch of whitespace characters
    /// and reset any which are found to the paragraph embedding
    /// level, as well. When we reach the *last* character in the
    /// text (which will also constitute, by definition, the last
    /// character in the line being processed here), check if it
    /// is whitespace. If so, reset it to the paragraph embedding
    /// level. Then scan backwards to find any contiguous stretch
    /// of whitespace characters and reset those as well.
    ///
    /// These checks for whitespace are done with the *original*
    /// Bidi_Class values for characters, not the resolved values.
    ///
    /// As for many rules, this rule simply ignores any character
    /// whose level has been set to NO_LEVEL, which is the way
    /// this reference algorithm "deletes" boundary neutrals and
    /// embedding and override controls from the text.
    fn reset_whitespace_levels(&self, line_range: Range<usize>) -> Vec<Level> {
        fn reset_contiguous_whitespace_before(
            line_range: Range<usize>,
            base_level: Level,
            orig_char_types: &[BidiClass],
            levels: &mut Vec<Level>,
        ) {
            for i in line_range.rev() {
                if orig_char_types[i] == BidiClass::WhiteSpace
                    || orig_char_types[i].is_iso_control()
                {
                    levels[i] = base_level;
                } else if levels[i].removed_by_x9() {
                    // Skip over deleted entries
                } else {
                    // end of contiguous section
                    break;
                }
            }
        }

        let mut levels = self.levels.clone();

        for (idx, orig_bc) in self
            .orig_char_types
            .iter()
            .enumerate()
            .skip(line_range.start)
            .take(line_range.end - line_range.start)
        {
            match orig_bc {
                // Explicit boundary
                BidiClass::SegmentSeparator | BidiClass::ParagraphSeparator => {
                    levels[idx] = self.base_level;
                    reset_contiguous_whitespace_before(
                        line_range.start..idx,
                        self.base_level,
                        &self.orig_char_types,
                        &mut levels,
                    );
                }
                _ => {}
            }
        }

        reset_contiguous_whitespace_before(
            line_range.clone(),
            self.base_level,
            &self.orig_char_types,
            &mut levels,
        );

        levels[line_range].to_vec()
    }

    /// Rules X1 through X8
    fn explicit_embedding_levels(&mut self) {
        // X1: initialize stack and other variables
        let mut stack = LevelStack::new();
        stack.push(self.base_level, Override::Neutral, false);

        let len = self.char_types.len();
        self.levels.resize(len, Level::default());

        let mut overflow_isolate = 0;
        let mut overflow_embedding = 0;
        let mut valid_isolate = 0;

        // X2..X8: process each character, setting embedding levels
        // and override status
        for idx in 0..len {
            let bc = self.char_types[idx];
            trace!("Considering idx={} {:?}", idx, bc);
            match bc {
                // X2
                BidiClass::RightToLeftEmbedding => {
                    if let Some(level) = stack.embedding_level().least_greater_odd() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            stack.push(level, Override::Neutral, false);
                            continue;
                        }
                    }
                    if overflow_isolate == 0 {
                        overflow_embedding += 1;
                    }
                }
                // X3
                BidiClass::LeftToRightEmbedding => {
                    if let Some(level) = stack.embedding_level().least_greater_even() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            stack.push(level, Override::Neutral, false);
                            continue;
                        }
                    }
                    if overflow_isolate == 0 {
                        overflow_embedding += 1;
                    }
                }
                // X4
                BidiClass::RightToLeftOverride => {
                    if let Some(level) = stack.embedding_level().least_greater_odd() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            stack.push(level, Override::RTL, false);
                            continue;
                        }
                    }
                    if overflow_isolate == 0 {
                        overflow_embedding += 1;
                    }
                }
                // X5
                BidiClass::LeftToRightOverride => {
                    if let Some(level) = stack.embedding_level().least_greater_even() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            stack.push(level, Override::LTR, false);
                            continue;
                        }
                    }
                    if overflow_isolate == 0 {
                        overflow_embedding += 1;
                    }
                }
                // X5a
                BidiClass::RightToLeftIsolate => {
                    self.levels[idx] = stack.embedding_level();
                    stack.apply_override(&mut self.char_types[idx]);
                    if let Some(level) = stack.embedding_level().least_greater_odd() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            valid_isolate += 1;
                            stack.push(level, Override::Neutral, true);
                            continue;
                        }
                    }
                    overflow_isolate += 1;
                }
                // X5b
                BidiClass::LeftToRightIsolate => {
                    self.levels[idx] = stack.embedding_level();
                    stack.apply_override(&mut self.char_types[idx]);
                    if let Some(level) = stack.embedding_level().least_greater_even() {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            valid_isolate += 1;
                            stack.push(level, Override::Neutral, true);
                            continue;
                        }
                    }
                    overflow_isolate += 1;
                }
                // X5c
                BidiClass::FirstStrongIsolate => {
                    let level =
                        paragraph_level(&self.char_types[idx + 1..], true, Direction::LeftToRight);
                    self.levels[idx] = stack.embedding_level();
                    stack.apply_override(&mut self.char_types[idx]);
                    let level = if level.0 == 1 {
                        stack.embedding_level().least_greater_odd()
                    } else {
                        stack.embedding_level().least_greater_even()
                    };
                    trace!(
                        "picked {:?} based on current stack level {:?}",
                        level,
                        stack.embedding_level()
                    );

                    if let Some(level) = level {
                        if overflow_isolate == 0 && overflow_embedding == 0 {
                            valid_isolate += 1;
                            stack.push(level, Override::Neutral, true);
                            continue;
                        }
                    }
                    overflow_isolate += 1;
                }
                // X6a
                BidiClass::PopDirectionalIsolate => {
                    if overflow_isolate > 0 {
                        overflow_isolate -= 1;
                    } else if valid_isolate == 0 {
                        // Do nothing
                    } else {
                        overflow_embedding = 0;
                        loop {
                            if stack.isolate_status() {
                                break;
                            }
                            stack.pop();
                        }
                        stack.pop();
                        valid_isolate -= 1;
                    }

                    self.levels[idx] = stack.embedding_level();
                    stack.apply_override(&mut self.char_types[idx]);
                }
                // X7
                BidiClass::PopDirectionalFormat => {
                    if overflow_isolate > 0 {
                        // Do nothing
                    } else if overflow_embedding > 0 {
                        overflow_embedding -= 1;
                    } else {
                        if !stack.isolate_status() {
                            if stack.depth() >= 2 {
                                stack.pop();
                            }
                        }
                    }
                }
                BidiClass::BoundaryNeutral => {}
                // X8
                BidiClass::ParagraphSeparator => {
                    // Terminates all embedding contexts.
                    // Should only ever be the last character in
                    // a paragraph if present at all.
                    self.levels[idx] = self.base_level;
                }
                // X6
                _ => {
                    self.levels[idx] = stack.embedding_level();
                    stack.apply_override(&mut self.char_types[idx]);
                }
            }
        }
    }

    /// X9
    fn delete_format_characters(&mut self) {
        for (bc, level) in self.char_types.iter().zip(&mut self.levels) {
            match bc {
                BidiClass::RightToLeftEmbedding
                | BidiClass::LeftToRightEmbedding
                | BidiClass::RightToLeftOverride
                | BidiClass::LeftToRightOverride
                | BidiClass::PopDirectionalFormat
                | BidiClass::BoundaryNeutral => {
                    *level = Level(NO_LEVEL);
                }
                _ => {}
            }
        }
    }

    /// X10
    fn identify_runs(&mut self) {
        let mut idx = 0;
        let len = self.char_types.len();
        self.runs.clear();

        while idx < len {
            let (span_level, span_len) = span_one_run(&self.char_types, &self.levels, idx);
            if !span_level.removed_by_x9() {
                self.runs.push(Run {
                    start: idx,
                    end: idx + span_len,
                    len: span_len,
                    seq_id: 0,
                    level: span_level,
                    sor: BidiClass::OtherNeutral,
                    eor: BidiClass::OtherNeutral,
                });
            }

            assert!(span_len > 0);
            idx += span_len;
        }

        self.calculate_sor_eor();

        trace!("\nRuns: {:#?}", self.runs);
    }

    fn calculate_sor_eor(&mut self) {
        let mut prior_run_level = self.base_level;
        let mut iter = self.runs.iter_mut().peekable();
        while let Some(run) = iter.next() {
            let next_run_level = match iter.peek() {
                Some(next) => next.level,
                None => self.base_level,
            };

            // Set sor based on the higher of the prior_run_level and the current level.
            run.sor = prior_run_level.max(run.level).as_bidi_class();
            run.eor = next_run_level.max(run.level).as_bidi_class();

            prior_run_level = run.level;
        }
    }

    /// This function applies only to UBA63. Once the embedding
    /// levels are identified, UBA63 requires further processing
    /// to assign each of the level runs to an isolating run sequence.
    ///
    /// Each level run must be uniquely assigned to exactly one
    /// isolating run sequence. Each isolating run sequence must
    /// have at least one level run, but may have more.
    ///
    /// The exact details on how to match up isolating run sequences
    /// with level runs are specified in BD13.
    ///
    /// The strategy taken here is to scan the level runs in order.
    ///
    /// If a level run is not yet assigned to an isolating run sequence,
    /// its seqID will be zero. Create a new isolating run sequence
    /// and add this level run to it.
    ///
    /// If the last BIDIUNIT of *this* level run is an isolate
    /// initiator (LRI/RLI/FSI), then scan ahead in the list of
    /// level runs seeking the next level run which meets the
    /// following criteria:
    ///   1. seqID = 0 (not yet assigned to an isolating run sequence)
    ///   2. its level matches the level we are processing
    ///   3. the first BIDIUNIT is a PDI
    /// If all those conditions are met, assign that next level run
    /// to this isolating run sequence (set its seqID, and append to
    /// the list).
    ///
    /// Repeat until we hit a level run that doesn't terminate with
    /// an isolate initiator or we hit the end of the list of level
    /// runs.
    ///
    /// That terminates the definition of the isolating run sequence
    /// we are working on. Append it to the list of isolating run
    /// sequences in the UBACONTEXT.
    ///
    /// Then advance to the next level run which has not yet been
    /// assigned to an isolating run sequence and repeat the process.
    ///
    /// Continue until all level runs have been assigned to an
    /// isolating run sequence.
    fn identify_isolating_run_sequences(&mut self) -> Vec<IsolatingRunSequence> {
        let mut seq_id = 0;
        let mut iso_runs = vec![];
        let num_runs = self.runs.len();

        for run_idx in 0..num_runs {
            let save_level;

            {
                let run = &mut self.runs[run_idx];
                if run.seq_id != 0 {
                    continue;
                }
                seq_id += 1;
                iso_runs.push(Self::new_iso_run_seq(run_idx, run));
                run.seq_id = seq_id;

                if !self.char_types[run.end - 1].is_iso_init() {
                    continue;
                }
                save_level = run.level;
            }

            // Look ahead to find the run with the corresponding
            // PopDirectionalIsolate
            for idx in run_idx + 1..num_runs {
                let run = &mut self.runs[idx];
                if run.seq_id == 0
                    && run.level == save_level
                    && run.first_significant_bidi_class(&self.char_types, &self.levels)
                        == Some(BidiClass::PopDirectionalIsolate)
                {
                    // we matched the criteria for adding this run to the sequence.
                    let iso_run = iso_runs.last_mut().unwrap();
                    iso_run.runs.push(idx);
                    iso_run.len += run.len;
                    run.seq_id = seq_id;

                    // Check if the last char in this run is also an
                    // isolate initiator. If not, this sequence is done.
                    if !self.char_types[run.end - 1].is_iso_init() {
                        break;
                    }
                }
            }
        }
        self.calculate_sos_eos(&mut iso_runs);
        self.build_text_chains(&mut iso_runs);
        iso_runs
    }

    /// In order to simplify later rule processing, assemble the indices
    /// of the characters in the isolating runs so that there is just a
    /// single list to iterate
    fn build_text_chains(&mut self, iso_runs: &mut [IsolatingRunSequence]) {
        for iso_run in iso_runs {
            for &run_idx in &iso_run.runs {
                let run = &self.runs[run_idx];
                iso_run.indices.extend(run.start..run.end);
            }
        }
    }

    /// Process the isolating run sequence list, calculating sos and eos values for
    /// each sequence. Each needs to be set to either L or R.
    ///
    /// Strategy: Instead of recalculating all the sos and eos values from
    /// scratch, as specified in X10, we can take a shortcut here, because
    /// we already have sor and eor values assigned to all the level runs.
    /// For any isolating run sequence, simply assign sos to the value of
    /// sor for the *first* run in that sequence, and assign eos to the
    /// value of eor for the *last* run in that sequence. This provides
    /// equivalent values, and is more straightforward to implement and
    /// understand.
    ///
    /// This strategy has to be modified for defective isolating run sequences,
    /// where the sequence ends with an LRI/RLI/FSI.
    /// In those cases the eot needs to be calculated based on
    /// the paragraph embedding level, rather than from the level run.
    /// Note that this only applies when an isolating run sequence
    /// terminating in an LRI/RLI/FSI but with no matching PDI.
    /// An example would be:
    ///
    ///    R  RLI    R
    /// <L-----R> <RR>
    /// <L------[          <== eot would be L, not R
    ///           <RR>
    ///
    fn calculate_sos_eos(&mut self, iso_runs: &mut [IsolatingRunSequence]) {
        for iso_run in iso_runs {
            // First inherit the sos and eos values from the
            // first and last runs in the sequence.
            let first_run_idx = iso_run.runs.first().cloned().expect("at least 1 run");
            let last_run_idx = iso_run.runs.last().cloned().expect("at least 1 run");
            iso_run.sos = self.runs[first_run_idx].sor;
            iso_run.eos = self.runs[last_run_idx].eor;
            // Next adjust for the special case when an isolating
            // run sequence terminates in an unmatched isolate
            // initiator.
            if self.char_types[self.runs[last_run_idx].end - 1].is_iso_init() {
                let higher_level = self.base_level.max(iso_run.level);
                iso_run.eos = higher_level.as_bidi_class();
            }
        }
    }

    fn new_iso_run_seq(run_idx: usize, run: &Run) -> IsolatingRunSequence {
        let len = run.len;
        let level = run.level;
        IsolatingRunSequence {
            runs: vec![run_idx],
            len,
            level,
            sos: BidiClass::OtherNeutral,
            eos: BidiClass::OtherNeutral,
            indices: vec![],
        }
    }
}

impl BidiClass {
    pub fn is_iso_init(self) -> bool {
        match self {
            BidiClass::RightToLeftIsolate
            | BidiClass::LeftToRightIsolate
            | BidiClass::FirstStrongIsolate => true,
            _ => false,
        }
    }

    pub fn is_iso_control(self) -> bool {
        match self {
            BidiClass::RightToLeftIsolate
            | BidiClass::LeftToRightIsolate
            | BidiClass::PopDirectionalIsolate
            | BidiClass::FirstStrongIsolate => true,
            _ => false,
        }
    }

    pub fn is_neutral(self) -> bool {
        match self {
            BidiClass::OtherNeutral
            | BidiClass::WhiteSpace
            | BidiClass::SegmentSeparator
            | BidiClass::ParagraphSeparator => true,
            _ => self.is_iso_control(),
        }
    }
}

#[derive(Debug)]
struct Run {
    /// char indices for start, end of run
    start: usize,
    end: usize,
    /// length of run
    len: usize,
    /// isolating run sequence id
    seq_id: usize,
    /// Embedding level of this run
    level: Level,
    /// Direction of start of run
    sor: BidiClass,
    /// Direction of end of run
    eor: BidiClass,
}

impl Run {
    fn first_significant_bidi_class(
        &self,
        types: &[BidiClass],
        levels: &[Level],
    ) -> Option<BidiClass> {
        for idx in self.start..self.end {
            if !levels[idx].removed_by_x9() {
                return types.get(idx).cloned();
            }
        }
        None
    }
}

#[derive(Debug)]
struct IsolatingRunSequence {
    /// List of the runs in this sequence. The values are indices
    /// into the runs array
    runs: Vec<usize>,
    /// length of the run
    len: usize,
    /// Embedding level of this run
    level: Level,
    /// Direction of start of run
    sos: BidiClass,
    /// Direction of end of run
    eos: BidiClass,
    /// The sequence of indices into the original paragraph,
    /// across the contained set of runs
    indices: Vec<usize>,
}

/// Starting from `start`, extract the first run containing characters
/// all with the same level
fn span_one_run(types: &[BidiClass], levels: &[Level], start: usize) -> (Level, usize) {
    let mut span_level = Level(NO_LEVEL);
    let mut isolate_init_found = false;
    let mut span_len = 0;

    trace!(
        "span_one_run called with types: {:?}, levels: {:?}, start={}",
        types,
        levels,
        start
    );

    for (idx, (bc, level)) in types
        .iter()
        .skip(start)
        .zip(levels.iter().skip(start))
        .enumerate()
    {
        trace!(
            "span_one_run: consider idx={} bc={:?} level={:?}",
            idx,
            bc,
            level
        );
        if !level.removed_by_x9() {
            if bc.is_iso_init() {
                isolate_init_found = true;
            }
            if span_level.removed_by_x9() {
                span_level = *level;
            } else if *level != span_level {
                // End of run
                break;
            }
        }
        span_len = idx;
        if isolate_init_found {
            break;
        }
    }

    (span_level, span_len + 1)
}

/// 3.3.1 Paragraph level.
/// We've been fed a single paragraph, which takes care of rule P1.
/// This function implements rules P2 and P3.
fn paragraph_level(types: &[BidiClass], respect_pdi: bool, fallback: Direction) -> Level {
    let mut isolate_count = 0;
    for &t in types {
        match t {
            BidiClass::RightToLeftIsolate
            | BidiClass::LeftToRightIsolate
            | BidiClass::FirstStrongIsolate => isolate_count += 1,
            BidiClass::PopDirectionalIsolate => {
                if isolate_count > 0 {
                    isolate_count -= 1;
                } else if respect_pdi {
                    break;
                }
            }
            BidiClass::LeftToRight if isolate_count == 0 => return Level(0),
            BidiClass::RightToLeft | BidiClass::ArabicLetter if isolate_count == 0 => {
                return Level(1)
            }
            _ => {}
        }
    }
    if fallback == Direction::LeftToRight {
        Level(0)
    } else {
        Level(1)
    }
}

struct Pair {
    opening_pos: usize,
    closing_pos: usize,
}

impl std::fmt::Debug for Pair {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Pair{{{},{}}}", self.opening_pos, self.closing_pos)
    }
}

const MAX_PAIRING_DEPTH: usize = 63;
struct BracketStack {
    closing_bracket: [char; MAX_PAIRING_DEPTH],
    position: [usize; MAX_PAIRING_DEPTH],
    depth: usize,
    pairs: Vec<Pair>,
}

impl std::fmt::Debug for BracketStack {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("BracketStack")
            .field("closing_bracket", &&self.closing_bracket[0..self.depth])
            .field("position", &&self.position[0..self.depth])
            .field("depth", &self.depth)
            .field("pairs", &self.pairs)
            .finish()
    }
}

impl BracketStack {
    pub fn new() -> Self {
        Self {
            closing_bracket: [' '; MAX_PAIRING_DEPTH],
            position: [0; MAX_PAIRING_DEPTH],
            depth: 0,
            pairs: vec![],
        }
    }

    pub fn clear(&mut self) {
        self.pairs.clear();
        self.depth = 0;
    }

    pub fn push(&mut self, closing_bracket: char, pos: usize) -> bool {
        let depth = self.depth;
        if depth >= MAX_PAIRING_DEPTH {
            return false;
        }
        self.closing_bracket[depth] = closing_bracket;
        self.position[depth] = pos;
        self.depth += 1;
        true
    }

    /// Seek an opening bracket pair for the closing bracket
    /// passed in.
    ///
    /// This is a stack based search.
    /// Start with the top element in the stack and search
    /// downwards until we either find a match or reach the
    /// bottom of the stack.
    ///
    /// If we find a match, construct and append the bracket
    /// pair to the pairList. Then pop the stack for all the
    /// levels down to the level where we found the match.
    /// (This approach is designed to discard pairs that
    /// are not cleanly nested.)
    ///
    /// If we search all the way to the bottom of the stack
    /// without finding a match, just return without changing
    /// state. This represents a closing bracket with no
    /// opening bracket to match it. Just discard and move on.
    pub fn seek_matching_open_bracket(&mut self, closing_bracket: char, pos: usize) -> bool {
        trace!(
            "seek_matching_open_bracket: closing_bracket={:?} pos={}\n{:?}",
            closing_bracket,
            pos,
            self
        );
        for depth in (0..self.depth).rev() {
            trace!("seek_matching_open_bracket: consider depth={}", depth);
            // The basic test is for the closingcp equal to the bpb value
            // stored in the bracketData. But to account for the canonical
            // equivalences for U+2329 and U+232A, tack on extra checks here
            // for the asymmetrical matches. This hard-coded check avoids
            // having to require full normalization of all the bracket code
            // points before checking. It is highly unlikely that additional
            // canonical singletons for bracket pairs will be added to future
            // versions of the UCD.
            if self.closing_bracket[depth] == closing_bracket
                || (self.closing_bracket[depth] == '\u{232a}' && closing_bracket == '\u{3009}')
                || (self.closing_bracket[depth] == '\u{3009}' && closing_bracket == '\u{232a}')
            {
                self.pairs.push(Pair {
                    opening_pos: self.position[depth],
                    closing_pos: pos,
                });
                // Pop back to this depth, pruning out any intermediates;
                // they are mismatched brackets
                self.depth = depth;
                return true;
            }
        }
        false
    }
}

fn lookup_closing(c: char) -> Option<(char, BracketType)> {
    use bidi_brackets::BIDI_BRACKETS;
    if let Ok(idx) = BIDI_BRACKETS.binary_search_by_key(&c, |&(left, _, _)| left) {
        let entry = &BIDI_BRACKETS[idx];
        return Some((entry.1, entry.2));
    }
    None
}

pub fn bidi_class_for_char(c: char) -> BidiClass {
    use std::cmp::Ordering;
    if let Ok(idx) = bidi_class::BIDI_CLASS.binary_search_by(|&(lower, upper, _)| {
        if c >= lower && c <= upper {
            Ordering::Equal
        } else if c < lower {
            Ordering::Greater
        } else if c > upper {
            Ordering::Less
        } else {
            unreachable!()
        }
    }) {
        let entry = &bidi_class::BIDI_CLASS[idx];
        if c >= entry.0 && c <= entry.1 {
            return entry.2;
        }
    }
    // extracted/DerivedBidiClass.txt says:
    // All code points not explicitly listed for Bidi_Class
    //  have the value Left_To_Right (L).
    BidiClass::LeftToRight
}

#[cfg(test)]
mod tests {
    use super::*;
    use k9::assert_equal as assert_eq;

    #[test]
    fn runs() {
        let text = vec!['א', 'ב', 'ג', 'a', 'b', 'c'];

        let mut context = BidiContext::new();
        context.resolve_paragraph(&text, ParagraphDirectionHint::AutoLeftToRight);
        k9::snapshot!(
            context.runs().collect::<Vec<_>>(),
            "
[
    BidiRun {
        direction: RightToLeft,
        level: Level(
            1,
        ),
        range: 0..3,
        removed_by_x9: [],
    },
    BidiRun {
        direction: LeftToRight,
        level: Level(
            2,
        ),
        range: 3..6,
        removed_by_x9: [],
    },
]
"
        );
    }

    #[test]
    fn mirror() {
        assert_eq!(lookup_closing('{'), Some(('}', BracketType::Open)));
        assert_eq!(lookup_closing('['), Some((']', BracketType::Open)));
        assert_eq!(lookup_closing(']'), Some(('[', BracketType::Close)));
    }

    #[test]
    fn bidi_class_resolve() {
        assert_eq!(bidi_class_for_char('\u{0}'), BidiClass::BoundaryNeutral);
        assert_eq!(bidi_class_for_char('\u{9}'), BidiClass::SegmentSeparator);
        assert_eq!(bidi_class_for_char(' '), BidiClass::WhiteSpace);
        assert_eq!(bidi_class_for_char('a'), BidiClass::LeftToRight);
        assert_eq!(bidi_class_for_char('\u{590}'), BidiClass::RightToLeft);
        assert_eq!(bidi_class_for_char('\u{5d0}'), BidiClass::RightToLeft);
        assert_eq!(bidi_class_for_char('\u{5d1}'), BidiClass::RightToLeft);
    }

    /// This example is taken from
    /// <https://terminal-wg.pages.freedesktop.org/bidi/recommendation/combining.html>
    #[test]
    fn reorder_nsm() {
        let shalom: Vec<char> = vec![
            '\u{5e9}', '\u{5b8}', '\u{5c1}', '\u{5dc}', '\u{5d5}', '\u{05b9}', '\u{5dd}',
        ];
        let mut context = BidiContext::new();
        context.set_reorder_non_spacing_marks(true);
        context.resolve_paragraph(&shalom, ParagraphDirectionHint::LeftToRight);

        let mut reordered = vec![];
        for run in context.reordered_runs(0..shalom.len()) {
            for idx in run.indices {
                reordered.push(shalom[idx]);
            }
        }

        let explicit_ltr = vec![
            '\u{5dd}', '\u{5d5}', '\u{5b9}', '\u{5dc}', '\u{5e9}', '\u{5b8}', '\u{5c1}',
        ];
        assert_eq!(reordered, explicit_ltr);
    }
}
