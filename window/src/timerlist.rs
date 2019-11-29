#![cfg(all(unix, not(target_os = "macos")))]
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct TimerEntry {
    pub callback: Box<dyn FnMut()>,
    pub due: Instant,
    pub interval: Duration,
}

#[derive(Default)]
pub struct TimerList {
    timers: VecDeque<TimerEntry>,
}

impl TimerList {
    pub fn new() -> Self {
        Default::default()
    }

    fn find_index_after(&self, due: &Instant) -> usize {
        for (idx, entry) in self.timers.iter().enumerate() {
            if entry.due.cmp(due) == Ordering::Greater {
                return idx;
            }
        }
        self.timers.len()
    }

    pub fn insert(&mut self, mut entry: TimerEntry) {
        entry.due = Instant::now() + entry.interval;
        let idx = self.find_index_after(&entry.due);
        self.timers.insert(idx, entry);
    }

    pub fn time_until_due(&self, now: Instant) -> Option<Duration> {
        self.timers.front().map(|entry| {
            if entry.due <= now {
                Duration::from_secs(0)
            } else {
                entry.due - now
            }
        })
    }

    fn first_is_ready(&self, now: Instant) -> bool {
        if let Some(first) = self.timers.front() {
            first.due <= now
        } else {
            false
        }
    }

    pub fn run_ready(&mut self) {
        let now = Instant::now();
        let mut requeue = vec![];
        while self.first_is_ready(now) {
            let mut first = self.timers.pop_front().expect("first_is_ready");
            (first.callback)();
            requeue.push(first);
        }

        for entry in requeue.into_iter() {
            self.insert(entry);
        }
    }
}
