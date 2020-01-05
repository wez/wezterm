use num::Integer;
use std::cmp::{max, min};
use std::fmt::Debug;
use std::ops::Range;

/// Track a set of integers, collapsing adjacent integers into ranges.
/// Internally stores the set in an array of ranges.
/// Allows adding and subtracting ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeSet<T: Integer + Copy> {
    ranges: Vec<Range<T>>,
}

fn range_is_empty<T: Integer>(range: &Range<T>) -> bool {
    range.start == range.end
}

/// Returns true if r1 intersects r2
fn intersects_range<T: Integer + Copy + Debug>(r1: &Range<T>, r2: &Range<T>) -> bool {
    let start = max(r1.start, r2.start);
    let end = min(r1.end, r2.end);

    end > start
}

/// Computes the intersection of r1 and r2
fn range_intersection<T: Integer + Copy + Debug>(r1: &Range<T>, r2: &Range<T>) -> Option<Range<T>> {
    let start = max(r1.start, r2.start);
    let end = min(r1.end, r2.end);

    if end > start {
        Some(start..end)
    } else {
        None
    }
}

/// Computes the r1 - r2, which may result in up to two non-overlapping ranges.
fn range_subtract<T: Integer + Copy + Debug>(
    r1: &Range<T>,
    r2: &Range<T>,
) -> (Option<Range<T>>, Option<Range<T>>) {
    let i_start = max(r1.start, r2.start);
    let i_end = min(r1.end, r2.end);

    if i_end > i_start {
        let a = if i_start == r1.start {
            // Intersection overlaps with the LHS
            None
        } else {
            // The LHS up to the intersection
            Some(r1.start..r1.end.min(i_start))
        };

        let b = if i_end == r1.end {
            // Intersection overlaps with the RHS
            None
        } else {
            // The intersection up to the RHS
            Some(r1.end.min(i_end)..r1.end)
        };

        (a, b)
    } else {
        // No intersection, so we're left with r1 with nothing removed
        (Some(r1.clone()), None)
    }
}

/// Merge two ranges to produce their union
fn range_union<T: Integer>(r1: Range<T>, r2: Range<T>) -> Range<T> {
    let start = r1.start.min(r2.start);
    let end = r1.end.max(r2.end);
    start..end
}

impl<T: Integer + Copy + Debug> RangeSet<T> {
    /// Create a new set
    pub fn new() -> Self {
        Self { ranges: vec![] }
    }

    /// Returns true if this set is empty
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Returns true if this set contains the specified integer
    pub fn contains(&self, value: T) -> bool {
        for r in &self.ranges {
            if r.contains(&value) {
                return true;
            }
        }
        false
    }

    /// Returns a rangeset containing all of the integers that are present
    /// in self but not in other.
    /// The current implementation is `O(n**2)` but this should be "OK"
    /// as the likely scenario is that there will be a large contiguous
    /// range for the scrollback, and a smaller contiguous range for changes
    /// in the viewport.
    /// If that doesn't hold up, we can improve this.
    pub fn difference(&self, other: &Self) -> Self {
        let mut result = Self::new();

        for my_range in &self.ranges {
            for other_range in &other.ranges {
                match range_subtract(my_range, other_range) {
                    (Some(a), Some(b)) => {
                        result.add_range(a);
                        result.add_range(b);
                    }
                    (Some(a), None) | (None, Some(a)) if a != *other_range => {
                        result.add_range(a);
                    }
                    _ => {}
                }
            }
        }

        result
    }

    pub fn intersection_with_range(&self, range: Range<T>) -> Self {
        let mut result = Self::new();

        for r in &self.ranges {
            if let Some(i) = range_intersection(r, &range) {
                result.add_range(i);
            }
        }

        result
    }

    /// Remove a single integer from the set
    pub fn remove(&mut self, value: T) {
        self.remove_range(value..value + num::one());
    }

    /// Remove a range of integers from the set
    pub fn remove_range(&mut self, range: Range<T>) {
        let mut to_add = vec![];
        let mut to_remove = vec![];

        for (idx, r) in self.ranges.iter().enumerate() {
            match range_subtract(r, &range) {
                (None, None) => to_remove.push(idx),
                (Some(a), Some(b)) => {
                    to_remove.push(idx);
                    to_add.push(a);
                    to_add.push(b);
                }
                (Some(a), None) | (None, Some(a)) if a != *r => {
                    to_remove.push(idx);
                    to_add.push(a);
                }
                _ => {}
            }
        }

        for idx in to_remove.into_iter().rev() {
            self.ranges.remove(idx);
        }

        for r in to_add {
            self.add_range(r);
        }
    }

    /// Remove a set of ranges from this set
    pub fn remove_set(&mut self, set: &Self) {
        for r in set.iter() {
            self.remove_range(r.clone());
        }
    }

    /// Add a single integer to the set
    pub fn add(&mut self, value: T) {
        self.add_range(value..value + num::one());
    }

    /// Add a range of integers to the set
    pub fn add_range(&mut self, range: Range<T>) {
        if range_is_empty(&range) {
            return;
        }

        if self.ranges.is_empty() {
            self.ranges.push(range.clone());
            return;
        }

        match self.intersection_helper(&range) {
            (Some(a), Some(b)) if b == a + 1 => {
                // This range intersects with two or more adjacent ranges and will
                // therefore join them together

                let second = self.ranges[b].clone();
                let merged = range_union(range, second);

                self.ranges.remove(b);
                return self.add_range(merged);
            }
            (Some(a), _) => self.merge_into_range(a, range),
            (None, Some(_)) => unreachable!(),
            (None, None) => {
                // No intersection, so find the insertion point
                let idx = self.insertion_point(&range);
                self.ranges.insert(idx, range.clone());
            }
        }
    }

    /// Add a set of ranges to this set
    pub fn add_set(&mut self, set: &Self) {
        for r in set.iter() {
            self.add_range(r.clone());
        }
    }

    fn merge_into_range(&mut self, idx: usize, range: Range<T>) {
        let existing = self.ranges[idx].clone();
        self.ranges[idx] = range_union(existing, range);
    }

    fn intersection_helper(&self, range: &Range<T>) -> (Option<usize>, Option<usize>) {
        let mut first = None;

        for (idx, r) in self.ranges.iter().enumerate() {
            if intersects_range(r, range) || r.end == range.start {
                if first.is_some() {
                    return (first, Some(idx));
                }
                first = Some(idx);
            }
        }

        (first, None)
    }

    fn insertion_point(&self, range: &Range<T>) -> usize {
        for (idx, r) in self.ranges.iter().enumerate() {
            if range.end < r.start {
                return idx;
            }
        }

        self.ranges.len()
    }

    /// Returns an iterator over the ranges that comprise the set
    pub fn iter(&self) -> impl Iterator<Item = &Range<T>> {
        self.ranges.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect<T: Integer + Copy + Debug>(set: &RangeSet<T>) -> Vec<Range<T>> {
        set.iter().cloned().collect()
    }

    #[test]
    fn add_range() {
        let mut set = RangeSet::new();
        set.add(1);
        set.add(2);
        set.add(4);
        assert_eq!(collect(&set), vec![1..3, 4..5]);

        set.add_range(2..6);
        assert_eq!(collect(&set), vec![1..6]);
    }

    #[test]
    fn remove_range() {
        let mut set = RangeSet::new();
        set.add_range(1..5);
        set.add_range(8..10);
        assert_eq!(collect(&set), vec![1..5, 8..10]);

        // Middle
        set.remove(2);
        assert_eq!(collect(&set), vec![1..2, 3..5, 8..10]);

        // RHS
        set.remove_range(4..7);
        assert_eq!(collect(&set), vec![1..2, 3..4, 8..10]);

        // Complete overlap of one range, LHS overlap with another
        set.remove_range(3..9);
        assert_eq!(collect(&set), vec![1..2, 9..10]);
    }

    #[test]
    fn difference() {
        let mut set = RangeSet::new();
        set.add_range(1..10);

        let mut other = RangeSet::new();
        other.add_range(1..15);

        let diff = other.difference(&set);
        assert_eq!(collect(&diff), vec![10..15]);

        let diff = set.difference(&other);
        assert_eq!(collect(&diff), vec![]);
    }

    #[test]
    fn difference_more() {
        let mut set = RangeSet::new();
        set.add(1);
        set.add(3);
        set.add(5);

        let mut other = RangeSet::new();
        other.add(2);
        other.add(4);
        other.add(6);

        let diff = other.difference(&set);
        assert_eq!(collect(&diff), vec![2..3, 4..5, 6..7]);

        let diff = set.difference(&other);
        assert_eq!(collect(&diff), vec![1..2, 3..4, 5..6]);
    }
}
