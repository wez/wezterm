use std::collections::VecDeque;

/// Represents a position within the history.
/// Smaller numbers are assumed to be before larger numbers,
/// and the indices are assumed to be contiguous.
pub type HistoryIndex = usize;

/// Defines the history interface for the line editor.
pub trait History {
    /// Lookup the line corresponding to an index.
    fn get(&self, idx: HistoryIndex) -> Option<&str>;
    /// Return the index for the most recently added entry.
    fn last(&self) -> Option<HistoryIndex>;
    /// Add an entry.
    /// Note that the LineEditor will not automatically call
    /// the add method.
    fn add(&mut self, line: &str);
}

/// A simple history implementation that holds entries in memory.
#[derive(Default)]
pub struct BasicHistory {
    entries: VecDeque<String>,
}

impl History for BasicHistory {
    fn get(&self, idx: HistoryIndex) -> Option<&str> {
        self.entries.get(idx).map(String::as_str)
    }

    fn last(&self) -> Option<HistoryIndex> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.len() - 1)
        }
    }

    fn add(&mut self, line: &str) {
        if self.entries.back().map(String::as_str) == Some(line) {
            // Ignore duplicates
            return;
        }
        self.entries.push_back(line.to_owned());
    }
}
