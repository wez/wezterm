//! The connection to the GUI subsystem
use super::EventHandle;
use failure::Fallible;
use promise::{BasicExecutor, SpawnFunc};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use winapi::um::winbase::INFINITE;
use winapi::um::winuser::*;

pub struct Connection {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
    event_handle: EventHandle,
}

thread_local! {
    static CONN: RefCell<Option<Arc<Connection>>> = RefCell::new(None);
}

impl Connection {
    pub fn get() -> Option<Arc<Self>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Arc::clone(mux));
            }
        });
        res
    }

    pub fn init() -> Fallible<Arc<Self>> {
        let spawned_funcs = Mutex::new(VecDeque::new());
        let event_handle = EventHandle::new_manual_reset()?;
        let conn = Arc::new(Self {
            spawned_funcs,
            event_handle,
        });
        CONN.with(|m| *m.borrow_mut() = Some(Arc::clone(&conn)));
        Ok(conn)
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
}

impl BasicExecutor for Connection {
    fn execute(&self, f: SpawnFunc) {
        self.spawn(f);
    }
}
