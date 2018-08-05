use std::ops::Range;

/// range.contains(), but that is unstable
pub fn in_range<T: Ord + Copy>(value: T, range: &Range<T>) -> bool {
    value >= range.start && value < range.end
}
