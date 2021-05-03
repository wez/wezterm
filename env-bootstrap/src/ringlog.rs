//! This module sets up a logger that captures recent log entries
//! into an in-memory ring-buffer, as well as passed them on to
//! a pretty logger on stderr.
//! This allows other code to collect the ring buffer and display it
//! within the application.
use chrono::prelude::*;
use log::{Level, Record};
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref RINGS: Mutex<Rings> = Mutex::new(Rings::new());
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Entry {
    pub then: DateTime<Local>,
    pub level: Level,
    pub target: String,
    pub msg: String,
}

struct LevelRing {
    entries: Vec<Entry>,
    first: usize,
    last: usize,
}

impl LevelRing {
    fn new(level: Level) -> Self {
        let mut entries = vec![];
        let now = Local::now();
        for _ in 0..16 {
            entries.push(Entry {
                then: now,
                level,
                target: String::new(),
                msg: String::new(),
            });
        }
        Self {
            entries,
            first: 0,
            last: 0,
        }
    }

    // Returns the number of entries in the ring
    fn len(&self) -> usize {
        if self.last >= self.first {
            self.last - self.first
        } else {
            // Wrapped around.
            (self.entries.len() - self.first) + self.last
        }
    }

    fn rolling_inc(&self, value: usize) -> usize {
        let incremented = value + 1;
        if incremented >= self.entries.len() {
            0
        } else {
            incremented
        }
    }

    fn push(&mut self, entry: Entry) {
        if self.len() == self.entries.len() {
            // We are full; effectively pop the first entry to
            // make room
            self.entries[self.first] = entry;
            self.first = self.rolling_inc(self.first);
        } else {
            self.entries[self.last] = entry;
        }
        self.last = self.rolling_inc(self.last);
    }

    fn append_to_vec(&self, target: &mut Vec<Entry>) {
        if self.last >= self.first {
            target.extend_from_slice(&self.entries[self.first..self.last]);
        } else {
            target.extend_from_slice(&self.entries[self.first..]);
            target.extend_from_slice(&self.entries[..self.last]);
        }
    }
}

struct Rings {
    rings: HashMap<Level, LevelRing>,
}

impl Rings {
    fn new() -> Self {
        let mut rings = HashMap::new();
        for level in &[
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            rings.insert(*level, LevelRing::new(*level));
        }
        Self { rings }
    }

    fn get_entries(&self) -> Vec<Entry> {
        let mut results = vec![];
        for ring in self.rings.values() {
            ring.append_to_vec(&mut results);
        }
        results
    }

    fn log(&mut self, record: &Record) {
        if let Some(ring) = self.rings.get_mut(&record.level()) {
            ring.push(Entry {
                then: Local::now(),
                level: record.level(),
                target: record.target().to_string(),
                msg: record.args().to_string(),
            });
        }
    }
}

struct Logger {
    pretty: Box<dyn log::Log>,
}

impl Logger {
    fn new(pretty: Box<dyn log::Log>) -> Self {
        Self { pretty }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.pretty.enabled(metadata)
    }

    fn flush(&self) {
        self.pretty.flush()
    }

    fn log(&self, record: &Record) {
        RINGS.lock().unwrap().log(record);
        self.pretty.log(record)
    }
}

/// Returns the current set of log information, sorted by time
pub fn get_entries() -> Vec<Entry> {
    let mut entries = RINGS.lock().unwrap().get_entries();
    entries.sort();
    entries
}

pub fn setup_logger() {
    let mut builder = pretty_env_logger::formatted_timed_builder();
    if let Ok(s) = std::env::var("WEZTERM_LOG") {
        builder.parse_filters(&s);
    } else {
        builder.filter(None, log::LevelFilter::Info);
    }

    let pretty = builder.build();
    let max_level = pretty.filter();

    let pretty = Box::new(pretty);
    let logger = Logger::new(pretty);

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max_level);
    }
}
