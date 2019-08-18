//! The connection to the GUI subsystem
use super::EventHandle;
use super::{HWindow, WindowInner};
use crate::connection::ConnectionOps;
use crate::spawn::*;
use failure::Fallible;
use promise::{BasicExecutor, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::*;

pub struct Connection {
    event_handle: HANDLE,
    pub(crate) windows: Mutex<HashMap<HWindow, Rc<RefCell<WindowInner>>>>,
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn run_message_loop(&self) -> Fallible<()> {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            SPAWN_QUEUE.run();

            let res = unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) };
            if res != 0 {
                if msg.message == WM_QUIT {
                    // Clear our state before we exit, otherwise we can
                    // trigger `drop` handlers during shutdown and that
                    // can have bad interactions
                    self.windows.lock().unwrap().clear();
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

    fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F) {
        unimplemented!();
    }

    fn wake_task_by_id(slot: usize) {
        unimplemented!();
    }
}

impl Connection {
    pub(crate) fn create_new() -> Fallible<Self> {
        let event_handle = SPAWN_QUEUE.event_handle.0;
        Ok(Self {
            event_handle,
            windows: Mutex::new(HashMap::new()),
        })
    }

    fn wait_message(&self) {
        unsafe {
            MsgWaitForMultipleObjects(1, &self.event_handle, 0, INFINITE, QS_ALLEVENTS);
        }
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
    }

    fn get_window(&self, handle: HWindow) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.lock().unwrap().get(&handle).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<F: FnMut(&mut WindowInner) + Send + 'static>(
        window: HWindow,
        mut f: F,
    ) {
        SpawnQueueExecutor {}.execute(Box::new(move || {
            if let Some(handle) = Connection::get()
                .expect("Connection::init has not been called")
                .get_window(window)
            {
                let mut inner = handle.borrow_mut();
                f(&mut inner);
            }
        }));
    }
}
