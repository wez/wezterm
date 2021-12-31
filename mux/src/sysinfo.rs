#![cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, RefreshKind, System, SystemExt};

lazy_static::lazy_static! {
    static ref SYSTEM: Mutex<CachedSystemInfo> = Mutex::new(CachedSystemInfo::new());
}

pub struct CachedSystemInfo {
    last_update: Instant,
    system: sysinfo::System,
}

impl std::ops::Deref for CachedSystemInfo {
    type Target = sysinfo::System;

    fn deref(&self) -> &sysinfo::System {
        &self.system
    }
}

impl CachedSystemInfo {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(
                RefreshKind::new().with_processes(ProcessRefreshKind::new()),
            ),
            last_update: Instant::now(),
        }
    }

    pub fn refresh_now(&mut self) {
        self.system
            .refresh_processes_specifics(ProcessRefreshKind::new());
        self.last_update = Instant::now();
    }

    pub fn check_refresh(&mut self) {
        if self.last_update.elapsed() < Duration::from_millis(300) {
            return;
        }
        self.refresh_now();
    }
}

pub fn get() -> MutexGuard<'static, CachedSystemInfo> {
    let mut guard = SYSTEM.lock().unwrap();
    guard.check_refresh();
    guard
}

pub fn get_with_forced_refresh() -> MutexGuard<'static, CachedSystemInfo> {
    let mut guard = SYSTEM.lock().unwrap();
    guard.refresh_now();
    guard
}
