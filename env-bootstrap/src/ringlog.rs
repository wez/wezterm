//! This module sets up a logger that captures recent log entries
//! into an in-memory ring-buffer, as well as passed them on to
//! a pretty logger on stderr.
//! This allows other code to collect the ring buffer and display it
//! within the application.
use chrono::prelude::*;
use env_logger::filter::{Builder as FilterBuilder, Filter};
use log::{Level, LevelFilter, Log, Record};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use termwiz::istty::IsTty;

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
    file_name: PathBuf,
    file: Mutex<Option<BufWriter<File>>>,
    filter: Filter,
    padding: AtomicUsize,
    is_tty: bool,
}

impl Drop for Logger {
    fn drop(&mut self) {
        self.flush();
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn flush(&self) {
        if let Some(file) = self.file.lock().unwrap().as_mut() {
            let _ = file.flush();
        }
        let _ = std::io::stderr().flush();
    }

    fn log(&self, record: &Record) {
        if self.filter.matches(record) {
            RINGS.lock().unwrap().log(record);
            let ts = Local::now().format("%H:%M:%S%.3f").to_string();
            let level = record.level().as_str();
            let target = record.target().to_string();
            let msg = record.args().to_string();

            let padding = self.padding.fetch_max(target.len(), Ordering::SeqCst);

            let level_color = if self.is_tty {
                match record.level() {
                    Level::Error => "\u{1b}[31m",
                    Level::Warn => "\u{1b}[33m",
                    Level::Info => "\u{1b}[32m",
                    Level::Debug => "\u{1b}[36m",
                    Level::Trace => "\u{1b}[35m",
                }
            } else {
                ""
            };

            let reset = if self.is_tty { "\u{1b}[0m" } else { "" };
            let target_color = if self.is_tty { "\u{1b}[1m" } else { "" };

            {
                // We use writeln! here rather than eprintln! so that we can ignore
                // a failed log write in the case that stderr has been redirected
                // to a device that is out of disk space.
                // <https://github.com/wez/wezterm/issues/1839>
                let mut stderr = std::io::stderr();
                let _ = writeln!(
                    stderr,
                    "{}  {level_color}{:6}{reset} {target_color}{:padding$}{reset} > {}",
                    ts,
                    level,
                    target,
                    msg,
                    padding = padding,
                    level_color = level_color,
                    reset = reset,
                    target_color = target_color
                );
                let _ = stderr.flush();
            }

            let mut file = self.file.lock().unwrap();
            if file.is_none() {
                if let Ok(f) = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&self.file_name)
                {
                    file.replace(BufWriter::new(f));
                }
            }
            if let Some(file) = file.as_mut() {
                let _ = writeln!(
                    file,
                    "{}  {:6} {:padding$} > {}",
                    ts,
                    level,
                    target,
                    msg,
                    padding = padding
                );
                let _ = file.flush();
            }
        }
    }
}

/// Returns the current set of log information, sorted by time
pub fn get_entries() -> Vec<Entry> {
    let mut entries = RINGS.lock().unwrap().get_entries();
    entries.sort();
    entries
}

fn prune_old_logs() {
    let one_week = std::time::Duration::from_secs(86400 * 7);
    if let Ok(dir) = std::fs::read_dir(&*config::RUNTIME_DIR) {
        for entry in dir {
            if let Ok(entry) = entry {
                if let Some(name) = entry.file_name().to_str() {
                    if name.contains("-log-") {
                        if let Ok(meta) = entry.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if let Ok(elapsed) = modified.elapsed() {
                                    if elapsed > one_week {
                                        let _ = std::fs::remove_file(entry.path());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn setup_pretty() -> (LevelFilter, Logger) {
    prune_old_logs();

    let base_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "wezterm".to_string());

    let log_file_name = config::RUNTIME_DIR.join(format!("{}-log-{}.txt", base_name, unsafe {
        libc::getpid()
    }));

    let mut filters = FilterBuilder::new();
    for (module, level) in [
        ("wgpu_core", LevelFilter::Error),
        ("wgpu_hal", LevelFilter::Error),
        ("gfx_backend_metal", LevelFilter::Error),
    ] {
        filters.filter_module(module, level);
    }

    if let Ok(s) = std::env::var("WEZTERM_LOG") {
        filters.parse(&s);
    } else {
        filters.filter_level(LevelFilter::Info);
    }
    let filter = filters.build();
    let max_level = filter.filter();

    (
        max_level,
        Logger {
            file_name: log_file_name,
            file: Mutex::new(None),
            filter,
            padding: AtomicUsize::new(0),
            is_tty: std::io::stderr().is_tty(),
        },
    )
}

pub fn setup_logger() {
    let (max_level, logger) = setup_pretty();
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max_level);
    }
}
