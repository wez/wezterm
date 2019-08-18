#[cfg(windows)]
use crate::os::windows::event::EventHandle;
use failure::Fallible;
#[cfg(all(unix, not(target_os = "macos")))]
use filedescriptor::{FileDescriptor, Pipe};
#[cfg(all(unix, not(target_os = "macos")))]
use mio::unix::EventedFd;
#[cfg(all(unix, not(target_os = "macos")))]
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use promise::{BasicExecutor, SpawnFunc};
use std::collections::VecDeque;
#[cfg(all(unix, not(target_os = "macos")))]
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    pub(crate) static ref SPAWN_QUEUE: Arc<SpawnQueue> = Arc::new(SpawnQueue::new().expect("failed to create SpawnQueue"));
}

#[cfg(windows)]
pub(crate) struct SpawnQueue {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
    pub event_handle: EventHandle,
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) struct SpawnQueue {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
    write: Mutex<FileDescriptor>,
    read: Mutex<FileDescriptor>,
}

impl SpawnQueue {
    pub fn new() -> Fallible<Self> {
        Self::new_impl()
    }

    pub fn spawn(&self, f: SpawnFunc) {
        self.spawn_impl(f)
    }

    pub fn run(&self) {
        self.run_impl()
    }
}

#[cfg(windows)]
impl SpawnQueue {
    fn new_impl() -> Fallible<Self> {
        let spawned_funcs = Mutex::new(VecDeque::new());
        let event_handle = EventHandle::new_manual_reset().expect("EventHandle creation failed");
        Ok(Self {
            spawned_funcs,
            event_handle,
        })
    }

    fn spawn_impl(&self, f: SpawnFunc) {
        self.spawned_funcs.lock().unwrap().push_back(f);
        self.event_handle.set_event();
    }

    fn run_impl(&self) {
        self.event_handle.reset_event();
        loop {
            if let Some(func) = self.spawned_funcs.lock().unwrap().pop_front() {
                func();
            } else {
                return;
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl SpawnQueue {
    fn new_impl() -> Fallible<Self> {
        let pipe = Pipe::new()?;
        Ok(Self {
            spawned_funcs: Mutex::new(VecDeque::new()),
            write: Mutex::new(pipe.write),
            read: Mutex::new(pipe.read),
        })
    }

    fn spawn_impl(&self, f: SpawnFunc) {
        use std::io::Write;

        self.spawned_funcs.lock().unwrap().push_back(f);
        self.write.lock().unwrap().write(b"x").ok();
    }

    fn run_impl(&self) {
        use std::io::Read;
        while let Some(func) = self.spawned_funcs.lock().unwrap().pop_front() {
            func();

            let mut byte = [0u8];
            self.read.lock().unwrap().read(&mut byte).ok();
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl Evented for SpawnQueue {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.read.lock().unwrap().as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.read.lock().unwrap().as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.read.lock().unwrap().as_raw_fd()).deregister(poll)
    }
}
