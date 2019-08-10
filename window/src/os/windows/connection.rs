//! The connection to the GUI subsystem
use super::EventHandle;
use failure::Fallible;
use promise::{BasicExecutor, SpawnFunc};
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use winapi::um::winbase::INFINITE;
use winapi::um::winuser::*;

pub struct Connection {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
    event_handle: EventHandle,
}

lazy_static::lazy_static! {
    static ref CONN: Arc<Connection> = Arc::new(Connection::new());
}

impl Connection {
    pub fn get() -> Option<Arc<Self>> {
        Some(Arc::clone(&CONN))
    }

    fn new() -> Self {
        let spawned_funcs = Mutex::new(VecDeque::new());
        let event_handle = EventHandle::new_manual_reset().expect("EventHandle creation failed");
        Self {
            spawned_funcs,
            event_handle,
        }
    }

    pub fn init() -> Fallible<Arc<Self>> {
        Ok(Arc::clone(&CONN))
    }

    pub fn terminate_message_loop(&self) {
        unsafe {
            PostQuitMessage(0);
        }
    }

    pub fn run_message_loop(&self) -> Fallible<()> {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            self.process_spawns();

            let res = unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) };
            if res != 0 {
                if msg.message == WM_QUIT {
                    return Ok(());
                }

                unsafe {
                    TranslateMessage(&mut msg);
                    DispatchMessageW(&mut msg);
                }
            } else {
                self.wait_message();
            }
        }
    }

    fn wait_message(&self) {
        unsafe {
            MsgWaitForMultipleObjects(1, &self.event_handle.0, 0, INFINITE, QS_ALLEVENTS);
        }
    }

    fn process_spawns(&self) {
        self.event_handle.reset_event();
        loop {
            if let Some(func) = self.spawned_funcs.lock().unwrap().pop_front() {
                func();
            } else {
                return;
            }
        }
    }

    fn spawn(&self, f: SpawnFunc) {
        self.spawned_funcs.lock().unwrap().push_back(f);
        self.event_handle.set_event();
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
    }
}

struct SpawnQueueExecutor;
impl BasicExecutor for SpawnQueueExecutor {
    fn execute(&self, f: SpawnFunc) {
        CONN.spawn(f)
    }
}

impl BasicExecutor for Connection {
    fn execute(&self, f: SpawnFunc) {
        self.spawn(f);
    }
}
